//! Typed data/revision/allocator/receipt updates in one bounded transaction.
use crate::{
    Binding, Command, Error, Expected, Fault, Profile, Receipt, RecordVersion, Request, Result,
    Scope,
};
use acp_store::profile;
use rusqlite::{Connection, OptionalExtension, TransactionBehavior, params};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
type Key = (String, String, String);
pub struct Writer<'a> {
    pub(crate) db: Connection,
    pub(crate) profile: &'a Profile,
}
impl std::fmt::Debug for Writer<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Writer(redacted)")
    }
}
fn project_key(name: &str) -> Key {
    ("project".into(), String::new(), name.into())
}
fn pref_key(id: i64) -> Key {
    ("preference".into(), String::new(), id.to_string())
}
fn handoff_key(project: &str, task: &str) -> Key {
    ("handoff".into(), project.into(), task.into())
}
fn binding(current: &Binding, wanted: &Binding) -> Result<()> {
    if current.profile_id != wanted.profile_id || current.owner_epoch != wanted.owner_epoch {
        Err(Error::OwnerMismatch)
    } else if current.generation != wanted.generation {
        Err(Error::StaleGeneration)
    } else {
        Ok(())
    }
}
fn cas(current: Option<i64>, expected: &Expected) -> Result<()> {
    match (current, expected) {
        (None, Expected::Absent) => Ok(()),
        (Some(actual), Expected::Revision(wanted)) if actual == *wanted => Ok(()),
        _ => Err(Error::Conflict),
    }
}
fn next_revision(n: i64) -> Result<i64> {
    n.checked_add(1).ok_or(Error::Capacity)
}
fn scope(scope: &Scope) -> (&str, Option<&str>) {
    match scope {
        Scope::Global => ("global", None),
        Scope::Project { name } => ("project", Some(name)),
    }
}
fn check_project(db: &Connection, name: &str) -> Result<()> {
    if db
        .query_row("SELECT 1 FROM projects WHERE name=?1", [name], |_| Ok(()))
        .optional()
        .map_err(crate::sql_error)?
        .is_none()
    {
        return Err(Error::Conflict);
    }
    Ok(())
}
impl Writer<'_> {
    pub fn execute(&mut self, request: &Request) -> Result<Receipt> {
        self.execute_inner(request, Fault::None)
    }
    pub(crate) fn execute_inner(&mut self, request: &Request, _fault: Fault) -> Result<Receipt> {
        request.validate()?;
        self.profile.check()?;
        // All filesystem work is outside the write transaction. Registration is
        // storage data, not the later daemon's independent sharing allowlist.
        if let Command::RegisterProject { root, .. } = &request.command {
            let p = std::path::Path::new(root);
            if !p.is_dir()
                || p.canonicalize()
                    .map_err(|_| Error::InvalidCommand)?
                    .to_str()
                    != Some(root)
            {
                return Err(Error::InvalidCommand);
            }
        }
        binding(&self.profile.binding, &request.binding)?;
        let tx = self
            .db
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(crate::sql_error)?;
        let metadata = profile::verify(&tx)?;
        binding(&metadata.binding, &request.binding)?;
        let bytes = serde_json::to_vec(request).map_err(|_| Error::InvalidCommand)?;
        let digest = Sha256::digest(bytes).to_vec();
        let old: Option<(Vec<u8>, String)> = tx
            .query_row(
                "SELECT request_digest,result_json FROM acp_receipts WHERE operation_id=?1",
                [&request.operation_id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()
            .map_err(crate::sql_error)?;
        if let Some((before, result)) = old {
            if before != digest {
                return Err(Error::IdempotencyConflict);
            }
            return serde_json::from_str(&result).map_err(|_| Error::InvalidRecord);
        }
        if metadata.commit_sequence >= 4096 {
            return Err(Error::Capacity);
        }
        let sequence = next_revision(metadata.commit_sequence)?;
        let now = profile::timestamp()?;
        let versions = mutate(&tx, &request.command, &now, metadata.next_preference_id)?;
        #[cfg(test)]
        if matches!(_fault, Fault::AfterData | Fault::AfterRevisions) {
            return Err(Error::BeforeCommit);
        }
        let affected = versions
            .into_iter()
            .map(
                |((kind, project, record_id), (revision, deleted))| RecordVersion {
                    kind,
                    project,
                    record_id,
                    revision,
                    deleted,
                },
            )
            .collect::<Vec<_>>();
        let receipt = Receipt {
            generation: request.binding.generation.clone(),
            operation_id: request.operation_id.clone(),
            commit_sequence: sequence,
            affected,
        };
        let raw = serde_json::to_string(&receipt).map_err(|_| Error::Capacity)?;
        if raw.len() > 65536 || receipt.affected.len() > 1024 {
            return Err(Error::Capacity);
        }
        tx.execute(
            "UPDATE acp_profile SET commit_sequence=?1 WHERE singleton=1",
            [sequence],
        )
        .map_err(crate::sql_error)?;
        tx.execute(
            "INSERT INTO acp_receipts VALUES(?1,?2,?3,?4,?5)",
            params![
                request.operation_id,
                request.binding.generation,
                digest,
                sequence,
                raw
            ],
        )
        .map_err(crate::sql_error)?;
        #[cfg(test)]
        {
            if _fault == Fault::StorageFull {
                return Err(Error::StorageFull);
            }
            if _fault == Fault::BeforeCommit {
                return Err(Error::BeforeCommit);
            }
        }
        if let Err(error) = tx.execute_batch("COMMIT") {
            if crate::sql_error(error) == Error::Busy {
                // COMMIT Busy has not committed. An explicit successful rollback
                // lets us report a definite bounded refusal instead of guessing.
                tx.rollback().map_err(|_| Error::OutcomeUnknown)?;
                return Err(Error::Busy);
            }
            return Err(Error::OutcomeUnknown);
        }
        drop(tx);
        #[cfg(test)]
        {
            if _fault == Fault::AfterCommit {
                return Err(Error::OutcomeUnknown);
            }
        }
        Ok(receipt)
    }
}

fn mutate(
    db: &Connection,
    command: &Command,
    now: &str,
    allocated: i64,
) -> Result<BTreeMap<Key, (i64, bool)>> {
    let mut versions = BTreeMap::new();
    match command {
        Command::RegisterProject {
            name,
            root,
            expected,
        } => {
            let revision: Option<i64> = db
                .query_row("SELECT revision FROM projects WHERE name=?1", [name], |r| {
                    r.get(0)
                })
                .optional()
                .map_err(crate::sql_error)?;
            cas(revision, expected)?;
            if db
                .query_row("SELECT 1 FROM projects WHERE root=?1", [root], |_| Ok(()))
                .optional()
                .map_err(crate::sql_error)?
                .is_some()
            {
                return Err(Error::Conflict);
            }
            db.execute("INSERT INTO projects VALUES(?1,?2,1,?3)", (name, root, now))
                .map_err(crate::sql_error)?;
            versions.insert(project_key(name), (1, false));
        }
        Command::SuggestPreference {
            scope: requested,
            key,
            value,
            source,
            ..
        } => {
            let (scope, project) = scope(requested);
            if let Some(p) = project {
                check_project(db, p)?
            }
            let next = next_revision(allocated)?;
            db.execute(
                "INSERT INTO preferences VALUES(?1,?2,?3,?4,?5,'suggested',?6,1,?7,?7)",
                params![allocated, scope, project, key, value, source, now],
            )
            .map_err(crate::sql_error)?;
            db.execute(
                "UPDATE acp_profile SET next_preference_id=?1 WHERE singleton=1",
                [next],
            )
            .map_err(crate::sql_error)?;
            versions.insert(pref_key(allocated), (1, false));
        }
        Command::ConfirmPreference {
            id,
            scope: requested,
            expected,
        }
        | Command::RejectPreference {
            id,
            scope: requested,
            expected,
        }
        | Command::DeprecatePreference {
            id,
            scope: requested,
            expected,
        }
        | Command::DeletePreference {
            id,
            scope: requested,
            expected,
        } => {
            let row: Option<(String, Option<String>, String, String, i64)> = db
                .query_row(
                    "SELECT scope,project,pref_key,status,revision FROM preferences WHERE id=?1",
                    [id],
                    |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)),
                )
                .optional()
                .map_err(crate::sql_error)?;
            cas(row.as_ref().map(|r| r.4), expected)?;
            let (stored_scope, project, key, status, revision) = row.ok_or(Error::Conflict)?;
            let (want_scope, want_project) = scope(requested);
            if stored_scope != want_scope || project.as_deref() != want_project {
                return Err(Error::ScopeMismatch);
            }
            let revision = next_revision(revision)?;
            if matches!(command, Command::DeletePreference { .. }) {
                db.execute("DELETE FROM preferences WHERE id=?1", [id])
                    .map_err(crate::sql_error)?;
                versions.insert(pref_key(*id), (revision, true));
            } else {
                let next = match command {
                    Command::ConfirmPreference { .. } if status == "suggested" => "confirmed",
                    Command::RejectPreference { .. } if status == "suggested" => "rejected",
                    Command::DeprecatePreference { .. } if status == "confirmed" => "deprecated",
                    _ => return Err(Error::InvalidTransition),
                };
                if next == "confirmed" {
                    let mut stmt=db.prepare("SELECT id,revision FROM preferences WHERE scope=?1 AND project IS ?2 AND pref_key=?3 AND status='confirmed' AND id<>?4 ORDER BY id").map_err(crate::sql_error)?;
                    let peers = stmt
                        .query_map(params![want_scope, want_project, key, id], |r| {
                            Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)?))
                        })
                        .map_err(crate::sql_error)?
                        .collect::<rusqlite::Result<Vec<_>>>()
                        .map_err(crate::sql_error)?;
                    for (peer, revision) in peers {
                        let revision = next_revision(revision)?;
                        db.execute("UPDATE preferences SET status='deprecated',revision=?1,updated_at=?2 WHERE id=?3",(revision,now,peer)).map_err(crate::sql_error)?;
                        versions.insert(pref_key(peer), (revision, false));
                    }
                }
                db.execute(
                    "UPDATE preferences SET status=?1,revision=?2,updated_at=?3 WHERE id=?4",
                    (next, revision, now, id),
                )
                .map_err(crate::sql_error)?;
                versions.insert(pref_key(*id), (revision, false));
            }
        }
        Command::PutHandoff {
            project,
            task_id,
            objective,
            completed,
            next_action,
            validation_json,
            snapshot,
            expected,
        } => {
            check_project(db, project)?;
            let current: Option<i64> = db
                .query_row(
                    "SELECT revision FROM handoffs WHERE project=?1 AND task_id=?2",
                    (project, task_id),
                    |r| r.get(0),
                )
                .optional()
                .map_err(crate::sql_error)?;
            cas(current, expected)?;
            let revision = next_revision(current.unwrap_or(0))?;
            db.execute("INSERT INTO handoffs VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12) ON CONFLICT(project,task_id) DO UPDATE SET objective=excluded.objective,base_sha=excluded.base_sha,branch=excluded.branch,worktree_state=excluded.worktree_state,worktree_fingerprint=excluded.worktree_fingerprint,completed=excluded.completed,next_action=excluded.next_action,validation_json=excluded.validation_json,revision=excluded.revision,updated_at=excluded.updated_at",params![project,task_id,objective,snapshot.base_sha,snapshot.branch,snapshot.state,snapshot.fingerprint,completed,next_action,validation_json,revision,now]).map_err(crate::sql_error)?;
            versions.insert(handoff_key(project, task_id), (revision, false));
        }
    }
    Ok(versions)
}
