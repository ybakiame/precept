//! Project-bound, redacted repository-rule diagnostics for the Operator.

use crate::profiles::Profiles;
use precept_ipc::{Access, Error, Result};
use serde::Deserialize;
use serde_json::Value;

const MAX_REPORT_BYTES: usize = 768 * 1024;

fn bounded(value: Value) -> Result<Value> {
    if serde_json::to_vec(&value)
        .map_err(|_| Error::Unavailable)?
        .len()
        > MAX_REPORT_BYTES
    {
        return Err(Error::TooLarge);
    }
    Ok(value)
}

pub(super) fn report(access: Access, params: Value, profiles: &Profiles) -> Result<Value> {
    if access != Access::Operator {
        return Err(Error::Unauthorized);
    }
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Lint {
        project: String,
    }
    let p: Lint = serde_json::from_value(params).map_err(|_| Error::InvalidRequest)?;
    if !profiles.allowed(&p.project) {
        return Err(Error::NotFound);
    }
    let report = profiles.snapshot(&p.project)?.lint();
    bounded(serde_json::to_value(report).map_err(|_| Error::Unavailable)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn report_reply_is_bounded_before_ipc_framing() {
        let value = json!({"padding":"x".repeat(MAX_REPORT_BYTES)});
        assert_eq!(bounded(value), Err(Error::TooLarge));
        assert_eq!(bounded(json!({"ok":true})), Ok(json!({"ok":true})));
    }
}
