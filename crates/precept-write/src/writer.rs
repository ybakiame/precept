//! Typed data/revision/allocator/receipt updates in one bounded transaction.
use crate::{
    Binding, Command, Error, Expected, Fault, Profile, Receipt, RecordVersion, Request, Result,
    Scope,
};
use precept_store::profile;
use rusqlite::{Connection, OptionalExtension, TransactionBehavior, params};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
type Key = (String, String, String);
/// Stored scope/project/key/status/revision/digest read inside a decision.
/// scope, project, status, revision, proposal digest, enabled.
type SwitchRow = (String, Option<String>, String, i64, Vec<u8>, i64);
type ProposalRow = (String, Option<String>, String, String, i64, Vec<u8>);
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
        Scope::Environment { name } => ("environment", Some(name)),
    }
}
fn channel_name(channel: precept_core::operator::Channel) -> &'static str {
    match channel {
        precept_core::operator::Channel::Cli => "cli",
        precept_core::operator::Channel::Desktop => "desktop",
        precept_core::operator::Channel::Tui => "tui",
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
        self.execute_checked(request, _fault, &|| Ok(()))
    }
    pub(crate) fn execute_checked(
        &mut self,
        request: &Request,
        _fault: Fault,
        admit: &dyn Fn() -> Result<()>,
    ) -> Result<Receipt> {
        request.validate()?;
        self.profile.check()?;
        // All filesystem work is outside the write transaction. Registration is
        // storage data, not the later daemon's independent sharing allowlist.
        let registration_valid = if let Command::RegisterProject { root, .. } = &request.command {
            let p = std::path::Path::new(root);
            p.is_dir() && p.canonicalize().ok().as_deref() == Some(p)
        } else {
            true
        };
        binding(&self.profile.binding, &request.binding)?;
        admit()?;
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
        // Current directory availability must not invalidate a committed replay.
        // The observation was made before BEGIN; no filesystem work occurs here.
        if !registration_valid {
            return Err(Error::InvalidCommand);
        }
        let sequence = next_revision(metadata.commit_sequence)?;
        let now = profile::timestamp()?;
        let versions = mutate(
            &tx,
            &request.command,
            &now,
            metadata.next_preference_id,
            sequence,
            metadata.next_review_id,
        )?;
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
    sequence: i64,
    next_review_id: i64,
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
            tags,
            origin,
            rule,
            ..
        } => {
            let (scope, project) = scope(requested);
            if let Some(p) = project {
                check_project(db, p)?
            }
            let next = next_revision(allocated)?;
            let digest = precept_core::operator::proposal_digest(
                requested, project, key, value, *origin, tags, rule,
            );
            db.execute(
                "INSERT INTO preferences(id,scope,project,pref_key,value,status,tags,revision,created_at,updated_at,origin,proposal_digest,applies_to,priority,enforcement,enabled) VALUES(?1,?2,?3,?4,?5,'suggested',?6,1,?7,?7,?8,?9,?10,?11,?12,1)",
                params![
                    allocated,
                    scope,
                    project,
                    key,
                    value,
                    precept_core::operator::tags_json(tags),
                    now,
                    origin.as_str(),
                    digest.to_vec(),
                    rule.applies_to.json(),
                    rule.priority,
                    rule.enforcement.as_str()
                ],
            )
            .map_err(crate::sql_error)?;
            db.execute(
                "UPDATE acp_profile SET next_preference_id=?1 WHERE singleton=1",
                [next],
            )
            .map_err(crate::sql_error)?;
            versions.insert(pref_key(allocated), (1, false));
        }
        Command::SetPreferenceEnabled {
            id,
            scope: requested,
            enabled,
            channel,
            expected,
        } => {
            let row: Option<SwitchRow> = db
                .query_row(
                    "SELECT scope,project,status,revision,proposal_digest,enabled FROM preferences WHERE id=?1",
                    [id],
                    |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?)),
                )
                .optional()
                .map_err(crate::sql_error)?;
            cas(row.as_ref().map(|r| r.3), expected)?;
            let (stored_scope, project, status, previous, digest, current) =
                row.ok_or(Error::Conflict)?;
            let (want_scope, want_project) = scope(requested);
            if stored_scope != want_scope || project.as_deref() != want_project {
                return Err(Error::ScopeMismatch);
            }
            if status != "confirmed" || (current == 1) == *enabled {
                return Err(Error::InvalidTransition);
            }
            let revision = next_revision(previous)?;
            db.execute(
                "UPDATE preferences SET enabled=?1,revision=?2,updated_at=?3 WHERE id=?4",
                (i64::from(*enabled), revision, now, id),
            )
            .map_err(crate::sql_error)?;
            db.execute(
                "INSERT INTO acp_review_decisions(id,scope,project,preference_id,action,from_revision,to_revision,proposal_digest,receipt_commit_sequence,channel,decided_at) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11)",
                params![
                    next_review_id,
                    want_scope,
                    want_project,
                    id,
                    if *enabled { "enable" } else { "disable" },
                    previous,
                    revision,
                    digest,
                    sequence,
                    channel_name(*channel),
                    now
                ],
            )
            .map_err(crate::sql_error)?;
            db.execute(
                "UPDATE acp_profile SET next_review_id=?1 WHERE singleton=1",
                [next_review_id.checked_add(1).ok_or(Error::Capacity)?],
            )
            .map_err(crate::sql_error)?;
            versions.insert(pref_key(*id), (revision, false));
        }
        Command::ConfirmPreference {
            id,
            scope: requested,
            proposal_digest,
            channel,
            expected,
            ..
        }
        | Command::RejectPreference {
            id,
            scope: requested,
            proposal_digest,
            channel,
            expected,
            ..
        }
        | Command::DeprecatePreference {
            id,
            scope: requested,
            proposal_digest,
            channel,
            expected,
            ..
        }
        | Command::DeletePreference {
            id,
            scope: requested,
            proposal_digest,
            channel,
            expected,
        } => {
            let row: Option<ProposalRow> = db
                .query_row(
                    "SELECT scope,project,pref_key,status,revision,proposal_digest FROM preferences WHERE id=?1",
                    [id],
                    |r| {
                        Ok((
                            r.get(0)?,
                            r.get(1)?,
                            r.get(2)?,
                            r.get(3)?,
                            r.get(4)?,
                            r.get(5)?,
                        ))
                    },
                )
                .optional()
                .map_err(crate::sql_error)?;
            cas(row.as_ref().map(|r| r.4), expected)?;
            let (stored_scope, project, key, status, previous, stored_digest) =
                row.ok_or(Error::Conflict)?;
            let (want_scope, want_project) = scope(requested);
            if stored_scope != want_scope || project.as_deref() != want_project {
                return Err(Error::ScopeMismatch);
            }
            // The decision names the exact proposal the operator inspected.
            let supplied = precept_core::operator::digest_from_hex(proposal_digest)
                .ok_or(Error::ReviewMismatch)?;
            if supplied.as_slice() != stored_digest.as_slice() {
                return Err(Error::ReviewMismatch);
            }
            let revision = next_revision(previous)?;
            if matches!(command, Command::DeletePreference { .. }) {
                // Erasure removes the decision history with the record; no row.
                db.execute(
                    "DELETE FROM acp_review_decisions WHERE preference_id=?1",
                    [id],
                )
                .map_err(crate::sql_error)?;
                db.execute("DELETE FROM preferences WHERE id=?1", [id])
                    .map_err(crate::sql_error)?;
                versions.insert(pref_key(*id), (revision, true));
            } else {
                let (next, action) = match command {
                    Command::ConfirmPreference { .. } if status == "suggested" => {
                        ("confirmed", "confirm")
                    }
                    Command::RejectPreference { .. } if status == "suggested" => {
                        ("rejected", "reject")
                    }
                    Command::DeprecatePreference { .. } if status == "confirmed" => {
                        ("deprecated", "deprecate")
                    }
                    _ => return Err(Error::InvalidTransition),
                };
                let channel = channel_name(*channel);
                let mut review_id = next_review_id;
                let note = command.note();
                let mut decision = |preference_id: i64,
                                    action: &str,
                                    from: i64,
                                    to: i64,
                                    digest: &[u8],
                                    note: Option<&str>|
                 -> Result<()> {
                    let id = review_id;
                    review_id = review_id.checked_add(1).ok_or(Error::Capacity)?;
                    db.execute(
                        "INSERT INTO acp_review_decisions(id,scope,project,preference_id,action,from_revision,to_revision,proposal_digest,receipt_commit_sequence,channel,decided_at,note) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12)",
                        params![
                            id,
                            want_scope,
                            want_project,
                            preference_id,
                            action,
                            from,
                            to,
                            digest.to_vec(),
                            sequence,
                            channel,
                            now,
                            note
                        ],
                    )
                    .map_err(crate::sql_error)?;
                    Ok(())
                };
                if next == "confirmed" {
                    let mut stmt=db.prepare("SELECT id,revision,proposal_digest FROM preferences WHERE scope=?1 AND project IS ?2 AND pref_key=?3 AND status='confirmed' AND id<>?4 ORDER BY id").map_err(crate::sql_error)?;
                    let peers = stmt
                        .query_map(params![want_scope, want_project, key, id], |r| {
                            Ok((
                                r.get::<_, i64>(0)?,
                                r.get::<_, i64>(1)?,
                                r.get::<_, Vec<u8>>(2)?,
                            ))
                        })
                        .map_err(crate::sql_error)?
                        .collect::<rusqlite::Result<Vec<_>>>()
                        .map_err(crate::sql_error)?;
                    for (peer, peer_previous, peer_digest) in peers {
                        let peer_revision = next_revision(peer_previous)?;
                        db.execute("UPDATE preferences SET status='deprecated',revision=?1,updated_at=?2 WHERE id=?3",(peer_revision,now,peer)).map_err(crate::sql_error)?;
                        decision(
                            peer,
                            "demote",
                            peer_previous,
                            peer_revision,
                            &peer_digest,
                            None,
                        )?;
                        versions.insert(pref_key(peer), (peer_revision, false));
                    }
                }
                db.execute(
                    "UPDATE preferences SET status=?1,revision=?2,updated_at=?3 WHERE id=?4",
                    (next, revision, now, id),
                )
                .map_err(crate::sql_error)?;
                decision(*id, action, previous, revision, &stored_digest, note)?;
                db.execute(
                    "UPDATE acp_profile SET next_review_id=?1 WHERE singleton=1",
                    [review_id],
                )
                .map_err(crate::sql_error)?;
                versions.insert(pref_key(*id), (revision, false));
            }
        }
        Command::ReviseSuggestion {
            id,
            scope: requested,
            proposal_digest,
            key,
            value,
            tags,
            rule,
            channel,
            expected,
            note,
        } => {
            let row: Option<ProposalRow> = db
                .query_row(
                    "SELECT scope,project,pref_key,status,revision,proposal_digest FROM preferences WHERE id=?1",
                    [id],
                    |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?)),
                )
                .optional()
                .map_err(crate::sql_error)?;
            cas(row.as_ref().map(|r| r.4), expected)?;
            let (stored_scope, project, _, status, previous, stored_digest) =
                row.ok_or(Error::Conflict)?;
            let (want_scope, want_project) = scope(requested);
            if stored_scope != want_scope || project.as_deref() != want_project {
                return Err(Error::ScopeMismatch);
            }
            let supplied = precept_core::operator::digest_from_hex(proposal_digest)
                .ok_or(Error::ReviewMismatch)?;
            if supplied.as_slice() != stored_digest.as_slice() {
                return Err(Error::ReviewMismatch);
            }
            if status != "suggested" {
                return Err(Error::InvalidTransition);
            }
            // The edited version: a new suggestion in the same scope, by the
            // operator (it is their wording now), still to be confirmed.
            let origin = precept_core::operator::Origin::Operator;
            let digest = precept_core::operator::proposal_digest(
                requested,
                want_project,
                key,
                value,
                origin,
                tags,
                rule,
            );
            db.execute(
                "INSERT INTO preferences(id,scope,project,pref_key,value,status,tags,revision,created_at,updated_at,origin,proposal_digest,applies_to,priority,enforcement,enabled) VALUES(?1,?2,?3,?4,?5,'suggested',?6,1,?7,?7,?8,?9,?10,?11,?12,1)",
                params![
                    allocated,
                    want_scope,
                    want_project,
                    key,
                    value,
                    precept_core::operator::tags_json(tags),
                    now,
                    origin.as_str(),
                    digest.to_vec(),
                    rule.applies_to.json(),
                    rule.priority,
                    rule.enforcement.as_str()
                ],
            )
            .map_err(crate::sql_error)?;
            db.execute(
                "UPDATE acp_profile SET next_preference_id=?1 WHERE singleton=1",
                [next_revision(allocated)?],
            )
            .map_err(crate::sql_error)?;
            versions.insert(pref_key(allocated), (1, false));
            // The old one is closed, naming its replacement.
            let revision = next_revision(previous)?;
            db.execute(
                "UPDATE preferences SET status='rejected',revision=?1,updated_at=?2 WHERE id=?3",
                (revision, now, id),
            )
            .map_err(crate::sql_error)?;
            db.execute(
                "INSERT INTO acp_review_decisions(id,scope,project,preference_id,action,from_revision,to_revision,proposal_digest,receipt_commit_sequence,channel,decided_at,note,replaced_by) VALUES(?1,?2,?3,?4,'replace',?5,?6,?7,?8,?9,?10,?11,?12)",
                params![
                    next_review_id,
                    want_scope,
                    want_project,
                    id,
                    previous,
                    revision,
                    stored_digest,
                    sequence,
                    channel_name(*channel),
                    now,
                    note,
                    allocated
                ],
            )
            .map_err(crate::sql_error)?;
            db.execute(
                "UPDATE acp_profile SET next_review_id=?1 WHERE singleton=1",
                [next_review_id.checked_add(1).ok_or(Error::Capacity)?],
            )
            .map_err(crate::sql_error)?;
            versions.insert(pref_key(*id), (revision, false));
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
