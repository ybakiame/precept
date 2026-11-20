//! The AGENTS.md pointer: a short, fixed, Precept-marked block telling any
//! agent that reads `AGENTS.md` where this project's reviewed context is and
//! when to read it. Pure text functions; the daemon does the file I/O, only on
//! an explicit operator action. Everything outside the markers is preserved
//! byte for byte; writing the same block twice changes nothing.
use sha2::{Digest, Sha256};

pub const FILE: &str = "AGENTS.md";
pub const BEGIN: &str = "<!-- precept:begin v1 -->";
const BEGIN_PREFIX: &str = "<!-- precept:begin";
pub const END: &str = "<!-- precept:end -->";
/// The largest AGENTS.md Precept edits.
pub const MAX_BYTES: usize = 1024 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum State {
    /// No file, or a file without the block.
    Absent,
    /// The current block.
    Current,
    /// A Precept block of another version or text.
    Outdated,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Malformed;

/// The block for `project` (no trailing newline).
pub fn block(project: &str) -> String {
    format!(
        "{BEGIN}\n## Precept\n\nThis project's confirmed engineering context (the developer's reviewed \
preferences and the repository's contracts) is served by Precept. Before planning or editing, call \
`get_effective_context` on the `precept-{project}` MCP server, and again after your context is \
compacted. Current repository files and contracts outrank those preferences: report a conflict \
instead of choosing silently.\n{END}"
    )
}

/// Lowercase hex SHA-256 of `bytes` (the preview's identity of a file).
pub fn digest(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// The byte range of the existing block (markers included), if exactly one.
fn span(text: &str) -> Result<Option<(usize, usize)>, Malformed> {
    let begins: Vec<usize> = text.match_indices(BEGIN_PREFIX).map(|(i, _)| i).collect();
    let ends: Vec<usize> = text.match_indices(END).map(|(i, _)| i).collect();
    match (begins.as_slice(), ends.as_slice()) {
        ([], []) => Ok(None),
        ([b], [e]) if b < e => Ok(Some((*b, e + END.len()))),
        _ => Err(Malformed),
    }
}

pub fn state(existing: Option<&str>, project: &str) -> Result<State, Malformed> {
    let Some(text) = existing else {
        return Ok(State::Absent);
    };
    Ok(match span(text)? {
        None => State::Absent,
        Some((b, e)) if text[b..e] == block(project) => State::Current,
        Some(_) => State::Outdated,
    })
}

/// `existing` (or a new file) with the current block in place of an old one,
/// or appended after a blank line.
pub fn upsert(existing: Option<&str>, project: &str) -> Result<String, Malformed> {
    let block = block(project);
    let Some(text) = existing else {
        return Ok(format!("{block}\n"));
    };
    Ok(match span(text)? {
        Some((b, e)) => format!("{}{block}{}", &text[..b], &text[e..]),
        None if text.trim().is_empty() => format!("{block}\n"),
        None => {
            let base = text.trim_end_matches('\n');
            format!("{base}\n\n{block}\n")
        }
    })
}

/// `existing` without the block (and the blank line before it), or `None`
/// when nothing else would remain (the file can be deleted).
pub fn remove(existing: &str) -> Result<Option<String>, Malformed> {
    let Some((b, e)) = span(existing)? else {
        return Ok(Some(existing.to_owned()));
    };
    let before = existing[..b].trim_end_matches('\n');
    let after = existing[e..].trim_start_matches('\n');
    let joined = match (before.is_empty(), after.is_empty()) {
        (true, true) => return Ok(None),
        (true, false) => after.to_owned(),
        (false, true) => format!("{before}\n"),
        (false, false) => format!("{before}\n\n{after}"),
    };
    Ok(Some(joined))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn upsert_is_idempotent_and_preserves_everything_else() {
        let created = upsert(None, "alpha").unwrap();
        assert_eq!(state(Some(&created), "alpha"), Ok(State::Current));
        assert_eq!(upsert(Some(&created), "alpha").unwrap(), created);
        assert_eq!(remove(&created), Ok(None));

        let prose = "# Repo rules\n\nUse tabs.\n";
        let with = upsert(Some(prose), "alpha").unwrap();
        assert!(with.starts_with(prose));
        assert_eq!(upsert(Some(&with), "alpha").unwrap(), with);
        assert_eq!(remove(&with).unwrap().unwrap(), prose);

        let middle = format!("Top\n\n{}\n\nBottom\n", block("old"));
        assert_eq!(state(Some(&middle), "alpha"), Ok(State::Outdated));
        let updated = upsert(Some(&middle), "alpha").unwrap();
        assert_eq!(updated, format!("Top\n\n{}\n\nBottom\n", block("alpha")));
        assert_eq!(remove(&updated).unwrap().unwrap(), "Top\n\nBottom\n");
        let v0 = middle.replace("precept:begin v1", "precept:begin v0");
        assert_eq!(state(Some(&v0), "old"), Ok(State::Outdated));
    }

    #[test]
    fn unbalanced_markers_are_refused() {
        for bad in [
            format!("x\n{BEGIN}\n"),
            format!("{END}\n{BEGIN}\n"),
            format!("{}\n{}\n", block("a"), block("a")),
        ] {
            assert_eq!(upsert(Some(&bad), "a"), Err(Malformed));
            assert_eq!(remove(&bad), Err(Malformed));
            assert_eq!(state(Some(&bad), "a"), Err(Malformed));
        }
        assert_eq!(state(None, "a"), Ok(State::Absent));
        assert_eq!(digest(b"").len(), 64);
    }
}
