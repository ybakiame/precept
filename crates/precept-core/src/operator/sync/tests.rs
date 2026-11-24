use super::*;

fn version(device: &str, counter: u64, value: &str) -> PersonalSyncVersion {
    let content = PersonalSyncContent {
        key: "tone".into(),
        value: value.into(),
        tags: vec!["style".into()],
        rule: RuleOptions::default(),
        enabled: true,
    };
    let digest = content.digest();
    PersonalSyncVersion {
        key: content.key.clone(),
        clock: BTreeMap::from([(device.to_owned(), counter)]),
        digest,
        content: Some(content),
    }
}

#[test]
fn vector_clocks_compare_join_and_tick_without_losing_a_device() {
    let a = BTreeMap::from([("a".repeat(32), 2), ("b".repeat(32), 1)]);
    let b = BTreeMap::from([("a".repeat(32), 1), ("c".repeat(32), 3)]);
    assert_eq!(clock_cmp(&a, &b), None);
    assert_eq!(
        clock_cmp(&clock_tick(&a, &"a".repeat(32)).unwrap(), &a),
        Some(std::cmp::Ordering::Greater)
    );
    let joined = clock_join(&a, &b).unwrap();
    assert_eq!(joined.len(), 3);
    assert_eq!(joined[&"c".repeat(32)], 3);
    let full: VectorClock = (0..MAX_SYNC_CLOCK_DEVICES)
        .map(|index| (format!("{index:032x}"), 1))
        .collect();
    let other: VectorClock = BTreeMap::from([("1".repeat(32), 1)]);
    assert!(clock_join(&full, &other).is_none());
    assert!(clock_tick(&full, &"1".repeat(32)).is_none());
}

#[test]
fn merge_refuses_equal_clock_drift_and_concurrent_edits() {
    let local = version(&"a".repeat(32), 2, "local");
    let equal_drift = version(&"a".repeat(32), 2, "remote");
    assert_eq!(
        classify_merge(&equal_drift, &local),
        PersonalSyncMerge::Conflict
    );
    let newer = version(&"a".repeat(32), 3, "remote");
    assert_eq!(classify_merge(&newer, &local), PersonalSyncMerge::Accept);
    assert_eq!(classify_merge(&local, &newer), PersonalSyncMerge::Stale);
    let mut concurrent = version(&"b".repeat(32), 1, "remote");
    concurrent.clock.insert("a".repeat(32), 1);
    assert_eq!(
        classify_merge(&concurrent, &local),
        PersonalSyncMerge::Conflict
    );
    assert_eq!(classify_merge(&local, &local), PersonalSyncMerge::Noop);
    assert_eq!(
        classify_merge(
            &version(&"b".repeat(32), 1, "new"),
            &PersonalSyncVersion::absent("tone")
        ),
        PersonalSyncMerge::Accept
    );
    let deletion = PersonalSyncVersion {
        key: "tone".into(),
        clock: BTreeMap::from([("b".repeat(32), 1)]),
        digest: tombstone_digest("tone"),
        content: None,
    };
    assert_eq!(
        classify_merge(&deletion, &PersonalSyncVersion::absent("tone")),
        PersonalSyncMerge::Accept
    );
    assert_eq!(
        classify_merge(&deletion, &deletion),
        PersonalSyncMerge::Noop
    );
    assert!(deletion.valid());
    let mut recreated = version(&"c".repeat(32), 2, "recreated");
    recreated.clock.insert("b".repeat(32), 1);
    assert_eq!(
        classify_merge(&recreated, &deletion),
        PersonalSyncMerge::Accept
    );
}

fn sync_bundle(versions: &[PersonalSyncVersion]) -> PersonalSyncBundle {
    let mut entries = Vec::new();
    let mut tombstones = Vec::new();
    for version in versions {
        match &version.content {
            Some(content) => entries.push(PersonalSyncEntry {
                content: content.clone(),
                clock: version.clock.clone(),
                digest: version.digest.clone(),
            }),
            None => tombstones.push(PersonalSyncTombstone {
                key: version.key.clone(),
                clock: version.clock.clone(),
                digest: version.digest.clone(),
            }),
        }
    }
    PersonalSyncBundle {
        format: PERSONAL_SYNC_FORMAT.into(),
        version: PERSONAL_SYNC_VERSION,
        bundle_id: "b".repeat(32),
        owner_id: "c".repeat(32),
        created_at_ms: 1,
        entries,
        tombstones,
    }
}

fn sync_selection(
    incoming: &PersonalSyncVersion,
    local: &PersonalSyncVersion,
    resolution: PersonalSyncResolution,
) -> PersonalSyncSelection {
    PersonalSyncSelection {
        key: incoming.key.clone(),
        incoming_digest: incoming.digest.clone(),
        expected_local_digest: local.digest.clone(),
        resolution,
    }
}

