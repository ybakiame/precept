//! Exact operator reads, with metadata/content/revision in one verified snapshot.
use crate::{Error, Result, SqliteStore};
use acp_core::operator::*;
use rusqlite::OptionalExtension;

fn preference(row: &rusqlite::Row<'_>) -> rusqlite::Result<PreferenceRecord> {
    let project: Option<String> = row.get(1)?;
    Ok(PreferenceRecord {
        id: row.get(0)?,
        scope: project.map_or(Scope::Global, |name| Scope::Project { name }),
        key: row.get(2)?,
        value: row.get(3)?,
        source: row.get(4)?,
        status: row.get(5)?,
        revision: row.get(6)?,
    })
}
impl SqliteStore {
    pub fn operator_preferences(&self, scope: &Scope, after_id: i64) -> Result<PreferencePage> {
        if after_id < 0 {
            return Err(Error::InvalidRecord);
        }
        self.snapshot(|metadata| {
            let (kind, project) = match scope {
                Scope::Global => ("global", None),
                Scope::Project { name } if crate::profile::name_valid(name) => {
                    super::project_record(&self.db, name)?;
                    ("project", Some(name.as_str()))
                }
                _ => return Err(Error::InvalidRecord),
            };
            let mut stmt = self.db.prepare("SELECT id,project,pref_key,value,source,status,revision FROM preferences WHERE scope=?1 AND project IS ?2 AND id>?3 ORDER BY id LIMIT 13").map_err(|_|Error::ReadFailed)?;
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
            let (kind, project) = match scope {
                Scope::Global => ("global", None),
                Scope::Project { name } if crate::profile::name_valid(name) => {
                    super::project_record(&self.db, name)?;
                    ("project", Some(name.as_str()))
                }
                _ => return Err(Error::InvalidRecord),
            };
            let record = self.db.query_row("SELECT id,project,pref_key,value,source,status,revision FROM preferences WHERE scope=?1 AND project IS ?2 AND id=?3", (kind,project,id), preference)
                .optional().map_err(|_|Error::ReadFailed)?.ok_or(Error::PreferenceNotFound)?;
            Ok(PreferencePreview { binding:metadata.binding,record })
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
