//! JSON that keeps object keys in file order, for editing other tools'
//! configuration files without reordering them. (`serde_json::Value` sorts
//! keys here; turning on its order-preserving feature would change it for
//! the whole binary.) Numbers keep their exact text through `serde_json`.
use serde::{
    Deserialize, Deserializer, Serialize, Serializer,
    de::{MapAccess, SeqAccess, Visitor},
    ser::{SerializeMap, SerializeSeq},
};
use std::fmt;

#[derive(Clone, Debug, PartialEq)]
pub enum J {
    Null,
    Bool(bool),
    Num(serde_json::Number),
    Str(String),
    Arr(Vec<J>),
    Obj(Vec<(String, J)>),
}

impl J {
    pub fn get(&self, key: &str) -> Option<&J> {
        match self {
            J::Obj(m) => m.iter().find(|(k, _)| k == key).map(|(_, v)| v),
            _ => None,
        }
    }
    /// The value under `key`, inserted (at the end) as `default` when absent.
    pub fn entry(&mut self, key: &str, default: J) -> Option<&mut J> {
        let J::Obj(m) = self else { return None };
        let i = match m.iter().position(|(k, _)| k == key) {
            Some(i) => i,
            None => {
                m.push((key.to_owned(), default));
                m.len() - 1
            }
        };
        Some(&mut m[i].1)
    }
    pub fn remove(&mut self, key: &str) {
        if let J::Obj(m) = self {
            m.retain(|(k, _)| k != key);
        }
    }
    pub fn as_str(&self) -> Option<&str> {
        match self {
            J::Str(s) => Some(s),
            _ => None,
        }
    }
    pub fn str(s: impl Into<String>) -> J {
        J::Str(s.into())
    }
}

impl Serialize for J {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        match self {
            J::Null => s.serialize_unit(),
            J::Bool(b) => s.serialize_bool(*b),
            J::Num(n) => n.serialize(s),
            J::Str(v) => s.serialize_str(v),
            J::Arr(a) => {
                let mut seq = s.serialize_seq(Some(a.len()))?;
                for v in a {
                    seq.serialize_element(v)?;
                }
                seq.end()
            }
            J::Obj(m) => {
                let mut map = s.serialize_map(Some(m.len()))?;
                for (k, v) in m {
                    map.serialize_entry(k, v)?;
                }
                map.end()
            }
        }
    }
}

struct JVisitor;
impl<'de> Visitor<'de> for JVisitor {
    type Value = J;
    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("JSON")
    }
    fn visit_unit<E>(self) -> Result<J, E> {
        Ok(J::Null)
    }
    fn visit_bool<E>(self, v: bool) -> Result<J, E> {
        Ok(J::Bool(v))
    }
    fn visit_i64<E>(self, v: i64) -> Result<J, E> {
        Ok(J::Num(v.into()))
    }
    fn visit_u64<E>(self, v: u64) -> Result<J, E> {
        Ok(J::Num(v.into()))
    }
    fn visit_f64<E: serde::de::Error>(self, v: f64) -> Result<J, E> {
        serde_json::Number::from_f64(v)
            .map(J::Num)
            .ok_or_else(|| E::custom("number"))
    }
    fn visit_str<E>(self, v: &str) -> Result<J, E> {
        Ok(J::Str(v.to_owned()))
    }
    fn visit_string<E>(self, v: String) -> Result<J, E> {
        Ok(J::Str(v))
    }
    fn visit_seq<A: SeqAccess<'de>>(self, mut a: A) -> Result<J, A::Error> {
        let mut out = Vec::new();
        while let Some(v) = a.next_element()? {
            out.push(v);
        }
        Ok(J::Arr(out))
    }
    fn visit_map<A: MapAccess<'de>>(self, mut a: A) -> Result<J, A::Error> {
        let mut out: Vec<(String, J)> = Vec::new();
        while let Some((k, v)) = a.next_entry::<String, J>()? {
            // A repeated key keeps its first place, the last value (as JSON readers do).
            match out.iter_mut().find(|(e, _)| *e == k) {
                Some(slot) => slot.1 = v,
                None => out.push((k, v)),
            }
        }
        Ok(J::Obj(out))
    }
}
impl<'de> Deserialize<'de> for J {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<J, D::Error> {
        d.deserialize_any(JVisitor)
    }
}

/// Parses `text`; `None` when it is not JSON.
pub fn parse(text: &str) -> Option<J> {
    serde_json::from_str(text).ok()
}
/// Two-space pretty JSON with a final newline.
pub fn pretty(j: &J) -> String {
    let mut out = serde_json::to_string_pretty(j).unwrap_or_default();
    out.push('\n');
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn keeps_key_order_and_numbers() {
        let text = "{\n  \"zeta\": 1,\n  \"alpha\": {\"b\": 2.50, \"a\": [true, null, \"x\"]},\n  \"mid\": 12345678901234567890\n}\n";
        let j = parse(text).unwrap();
        let out = pretty(&j);
        let keys: Vec<&str> = match &j {
            J::Obj(m) => m.iter().map(|(k, _)| k.as_str()).collect(),
            _ => vec![],
        };
        assert_eq!(keys, ["zeta", "alpha", "mid"]);
        assert!(out.find("\"b\"").unwrap() < out.find("\"a\"").unwrap());
        assert!(out.contains("12345678901234567890"));
        assert_eq!(parse(&out).unwrap(), j);
        assert!(parse("{bad").is_none());
    }
}
