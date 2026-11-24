use crate::state::StateDir;
use precept_ipc::Error;
use serde::{Deserialize, Serialize};
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use super::model::{
    BRIEF_BYTES, BRIEF_ENVIRONMENT, BRIEF_RELEVANT, Entry, Kind, MAX_BASIS, MAX_BODY, MAX_ENTRIES,
    MAX_TITLE, PROPOSE_AFTER, Remembered,
};
use super::ranking::terms;
use super::validation::{new_id, normal, text, valid_id};

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
                    proposed: None,
                    basis: None,
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
    /// An entry is still true: one more confirmation, with the basis that
    /// verified it when stated (kept until a correction).
    pub fn reaffirm(
        &self,
        state: &StateDir,
        project: &str,
        id: &str,
        basis: Option<&str>,
    ) -> Result<Entry, Error> {
        let basis = basis.map(|b| text(b, MAX_BASIS)).transpose()?;
        self.change(state, project, id, |e, now| {
            e.confirmations = e.confirmations.saturating_add(1);
            e.confirmed_ms = now;
            if basis.is_some() {
                e.basis = basis;
            }
            Ok(())
        })
    }
    /// One entry by id, in full (a recall index line's follow-up read);
    /// counts no recall.
    pub fn get(&self, state: &StateDir, project: &str, id: &str) -> Result<Entry, Error> {
        if !valid_id(id) {
            return Err(Error::InvalidRequest);
        }
        let _held = self.lock.lock().unwrap();
        let all = Self::load(state, project)?;
        all.into_iter().find(|e| e.id == id).ok_or(Error::NotFound)
    }
    /// Whether `e` should now be filed as a suggested rule: a caution, lesson
    /// or attempt confirmed [`PROPOSE_AFTER`] times, not filed before.
    pub fn ripe(e: &Entry) -> bool {
        e.kind != Kind::Environment && e.confirmations >= PROPOSE_AFTER && e.proposed.is_none()
    }
    /// Records the suggestion a note was filed as.
    pub fn mark_proposed(
        &self,
        state: &StateDir,
        project: &str,
        id: &str,
        rule: &str,
    ) -> Result<Entry, Error> {
        let rule = rule.to_owned();
        self.change(state, project, id, move |e, _| {
            e.proposed = Some(rule);
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
        let terms = query.as_deref().map(terms).unwrap_or_default();
        // Each entry's score: matched terms, a title match counting double;
        // with a query, an entry must match at least one term.
        let mut hits: Vec<(usize, usize)> = all
            .iter()
            .enumerate()
            .filter(|(_, e)| kind.is_none_or(|k| e.kind == k))
            .filter_map(|(i, e)| {
                if terms.is_empty() {
                    return Some((i, 0));
                }
                let (title, body) = (e.title.to_lowercase(), e.body.to_lowercase());
                let score: usize = terms
                    .iter()
                    .map(|t| {
                        if title.contains(t.as_str()) {
                            2
                        } else {
                            usize::from(body.contains(t.as_str()))
                        }
                    })
                    .sum();
                (score > 0).then_some((i, score))
            })
            .collect();
        hits.sort_by(|a, b| {
            b.1.cmp(&a.1)
                .then(all[b.0].weight(now).total_cmp(&all[a.0].weight(now)))
        });
        let mut hits: Vec<usize> = hits.into_iter().map(|(i, _)| i).collect();
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
