use super::{
    Candidate, DAEMON_FIELD_RESERVE, MAX_CANDIDATES, MAX_DETAIL, MAX_KEY, MAX_REPLY_BYTES,
    MAX_SKIPPED, MAX_VALUE, REPLY_RESERVE, Skipped,
};
use sha2::{Digest, Sha256};

#[derive(Default)]
pub struct Scan {
    pub candidates: Vec<Candidate>,
    pub skipped: Vec<Skipped>,
    pub(super) bytes: usize,
    exhausted: bool,
}

impl Scan {
    pub fn exhausted(&self) -> bool {
        self.exhausted
    }

    pub fn into_parts(self) -> (Vec<Candidate>, Vec<Skipped>) {
        (self.candidates, self.skipped)
    }

    pub fn skip(&mut self, mut skipped: Skipped) {
        if self.exhausted || self.skipped.len() >= MAX_SKIPPED {
            self.exhausted = true;
            return;
        }
        skipped.detail = skipped.detail.map(|d| {
            d.chars()
                .map(|c| if c.is_control() { ' ' } else { c })
                .take(MAX_DETAIL)
                .collect()
        });
        let size = serde_json::to_vec(&skipped).map_or(MAX_REPLY_BYTES, |v| v.len());
        let limit = if skipped.reason == "budget_exhausted" {
            MAX_REPLY_BYTES
        } else {
            MAX_REPLY_BYTES - REPLY_RESERVE
        };
        if self.bytes.saturating_add(size) > limit {
            self.exhausted = true;
            return;
        }
        self.bytes += size;
        self.skipped.push(skipped);
    }

    fn budget(&mut self, source: &str, title: &str) {
        if self.exhausted {
            return;
        }
        self.skip(Skipped {
            source: source.into(),
            title: title.into(),
            reason: "budget_exhausted",
            detail: None,
        });
        self.exhausted = true;
    }

    pub(super) fn candidate(&mut self, mut candidate: Candidate) {
        if self.exhausted {
            return;
        }
        candidate.value = candidate.value.trim().to_owned();
        candidate.title = candidate
            .title
            .trim()
            .chars()
            .map(|c| if c.is_control() { ' ' } else { c })
            .take(256)
            .collect();
        candidate.description = candidate.description.map(|d| {
            d.trim()
                .chars()
                .map(|c| if c.is_control() { ' ' } else { c })
                .take(MAX_DETAIL)
                .collect()
        });
        candidate.tags = crate::operator::canonical_tags(&candidate.tags)
            .unwrap_or_else(|| vec!["imported".into()]);
        if candidate.value.is_empty() {
            self.skip(Skipped {
                source: candidate.source,
                title: candidate.title,
                reason: "empty",
                detail: None,
            });
            return;
        }
        if candidate.value.len() > MAX_VALUE {
            self.skip(Skipped {
                source: candidate.source,
                title: candidate.title,
                reason: "too_long",
                detail: None,
            });
            return;
        }
        if let Some(existing) = self
            .candidates
            .iter()
            .find(|c| c.value == candidate.value && c.rule == candidate.rule)
        {
            self.skip(Skipped {
                source: candidate.source,
                title: candidate.title,
                reason: "duplicate",
                detail: Some(existing.source.clone()),
            });
            return;
        }
        if self.candidates.len() >= MAX_CANDIDATES {
            self.budget(&candidate.source, &candidate.title);
            return;
        }
        candidate.key = unique_key(
            &format!("{}-{}", candidate.kind.prefix(), candidate.key),
            &candidate.source,
            &candidate.title,
            &self.candidates,
        );
        let size = serde_json::to_vec(&candidate).map_or(MAX_REPLY_BYTES, |v| v.len());
        let reply_size = size.saturating_add(DAEMON_FIELD_RESERVE);
        if self.bytes.saturating_add(reply_size) > MAX_REPLY_BYTES - REPLY_RESERVE {
            self.budget(&candidate.source, &candidate.title);
            return;
        }
        self.bytes += reply_size;
        self.candidates.push(candidate);
    }
}

pub(super) fn slug(title: &str) -> String {
    let mut out = String::new();
    for c in title.chars().flat_map(char::to_lowercase) {
        if c.is_ascii_alphanumeric() {
            out.push(c);
        } else if !out.is_empty() && !out.ends_with('-') {
            out.push('-');
        }
    }
    while out.ends_with('-') {
        out.pop();
    }
    out.truncate(80);
    while out.ends_with('-') {
        out.pop();
    }
    out
}

fn short_hash(source: &str, title: &str) -> String {
    let digest = Sha256::digest(format!("{source}\0{title}").as_bytes());
    let hex = format!("{digest:x}");
    hex[..16].into()
}

fn fit_key(base: &str, source: &str, title: &str) -> String {
    if base.len() <= MAX_KEY {
        return base.into();
    }
    let suffix = format!("-{}", short_hash(source, title));
    let keep = MAX_KEY - suffix.len();
    let mut out = base[..keep].trim_end_matches('-').to_owned();
    while out.len() + suffix.len() > MAX_KEY {
        out.pop();
    }
    out.push_str(&suffix);
    out
}

fn unique_key(base: &str, source: &str, title: &str, candidates: &[Candidate]) -> String {
    let mut key = fit_key(base, source, title);
    let mut n = 2;
    while candidates.iter().any(|c| c.key == key) {
        key = fit_key(&format!("{base}-{n}"), source, &format!("{title}-{n}"));
        n += 1;
    }
    key
}
