use super::*;
use precept_write::Command;

impl Profiles {
    /// A confirmed record of `scope` an agent of `project` may propose an
    /// update to (its own project, or personal).
    pub fn confirmed_record(
        &self,
        project: &str,
        scope: &Scope,
        id: i64,
    ) -> Result<precept_core::operator::PreferenceRecord> {
        let s = self.state.lock().unwrap();
        match scope {
            Scope::Global => {}
            Scope::Project { name } if name == project => {}
            _ => return Err(Error::ScopeMismatch),
        }
        if !s.switches.proposers.contains(project) || !s.roots.contains_key(project) {
            return Err(Error::Unauthorized);
        }
        let reader = s
            .selected
            .as_ref()
            .ok_or(Error::Unavailable)?
            .reader()
            .map_err(storage_error)?;
        let record = reader
            .operator_preference(scope, id)
            .map_err(|e| match e {
                precept_store::Error::PreferenceNotFound => Error::NotFound,
                _ => Error::Unavailable,
            })?
            .record;
        if record.status != "confirmed" {
            return Err(Error::NotFound);
        }
        Ok(record)
    }
    /// An agent of `project` suggests a rule. Accepted only while the operator
    /// allows suggestions for the project and has writes on for this run;
    /// duplicates are refused. The record lands as `suggested` with origin
    /// `agent`: confirming it stays the operator's.
    pub fn agent_suggest(
        &self,
        project: &str,
        scope: Scope,
        key: String,
        value: String,
        tags: Vec<String>,
        rule: precept_core::operator::RuleOptions,
    ) -> Result<Value> {
        let s = self.state.lock().unwrap();
        if !s.switches.proposers.contains(project) || !s.roots.contains_key(project) {
            return Err(Error::Unauthorized);
        }
        // Agents reach their own project or the personal scope.
        match &scope {
            Scope::Global => {}
            Scope::Project { name } if name == project => {}
            _ => return Err(Error::ScopeMismatch),
        }
        if value.trim().is_empty() || value.len() > MAX_AGENT_VALUE {
            return Err(Error::InvalidRequest);
        }
        let (owner, grant) = match (s.owner.as_ref(), s.grant.as_ref()) {
            (Some(o), Some(g)) if g.check().is_ok() => (o, g.clone()),
            // Writes are off for this run: nothing is queued anywhere.
            _ => return Err(Error::Unavailable),
        };
        let reader = s
            .selected
            .as_ref()
            .ok_or(Error::Unavailable)?
            .reader()
            .map_err(storage_error)?;
        let (duplicate, _) = reader
            .agent_suggestion_state(&scope, &key, &value)
            .map_err(|_| Error::Unavailable)?;
        if duplicate {
            return Err(Error::Conflict);
        }
        let b = owner.binding();
        let request = precept_write::Request {
            version: 2,
            binding: precept_write::Binding {
                profile_id: b.profile_id,
                generation: b.generation,
                owner_epoch: b.owner_epoch,
            },
            operation_id: precept_ipc::operator::operation_id()?,
            command: Command::SuggestPreference {
                scope,
                key,
                value,
                tags,
                origin: precept_core::operator::Origin::Agent,
                rule,
                expected: precept_write::Expected::Absent,
            },
        };
        let receipt = owner
            .execute(&request, &|| {
                grant
                    .check()
                    .map_err(|_| precept_write::Error::Unauthorized)
            })
            .map_err(storage_error)?;
        let id = receipt
            .affected
            .first()
            .map(|a| a.record_id.clone())
            .unwrap_or_default();
        Ok(json!({"status":"suggested","id":id}))
    }
}
