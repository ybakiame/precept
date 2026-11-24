//! Exact operator reads, with metadata/content/revision in one verified snapshot.
use crate::{Error, Result, SqliteStore};
use precept_core::operator::*;
use rusqlite::OptionalExtension;

/// One column list for every exact preference read, including review provenance.
const PREFERENCE_COLUMNS: &str = "id,project,pref_key,value,tags,status,revision,origin,proposal_digest,created_at,updated_at,scope,applies_to,priority,enforcement,enabled";

fn malformed(index: usize, column: &str) -> rusqlite::Error {
    rusqlite::Error::InvalidColumnType(index, column.to_owned(), rusqlite::types::Type::Text)
}
fn hex_digest(row: &rusqlite::Row<'_>, index: usize, column: &str) -> rusqlite::Result<String> {
    let blob: Vec<u8> = row.get(index)?;
    let bytes: [u8; 32] = blob.try_into().map_err(|_| malformed(index, column))?;
    Ok(digest_hex(&bytes))
}
fn preference(row: &rusqlite::Row<'_>) -> rusqlite::Result<PreferenceRecord> {
    let project: Option<String> = row.get(1)?;
    let origin: String = row.get(7)?;
    let tags: String = row.get(4)?;
    let kind: String = row.get(11)?;
    let scope = match (kind.as_str(), project) {
        ("global", None) => Scope::Global,
        ("project", Some(name)) => Scope::Project { name },
        _ => return Err(malformed(11, "scope")),
    };
    let applies: String = row.get(12)?;
    let enforcement: String = row.get(14)?;
    let rule = crate::profile::stored_rule(&applies, row.get(13)?, &enforcement)
        .ok_or_else(|| malformed(12, "applies_to"))?;
    let enabled: i64 = row.get(15)?;
    Ok(PreferenceRecord {
        id: row.get(0)?,
        scope,
        rule,
        enabled: enabled == 1,
        key: row.get(2)?,
        value: row.get(3)?,
        tags: crate::profile::stored_tags(&tags).ok_or_else(|| malformed(4, "tags"))?,
        status: row.get(5)?,
        revision: row.get(6)?,
        origin: Origin::parse(&origin).ok_or_else(|| malformed(7, "origin"))?,
        proposal_digest: hex_digest(row, 8, "proposal_digest")?,
        created_at: row.get(9)?,
        updated_at: row.get(10)?,
    })
}
fn decision(row: &rusqlite::Row<'_>) -> rusqlite::Result<ReviewDecisionRecord> {
    let action: String = row.get(1)?;
    let channel: String = row.get(6)?;
    Ok(ReviewDecisionRecord {
        preference_id: row.get(0)?,
        action: serde_json::from_value(serde_json::Value::String(action))
            .map_err(|_| malformed(1, "action"))?,
        from_revision: row.get(2)?,
        to_revision: row.get(3)?,
        proposal_digest: hex_digest(row, 4, "proposal_digest")?,
        receipt_commit_sequence: row.get(5)?,
        channel: serde_json::from_value(serde_json::Value::String(channel))
            .map_err(|_| malformed(6, "channel"))?,
        decided_at: row.get(7)?,
        note: row.get(8)?,
        replaced_by: row.get(9)?,
    })
}
/// Exact scope admission shared by every operator read; never a union of scopes.
fn scoped<'a>(store: &SqliteStore, scope: &'a Scope) -> Result<(&'static str, Option<&'a str>)> {
    match scope {
        Scope::Global => Ok(("global", None)),
        Scope::Project { name } if crate::profile::name_valid(name) => {
            super::project_record(&store.db, name)?;
            Ok((scope.kind(), Some(name.as_str())))
        }
        _ => Err(Error::InvalidRecord),
    }
}
impl SqliteStore {
    pub fn operator_preferences(&self, scope: &Scope, after_id: i64) -> Result<PreferencePage> {
        if after_id < 0 {
            return Err(Error::InvalidRecord);
        }
        self.snapshot(|metadata| {
            let (kind, project) = scoped(self, scope)?;
            let mut stmt = self.db.prepare(&format!("SELECT {PREFERENCE_COLUMNS} FROM preferences WHERE scope=?1 AND project IS ?2 AND id>?3 ORDER BY id LIMIT 13")).map_err(|_|Error::ReadFailed)?;
            let mut records = stmt.query_map((kind, project, after_id), preference)
                .map_err(|_|Error::ReadFailed)?.collect::<std::result::Result<Vec<_>,_>>()
                .map_err(|_|Error::InvalidRecord)?;
            let next_after_id = (records.len() > 12).then(|| records[11].id);
            records.truncate(12);
            Ok(PreferencePage { binding: metadata.binding, records, next_after_id })
        })
    }
    pub fn operator_preference(&self, scope: &Scope, id: i64) -> Result<PreferencePreview> {
        if id <= 0 {
            return Err(Error::InvalidRecord);
        }
        self.snapshot(|metadata| {
            let (kind, project) = scoped(self, scope)?;
            let record = self.db.query_row(&format!("SELECT {PREFERENCE_COLUMNS} FROM preferences WHERE scope=?1 AND project IS ?2 AND id=?3"), (kind,project,id), preference)
                .optional().map_err(|_|Error::ReadFailed)?.ok_or(Error::PreferenceNotFound)?;
            Ok(PreferencePreview { binding:metadata.binding,record })
        })
    }
    /// Pending review only. Paging is not a snapshot; the watermark discloses drift.
    pub fn operator_review_queue(&self, scope: &Scope, after_id: i64) -> Result<ReviewQueue> {
        if after_id < 0 {
            return Err(Error::InvalidRecord);
        }
        self.snapshot(|metadata| {
            let (kind, project) = scoped(self, scope)?;
            let mut stmt = self.db.prepare(&format!("SELECT {PREFERENCE_COLUMNS} FROM preferences WHERE scope=?1 AND project IS ?2 AND status='suggested' AND id>?3 ORDER BY id LIMIT 13")).map_err(|_|Error::ReadFailed)?;
            let mut records = stmt.query_map((kind, project, after_id), preference)
                .map_err(|_|Error::ReadFailed)?.collect::<std::result::Result<Vec<_>,_>>()
                .map_err(|_|Error::InvalidRecord)?;
            let next_after_id = (records.len() > 12).then(|| records[11].id);
            records.truncate(12);
            Ok(ReviewQueue { binding: metadata.binding, commit_sequence: metadata.commit_sequence, records, next_after_id })
        })
    }
    /// How many records of the scope are suggested, at one watermark.
    pub fn operator_review_count(&self, scope: &Scope) -> Result<ReviewCount> {
        self.snapshot(|metadata| {
            let (kind, project) = scoped(self, scope)?;
            let suggested: i64 = self
                .db
                .query_row(
                    "SELECT COUNT(*) FROM preferences WHERE scope=?1 AND project IS ?2 AND status='suggested'",
                    (kind, project),
                    |row| row.get(0),
                )
                .map_err(|_| Error::ReadFailed)?;
            Ok(ReviewCount {
                binding: metadata.binding,
                commit_sequence: metadata.commit_sequence,
                suggested,
            })
        })
    }
    /// Before an agent's suggestion: whether the scope already holds this exact
    /// key and value (suggested or confirmed), and how many agent suggestions
    /// of the scope still wait for review.
    pub fn agent_suggestion_state(
        &self,
        scope: &Scope,
        key: &str,
        value: &str,
    ) -> Result<(bool, i64)> {
        self.snapshot(|_| {
            let (kind, project) = scoped(self, scope)?;
            let duplicate: i64 = self
                .db
                .query_row(
                    "SELECT COUNT(*) FROM preferences WHERE scope=?1 AND project IS ?2 AND pref_key=?3 AND value=?4 AND status IN ('suggested','confirmed')",
                    (kind, project, key, value),
                    |row| row.get(0),
                )
                .map_err(|_| Error::ReadFailed)?;
            let pending: i64 = self
                .db
                .query_row(
                    "SELECT COUNT(*) FROM preferences WHERE scope=?1 AND project IS ?2 AND status='suggested' AND origin='agent'",
                    (kind, project),
                    |row| row.get(0),
                )
                .map_err(|_| Error::ReadFailed)?;
            Ok((duplicate > 0, pending))
        })
    }
    /// Exact record, bounded recent history and the projected peer-demotion set.
    /// The authoritative demotion set is still computed inside the write transaction.
    pub fn operator_review_inspect(&self, scope: &Scope, id: i64) -> Result<ReviewInspect> {
        if id <= 0 {
            return Err(Error::InvalidRecord);
        }
        self.snapshot(|metadata| {
            let (kind, project) = scoped(self, scope)?;
            let record = self.db.query_row(&format!("SELECT {PREFERENCE_COLUMNS} FROM preferences WHERE scope=?1 AND project IS ?2 AND id=?3"), (kind,project,id), preference)
                .optional().map_err(|_|Error::ReadFailed)?.ok_or(Error::PreferenceNotFound)?;
            let mut stmt = self.db.prepare("SELECT preference_id,action,from_revision,to_revision,proposal_digest,receipt_commit_sequence,channel,decided_at,note,replaced_by FROM acp_review_decisions WHERE scope=?1 AND project IS ?2 AND preference_id=?3 ORDER BY id DESC LIMIT 8").map_err(|_|Error::ReadFailed)?;
            let decisions = stmt.query_map((kind, project, id), decision)
                .map_err(|_|Error::ReadFailed)?.collect::<std::result::Result<Vec<_>,_>>()
                .map_err(|_|Error::InvalidRecord)?;
            let mut stmt = self.db.prepare("SELECT id,revision FROM preferences WHERE scope=?1 AND project IS ?2 AND status='confirmed' AND pref_key=?3 AND id<>?4 ORDER BY id LIMIT 12").map_err(|_|Error::ReadFailed)?;
            let peer_demotions = stmt.query_map((kind, project, record.key.as_str(), id), |row| {
                Ok(PeerDemotion { id: row.get(0)?, revision: row.get(1)? })
            }).map_err(|_|Error::ReadFailed)?.collect::<std::result::Result<Vec<_>,_>>()
                .map_err(|_|Error::InvalidRecord)?;
            let peer_demotion_total: i64 = self.db.query_row("SELECT COUNT(*) FROM preferences WHERE scope=?1 AND project IS ?2 AND status='confirmed' AND pref_key=?3 AND id<>?4", (kind, project, record.key.as_str(), id), |row| row.get(0))
                .map_err(|_|Error::ReadFailed)?;
            let related_total: i64 = self.db.query_row(
                "SELECT COUNT(*) FROM preferences WHERE scope=?1 AND project IS ?2 AND status IN ('suggested','confirmed') AND id<>?3 AND (pref_key=?4 OR value=?5)",
                (kind, project, id, record.key.as_str(), record.value.as_str()),
                |row| row.get(0),
            ).map_err(|_|Error::ReadFailed)?;
            let mut stmt = self.db.prepare(&format!(
                "SELECT {PREFERENCE_COLUMNS} FROM preferences WHERE scope=?1 AND project IS ?2 AND status IN ('suggested','confirmed') AND id<>?3 AND (pref_key=?4 OR value=?5) ORDER BY CASE WHEN pref_key=?4 AND value=?5 AND tags=?6 AND applies_to=?7 AND priority=?8 AND enforcement=?9 THEN 0 WHEN pref_key=?4 THEN 1 ELSE 2 END, id LIMIT 12"
            )).map_err(|_|Error::ReadFailed)?;
            let related: Vec<ReviewRelated> = stmt.query_map((
                kind,
                project,
                id,
                record.key.as_str(),
                record.value.as_str(),
                precept_core::operator::tags_json(&record.tags),
                record.rule.applies_to.json(),
                record.rule.priority,
                record.rule.enforcement.as_str(),
            ), preference)
                .map_err(|_|Error::ReadFailed)?
                .collect::<std::result::Result<Vec<_>,_>>()
                .map_err(|_|Error::InvalidRecord)?
                .into_iter()
                .map(|other| {
                    let relation = if other.key == record.key
                        && other.value == record.value
                        && other.tags == record.tags
                        && other.rule == record.rule
                    {
                        ReviewRelation::Duplicate
                    } else if other.key == record.key {
                        ReviewRelation::SameKey
                    } else {
                        ReviewRelation::SameValue
                    };
                    ReviewRelated { id: other.id, revision: other.revision, key: other.key, relation }
                })
                .collect();
            Ok(ReviewInspect { binding: metadata.binding, commit_sequence: metadata.commit_sequence, record, decisions, peer_demotions, peer_demotion_total, related, related_total, evidence: None })
        })
    }
    /// A project's saved Handoffs, newest first, at most 50.
    pub fn operator_handoffs(&self, project: &str) -> Result<HandoffList> {
        if !crate::profile::name_valid(project) {
            return Err(Error::InvalidRecord);
        }
        self.snapshot(|metadata| {
            super::project_record(&self.db, project)?;
            let mut stmt = self
                .db
                .prepare("SELECT task_id,objective,branch,revision,updated_at FROM handoffs WHERE project=?1 ORDER BY updated_at DESC, task_id LIMIT 50")
                .map_err(|_| Error::ReadFailed)?;
            let handoffs = stmt
                .query_map([project], |row| {
                    let objective: String = row.get(1)?;
                    Ok(HandoffSummary {
                        task_id: row.get(0)?,
                        objective: objective
                            .lines()
                            .next()
                            .unwrap_or_default()
                            .chars()
                            .take(200)
                            .collect(),
                        branch: row.get(2)?,
                        revision: row.get(3)?,
                        updated_at: row.get(4)?,
                    })
                })
                .map_err(|_| Error::ReadFailed)?
                .collect::<std::result::Result<Vec<_>, _>>()
                .map_err(|_| Error::InvalidRecord)?;
            Ok(HandoffList { binding: metadata.binding, handoffs })
        })
    }
    pub fn operator_handoff(&self, project: &str, task: &str) -> Result<HandoffPreview> {
        if !crate::profile::name_valid(project) || !crate::profile::text(task, 120, true) {
            return Err(Error::InvalidRecord);
        }
        self.snapshot(|metadata| {
            super::project_record(&self.db,project)?;
            let record = self.db.query_row("SELECT project,task_id,revision,objective,completed,next_action,validation_json,base_sha,branch,worktree_state,worktree_fingerprint FROM handoffs WHERE project=?1 AND task_id=?2",(project,task),|row| Ok(HandoffRecord {
                project:row.get(0)?,task_id:row.get(1)?,revision:row.get(2)?,objective:row.get(3)?,completed:row.get(4)?,next_action:row.get(5)?,validation_json:row.get(6)?,
                snapshot:Snapshot { base_sha:row.get(7)?,branch:row.get(8)?,state:row.get(9)?,fingerprint:row.get(10)? }
            })).optional().map_err(|_|Error::ReadFailed)?;
            Ok(HandoffPreview { binding:metadata.binding,record })
        })
    }
}
