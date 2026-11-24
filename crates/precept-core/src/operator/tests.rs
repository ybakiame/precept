use super::*;
#[test]
fn scope_refuses_extra_fields_including_unit_variant_null() {
    for raw in [
        r#"{"kind":"global","name":"alpha"}"#,
        r#"{"kind":"global","name":null}"#,
        r#"{"kind":"global","extra":false}"#,
        r#"{"kind":"project","name":"alpha","extra":true}"#,
    ] {
        assert!(serde_json::from_str::<Scope>(raw).is_err());
    }
    for scope in [
        Scope::Global,
        Scope::Project {
            name: "alpha".into(),
        },
    ] {
        let raw = serde_json::to_string(&scope).unwrap();
        assert!(serde_json::from_str::<Scope>(&raw).unwrap() == scope);
    }
}

#[test]
fn rule_options_are_canonical_bounded_and_digested() {
    let raw = AppliesTo {
        agents: vec!["Claude-Code".into(), "claude-code".into()],
        languages: vec![" Rust ".into()],
        paths: vec!["apps/ui/".into(), "Cargo.toml".into()],
        task_tags: vec![],
    };
    let c = raw.canonical().unwrap();
    assert_eq!(c.agents, ["claude-code"]);
    assert_eq!(c.languages, ["rust"]);
    assert_eq!(c.paths, ["Cargo.toml", "apps/ui/"]);
    assert!(c.is_canonical() && !raw.is_canonical());
    assert_eq!(
        c.json(),
        r#"{"agents":["claude-code"],"languages":["rust"],"paths":["Cargo.toml","apps/ui/"],"task_tags":[]}"#
    );
    for bad in ["../x", "/abs", "a//b", "c:\\x", "src/*.rs", ""] {
        let a = AppliesTo {
            paths: vec![bad.into()],
            ..Default::default()
        };
        assert!(a.canonical().is_none(), "{bad}");
    }
    let many = AppliesTo {
        agents: (0..17).map(|i| format!("a{i}")).collect(),
        ..Default::default()
    };
    assert!(many.canonical().is_none());
    let hard = RuleOptions {
        enforcement: Enforcement::Hard,
        ..Default::default()
    };
    assert!(hard.valid());
    assert!(
        !RuleOptions {
            priority: 101,
            ..Default::default()
        }
        .valid()
    );
    let tags = vec!["style".to_owned()];
    let base = proposal_digest(
        &Scope::Global,
        None,
        "k",
        "v",
        Origin::Operator,
        &tags,
        &RuleOptions::default(),
    );
    let scoped = RuleOptions {
        applies_to: c,
        ..Default::default()
    };
    for rule in [
        scoped,
        RuleOptions {
            priority: 5,
            ..Default::default()
        },
        hard,
    ] {
        assert_ne!(
            base,
            proposal_digest(
                &Scope::Global,
                None,
                "k",
                "v",
                Origin::Operator,
                &tags,
                &rule
            )
        );
    }
    assert_ne!(
        base,
        proposal_digest(
            &Scope::Project {
                name: "alpha".into()
            },
            Some("alpha"),
            "k",
            "v",
            Origin::Operator,
            &tags,
            &RuleOptions::default()
        )
    );
}

#[test]
fn canonical_digest_is_deterministic_and_field_sensitive() {
    let tags = vec!["style".to_owned()];
    let d1 = proposal_digest(
        &Scope::Global,
        None,
        "editor",
        "vim",
        Origin::Agent,
        &tags,
        &RuleOptions::default(),
    );
    let d2 = proposal_digest(
        &Scope::Global,
        None,
        "editor",
        "vim",
        Origin::Agent,
        &tags,
        &RuleOptions::default(),
    );
    assert_eq!(d1, d2);
    let d3 = proposal_digest(
        &Scope::Global,
        None,
        "editor",
        "emacs",
        Origin::Agent,
        &tags,
        &RuleOptions::default(),
    );
    assert_ne!(d1, d3);
    let d4 = proposal_digest(
        &Scope::Global,
        None,
        "editor",
        "vim",
        Origin::Operator,
        &tags,
        &RuleOptions::default(),
    );
    assert_ne!(d1, d4);
    let d5 = proposal_digest(
        &Scope::Project {
            name: "alpha".into(),
        },
        Some("alpha"),
        "editor",
        "vim",
        Origin::Agent,
        &tags,
        &RuleOptions::default(),
    );
    assert_ne!(d1, d5);
    let d6 = proposal_digest(
        &Scope::Global,
        None,
        "editor",
        "vim",
        Origin::Agent,
        &["git".to_owned()],
        &RuleOptions::default(),
    );
    assert_ne!(d1, d6);
}

