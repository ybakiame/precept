use super::*;
use crate::personal_sync;
use precept_core::operator::{
    PersonalSyncAbortRequest, PersonalSyncApplySubmission, PersonalSyncExportRequest,
    PersonalSyncOpenRequest, PersonalSyncProfileScope, PersonalSyncResetRequest,
    PersonalSyncRevokeRequest, PersonalSyncTrustRequest,
};
use precept_ipc::{Access, peer::Peer};
use serde_json::Value;

fn bind_profile(params: Value, binding: &precept_write::Binding) -> Result<Value> {
    let Value::Object(mut fields) = params else {
        return Err(Error::InvalidRequest);
    };
    let expected = fields
        .remove("expected_binding")
        .ok_or(Error::InvalidRequest)?;
    let expected: PersonalSyncProfileScope = crate::decode(expected)?;
    if !expected.valid()
        || expected.profile_id != binding.profile_id
        || expected.generation != binding.generation
    {
        return Err(Error::Unauthorized);
    }
    Ok(Value::Object(fields))
}

impl Profiles {
    pub(super) fn sync(&self, authority: &Peer, method: &str, params: Value) -> Result<Value> {
        let mut params = params;
        let write = matches!(
            method,
            "profile.sync.init"
                | "profile.sync.pair"
                | "profile.sync.trust"
                | "profile.sync.revoke"
                | "profile.sync.rotate"
                | "profile.sync.reset"
                | "profile.sync.export"
                | "profile.sync.abort"
                | "profile.sync.apply"
        );
        let _sync = self.sync_lock.lock().unwrap();
        let state = self.state.lock().unwrap();
        authority.check()?;
        let selected = state.selected.clone().ok_or(Error::Unauthorized)?;
        let store = selected.reader().map_err(|_| Error::Unavailable)?;
        if write {
            if !matches!(authority.access(), Access::OperatorWrite { .. }) {
                return Err(Error::Unauthorized);
            }
            let grant = state.grant.as_ref().ok_or(Error::Unauthorized)?;
            if grant.access() != authority.access() || grant.check().is_err() {
                return Err(Error::Unauthorized);
            }
            let owner = state.owner.as_ref().ok_or(Error::Unauthorized)?;
            let metadata = store.metadata().map_err(|_| Error::Unavailable)?;
            let binding = &metadata.binding;
            let Access::OperatorWrite {
                profile_id,
                generation,
                owner_epoch,
            } = authority.access()
            else {
                unreachable!()
            };
            if binding.profile_id != profile_id
                || binding.generation != generation
                || binding.owner_epoch != owner_epoch
            {
                return Err(Error::Unauthorized);
            }
            params = bind_profile(params, binding)?;
            let lookup = |request: &precept_write::Request| {
                store
                    .operator_receipt(&request.operation_id)
                    .map_err(|_| Error::Unavailable)
            };
            if method == "profile.sync.abort" {
                let request: PersonalSyncAbortRequest = crate::decode(params)?;
                return personal_sync::abort(&self.state_dir, binding, request, lookup).and_then(
                    |result| serde_json::to_value(result).map_err(|_| Error::Unavailable),
                );
            }
            if method == "profile.sync.apply" {
                let submission: PersonalSyncApplySubmission = crate::decode(params)?;
                return personal_sync::apply(
                    &self.state_dir,
                    binding,
                    submission,
                    &personal_sync::current_globals(&store)?,
                    metadata.commit_sequence,
                    |request| {
                        owner
                            .execute(request, &|| {
                                grant
                                    .check()
                                    .map_err(|_| precept_write::Error::Unauthorized)
                            })
                            .map_err(storage_error)
                    },
                    lookup,
                )
                .and_then(|result| serde_json::to_value(result).map_err(|_| Error::Unavailable));
            }
        } else if authority.access() != Access::Operator {
            return Err(Error::Unauthorized);
        }
        drop(state);
        let metadata = store.metadata().map_err(|_| Error::Unavailable)?;
        if method == "profile.sync.preview" {
            params = bind_profile(params, &metadata.binding)?;
        }
        let current = personal_sync::current_globals(&store)?;
        macro_rules! reply {
            ($value:expr) => {
                serde_json::to_value($value).map_err(|_| Error::Unavailable)
            };
        }
        match method {
            "profile.sync.status" => {
                crate::decode::<crate::Empty>(params)?;
                reply!(personal_sync::status(
                    &self.state_dir,
                    &metadata.binding.profile_id
                )?)
            }
            "profile.sync.init" => {
                crate::decode::<crate::Empty>(params)?;
                reply!(personal_sync::init(
                    &self.state_dir,
                    &metadata.binding.profile_id,
                    &current,
                )?)
            }
            "profile.sync.invitation" => {
                crate::decode::<crate::Empty>(params)?;
                reply!(personal_sync::get_invitation(
                    &self.state_dir,
                    &metadata.binding.profile_id
                )?)
            }
            "profile.sync.pair" => {
                let request: PersonalSyncTrustRequest = crate::decode(params)?;
                reply!(personal_sync::pair(
                    &self.state_dir,
                    &metadata.binding.profile_id,
                    request,
                    &current,
                )?)
            }
            "profile.sync.trust" => {
                let request: PersonalSyncTrustRequest = crate::decode(params)?;
                reply!(personal_sync::trust(
                    &self.state_dir,
                    &metadata.binding.profile_id,
                    request,
                )?)
            }
            "profile.sync.revoke" => {
                let request: PersonalSyncRevokeRequest = crate::decode(params)?;
                reply!(personal_sync::revoke(
                    &self.state_dir,
                    &metadata.binding.profile_id,
                    request,
                )?)
            }
            "profile.sync.rotate" => {
                crate::decode::<crate::Empty>(params)?;
                reply!(personal_sync::rotate(
                    &self.state_dir,
                    &metadata.binding.profile_id
                )?)
            }
            "profile.sync.reset" => {
                let request: PersonalSyncResetRequest = crate::decode(params)?;
                reply!(personal_sync::reset(
                    &self.state_dir,
                    &metadata.binding.profile_id,
                    request,
                    &current,
                )?)
            }
            "profile.sync.export" => {
                let request: PersonalSyncExportRequest = crate::decode(params)?;
                reply!(personal_sync::export(
                    &self.state_dir,
                    &metadata.binding.profile_id,
                    request,
                    &current,
                )?)
            }
            "profile.sync.preview" => {
                let request: PersonalSyncOpenRequest = crate::decode(params)?;
                reply!(personal_sync::preview(
                    &self.state_dir,
                    &metadata.binding.profile_id,
                    request,
                    &current,
                    metadata.commit_sequence,
                )?)
            }
            _ => Err(Error::Unsupported),
        }
    }
}
