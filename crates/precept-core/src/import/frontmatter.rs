use super::{
    Candidate, FrontmatterKind, Scan, Skipped, SourceSpec, markdown::first_heading, scan::slug,
};
use crate::operator::{AppliesTo, RuleOptions};
use std::collections::BTreeMap;

#[derive(Debug, PartialEq, Eq)]
enum FrontmatterError {
    Malformed,
    UnsupportedApplicability(String),
}

#[derive(Debug, Default)]
struct Metadata {
    title: Option<String>,
    name: Option<String>,
    description: Option<String>,
    always_apply: Option<bool>,
    selectors: Vec<String>,
}

#[derive(Debug)]
enum Value {
    Scalar(String),
    List(Vec<String>),
}

fn scalar(raw: &str) -> Result<String, FrontmatterError> {
    let raw = raw.trim();
    if raw.is_empty() {
        return Ok(String::new());
    }
    if raw.starts_with('"') {
        return serde_json::from_str(raw).map_err(|_| FrontmatterError::Malformed);
    }
    if raw.starts_with('\'') {
        if raw.len() < 2 || !raw.ends_with('\'') {
            return Err(FrontmatterError::Malformed);
        }
        return Ok(raw[1..raw.len() - 1].replace("''", "'"));
    }
    if raw.ends_with('"') || raw.ends_with('\'') || raw.starts_with(['&', '*', '|', '>']) {
        return Err(FrontmatterError::Malformed);
    }
    Ok(raw.into())
}

fn list(raw: &str) -> Result<Vec<String>, FrontmatterError> {
    let raw = raw.trim();
    if !raw.starts_with('[') || !raw.ends_with(']') {
        return Err(FrontmatterError::Malformed);
    }
    let inner = &raw[1..raw.len() - 1];
    if inner.trim().is_empty() {
        return Ok(Vec::new());
    }
    let mut out = Vec::new();
    let mut quoted = None;
    let mut escaped = false;
    let mut start = 0;
    for (index, c) in inner.char_indices() {
        if escaped {
            escaped = false;
            continue;
        }
        if quoted == Some('"') && c == '\\' {
            escaped = true;
            continue;
        }
        if let Some(quote) = quoted {
            if c == quote {
                quoted = None;
            }
            continue;
        }
        if matches!(c, '\'' | '"') {
            quoted = Some(c);
        } else if c == ',' {
            out.push(scalar(&inner[start..index])?);
            start = index + 1;
        }
    }
    if quoted.is_some() {
        return Err(FrontmatterError::Malformed);
    }
    out.push(scalar(&inner[start..])?);
    Ok(out)
}

fn frontmatter(text: &str) -> Result<Option<(Metadata, String)>, FrontmatterError> {
    let mut lines = text.lines();
    if lines.next() != Some("---") {
        return Ok(None);
    }
    let mut fields: BTreeMap<String, Value> = BTreeMap::new();
    let mut current: Option<String> = None;
    let mut block_list: Option<String> = None;
    let mut closed = false;
    for line in lines.by_ref() {
        if line == "---" {
            closed = true;
            break;
        }
        if line.contains('\t') {
            return Err(FrontmatterError::Malformed);
        }
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        if let Some(item) = trimmed.strip_prefix("- ") {
            let key = current.clone().ok_or(FrontmatterError::Malformed)?;
            if block_list.as_deref() != Some(key.as_str()) {
                return Err(FrontmatterError::Malformed);
            }
            let Value::List(values) = fields.get_mut(&key).ok_or(FrontmatterError::Malformed)?
            else {
                return Err(FrontmatterError::Malformed);
            };
            values.push(scalar(item)?);
            continue;
        }
        if line.starts_with(char::is_whitespace) {
            return Err(FrontmatterError::Malformed);
        }
        let (key, raw) = line.split_once(':').ok_or(FrontmatterError::Malformed)?;
        if key.is_empty()
            || !key
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-'))
            || fields.contains_key(key)
        {
            return Err(FrontmatterError::Malformed);
        }
        let value = if raw.trim().starts_with('[') {
            block_list = None;
            Value::List(list(raw)?)
        } else if raw.trim().is_empty() && matches!(key, "globs" | "applyTo" | "paths") {
            block_list = Some(key.into());
            Value::List(Vec::new())
        } else {
            block_list = None;
            Value::Scalar(scalar(raw)?)
        };
        current = Some(key.into());
        fields.insert(key.into(), value);
    }
    if !closed {
        return Err(FrontmatterError::Malformed);
    }
    let body = lines.collect::<Vec<_>>().join("\n");
    let get = |key: &str| -> Result<Option<&str>, FrontmatterError> {
        match fields.get(key) {
            None => Ok(None),
            Some(Value::Scalar(value)) => Ok(Some(value.as_str())),
            Some(Value::List(_)) => Err(FrontmatterError::Malformed),
        }
    };
    let mut metadata = Metadata {
        title: get("title")?.map(str::to_owned),
        name: get("name")?.map(str::to_owned),
        description: get("description")?.map(str::to_owned),
        ..Metadata::default()
    };
    if let Some(raw) = get("alwaysApply")? {
        metadata.always_apply = Some(match raw {
            "true" => true,
            "false" => false,
            _ => return Err(FrontmatterError::Malformed),
        });
    }
    for key in ["globs", "applyTo", "paths"] {
        if let Some(value) = fields.get(key) {
            match value {
                Value::Scalar(value) => metadata.selectors.extend(
                    value
                        .split(',')
                        .map(str::trim)
                        .filter(|value| !value.is_empty())
                        .map(str::to_owned),
                ),
                Value::List(values) => metadata.selectors.extend(values.iter().cloned()),
            }
        }
    }
    Ok(Some((metadata, body)))
}

