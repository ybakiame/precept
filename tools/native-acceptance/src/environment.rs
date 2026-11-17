//! Child-process environment isolation for acceptance launches.
//!
//! Product and UI behavior can be steered by `ACP_*` variables (for example
//! `ACP_UI_GALLERY` replaces the operator UI). A developer host must never leak
//! them into an acceptance child. Windows variable names are case-insensitive,
//! so the prefix test is too. Callers clear every inherited `ACP_*` variable,
//! then re-add exactly the test variables their surface needs.
use std::{
    ffi::{OsStr, OsString},
    process::Command,
};

/// True for an `ACP_*` control variable name, compared case-insensitively.
pub fn is_acp_control(key: &OsStr) -> bool {
    key.as_encoded_bytes()
        .get(..4)
        .is_some_and(|prefix| prefix.eq_ignore_ascii_case(b"ACP_"))
}

/// Removes from `command` every `ACP_*` variable present in `inherited`.
pub fn clear_acp(command: &mut Command, inherited: impl IntoIterator<Item = (OsString, OsString)>) {
    for (key, _) in inherited {
        if is_acp_control(&key) {
            command.env_remove(key);
        }
    }
}

/// Removes every `ACP_*` variable this process would pass to `command`.
pub fn clear_inherited_acp(command: &mut Command) -> &mut Command {
    clear_acp(command, std::env::vars_os());
    command
}

/// `inherited` without any `ACP_*` variable, for raw environment blocks.
pub fn without_acp(
    inherited: impl IntoIterator<Item = (OsString, OsString)>,
) -> Vec<(OsString, OsString)> {
    inherited
        .into_iter()
        .filter(|(key, _)| !is_acp_control(key))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn vars(names: &[&str]) -> Vec<(OsString, OsString)> {
        names.iter().map(|n| (n.into(), "1".into())).collect()
    }

    #[test]
    fn prefix_is_case_insensitive_and_exact() {
        for name in [
            "ACP_UI_GALLERY",
            "acp_ui_gallery",
            "Acp_Ui_Gallery",
            "aCp_x",
            "ACP_",
        ] {
            assert!(is_acp_control(OsStr::new(name)), "{name}");
        }
        for name in ["ACP", "ACPX_UI", "MY_ACP_UI", "", "PATH", "AC"] {
            assert!(!is_acp_control(OsStr::new(name)), "{name}");
        }
    }

    #[test]
    fn mixed_case_inherited_controls_are_removed_and_test_values_re_added() {
        let inherited = vars(&[
            "Acp_Ui_Gallery",
            "acp_ui_probe",
            "ACP_NATIVE_FOCUS_FILE",
            "PATH",
        ]);
        let mut command = Command::new("child");
        clear_acp(&mut command, inherited.clone());
        command.env("ACP_UI_PROBE", "1");
        let envs: Vec<_> = command
            .get_envs()
            .map(|(k, v)| (k.to_string_lossy().to_uppercase(), v.is_some()))
            .collect();
        assert!(envs.contains(&("ACP_UI_GALLERY".into(), false)));
        assert!(envs.contains(&("ACP_NATIVE_FOCUS_FILE".into(), false)));
        assert!(envs.contains(&("ACP_UI_PROBE".into(), true)), "{envs:?}");
        assert!(!envs.iter().any(|(k, _)| k == "PATH"));
        let block = without_acp(inherited);
        assert_eq!(block, vars(&["PATH"]));
    }
}
