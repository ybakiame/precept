use super::{Contract, SelectorQuery};
use crate::typed_context as typed;
use serde::{
    Deserialize,
    de::{self, DeserializeSeed, MapAccess, SeqAccess, Visitor},
};
use serde_json::Value;
use std::fmt;

pub const SOURCE: &str = "ACP.contracts.md";
pub const MAX_BYTES: usize = 16_384;

/// Sanitized whole-source refusal; parser diagnostics never leave this boundary.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct InvalidSource;

// Deserialize recursively while rejecting duplicate keys and bounding containers.
// Going through serde_json::Value directly would silently retain the last key.
struct Strict(usize);
impl<'de> DeserializeSeed<'de> for Strict {
    type Value = Value;
    fn deserialize<D: de::Deserializer<'de>>(self, d: D) -> Result<Value, D::Error> {
        d.deserialize_any(self)
    }
}
impl<'de> Visitor<'de> for Strict {
    type Value = Value;
    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("bounded JSON")
    }
    fn visit_bool<E: de::Error>(self, v: bool) -> Result<Value, E> {
        Ok(v.into())
    }
    fn visit_i64<E: de::Error>(self, v: i64) -> Result<Value, E> {
        Ok(v.into())
    }
    fn visit_u64<E: de::Error>(self, v: u64) -> Result<Value, E> {
        Ok(v.into())
    }
    fn visit_f64<E: de::Error>(self, v: f64) -> Result<Value, E> {
        Ok(Value::from(v))
    }
    fn visit_str<E: de::Error>(self, v: &str) -> Result<Value, E> {
        Ok(v.into())
    }
    fn visit_string<E: de::Error>(self, v: String) -> Result<Value, E> {
        Ok(v.into())
    }
    fn visit_unit<E: de::Error>(self) -> Result<Value, E> {
        Ok(Value::Null)
    }
    fn visit_seq<A: SeqAccess<'de>>(self, mut a: A) -> Result<Value, A::Error> {
        if self.0 >= 16 {
            return Err(de::Error::custom("depth"));
        }
        let mut out = vec![];
        while let Some(v) = a.next_element_seed(Strict(self.0 + 1))? {
            out.push(v);
        }
        Ok(Value::Array(out))
    }
    fn visit_map<A: MapAccess<'de>>(self, mut a: A) -> Result<Value, A::Error> {
        if self.0 >= 16 {
            return Err(de::Error::custom("depth"));
        }
        let mut out = serde_json::Map::new();
        while let Some(key) = a.next_key::<String>()? {
            if out.contains_key(&key) {
                return Err(de::Error::custom("duplicate"));
            }
            out.insert(key, a.next_value_seed(Strict(self.0 + 1))?);
        }
        Ok(Value::Object(out))
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Payload {
    schema_version: u32,
    rules: Vec<Contract>,
}

/// Optional Markdown wrapper; only an exact top-level versioned fence activates.
pub fn parse(raw: &[u8]) -> Result<Vec<Contract>, InvalidSource> {
    if raw.len() > MAX_BYTES {
        return Err(InvalidSource);
    }
    let text = std::str::from_utf8(raw).map_err(|_| InvalidSource)?;
    let mut ordinary: Option<(u8, usize)> = None;
    let mut inside = false;
    let mut found = false;
    let mut json = String::new();
    for line in text.lines() {
        if inside {
            if line == "```" {
                inside = false;
            } else {
                json.push_str(line);
                json.push('\n');
            }
            continue;
        }
        let trimmed = line.trim_start_matches(' ');
        if line.len() - trimmed.len() > 3 {
            continue;
        }
        let bytes = trimmed.as_bytes();
        let marker = bytes.first().copied().unwrap_or(0);
        let count = bytes.iter().take_while(|&&b| b == marker).count();
        if let Some((kind, length)) = ordinary {
            if marker == kind && count >= length && trimmed[count..].trim().is_empty() {
                ordinary = None;
            }
            continue;
        }
        if line.starts_with("```acp-contracts-") {
            if found || line != "```acp-contracts-v1" {
                return Err(InvalidSource);
            }
            inside = true;
            found = true;
        } else if matches!(marker, b'`' | b'~') && count >= 3 {
            ordinary = Some((marker, count));
        }
    }
    if !found || inside || ordinary.is_some() {
        return Err(InvalidSource);
    }
    let mut decoder = serde_json::Deserializer::from_str(&json);
    let value = Strict(0)
        .deserialize(&mut decoder)
        .map_err(|_| InvalidSource)?;
    decoder.end().map_err(|_| InvalidSource)?;
    let payload: Payload = serde_json::from_value(value).map_err(|_| InvalidSource)?;
    if payload.schema_version != 1 || payload.rules.len() > 64 {
        return Err(InvalidSource);
    }
    for c in &payload.rules {
        if c.allowed_values.len() > 64
            || c.supersedes.len() > 64
            || [
                &c.selector.agents,
                &c.selector.paths,
                &c.selector.languages,
                &c.selector.task_tags,
            ]
            .into_iter()
            .flatten()
            .any(|v| v.len() > 64)
        {
            return Err(InvalidSource);
        }
    }
    // Validate even pending rules with the same frozen typed-v1 grammar. Temporarily
    // mark them reviewed for validation only; the returned claim is never promoted.
    let mut rules: Vec<_> = payload
        .rules
        .iter()
        .map(|c| c.rule("bound", "validation"))
        .collect();
    for r in &mut rules {
        r.review = typed::ReviewState::Accepted;
    }
    let resolved = typed::resolve(&SelectorQuery::default().input("bound", rules))
        .map_err(|_| InvalidSource)?;
    if resolved.explanations.iter().any(|e| {
        e.reasons.iter().any(|r| {
            matches!(
                r,
                typed::Reason::InvalidSelector
                    | typed::Reason::InvalidSchema
                    | typed::Reason::InvalidRuleShape
                    | typed::Reason::InvalidIdentity
                    | typed::Reason::InvalidSource
            )
        })
    }) {
        return Err(InvalidSource);
    }
    Ok(payload.rules)
}