#[test]
fn apply_planning_accepts_newer_versions_and_records_each_selection() {
    let local = version(&"a".repeat(32), 1, "local");
    let incoming = version(&"a".repeat(32), 2, "remote");
    let request = PersonalSyncApplyRequest {
        bundle: sync_bundle(std::slice::from_ref(&incoming)),
        local_versions: vec![local.clone()],
        sender_device_id: "a".repeat(32),
        device_id: "d".repeat(32),
        selections: vec![sync_selection(
            &incoming,
            &local,
            PersonalSyncResolution::Accept,
        )],
    };
    let plan = plan_personal_sync_apply(&request).unwrap();
    assert_eq!(plan.changes.len(), 1);
    assert_eq!(plan.versions[0].clock[&"a".repeat(32)], 2);
    assert_eq!(plan.versions[0].clock[&"d".repeat(32)], 1);
    assert_eq!(plan.applied[0].digest, incoming.digest);
    assert_eq!(plan.bundle_digest, request.bundle.digest());
}

#[test]
fn conflict_resolutions_dominate_both_clocks_and_keep_local_requires_no_rewrite() {
    let local = version(&"a".repeat(32), 1, "local");
    let incoming = version(&"b".repeat(32), 1, "remote");
    let mut request = PersonalSyncApplyRequest {
        bundle: sync_bundle(std::slice::from_ref(&incoming)),
        local_versions: vec![local.clone()],
        sender_device_id: "a".repeat(32),
        device_id: "d".repeat(32),
        selections: vec![sync_selection(
            &incoming,
            &local,
            PersonalSyncResolution::KeepLocal,
        )],
    };
    let kept = plan_personal_sync_apply(&request).unwrap();
    assert_eq!(kept.versions[0].digest, local.digest);
    assert_eq!(kept.versions[0].clock[&"d".repeat(32)], 1);
    assert!(kept.changes.is_empty());
    request.selections[0].resolution = PersonalSyncResolution::Manual(PersonalSyncContent {
        key: "tone".into(),
        value: "merged".into(),
        tags: vec!["style".into()],
        rule: RuleOptions::default(),
        enabled: true,
    });
    let merged = plan_personal_sync_apply(&request).unwrap();
    assert_eq!(merged.versions[0].content.as_ref().unwrap().value, "merged");
    assert_eq!(merged.versions[0].clock.len(), 3);
}

#[test]
fn concurrent_identical_content_joins_clocks_without_rewriting_the_profile() {
    let local = version(&"a".repeat(32), 1, "same");
    let incoming = version(&"b".repeat(32), 1, "same");
    let request = PersonalSyncApplyRequest {
        bundle: sync_bundle(std::slice::from_ref(&incoming)),
        local_versions: vec![local],
        sender_device_id: "a".repeat(32),
        device_id: "d".repeat(32),
        selections: vec![sync_selection(
            &incoming,
            &version(&"a".repeat(32), 1, "same"),
            PersonalSyncResolution::KeepLocal,
        )],
    };
    let plan = plan_personal_sync_apply(&request).unwrap();
    assert!(plan.changes.is_empty());
    assert_eq!(plan.versions[0].clock.len(), 3);
}

#[test]
fn apply_planning_refuses_stale_digests_duplicate_keys_and_unknown_items() {
    let local = version(&"a".repeat(32), 1, "local");
    let incoming = version(&"a".repeat(32), 2, "remote");
    let mut request = PersonalSyncApplyRequest {
        bundle: sync_bundle(std::slice::from_ref(&incoming)),
        local_versions: vec![local.clone()],
        sender_device_id: "a".repeat(32),
        device_id: "d".repeat(32),
        selections: vec![sync_selection(
            &incoming,
            &local,
            PersonalSyncResolution::Accept,
        )],
    };
    request.selections[0].expected_local_digest = "0".repeat(64);
    assert!(plan_personal_sync_apply(&request).is_none());
    request.selections[0].expected_local_digest = local.digest;
    request.selections.push(request.selections[0].clone());
    assert!(plan_personal_sync_apply(&request).is_none());
    request.selections.pop();
    request.selections[0].key = "unknown".into();
    assert!(plan_personal_sync_apply(&request).is_none());
}

#[test]
fn malformed_clocks_digests_and_duplicate_keys_are_refused() {
    let mut entry = version(&"a".repeat(32), 1, "value");
    let clock = entry.clock.clone();
    let digest = entry.digest.clone();
    let content = entry.content.clone().unwrap();
    let bundle = PersonalSyncBundle {
        format: PERSONAL_SYNC_FORMAT.into(),
        version: PERSONAL_SYNC_VERSION,
        bundle_id: "b".repeat(32),
        owner_id: "c".repeat(32),
        created_at_ms: 1,
        entries: vec![PersonalSyncEntry {
            content: content.clone(),
            clock: clock.clone(),
            digest: digest.clone(),
        }],
        tombstones: vec![],
    };
    assert!(bundle.valid());
    let mut bad_clock = bundle.clone();
    bad_clock.entries[0].clock.insert("d".repeat(32), 0);
    assert!(!bad_clock.valid());
    let mut bad_digest = bundle.clone();
    bad_digest.entries[0].digest = "0".repeat(64);
    assert!(!bad_digest.valid());
    let mut duplicate = bundle;
    duplicate.entries.push(PersonalSyncEntry {
        content,
        clock,
        digest,
    });
    assert!(!duplicate.valid());
    assert!(entry.valid());
    entry.digest = "0".repeat(64);
    assert!(!entry.valid());
}
