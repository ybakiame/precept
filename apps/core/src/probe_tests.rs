use super::*;
use connection::{DISCOVERY_VERSION, DaemonStatus, Discovery, RELEASE, Target};
fn report() -> Discovery {
    Discovery {
        version: DISCOVERY_VERSION,
        user: "operator".into(),
        helper: "/usr/bin/precept-bridge".into(),
        runtime: "/home/operator/.local/state/precept/runtime".into(),
        endpoint: "/home/operator/.local/state/precept/runtime/control.sock".into(),
        systemd: true,
        unit_installed: true,
        release: RELEASE.into(),
        daemon: DaemonStatus::Ready {
            profile_selected: false,
        },
    }
}
#[test]
fn stale_or_superseded_checks_never_overwrite_the_draft() {
    let mut m = Model::loading();
    m.select_target(Target::Wsl, Some("archlinux"));
    let first = m.begin_probe(false);
    // Another distribution is chosen before the first result arrives.
    m.select_target(Target::Wsl, Some("Ubuntu-24.04"));
    let second = m.begin_probe(false);
    m.finish_probe(first, Ok(report()));
    assert_eq!(m.probe, Probe::Checking);
    assert!(m.connection_draft.wsl_runtime.is_empty());
    m.finish_probe(second, Ok(report()));
    assert!(matches!(m.probe, Probe::Found(_)));
    assert_eq!(m.connection_draft.distribution, "Ubuntu-24.04");
    assert!(m.connection_draft.validate());
    // A manual edit during a check wins over its late result.
    let third = m.begin_probe(false);
    m.edit_draft(|d| d.wsl_runtime = "/srv/precept/runtime".into());
    m.finish_probe(third, Ok(report()));
    assert_eq!(m.connection_draft.wsl_runtime, "/srv/precept/runtime");
    assert_eq!(m.probe, Probe::Idle);
    // An invalid report fails closed and changes nothing.
    let fourth = m.begin_probe(false);
    m.finish_probe(
        fourth,
        Ok(Discovery {
            user: "-x".into(),
            ..report()
        }),
    );
    assert_eq!(
        m.probe,
        Probe::Failed(connection::discovery_status::INVALID_REPORT)
    );
    assert_eq!(m.connection_draft.wsl_runtime, "/srv/precept/runtime");
    // Applying never happens implicitly.
    assert_eq!(m.connection.target, Target::Synthetic);
}
