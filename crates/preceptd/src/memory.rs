//! Project memory: what agents learn while working in one project — this
//! machine's environment, cautions, lessons and attempts — written and kept
//! current by the agents themselves, so it survives a change of agent, harness
//! or model. Reference only: reviewed rules and the current code outrank it.
//! Kept in the daemon's private state (one file per project), never in the
//! Profile, bounded; the operator edits, pins, deletes or clears it at will.
//! Agent writes need the operator's per-project permission for agent writes.
use crate::state::StateDir;
use precept_ipc::Error;
use serde::{Deserialize, Serialize};
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

/// The most entries kept per project.
pub const MAX_ENTRIES: usize = 300;
pub const MAX_TITLE: usize = 120;
pub const MAX_BODY: usize = 2000;
/// Earlier bodies kept per entry.
pub const MAX_HISTORY: usize = 5;
/// Environment entries a context carries (all of them, up to this).
pub const BRIEF_ENVIRONMENT: usize = 8;
/// Other entries a context carries, by weight.
pub const BRIEF_RELEVANT: usize = 5;
/// The most text the memory in a context carries, in bytes.
pub const BRIEF_BYTES: usize = 2560;

const DAY_MS: u64 = 86_400_000;
/// Days after the last confirmation before an entry starts to fade.
pub const FADE_AFTER_DAYS: u64 = 60;
/// Changes within this window that make an entry unstable.
const UNSTABLE_WINDOW_MS: u64 = 7 * DAY_MS;
const UNSTABLE_CHANGES: usize = 3;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    /// Facts about this machine: toolchains, paths, services.
    Environment,
    /// Pitfalls and steps that must come first.
    Caution,
    /// Ways of working that proved right.
    Lesson,
    /// What was tried and how it went (failures included).
    Attempt,
}
impl Kind {
    pub const ALL: [Kind; 4] = [
        Kind::Environment,
        Kind::Caution,
        Kind::Lesson,
        Kind::Attempt,
    ];
    fn base(self) -> f64 {
        match self {
            Kind::Environment => 100.0,
            Kind::Caution => 60.0,
            Kind::Lesson => 40.0,
            Kind::Attempt => 20.0,
        }
    }
    pub fn as_str(self) -> &'static str {
        match self {
            Kind::Environment => "environment",
            Kind::Caution => "caution",
            Kind::Lesson => "lesson",
            Kind::Attempt => "attempt",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Earlier {
    pub body: String,
    pub until_ms: u64,
    pub by: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Entry {
    pub id: String,
    pub kind: Kind,
    pub title: String,
    pub body: String,
    #[serde(default)]
    pub pinned: bool,
    #[serde(default)]
    pub confirmations: u32,
    #[serde(default)]
    pub recalls: u32,
    pub created_ms: u64,
    pub updated_ms: u64,
    pub confirmed_ms: u64,
    #[serde(default)]
    pub edited_by_operator: bool,
    /// Who wrote the current body: the agent's client label, `operator`, or
    /// none when unknown.
    pub by: Option<String>,
    /// The work session it was written in, if any.
    #[serde(default)]
    pub session: Option<String>,
    #[serde(default)]
    pub history: Vec<Earlier>,
    /// When the body last changed (within the unstable window).
    #[serde(default)]
    pub changes: Vec<u64>,
}

impl Entry {
    /// The entry's weight at `now`: its kind, pin, confirmations, recalls and
    /// an operator's edit, less fading after [`FADE_AFTER_DAYS`].
    pub fn weight(&self, now: u64) -> f64 {
        let base = self.kind.base();
        let idle_days = now.saturating_sub(self.confirmed_ms) / DAY_MS;
        let fade = (idle_days.saturating_sub(FADE_AFTER_DAYS) as f64 * 0.5).min(base);
        base + if self.pinned { 1000.0 } else { 0.0 }
            + 4.0 * f64::from(self.confirmations.min(10))
            + f64::from(self.recalls.min(20))
            + if self.edited_by_operator { 10.0 } else { 0.0 }
            - fade
    }
    /// Changed often lately: shown so, never boosted for it.
    pub fn unstable(&self, now: u64) -> bool {
        self.changes
            .iter()
            .filter(|t| now.saturating_sub(**t) <= UNSTABLE_WINDOW_MS)
            .count()
            >= UNSTABLE_CHANGES
    }
    /// Fading: not confirmed for [`FADE_AFTER_DAYS`].
    pub fn fading(&self, now: u64) -> bool {
        now.saturating_sub(self.confirmed_ms) / DAY_MS > FADE_AFTER_DAYS
    }
    /// The entry with its weight and flags, as reads return it.
    pub fn view(&self, now: u64) -> serde_json::Value {
        let mut v = serde_json::to_value(self).unwrap_or_default();
        v["weight"] = serde_json::json!((self.weight(now) * 10.0).round() / 10.0);
        v["unstable"] = serde_json::json!(self.unstable(now));
        v["fading"] = serde_json::json!(self.fading(now));
        v
    }
    /// A new body: the earlier one goes to history; a correction resets
    /// confirmations (they were for the old text) but keeps the pin.
    fn correct(&mut self, body: String, by: Option<String>, session: Option<String>, now: u64) {
        self.history.insert(
            0,
            Earlier {
                body: std::mem::take(&mut self.body),
                until_ms: now,
                by: self.by.take(),
            },
        );
        self.history.truncate(MAX_HISTORY);
        self.body = body;
        self.by = by;
        self.session = session;
        self.confirmations = 0;
        self.updated_ms = now;
        self.confirmed_ms = now;
        self.changes
            .retain(|t| now.saturating_sub(*t) <= UNSTABLE_WINDOW_MS);
        self.changes.push(now);
    }
}

#[derive(Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct File {
    format: u32,
    entries: Vec<Entry>,
}

pub fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// Bounded text: trimmed, no control characters but line breaks and tabs.
fn text(raw: &str, max: usize) -> Result<String, Error> {
    let t = raw.trim();
    if t.is_empty()
        || t.chars().count() > max
        || t.chars().any(|c| c.is_control() && c != '\n' && c != '\t')
    {
        return Err(Error::InvalidRequest);
    }
    Ok(t.to_owned())
}
/// A title as compared: lowercase, runs of space as one.
fn normal(title: &str) -> String {
    title
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}
fn new_id() -> Result<String, Error> {
    let mut raw = [0u8; 8];
    getrandom::fill(&mut raw).map_err(|_| Error::Unavailable)?;
    Ok(raw.iter().map(|b| format!("{b:02x}")).collect())
}
fn valid_id(id: &str) -> bool {
    id.len() == 16 && id.bytes().all(|b| b.is_ascii_hexdigit())
}

/// What `remember` did.
#[derive(Debug, PartialEq, Eq)]
pub enum Remembered {
    Added,
    Reaffirmed,
    Corrected,
}
impl Remembered {
    pub fn as_str(&self) -> &'static str {
        match self {
            Remembered::Added => "added",
            Remembered::Reaffirmed => "reaffirmed",
            Remembered::Corrected => "corrected",
        }
    }
}

/// The memory store; one lock for every project's file.
pub struct Memory {
    lock: Mutex<()>,
}

fn file_name(project: &str) -> String {
    format!("memory-{project}.json")
}

impl Memory {
    pub fn new() -> Self {
        Self {
            lock: Mutex::new(()),
        }
    }
    fn load(state: &StateDir, project: &str) -> Result<Vec<Entry>, Error> {
        let Some(text) = state.read(&file_name(project))? else {
            return Ok(Vec::new());
        };
        let file: File = serde_json::from_str(&text).map_err(|_| Error::InvalidRequest)?;
        if file.format != 1 || file.entries.len() > MAX_ENTRIES {
            return Err(Error::InvalidRequest);
        }
        Ok(file.entries)
    }
    fn save(state: &StateDir, project: &str, entries: Vec<Entry>) -> Result<(), Error> {
        let body = serde_json::to_vec_pretty(&File { format: 1, entries })
            .map_err(|_| Error::Unavailable)?;
        state.write(&file_name(project), &body)
    }
    /// Every entry, heaviest first (the operator's read).
    pub fn list(&self, state: &StateDir, project: &str) -> Result<Vec<Entry>, Error> {
        let _held = self.lock.lock().unwrap();
        let mut all = Self::load(state, project)?;
        let now = now_ms();
        all.sort_by(|a, b| b.weight(now).total_cmp(&a.weight(now)));
        Ok(all)
    }
    /// Adds an entry, or updates the one of the same kind and title: the same
    /// body reaffirms it, another body corrects it. Past the bound, the
    /// lightest unpinned entry goes.
    #[allow(clippy::too_many_arguments)]
    pub fn remember(
        &self,
        state: &StateDir,
        project: &str,
        kind: Kind,
        title: &str,
        body: &str,
        by: Option<String>,
        session: Option<String>,
    ) -> Result<(Entry, Remembered), Error> {
        let title = text(title, MAX_TITLE)?;
        let body = text(body, MAX_BODY)?;
        let _held = self.lock.lock().unwrap();
        let mut all = Self::load(state, project)?;
        let now = now_ms();
        let key = normal(&title);
        let (entry, what) = match all
            .iter_mut()
            .find(|e| e.kind == kind && normal(&e.title) == key)
        {
            Some(e) if e.body == body => {
                e.confirmations = e.confirmations.saturating_add(1);
                e.confirmed_ms = now;
                (e.clone(), Remembered::Reaffirmed)
            }
            Some(e) => {
                e.correct(body, by, session, now);
                e.edited_by_operator = false;
                (e.clone(), Remembered::Corrected)
            }
            None => {
                let e = Entry {
                    id: new_id()?,
                    kind,
                    title,
                    body,
                    pinned: false,
                    confirmations: 0,
                    recalls: 0,
                    created_ms: now,
                    updated_ms: now,
                    confirmed_ms: now,
                    edited_by_operator: false,
                    by,
                    session,
                    history: vec![],
                    changes: vec![],
                };
                all.push(e.clone());
                (e, Remembered::Added)
            }
        };
        while all.len() > MAX_ENTRIES {
            let lightest = all
                .iter()
                .enumerate()
                .filter(|(_, e)| !e.pinned && e.id != entry.id)
                .min_by(|a, b| a.1.weight(now).total_cmp(&b.1.weight(now)))
                .map(|(i, _)| i)
                .ok_or(Error::Capacity)?;
            all.remove(lightest);
        }
        Self::save(state, project, all)?;
        Ok((entry, what))
    }
    /// An entry is still true: one more confirmation.
    pub fn reaffirm(&self, state: &StateDir, project: &str, id: &str) -> Result<Entry, Error> {
        self.change(state, project, id, |e, now| {
            e.confirmations = e.confirmations.saturating_add(1);
            e.confirmed_ms = now;
            Ok(())
        })
    }
    /// Removes an entry (an agent's `forget`, the operator's delete).
    pub fn forget(&self, state: &StateDir, project: &str, id: &str) -> Result<(), Error> {
        if !valid_id(id) {
            return Err(Error::InvalidRequest);
        }
        let _held = self.lock.lock().unwrap();
        let mut all = Self::load(state, project)?;
        let before = all.len();
        all.retain(|e| e.id != id);
        if all.len() == before {
            return Err(Error::NotFound);
        }
        Self::save(state, project, all)
    }
    /// Removes every entry (the operator's clear).
    pub fn clear(&self, state: &StateDir, project: &str) -> Result<usize, Error> {
        let _held = self.lock.lock().unwrap();
        let n = Self::load(state, project)?.len();
        Self::save(state, project, vec![])?;
        Ok(n)
    }
    /// The operator's edit: a new title or body (a correction by the
    /// operator), or the pin.
    pub fn edit(
        &self,
        state: &StateDir,
        project: &str,
        id: &str,
        title: Option<&str>,
        body: Option<&str>,
        pinned: Option<bool>,
    ) -> Result<Entry, Error> {
        let title = title.map(|t| text(t, MAX_TITLE)).transpose()?;
        let body = body.map(|b| text(b, MAX_BODY)).transpose()?;
        let _held = self.lock.lock().unwrap();
        let mut all = Self::load(state, project)?;
        if let Some(t) = &title {
            let (kind, key) = match all.iter().find(|e| e.id == id) {
                Some(e) => (e.kind, normal(t)),
                None => return Err(Error::NotFound),
            };
            if all
                .iter()
                .any(|e| e.id != id && e.kind == kind && normal(&e.title) == key)
            {
                return Err(Error::Conflict);
            }
        }
        let now = now_ms();
        let e = all.iter_mut().find(|e| e.id == id).ok_or(Error::NotFound)?;
        if let Some(t) = title {
            e.title = t;
        }
        if let Some(b) = body
            && b != e.body
        {
            e.correct(b, Some("operator".into()), None, now);
            e.edited_by_operator = true;
        }
        if let Some(p) = pinned {
            e.pinned = p;
        }
        e.updated_ms = now;
        let out = e.clone();
        Self::save(state, project, all)?;
        Ok(out)
    }
    /// Puts back an earlier body (the operator's restore): a correction.
    pub fn restore(
        &self,
        state: &StateDir,
        project: &str,
        id: &str,
        index: usize,
    ) -> Result<Entry, Error> {
        self.change(state, project, id, |e, now| {
            let body = e.history.get(index).ok_or(Error::NotFound)?.body.clone();
            e.correct(body, Some("operator".into()), None, now);
            e.edited_by_operator = true;
            Ok(())
        })
    }
    fn change(
        &self,
        state: &StateDir,
        project: &str,
        id: &str,
        f: impl FnOnce(&mut Entry, u64) -> Result<(), Error>,
    ) -> Result<Entry, Error> {
        if !valid_id(id) {
            return Err(Error::InvalidRequest);
        }
        let _held = self.lock.lock().unwrap();
        let mut all = Self::load(state, project)?;
        let now = now_ms();
        let e = all.iter_mut().find(|e| e.id == id).ok_or(Error::NotFound)?;
        f(e, now)?;
        let out = e.clone();
        Self::save(state, project, all)?;
        Ok(out)
    }
    /// Entries matching `query` (case-insensitive, in title or body) and
    /// `kind`, heaviest first, at most `limit`; each returned counts a
    /// recall.
    pub fn recall(
        &self,
        state: &StateDir,
        project: &str,
        query: Option<&str>,
        kind: Option<Kind>,
        limit: usize,
    ) -> Result<Vec<Entry>, Error> {
        let query = query
            .map(|q| q.trim().to_lowercase())
            .filter(|q| !q.is_empty());
        if query.as_ref().is_some_and(|q| q.chars().count() > 200) {
            return Err(Error::InvalidRequest);
        }
        let _held = self.lock.lock().unwrap();
        let mut all = Self::load(state, project)?;
        let now = now_ms();
        let mut hits: Vec<usize> = all
            .iter()
            .enumerate()
            .filter(|(_, e)| kind.is_none_or(|k| e.kind == k))
            .filter(|(_, e)| {
                query.as_ref().is_none_or(|q| {
                    e.title.to_lowercase().contains(q.as_str())
                        || e.body.to_lowercase().contains(q.as_str())
                })
            })
            .map(|(i, _)| i)
            .collect();
        hits.sort_by(|a, b| all[*b].weight(now).total_cmp(&all[*a].weight(now)));
        hits.truncate(limit.clamp(1, 20));
        for i in &hits {
            all[*i].recalls = all[*i].recalls.saturating_add(1);
        }
        let out = hits.iter().map(|i| all[*i].clone()).collect();
        Self::save(state, project, all)?;
        Ok(out)
    }
    /// What a context carries: counts by kind, every environment entry (up
    /// to [`BRIEF_ENVIRONMENT`]), then the heaviest others (up to
    /// [`BRIEF_RELEVANT`]), within [`BRIEF_BYTES`] of text.
    pub fn for_context(&self, state: &StateDir, project: &str) -> serde_json::Value {
        let all = self.list(state, project).unwrap_or_default();
        let now = now_ms();
        let mut counts = serde_json::Map::new();
        for k in Kind::ALL {
            counts.insert(
                k.as_str().into(),
                all.iter().filter(|e| e.kind == k).count().into(),
            );
        }
        let mut budget = BRIEF_BYTES;
        let mut take = |e: &Entry| -> Option<serde_json::Value> {
            let size = e.title.len() + e.body.len();
            if size > budget {
                return None;
            }
            budget -= size;
            Some(serde_json::json!({
                "id": e.id, "kind": e.kind, "title": e.title, "body": e.body,
                "pinned": e.pinned, "confirmations": e.confirmations,
                "unstable": e.unstable(now),
            }))
        };
        let environment: Vec<_> = all
            .iter()
            .filter(|e| e.kind == Kind::Environment)
            .take(BRIEF_ENVIRONMENT)
            .filter_map(&mut take)
            .collect();
        let relevant: Vec<_> = all
            .iter()
            .filter(|e| e.kind != Kind::Environment && !e.fading(now))
            .take(BRIEF_RELEVANT)
            .filter_map(&mut take)
            .collect();
        serde_json::json!({"counts": counts, "environment": environment, "relevant": relevant})
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state() -> (tempfile::TempDir, StateDir) {
        let temp = tempfile::tempdir().unwrap();
        let s = StateDir::open(&temp.path().join("state")).unwrap();
        (temp, s)
    }

    #[test]
    fn same_body_reaffirms_changed_body_corrects_and_keeps_history() {
        let (_t, s) = state();
        let m = Memory::new();
        let (e, what) = m
            .remember(
                &s,
                "alpha",
                Kind::Lesson,
                "Run tests",
                "cargo test",
                Some("a".into()),
                None,
            )
            .unwrap();
        assert_eq!(what, Remembered::Added);
        let (again, what) = m
            .remember(
                &s,
                "alpha",
                Kind::Lesson,
                "run  TESTS",
                "cargo test",
                Some("b".into()),
                None,
            )
            .unwrap();
        assert_eq!(what, Remembered::Reaffirmed);
        assert_eq!(again.id, e.id);
        assert_eq!(again.confirmations, 1);
        m.edit(&s, "alpha", &e.id, None, None, Some(true)).unwrap();
        let (fixed, what) = m
            .remember(
                &s,
                "alpha",
                Kind::Lesson,
                "Run tests",
                "cargo nextest run",
                Some("c".into()),
                None,
            )
            .unwrap();
        assert_eq!(what, Remembered::Corrected);
        assert_eq!(
            fixed.confirmations, 0,
            "confirmations were for the old text"
        );
        assert!(fixed.pinned, "the pin stays");
        assert_eq!(fixed.history[0].body, "cargo test");
        assert_eq!(fixed.history[0].by.as_deref(), Some("a"));
        // The same title in another kind is another entry.
        let (other, what) = m
            .remember(&s, "alpha", Kind::Caution, "Run tests", "x", None, None)
            .unwrap();
        assert_eq!(what, Remembered::Added);
        assert_ne!(other.id, e.id);
        let restored = m.restore(&s, "alpha", &e.id, 0).unwrap();
        assert_eq!(restored.body, "cargo test");
        assert!(restored.edited_by_operator);
    }

    #[test]
    fn weight_follows_kind_pin_confirmations_and_fades() {
        let now = 1_000 * DAY_MS;
        let entry = |kind, confirmed_days_ago: u64| Entry {
            id: "0".repeat(16),
            kind,
            title: "t".into(),
            body: "b".into(),
            pinned: false,
            confirmations: 0,
            recalls: 0,
            created_ms: 0,
            updated_ms: 0,
            confirmed_ms: now - confirmed_days_ago * DAY_MS,
            edited_by_operator: false,
            by: None,
            session: None,
            history: vec![],
            changes: vec![],
        };
        let env = entry(Kind::Environment, 0);
        let lesson = entry(Kind::Lesson, 0);
        assert!(env.weight(now) > lesson.weight(now));
        let mut pinned = entry(Kind::Attempt, 0);
        pinned.pinned = true;
        assert!(pinned.weight(now) > env.weight(now));
        let old = entry(Kind::Lesson, 100);
        assert!(old.fading(now));
        assert_eq!(old.weight(now), 40.0 - 20.0);
        let mut busy = entry(Kind::Lesson, 0);
        busy.changes = vec![now - DAY_MS, now - 2 * DAY_MS, now - 3 * DAY_MS];
        assert!(busy.unstable(now));
        assert_eq!(
            busy.weight(now),
            lesson.weight(now),
            "never boosted for changing"
        );
    }

    #[test]
    fn recall_ranks_counts_and_the_context_keeps_environment_first() {
        let (_t, s) = state();
        let m = Memory::new();
        m.remember(
            &s,
            "alpha",
            Kind::Environment,
            "Node",
            "Node 22 via mise",
            None,
            None,
        )
        .unwrap();
        m.remember(
            &s,
            "alpha",
            Kind::Attempt,
            "Tried pnpm",
            "pnpm broke the lockfile",
            None,
            None,
        )
        .unwrap();
        m.remember(
            &s,
            "alpha",
            Kind::Caution,
            "WSL 路径",
            "在 WSL 中使用 /mnt/c 很慢",
            None,
            None,
        )
        .unwrap();
        let hits = m.recall(&s, "alpha", Some("wsl"), None, 5).unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(
            m.recall(&s, "alpha", Some("很慢"), None, 5).unwrap().len(),
            1
        );
        assert_eq!(m.list(&s, "alpha").unwrap()[0].kind, Kind::Environment);
        let ctx = m.for_context(&s, "alpha");
        assert_eq!(ctx["counts"]["environment"], 1);
        assert_eq!(ctx["environment"][0]["title"], "Node");
        assert_eq!(ctx["relevant"][0]["kind"], "caution");
        assert_eq!(
            m.recall(&s, "alpha", None, Some(Kind::Attempt), 5).unwrap()[0].recalls,
            1
        );
    }

    #[test]
    fn malformed_input_and_unknown_ids_are_refused() {
        let (_t, s) = state();
        let m = Memory::new();
        assert_eq!(
            m.remember(&s, "a", Kind::Lesson, " ", "b", None, None)
                .unwrap_err(),
            Error::InvalidRequest
        );
        assert_eq!(
            m.remember(&s, "a", Kind::Lesson, "t", "bad\u{7}", None, None)
                .unwrap_err(),
            Error::InvalidRequest
        );
        assert_eq!(m.forget(&s, "a", "zz").unwrap_err(), Error::InvalidRequest);
        assert_eq!(
            m.forget(&s, "a", &"0".repeat(16)).unwrap_err(),
            Error::NotFound
        );
        let (e, _) = m
            .remember(&s, "a", Kind::Lesson, "x", "1", None, None)
            .unwrap();
        m.remember(&s, "a", Kind::Lesson, "y", "2", None, None)
            .unwrap();
        assert_eq!(
            m.edit(&s, "a", &e.id, Some("Y"), None, None).unwrap_err(),
            Error::Conflict
        );
        assert_eq!(m.clear(&s, "a").unwrap(), 2);
    }
}
