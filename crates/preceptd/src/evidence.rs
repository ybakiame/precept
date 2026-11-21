//! Evidence: each project's agent activity, persisted as a hash chain so it
//! survives restarts and any change in the middle shows. One line per agent
//! request (what kind, which client and session, the outcome and counts;
//! never values, selectors, paths or file contents), each carrying the
//! SHA-256 of the line before. Integrity only, not a signature: whoever can
//! write the daemon's private state can rewrite it whole, but not edit or
//! drop an entry unnoticed. Bounded; the oldest entries are dropped with the
//! hash of the last one kept as the chain's base.
use crate::activity::{Counts, Kind, Via};
use crate::state::StateDir;
use precept_core::pointer::digest;
use precept_ipc::Error;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::io::Write;
use std::sync::Mutex;

/// The most entries kept per project; past it the file is compacted to
/// [`KEEP`].
pub const MAX_ENTRIES: usize = 1500;
pub const KEEP: usize = 1000;
/// The largest evidence file read.
const MAX_FILE: u64 = 2 * 1024 * 1024;
/// The chain's base before anything was dropped.
const GENESIS: &str = "genesis";

/// What one entry records (hashed together with the previous hash).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Body {
    pub seq: u64,
    pub at_ms: u64,
    pub kind: Kind,
    pub via: Via,
    pub client: Option<String>,
    pub session: Option<String>,
    pub outcome: String,
    pub duration_ms: u64,
    pub counts: Option<Counts>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Line {
    #[serde(flatten)]
    body: Body,
    prev: String,
    hash: String,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Header {
    format: u32,
    base: String,
    dropped: u64,
}

fn hash(prev: &str, body: &Body) -> String {
    let json = serde_json::to_string(body).unwrap_or_default();
    digest(format!("{prev}\n{json}").as_bytes())
}

/// A project's chain as loaded: its entries, base and verification.
#[derive(Default)]
struct Chain {
    base: String,
    dropped: u64,
    lines: Vec<Line>,
    /// The first entry whose hash or link does not hold, if any.
    broken_at: Option<u64>,
}

impl Chain {
    fn verify(&mut self) {
        let mut prev = self.base.clone();
        self.broken_at = None;
        for l in &self.lines {
            if l.prev != prev || l.hash != hash(&l.prev, &l.body) {
                self.broken_at = Some(l.body.seq);
                return;
            }
            prev = l.hash.clone();
        }
    }
    fn last(&self) -> (String, u64) {
        self.lines.last().map_or_else(
            || (self.base.clone(), self.dropped),
            |l| (l.hash.clone(), l.body.seq),
        )
    }
}

/// The chain's state for display.
#[derive(Serialize, Debug, PartialEq, Eq)]
pub struct Status {
    pub ok: bool,
    pub entries: usize,
    pub dropped: u64,
    pub broken_at: Option<u64>,
}

/// Every project's chain, loaded on first use and kept in memory.
pub struct Evidence {
    chains: Mutex<BTreeMap<String, Chain>>,
}

fn file_name(project: &str) -> String {
    format!("evidence-{project}.jsonl")
}

fn load(state: &StateDir, project: &str) -> Result<Chain, Error> {
    let path = state.path().join(file_name(project));
    let meta = match std::fs::symlink_metadata(&path) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Ok(Chain {
                base: GENESIS.into(),
                ..Default::default()
            });
        }
        Err(_) => return Err(Error::UnsafeRuntime),
        Ok(m) => m,
    };
    if !meta.file_type().is_file() || meta.len() > MAX_FILE {
        return Err(Error::UnsafeRuntime);
    }
    let mut file = precept_ipc::transport::open_private(&path, false)?;
    let mut text = String::new();
    std::io::Read::read_to_string(&mut file, &mut text).map_err(|_| Error::InvalidRequest)?;
    let mut lines = text.lines();
    let header: Header = lines
        .next()
        .and_then(|h| serde_json::from_str(h).ok())
        .filter(|h: &Header| h.format == 1)
        .ok_or(Error::InvalidRequest)?;
    let mut chain = Chain {
        base: header.base,
        dropped: header.dropped,
        ..Default::default()
    };
    for (i, raw) in lines.enumerate() {
        match serde_json::from_str::<Line>(raw) {
            Ok(l) => chain.lines.push(l),
            // An unreadable line breaks the chain where it stands.
            Err(_) => {
                chain.broken_at = Some(chain.dropped + i as u64 + 1);
                return Ok(chain);
            }
        }
    }
    chain.verify();
    Ok(chain)
}

