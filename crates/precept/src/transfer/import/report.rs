use super::preflight::{Observation, memory_plan, preference_plan, project_name_valid};
use crate::transfer::format::Export;
use precept_ipc::operator::Scope;
use serde_json::{Value, json};

pub(super) fn build_report(export: &Export, observation: &Observation) -> Value {
    let preferences = preference_plan(export, &observation.projects, &observation.existing);
    let omitted_environment: Vec<_> = preferences
        .iter()
        .filter(|plan| plan["skip_reason"] == json!("environment"))
        .filter_map(|plan| plan["file_index"].as_u64())
        .collect();
    let conflicts: Vec<_> = preferences
        .iter()
        .filter(|plan| plan["action"] == json!("skip"))
        .map(|plan| json!({"file_index":plan["file_index"],"reason":plan["skip_reason"]}))
        .collect();
    let mut unserved = std::collections::BTreeSet::new();
    for p in &export.preferences {
        if let Scope::Project { name } = &p.scope
            && project_name_valid(name)
            && !observation.projects.contains(name)
        {
            unserved.insert(name.as_str());
        }
    }
    json!({
        "source":{"format":export.format,"version":export.version},
        "destination":{
            "selected":true,
            "write_enabled":observation.write_enabled,
            "schema":observation.schema,
            "binding":{
                "profile_id":observation.binding.profile_id,
                "generation":observation.binding.generation,
                "owner_epoch":observation.binding.owner_epoch
            }
        },
        "consistency":{
            "watermark":observation.commit_sequence,
            "strategy":"binding_and_profile_commit_sequence",
            "preflight_rechecked_before_apply":false,
            "limitation":"Preflight is not a transaction; each reviewed write rechecks the binding and exact absence. Operation-id receipts are not readable by preflight. Apply-time authorization, capacity and storage refusals remain authoritative and are reported per file_index."
        },
        "preferences":preferences,
        "conflicts":conflicts,
        "unserved_projects":unserved,
        "omitted":{
            "environment_scope":{"file_indexes":omitted_environment},
            "handoffs":observation.handoffs
        },
        "memory":memory_plan(export, observation)
    })
}
