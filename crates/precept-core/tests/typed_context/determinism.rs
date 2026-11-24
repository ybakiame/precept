use super::support::*;
use precept_core::typed_context::*;
use serde_json::{Value, json};

pub(super) fn normalization_and_input_permutations_produce_identical_serialized_output() {
    let mut i = input();
    i.query.path = Some("src/lib.rs".into());
    i.query.language = Some("rust".into());
    i.rules = vec![
        contract("b", "k", &["b", "a", "a"]),
        contract("a", "k", &["a", "b"]),
        preference(1, "k", "a", true),
        contract("z", "other", &["x"]),
        contract("conflict-a", "blocked", &["one"]),
        contract("conflict-b", "blocked", &["two"]),
        contract("conflict-c", "blocked-2", &["one"]),
        contract("conflict-d", "blocked-2", &["two"]),
    ];
    for r in &mut i.rules {
        r.selector.paths = Some(strings(&["src/main.rs", "src/lib.rs", "src/lib.rs"]));
        r.selector.languages = Some(strings(&["rust", "python"]));
        r.source.provenance.references = strings(&["z", "a", "a"]);
        r.source.provenance.historical_validation = vec![HistoricalValidation {
            reference: "claim".into(),
            origin: Origin::AgentReported,
            result: HistoricalResult::PASS,
            revision: Some("old-revision".into()),
        }];
    }
    i.policy_boundaries.push(PolicyBoundary {
        binding: binding("alpha"),
        key: "operation".into(),
        allowed_values: strings(&["write", "read", "read"]),
        source: source("policy"),
    });
    i.repository_facts.push(RepositoryFact {
        binding: binding("alpha"),
        key: "other".into(),
        value: "x".into(),
        source: source("fact"),
    });
    i.repository_facts.push(RepositoryFact {
        binding: binding("alpha"),
        key: "k".into(),
        value: "a".into(),
        source: source("fact-2"),
    });
    i.policy_boundaries.push(PolicyBoundary {
        binding: binding("alpha"),
        key: "another-policy".into(),
        allowed_values: strings(&["v"]),
        source: source("policy-2"),
    });
    let expected = serde_json::to_string(&resolve(&i).unwrap()).unwrap();
    for shift in 0..i.rules.len() {
        let mut p = i.clone();
        p.rules.rotate_left(shift);
        p.registered_projects.reverse();
        p.rules.reverse();
        p.rules.push(p.rules[0].clone());
        p.repository_facts.reverse();
        p.policy_boundaries.reverse();
        for r in &mut p.rules {
            r.allowed_values.reverse();
            r.source.provenance.references.reverse();
            r.selector.paths.as_mut().unwrap().reverse();
            r.selector.languages.as_mut().unwrap().reverse();
        }
        assert_eq!(
            serde_json::to_string(&resolve(&p).unwrap()).unwrap(),
            expected
        );
    }
    let out = resolve(&i).unwrap();
    assert_eq!(
        out.explanations
            .iter()
            .map(|r| r.rule.key.as_str())
            .collect::<Vec<_>>(),
        vec![
            "blocked",
            "blocked",
            "blocked-2",
            "blocked-2",
            "k",
            "k",
            "k",
            "other"
        ]
    );
    let claim = &out.explanations[0]
        .rule
        .source
        .provenance
        .historical_validation[0];
    assert_eq!(claim.origin, Origin::AgentReported);
    assert_eq!(claim.result, HistoricalResult::PASS);
    assert_eq!(claim.revision.as_deref(), Some("old-revision"));
    assert_eq!(
        serde_json::from_str::<EffectiveContextV1>(&expected).unwrap(),
        out
    );
}

pub(super) fn malformed_serialized_input_and_unknown_fields_are_not_silently_dropped() {
    let mut i = input();
    i.rules.push(contract("r", "k", &["v"]));
    let raw = serde_json::to_value(&i).unwrap();
    for field in ["selector", "source", "reference", "scope"] {
        let mut v = raw.clone();
        v["rules"][0][field]["unrecognized"] = json!(true);
        assert!(
            serde_json::from_value::<ResolutionInputV1>(v).is_err(),
            "{field}"
        );
    }
    for replacement in [Value::Null, json!([]), json!("src/lib.rs"), json!([1])] {
        let mut v = raw.clone();
        v["rules"][0]["selector"] = replacement;
        assert!(serde_json::from_value::<ResolutionInputV1>(v).is_err());
    }
    let mut v = raw.clone();
    v["rules"][0]["selector"]["paths"] = json!([1]);
    assert!(serde_json::from_value::<ResolutionInputV1>(v).is_err());
    let mut v = raw.clone();
    v["rules"][0]["kind"] = json!("unknown");
    assert!(serde_json::from_value::<ResolutionInputV1>(v).is_err());
    let mut v = raw.clone();
    v["rules"][0]["source"]["provenance"]["historical_validation"] =
        json!([{"reference":"claim","origin":"agent_reported","result":"BLOCKED","revision":null}]);
    assert!(serde_json::from_value::<ResolutionInputV1>(v).is_err());
    let mut v = raw;
    v["schema_version"] = json!(2);
    let i = serde_json::from_value(v).unwrap();
    assert_eq!(resolve(&i), Err(InputError::UnsupportedSchema));
}

pub(super) fn no_match_invalid_metadata_does_not_block_and_unmatched_observation_issues_stay_visible()
 {
    let mut i = input();
    let mut mismatch = contract("unrelated", "k", &[]);
    mismatch.selector.languages = Some(strings(&["rust"]));
    i.query.language = Some("python".into());
    i.rules = vec![contract("active", "k", &["v"]), mismatch];
    i.repository_facts.push(RepositoryFact {
        binding: binding("alpha"),
        key: "unmatched-fact".into(),
        value: "v".into(),
        source: Source {
            state: SourceState::Stale,
            ..source("stale-source")
        },
    });
    i.policy_boundaries.push(PolicyBoundary {
        binding: binding("alpha"),
        key: "unmatched-policy".into(),
        allowed_values: vec![],
        source: source("bad-policy"),
    });
    let out = resolve(&i).unwrap();
    assert!(effective(&out, "k").is_some());
    assert!(out.conflicts.iter().all(|c| c.key != "k"));
    assert!(out.conflicts.iter().any(|c| c.key == "unmatched-fact"
        && c.category == ConflictCategory::StaleSourceSnapshot
        && c.rules.is_empty()));
    assert!(out.conflicts.iter().any(|c| c.key == "unmatched-policy"
        && c.category == ConflictCategory::InvalidSourceSchema
        && c.rules.is_empty()));
    assert_eq!(row(&out, "unrelated").outcome, Outcome::Excluded);
}