fn allowed(kind: FrontmatterKind, key: &str) -> bool {
    match kind {
        FrontmatterKind::Cursor => matches!(key, "description" | "globs" | "alwaysApply"),
        FrontmatterKind::Continue => {
            matches!(key, "name" | "description" | "globs" | "alwaysApply")
        }
        FrontmatterKind::Copilot => matches!(key, "description" | "applyTo"),
        FrontmatterKind::Claude => matches!(key, "description" | "paths"),
    }
}

fn rule_options(
    kind: FrontmatterKind,
    metadata: &Metadata,
) -> Result<RuleOptions, FrontmatterError> {
    if metadata.always_apply == Some(true) {
        return Ok(RuleOptions::default());
    }
    if metadata.selectors.is_empty() {
        return if metadata.always_apply == Some(false) || kind == FrontmatterKind::Cursor {
            Err(FrontmatterError::UnsupportedApplicability(
                "manual activation".into(),
            ))
        } else {
            Ok(RuleOptions::default())
        };
    }
    let paths: Vec<String> = metadata
        .selectors
        .iter()
        .map(|s| s.trim().to_owned())
        .collect();
    let applies = AppliesTo {
        paths,
        ..AppliesTo::default()
    };
    let Some(applies) = applies.canonical() else {
        return Err(FrontmatterError::UnsupportedApplicability(
            metadata.selectors.join(", "),
        ));
    };
    Ok(RuleOptions {
        applies_to: applies,
        ..RuleOptions::default()
    })
}

fn file_stem(source: &str) -> String {
    std::path::Path::new(source)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("rule")
        .into()
}

pub(super) fn single(source: &str, spec: SourceSpec, text: &str, scan: &mut Scan) {
    let parsed = if spec.frontmatter.is_some() {
        match frontmatter(text.strip_prefix('\u{feff}').unwrap_or(text)) {
            Ok(parsed) => parsed,
            Err(FrontmatterError::Malformed) => {
                scan.skip(Skipped {
                    source: source.into(),
                    title: file_stem(source),
                    reason: "malformed_frontmatter",
                    detail: None,
                });
                return;
            }
            Err(FrontmatterError::UnsupportedApplicability(_)) => unreachable!(),
        }
    } else {
        None
    };
    let mut metadata = Metadata::default();
    let mut body = text.to_owned();
    if let Some((parsed, content)) = parsed {
        body = content;
        metadata = parsed;
        if let Some(kind) = spec.frontmatter {
            for line in text.lines().skip(1).take_while(|line| *line != "---") {
                let Some((key, _)) = line.split_once(':') else {
                    continue;
                };
                if !line.starts_with(char::is_whitespace) && !allowed(kind, key) {
                    scan.skip(Skipped {
                        source: source.into(),
                        title: file_stem(source),
                        reason: "unsupported_metadata",
                        detail: Some(key.into()),
                    });
                    return;
                }
            }
        }
    } else if matches!(spec.frontmatter, Some(FrontmatterKind::Cursor)) {
        scan.skip(Skipped {
            source: source.into(),
            title: file_stem(source),
            reason: "unsupported_metadata",
            detail: Some("activation".into()),
        });
        return;
    }
    let rule = match spec.frontmatter {
        Some(kind) => match rule_options(kind, &metadata) {
            Ok(rule) => rule,
            Err(FrontmatterError::UnsupportedApplicability(detail)) => {
                scan.skip(Skipped {
                    source: source.into(),
                    title: file_stem(source),
                    reason: "unsupported_applicability",
                    detail: Some(detail),
                });
                return;
            }
            Err(_) => unreachable!(),
        },
        None => RuleOptions::default(),
    };
    let title = metadata
        .title
        .filter(|s| !s.trim().is_empty())
        .or_else(|| metadata.name.filter(|s| !s.trim().is_empty()))
        .or_else(|| first_heading(&body))
        .unwrap_or_else(|| file_stem(source));
    scan.candidate(Candidate {
        source: source.into(),
        kind: spec.kind,
        title,
        description: metadata.description,
        key: slug(&file_stem(source)),
        value: body,
        tags: vec!["imported".into(), spec.kind.tag().into()],
        rule,
    });
}
