use super::{Candidate, RuleOptions, Scan, SourceKind, scan::slug};

fn without_pointer(text: &str) -> String {
    let mut out = String::new();
    let mut inside = false;
    for line in text.lines() {
        let t = line.trim();
        if t.starts_with("<!-- precept:begin") {
            inside = true;
            continue;
        }
        if inside {
            if t == crate::pointer::END {
                inside = false;
            }
            continue;
        }
        out.push_str(line);
        out.push('\n');
    }
    out
}

pub(super) fn first_heading(text: &str) -> Option<String> {
    let mut fence = false;
    for line in text.lines() {
        if line.trim_start().starts_with("```") {
            fence = !fence;
        } else if !fence && line.starts_with("# ") {
            return Some(line[2..].trim().into());
        }
    }
    None
}

pub(super) fn sections(source: &str, kind: SourceKind, text: &str, scan: &mut Scan) {
    let text = without_pointer(text);
    let mut parts: Vec<(usize, String, String)> = Vec::new();
    let mut fence = false;
    for line in text.lines() {
        if line.trim_start().starts_with("```") {
            fence = !fence;
        }
        let level = if fence {
            0
        } else if line.starts_with("### ") {
            3
        } else if line.starts_with("## ") {
            2
        } else {
            0
        };
        if level > 0 {
            parts.push((level, line[level + 1..].trim().into(), String::new()));
        } else if let Some(last) = parts.last_mut() {
            last.2.push_str(line);
            last.2.push('\n');
        }
    }
    let mut parent = String::new();
    for (index, (level, title, body)) in parts.iter().enumerate() {
        if scan.exhausted() {
            break;
        }
        let part = |t: &str| {
            let s = slug(t);
            if s.is_empty() {
                format!("section-{}", index + 1)
            } else {
                s
            }
        };
        let (full, key) = if *level == 2 {
            parent = title.clone();
            (title.clone(), part(title))
        } else {
            (
                format!("{parent} {title}"),
                format!("{}-{}", part(&parent), part(title)),
            )
        };
        let has_children =
            *level == 2 && parts.get(index + 1).is_some_and(|(next, _, _)| *next == 3);
        if has_children && body.trim().is_empty() {
            continue;
        }
        scan.candidate(Candidate {
            source: source.into(),
            kind,
            title: full,
            description: None,
            key,
            value: body.clone(),
            tags: vec!["imported".into(), kind.tag().into()],
            rule: RuleOptions::default(),
        });
    }
}