#[test]
fn digest_hex_roundtrip() {
    let d = proposal_digest(
        &Scope::Global,
        None,
        "k",
        "v",
        Origin::Operator,
        &["t".into()],
        &RuleOptions::default(),
    );
    let hex = digest_hex(&d);
    assert_eq!(hex.len(), 64);
    assert_eq!(digest_from_hex(&hex), Some(d));
    assert_eq!(digest_from_hex(&hex.to_uppercase()), None);
    assert_eq!(digest_from_hex("short"), None);
    assert_eq!(digest_from_hex(&"g".repeat(64)), None);
}

#[test]
fn channel_and_decision_action_closed_sets() {
    assert!(serde_json::from_str::<Channel>("\"cli\"").is_ok());
    assert!(serde_json::from_str::<Channel>("\"desktop\"").is_ok());
    assert!(serde_json::from_str::<Channel>("\"tui\"").is_err());
    assert!(serde_json::from_str::<Channel>("\"web\"").is_err());
    assert!(serde_json::from_str::<DecisionAction>("\"confirm\"").is_ok());
    assert!(serde_json::from_str::<DecisionAction>("\"demote\"").is_ok());
    assert!(serde_json::from_str::<DecisionAction>("\"approve\"").is_err());
}

#[test]
fn origin_is_a_closed_pair_defaulting_to_operator() {
    assert_eq!(Origin::default(), Origin::Operator);
    for o in Origin::ALL {
        assert_eq!(Origin::parse(o.as_str()), Some(o));
        let wire = serde_json::to_string(&o).unwrap();
        assert_eq!(wire, format!("\"{}\"", o.as_str()));
    }
    assert!(serde_json::from_str::<Origin>("\"agent_reported\"").is_err());
    assert_eq!(Origin::parse("legacy_unknown"), None);
}

#[test]
fn tags_normalize_dedupe_sort_and_bound() {
    assert_eq!(
        normalize_tag(" #Code Style ").as_deref(),
        Some("code-style")
    );
    assert_eq!(
        normalize_tag("snake_case__x").as_deref(),
        Some("snake-case-x")
    );
    assert_eq!(normalize_tag("测试").as_deref(), Some("测试"));
    assert_eq!(normalize_tag("--"), None);
    assert_eq!(normalize_tag("a/b"), None);
    assert_eq!(normalize_tag(&"x".repeat(33)), None);
    assert_eq!(
        canonical_tags(&["Git", "style", "git"]),
        Some(vec!["git".to_owned(), "style".to_owned()])
    );
    assert_eq!(canonical_tags::<&str>(&[]), None);
    assert!(canonical_tags(&["a", "b", "c", "d", "e", "f", "g", "h", "i", "j"]).is_some());
    assert_eq!(
        canonical_tags(&["a", "b", "c", "d", "e", "f", "g", "h", "i", "j", "k"]),
        None
    );
    assert!(tags_canonical(&["a".into(), "b".into()]));
    assert!(!tags_canonical(&["b".into(), "a".into()]));
    assert!(!tags_canonical(&["A".into()]));
    assert_eq!(tags_json(&["a".into(), "b".into()]), r#"["a","b"]"#);
}

#[test]
fn review_dtos_refuse_unknown_fields() {
    let raw = r#"{"preference_id":1,"action":"confirm","from_revision":1,"to_revision":2,"proposal_digest":"aa","receipt_commit_sequence":1,"channel":"cli","decided_at":"t","extra":true}"#;
    assert!(serde_json::from_str::<ReviewDecisionRecord>(raw).is_err());
}
