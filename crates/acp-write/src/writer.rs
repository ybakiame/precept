//! All mutation/revision/receipt work shares one private SQLite transaction.
use crate::{
    Binding, Command, Error, Expected, Fault, Receipt, RecordVersion, Request, Result, Scope,
    Snapshot, SyntheticWorkspace, schema,
};
use rusqlite::{Connection, OptionalExtension, TransactionBehavior, params};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
type Key = (String, String, String);

pub struct Writer<'a> {
    pub(crate) db: Connection,
    pub(crate) workspace: &'a SyntheticWorkspace,
}
impl std::fmt::Debug for Writer<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("SyntheticWriter(redacted)")
    }
}
#[derive(Clone, Copy)]
struct Profile {
    sequence: i64,
    limit: i64,
}
fn project_key(project: &str) -> Key {
    ("project".into(), String::new(), project.into())
}
fn pref_key(id: i64) -> Key {
    ("preference".into(), String::new(), id.to_string())
}
fn handoff_key(project: &str, task: &str) -> Key {
    ("handoff".into(), project.into(), task.into())
}
fn revisions(db: &Connection) -> Result<BTreeMap<Key, RecordVersion>> {
    let mut stmt=db.prepare("SELECT kind,project,record_id,revision,deleted FROM acp_revisions ORDER BY kind,project,record_id").map_err(crate::sql_error)?;
    let rows = stmt
        .query_map([], |r| {
            Ok(RecordVersion {
                kind: r.get(0)?,
                project: r.get(1)?,
                record_id: r.get(2)?,
                revision: r.get(3)?,
                deleted: r.get::<_, i64>(4)? != 0,
            })
        })
        .map_err(crate::sql_error)?;
    rows.map(|r| {
        r.map(|v| ((v.kind.clone(), v.project.clone(), v.record_id.clone()), v))
            .map_err(|_| Error::InvalidRecord)
    })
    .collect()
}
fn profile(db: &Connection, binding: &Binding) -> Result<Profile> {
    if schema::inspect(db)? != schema::Shape::V2 {
        return Err(Error::UnsupportedVersion);
    }
    let count: i64 = db
        .query_row("SELECT count(*) FROM acp_profile", [], |r| r.get(0))
        .map_err(|_| Error::InvalidRecord)?;
    if count != 1 {
        return Err(Error::InvalidRecord);
    }
    let (id,generation,epoch,sequence,limit):(String,String,i64,i64,i64)=db.query_row("SELECT profile_id,generation,owner_epoch,commit_sequence,receipt_limit FROM acp_profile WHERE singleton=1",[],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?))).map_err(|_|Error::InvalidRecord)?;
    if id != binding.profile_id || epoch != binding.owner_epoch {
        return Err(Error::OwnerMismatch);
    }
    if generation != binding.generation {
        return Err(Error::StaleGeneration);
    }
    if sequence < 0 || !(1..=4096).contains(&limit) {
        return Err(Error::InvalidRecord);
    }
    Ok(Profile { sequence, limit })
}
pub(crate) fn verify(db: &Connection, binding: &Binding) -> Result<()> {
    let metadata = profile(db, binding)?;
    let allocated = schema::validate_records(db)?;
    let records = schema::records(db)?;
    let revs = revisions(db)?;
    let mut live = BTreeMap::new();
    use rusqlite::types::Value;
    for row in &records[0] {
        if let Value::Text(name) = &row[0] {
            live.insert(project_key(name), ());
        }
    }
    for row in &records[1] {
        if let Value::Integer(id) = row[0] {
            live.insert(pref_key(id), ());
        }
    }
    for row in &records[2] {
        if let (Value::Text(project), Value::Text(task)) = (&row[0], &row[1]) {
            live.insert(handoff_key(project, task), ());
        }
    }
    for (key, rev) in &revs {
        if rev.revision <= 0 {
            return Err(Error::InvalidRecord);
        }
        if rev.deleted {
            let id = rev
                .record_id
                .parse::<i64>()
                .map_err(|_| Error::InvalidRecord)?;
            if rev.kind != "preference"
                || !rev.project.is_empty()
                || id <= 0
                || id.to_string() != rev.record_id
                || id > allocated
                || live.contains_key(key)
            {
                return Err(Error::InvalidRecord);
            }
        } else if live.remove(key).is_none() {
            return Err(Error::InvalidRecord);
        }
    }
    if !live.is_empty() {
        return Err(Error::InvalidRecord);
    }
    let mut stmt=db.prepare("SELECT operation_id,generation,request_digest,commit_sequence,result_json FROM acp_receipts ORDER BY commit_sequence").map_err(crate::sql_error)?;
    let rows = stmt
        .query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, Vec<u8>>(2)?,
                r.get::<_, i64>(3)?,
                r.get::<_, String>(4)?,
            ))
        })
        .map_err(crate::sql_error)?;
    let mut count = 0;
    for row in rows {
        let (operation, generation, digest, sequence, raw) =
            row.map_err(|_| Error::InvalidRecord)?;
        count += 1;
        if count != sequence
            || generation != binding.generation
            || digest.len() != 32
            || raw.len() > 65536
        {
            return Err(Error::InvalidRecord);
        }
        let receipt: Receipt = serde_json::from_str(&raw).map_err(|_| Error::InvalidRecord)?;
        if receipt.generation != generation
            || receipt.operation_id != operation
            || receipt.commit_sequence != sequence
            || receipt.affected.is_empty()
            || receipt.affected.len() > 1024
        {
            return Err(Error::InvalidRecord);
        }
        let mut previous = None;
        for affected in &receipt.affected {
            let key = (
                affected.kind.clone(),
                affected.project.clone(),
                affected.record_id.clone(),
            );
            if previous.as_ref().is_some_and(|p| p >= &key)
                || affected.revision <= 0
                || revs
                    .get(&key)
                    .is_none_or(|v| v.revision < affected.revision)
            {
                return Err(Error::InvalidRecord);
            }
            previous = Some(key);
        }
    }
    if count != metadata.sequence || count > metadata.limit {
        return Err(Error::InvalidRecord);
    }
    Ok(())
}
fn cas(db: &Connection, key: &Key, expected: &Expected) -> Result<()> {
    let current:Option<(i64,bool)>=db.query_row("SELECT revision,deleted FROM acp_revisions WHERE kind=?1 AND project=?2 AND record_id=?3",(&key.0,&key.1,&key.2),|r|Ok((r.get(0)?,r.get::<_,i64>(1)?!=0))).optional().map_err(crate::sql_error)?;
    match (expected, current) {
        (Expected::Absent, None) => Ok(()),
        (Expected::Revision(wanted), Some((actual, false))) if *wanted == actual => Ok(()),
        _ => Err(Error::Conflict),
    }
}
fn advance(db: &Connection, key: &Key, deleted: bool) -> Result<RecordVersion> {
    let current: Option<i64> = db
        .query_row(
            "SELECT revision FROM acp_revisions WHERE kind=?1 AND project=?2 AND record_id=?3",
            (&key.0, &key.1, &key.2),
            |r| r.get(0),
        )
        .optional()
        .map_err(crate::sql_error)?;
    let revision = current.unwrap_or(0).checked_add(1).ok_or(Error::Capacity)?;
    db.execute("INSERT INTO acp_revisions VALUES(?1,?2,?3,?4,?5) ON CONFLICT(kind,project,record_id) DO UPDATE SET revision=excluded.revision,deleted=excluded.deleted",params![key.0,key.1,key.2,revision,deleted]).map_err(crate::sql_error)?;
    Ok(RecordVersion {
        kind: key.0.clone(),
        project: key.1.clone(),
        record_id: key.2.clone(),
        revision,
        deleted,
    })
}
fn check_project(db: &Connection, project: &str) -> Result<()> {
    if db
        .query_row(
            "SELECT 1 FROM projects WHERE name=?1",
            [project],
            |_| Ok(()),
        )
        .optional()
        .map_err(crate::sql_error)?
        .is_none()
    {
        return Err(Error::Conflict);
    }
    Ok(())
}
fn scope(scope: &Scope) -> (&str, Option<&str>) {
    match scope {
        Scope::Global => ("global", None),
        Scope::Project { name } => ("project", Some(name.as_str())),
    }
}
impl Writer<'_> {
    pub fn execute(&mut self, request: &Request) -> Result<Receipt> {
        self.execute_inner(request, Fault::None)
    }
    pub(crate) fn execute_inner(&mut self, request: &Request, _fault: Fault) -> Result<Receipt> {
        request.validate()?;
        self.workspace.check()?;
        let tx = self
            .db
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(crate::sql_error)?;
        verify(&tx, &self.workspace.binding)?;
        if request.binding.profile_id != self.workspace.binding.profile_id
            || request.binding.owner_epoch != self.workspace.binding.owner_epoch
        {
            return Err(Error::OwnerMismatch);
        }
        if request.binding.generation != self.workspace.binding.generation {
            return Err(Error::StaleGeneration);
        }
        let metadata = profile(&tx, &request.binding)?;
        let canonical = serde_json::to_vec(request).map_err(|_| Error::InvalidCommand)?;
        let digest: Vec<u8> = Sha256::digest(&canonical).to_vec();
        let prior: Option<(Vec<u8>, String)> = tx
            .query_row(
                "SELECT request_digest,result_json FROM acp_receipts WHERE operation_id=?1",
                [&request.operation_id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()
            .map_err(crate::sql_error)?;
        if let Some((before, result)) = prior {
            if before != digest {
                return Err(Error::IdempotencyConflict);
            }
            return serde_json::from_str(&result).map_err(|_| Error::InvalidRecord);
        }
        if metadata.sequence >= metadata.limit {
            return Err(Error::Capacity);
        }
        let sequence = metadata.sequence.checked_add(1).ok_or(Error::Capacity)?;
        let now: String = tx
            .query_row(
                "SELECT strftime('%Y-%m-%dT%H:%M:%S+00:00','now')",
                [],
                |r| r.get(0),
            )
            .map_err(crate::sql_error)?;
        let affected = mutate(&tx, self.workspace, &request.command, &now)?;
        #[cfg(test)]
        if _fault == Fault::AfterData {
            return Err(Error::BeforeCommit);
        }
        let mut versions = Vec::new();
        for (key, deleted) in affected {
            versions.push(advance(&tx, &key, deleted)?);
        }
        versions.sort();
        #[cfg(test)]
        if _fault == Fault::AfterRevisions {
            return Err(Error::BeforeCommit);
        }
        let receipt = Receipt {
            generation: request.binding.generation.clone(),
            operation_id: request.operation_id.clone(),
            commit_sequence: sequence,
            affected: versions,
        };
        let result = serde_json::to_string(&receipt).map_err(|_| Error::Capacity)?;
        if receipt.affected.len() > 1024 || result.len() > 65536 {
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
                result
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
            if _fault == Fault::PauseBeforeCommit {
                pause_child();
            }
        }
        tx.commit().map_err(|_| Error::OutcomeUnknown)?;
        #[cfg(test)]
        if _fault == Fault::AfterCommit {
            return Err(Error::OutcomeUnknown);
        }
        #[cfg(test)]
        if _fault == Fault::PauseAfterCommit {
            pause_child();
        }
        Ok(receipt)
    }
}
fn mutate(
    db: &Connection,
    workspace: &SyntheticWorkspace,
    command: &Command,
    now: &str,
) -> Result<BTreeMap<Key, bool>> {
    let mut affected = BTreeMap::new();
    match command {
        Command::RegisterProject {
            name,
            root,
            expected,
        } => {
            let key = project_key(name);
            cas(db, &key, expected)?;
            let root = workspace.project_root(*root)?;
            if db
                .query_row("SELECT 1 FROM projects WHERE root=?1", [&root], |_| Ok(()))
                .optional()
                .map_err(crate::sql_error)?
                .is_some()
            {
                return Err(Error::Conflict);
            }
            db.execute("INSERT INTO projects VALUES(?1,?2,?3)", (name, root, now))
                .map_err(crate::sql_error)?;
            affected.insert(key, false);
        }
        Command::SuggestPreference {
            scope: requested,
            key,
            value,
            source,
            expected: _,
        } => {
            let (scope, project) = scope(requested);
            if let Some(project) = project {
                check_project(db, project)?;
            }
            db.execute("INSERT INTO preferences(scope,project,pref_key,value,status,source,created_at,updated_at) VALUES(?1,?2,?3,?4,'suggested',?5,?6,?6)",params![scope,project,key,value,source,now]).map_err(crate::sql_error)?;
            let id = db.last_insert_rowid();
            let key = pref_key(id);
            cas(db, &key, &Expected::Absent)?;
            affected.insert(key, false);
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
            let key = pref_key(*id);
            cas(db, &key, expected)?;
            let (stored_scope, project, pref_key, status): (
                String,
                Option<String>,
                String,
                String,
            ) = db
                .query_row(
                    "SELECT scope,project,pref_key,status FROM preferences WHERE id=?1",
                    [id],
                    |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
                )
                .map_err(|_| Error::InvalidRecord)?;
            let (wanted_scope, wanted_project) = scope(requested);
            if wanted_scope != stored_scope || wanted_project != project.as_deref() {
                return Err(Error::ScopeMismatch);
            }
            if matches!(command, Command::DeletePreference { .. }) {
                db.execute("DELETE FROM preferences WHERE id=?1", [id])
                    .map_err(crate::sql_error)?;
                affected.insert(key, true);
            } else {
                let next = match command {
                    Command::ConfirmPreference { .. } => "confirmed",
                    Command::RejectPreference { .. } => "rejected",
                    _ => "deprecated",
                };
                if !matches!(
                    (status.as_str(), next),
                    ("suggested", "confirmed" | "rejected") | ("confirmed", "deprecated")
                ) {
                    return Err(Error::InvalidTransition);
                }
                if next == "confirmed" {
                    let mut stmt=db.prepare("SELECT id FROM preferences WHERE id<>?1 AND scope=?2 AND project IS ?3 AND pref_key=?4 AND status='confirmed'").map_err(crate::sql_error)?;
                    let peers = stmt
                        .query_map(params![id, stored_scope, project, pref_key], |r| {
                            r.get::<_, i64>(0)
                        })
                        .map_err(crate::sql_error)?
                        .collect::<rusqlite::Result<Vec<_>>>()
                        .map_err(crate::sql_error)?;
                    if peers.len() >= 1024 {
                        return Err(Error::Capacity);
                    }
                    for peer in peers {
                        db.execute(
                            "UPDATE preferences SET status='deprecated',updated_at=?1 WHERE id=?2",
                            (now, peer),
                        )
                        .map_err(crate::sql_error)?;
                        affected.insert(crate::writer::pref_key(peer), false);
                    }
                }
                db.execute(
                    "UPDATE preferences SET status=?1,updated_at=?2 WHERE id=?3",
                    (next, now, id),
                )
                .map_err(crate::sql_error)?;
                affected.insert(key, false);
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
            let key = handoff_key(project, task_id);
            cas(db, &key, expected)?;
            let (state, fingerprint) = match snapshot {
                Snapshot::Clean => ("clean", Some("synthetic-clean")),
                Snapshot::Dirty => ("dirty", Some("synthetic-dirty")),
                Snapshot::UnknownFingerprint => ("clean", None),
            };
            db.execute("INSERT INTO handoffs(project,task_id,objective,base_sha,branch,worktree_state,worktree_fingerprint,completed,next_action,validation_json,updated_at) VALUES(?1,?2,?3,'synthetic-head','synthetic-branch',?4,?5,?6,?7,?8,?9) ON CONFLICT(project,task_id) DO UPDATE SET objective=excluded.objective,base_sha=excluded.base_sha,branch=excluded.branch,worktree_state=excluded.worktree_state,worktree_fingerprint=excluded.worktree_fingerprint,completed=excluded.completed,next_action=excluded.next_action,validation_json=excluded.validation_json,updated_at=excluded.updated_at",params![project,task_id,objective,state,fingerprint,completed,next_action,validation_json,now]).map_err(crate::sql_error)?;
            affected.insert(key, false);
        }
    }
    Ok(affected)
}

#[cfg(test)]
fn pause_child() -> ! {
    use std::io::Write;
    println!("READY");
    std::io::stdout().flush().unwrap();
    loop {
        std::thread::park_timeout(std::time::Duration::from_secs(1));
    }
}