fn rewrite(state: &StateDir, project: &str, chain: &Chain) -> Result<(), Error> {
    let mut body = serde_json::to_string(&Header {
        format: 1,
        base: chain.base.clone(),
        dropped: chain.dropped,
    })
    .map_err(|_| Error::Unavailable)?;
    body.push('\n');
    for l in &chain.lines {
        body.push_str(&serde_json::to_string(l).map_err(|_| Error::Unavailable)?);
        body.push('\n');
    }
    state.write(&file_name(project), body.as_bytes())
}

/// One entry to append (the chain fills in its number and hashes).
pub struct Entry {
    pub kind: Kind,
    pub via: Via,
    pub client: Option<String>,
    pub session: Option<String>,
    pub outcome: String,
    pub duration_ms: u64,
    pub counts: Option<Counts>,
    pub at_ms: u64,
}

impl Evidence {
    pub fn new() -> Self {
        Self {
            chains: Mutex::new(BTreeMap::new()),
        }
    }
    fn with<T>(
        &self,
        state: &StateDir,
        project: &str,
        f: impl FnOnce(&mut Chain) -> Result<T, Error>,
    ) -> Result<T, Error> {
        let mut chains = self.chains.lock().unwrap_or_else(|e| e.into_inner());
        if !chains.contains_key(project) {
            chains.insert(project.to_owned(), load(state, project)?);
        }
        f(chains.get_mut(project).expect("loaded above"))
    }
    /// Appends one entry. A broken chain is never extended silently: the
    /// entry is still kept, linked to the last hash, and the break stays
    /// reported.
    pub fn append(&self, state: &StateDir, project: &str, e: Entry) -> Result<(), Error> {
        self.with(state, project, |chain| {
            let (prev, seq) = chain.last();
            let body = Body {
                seq: seq + 1,
                at_ms: e.at_ms,
                kind: e.kind,
                via: e.via,
                client: e.client,
                session: e.session,
                outcome: e.outcome,
                duration_ms: e.duration_ms,
                counts: e.counts,
            };
            let line = Line {
                hash: hash(&prev, &body),
                prev,
                body,
            };
            chain.lines.push(line.clone());
            if chain.lines.len() > MAX_ENTRIES {
                let drop = chain.lines.len() - KEEP;
                let last_dropped = chain.lines[drop - 1].clone();
                chain.lines.drain(..drop);
                chain.base = last_dropped.hash;
                chain.dropped = last_dropped.body.seq;
                return rewrite(state, project, chain);
            }
            let path = state.path().join(file_name(project));
            if std::fs::symlink_metadata(&path).is_err() || chain.lines.len() == 1 {
                return rewrite(state, project, chain);
            }
            let mut file = precept_ipc::transport::open_private(&path, true)?;
            std::io::Seek::seek(&mut file, std::io::SeekFrom::End(0))
                .map_err(|_| Error::Unavailable)?;
            let mut raw = serde_json::to_string(&line).map_err(|_| Error::Unavailable)?;
            raw.push('\n');
            file.write_all(raw.as_bytes())
                .and_then(|_| file.sync_data())
                .map_err(|_| Error::Unavailable)
        })
    }
    /// Newest first, at most `limit`, older than `before` (a number) when
    /// given; and whether there are more.
    pub fn list(
        &self,
        state: &StateDir,
        project: &str,
        before: Option<u64>,
        limit: usize,
    ) -> Result<(Vec<Body>, bool), Error> {
        self.with(state, project, |chain| {
            let mut out: Vec<Body> = chain
                .lines
                .iter()
                .rev()
                .filter(|l| before.is_none_or(|b| l.body.seq < b))
                .take(limit + 1)
                .map(|l| l.body.clone())
                .collect();
            let more = out.len() > limit;
            out.truncate(limit);
            Ok((out, more))
        })
    }
    /// Reads the file again and verifies the whole chain.
    pub fn status(&self, state: &StateDir, project: &str) -> Result<Status, Error> {
        let mut chains = self.chains.lock().unwrap_or_else(|e| e.into_inner());
        let chain = load(state, project)?;
        let status = Status {
            ok: chain.broken_at.is_none(),
            entries: chain.lines.len(),
            dropped: chain.dropped,
            broken_at: chain.broken_at,
        };
        chains.insert(project.to_owned(), chain);
        Ok(status)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(kind: Kind, session: Option<&str>) -> Entry {
        Entry {
            kind,
            via: Via::Mcp,
            client: Some("claude-code 2.1".into()),
            session: session.map(str::to_owned),
            outcome: "ok".into(),
            duration_ms: 3,
            counts: None,
            at_ms: 1_000,
        }
    }

    #[test]
    fn appends_survive_a_reload_and_any_edit_shows() {
        let dir = tempfile::tempdir().unwrap();
        let st = StateDir::open(&dir.path().join("state")).unwrap();
        let ev = Evidence::new();
        for _ in 0..3 {
            ev.append(&st, "alpha", entry(Kind::Context, Some("ab12")))
                .unwrap();
        }
        ev.append(&st, "alpha", entry(Kind::Session, None)).unwrap();
        let fresh = Evidence::new();
        let s = fresh.status(&st, "alpha").unwrap();
        assert_eq!(
            s,
            Status {
                ok: true,
                entries: 4,
                dropped: 0,
                broken_at: None
            }
        );
        let (list, more) = fresh.list(&st, "alpha", None, 2).unwrap();
        assert_eq!(list.iter().map(|b| b.seq).collect::<Vec<_>>(), [4, 3]);
        assert!(more);
        assert_eq!(list[1].session.as_deref(), Some("ab12"));
        // Editing an entry in the middle breaks the chain there.
        let path = st.path().join("evidence-alpha.jsonl");
        let text = std::fs::read_to_string(&path).unwrap();
        std::fs::write(
            &path,
            text.replacen("\"outcome\":\"ok\"", "\"outcome\":\"NotFound\"", 2),
        )
        .unwrap();
        assert_eq!(fresh.status(&st, "alpha").unwrap().broken_at, Some(1));
        // Dropping a line breaks it too.
        let lines: Vec<&str> = text.lines().collect();
        let dropped = [lines[0], lines[1], lines[3], lines[4]].join("\n") + "\n";
        std::fs::write(&path, dropped).unwrap();
        assert_eq!(fresh.status(&st, "alpha").unwrap().broken_at, Some(3));
    }

    #[test]
    fn compaction_keeps_the_chain_verifiable_from_its_base() {
        let dir = tempfile::tempdir().unwrap();
        let st = StateDir::open(&dir.path().join("state")).unwrap();
        let ev = Evidence::new();
        for _ in 0..=MAX_ENTRIES {
            ev.append(&st, "alpha", entry(Kind::Context, None)).unwrap();
        }
        let s = Evidence::new().status(&st, "alpha").unwrap();
        assert!(s.ok);
        assert_eq!(s.entries, KEEP);
        assert_eq!(s.dropped, (MAX_ENTRIES + 1 - KEEP) as u64);
    }
}
