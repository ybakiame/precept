use super::*;
use crate::state::StateDir;
use precept_ipc::Error;

fn state() -> (tempfile::TempDir, StateDir) {
    let temp = tempfile::tempdir().unwrap();
    let s = StateDir::open(&temp.path().join("state")).unwrap();
    (temp, s)
}

#[test]
fn same_body_reaffirms_changed_body_corrects_and_keeps_history() {
    let (_t, s) = state();
    let m = Memory::new();
    let (e, what) = m
        .remember(
            &s,
            "alpha",
            Kind::Lesson,
            "Run tests",
            "cargo test",
            Some("a".into()),
            None,
        )
        .unwrap();
    assert_eq!(what, Remembered::Added);
    let (again, what) = m
        .remember(
            &s,
            "alpha",
            Kind::Lesson,
            "run  TESTS",
            "cargo test",
            Some("b".into()),
            None,
        )
        .unwrap();
    assert_eq!(what, Remembered::Reaffirmed);
    assert_eq!(again.id, e.id);
    assert_eq!(again.confirmations, 1);
    m.edit(&s, "alpha", &e.id, None, None, Some(true)).unwrap();
    let (fixed, what) = m
        .remember(
            &s,
            "alpha",
            Kind::Lesson,
            "Run tests",
            "cargo nextest run",
            Some("c".into()),
            None,
        )
        .unwrap();
    assert_eq!(what, Remembered::Corrected);
    assert_eq!(
        fixed.confirmations, 0,
        "confirmations were for the old text"
    );
    assert!(fixed.pinned, "the pin stays");
    assert_eq!(fixed.history[0].body, "cargo test");
    assert_eq!(fixed.history[0].by.as_deref(), Some("a"));
    // The same title in another kind is another entry.
    let (other, what) = m
        .remember(&s, "alpha", Kind::Caution, "Run tests", "x", None, None)
        .unwrap();
    assert_eq!(what, Remembered::Added);
    assert_ne!(other.id, e.id);
    let restored = m.restore(&s, "alpha", &e.id, 0).unwrap();
    assert_eq!(restored.body, "cargo test");
    assert!(restored.edited_by_operator);
}

#[test]
fn weight_follows_kind_pin_confirmations_and_fades() {
    let now = 1_000 * DAY_MS;
    let entry = |kind, confirmed_days_ago: u64| Entry {
        id: "0".repeat(16),
        kind,
        title: "t".into(),
        body: "b".into(),
        pinned: false,
        confirmations: 0,
        recalls: 0,
        created_ms: 0,
        updated_ms: 0,
        confirmed_ms: now - confirmed_days_ago * DAY_MS,
        edited_by_operator: false,
        by: None,
        session: None,
        history: vec![],
        changes: vec![],
        proposed: None,
        basis: None,
    };
    let env = entry(Kind::Environment, 0);
    let lesson = entry(Kind::Lesson, 0);
    assert!(env.weight(now) > lesson.weight(now));
    let mut pinned = entry(Kind::Attempt, 0);
    pinned.pinned = true;
    assert!(pinned.weight(now) > env.weight(now));
    let old = entry(Kind::Lesson, 100);
    assert!(old.fading(now));
    assert_eq!(old.weight(now), 40.0 - 20.0);
    let mut busy = entry(Kind::Lesson, 0);
    busy.changes = vec![now - DAY_MS, now - 2 * DAY_MS, now - 3 * DAY_MS];
    assert!(busy.unstable(now));
    assert_eq!(
        busy.weight(now),
        lesson.weight(now),
        "never boosted for changing"
    );
}

#[test]
fn recall_ranks_counts_and_the_context_keeps_environment_first() {
    let (_t, s) = state();
    let m = Memory::new();
    m.remember(
        &s,
        "alpha",
        Kind::Environment,
        "Node",
        "Node 22 via mise",
        None,
        None,
    )
    .unwrap();
    m.remember(
        &s,
        "alpha",
        Kind::Attempt,
        "Tried pnpm",
        "pnpm broke the lockfile",
        None,
        None,
    )
    .unwrap();
    m.remember(
        &s,
        "alpha",
        Kind::Caution,
        "WSL 路径",
        "在 WSL 中使用 /mnt/c 很慢",
        None,
        None,
    )
    .unwrap();
    let hits = m.recall(&s, "alpha", Some("wsl"), None, 5).unwrap();
    assert_eq!(hits.len(), 1);
    assert_eq!(
        m.recall(&s, "alpha", Some("很慢"), None, 5).unwrap().len(),
        1
    );
    // Words in any order; a phrase finds notes using its parts; the
    // better match first.
    let hits = m
        .recall(&s, "alpha", Some("lockfile pnpm"), None, 5)
        .unwrap();
    assert_eq!(hits[0].title, "Tried pnpm");
    assert_eq!(
        m.recall(&s, "alpha", Some("构建很慢吗"), None, 5)
            .unwrap()
            .len(),
        1
    );
    assert!(
        m.recall(&s, "alpha", Some("nothing-here"), None, 5)
            .unwrap()
            .is_empty()
    );
    let mut want = vec!["wsl", "构建很慢", "构建", "建很", "很慢"];
    want.sort();
    assert_eq!(terms("WSL 构建很慢"), want);
    assert_eq!(m.list(&s, "alpha").unwrap()[0].kind, Kind::Environment);
    let ctx = m.for_context(&s, "alpha");
    assert_eq!(ctx["counts"]["environment"], 1);
    assert_eq!(ctx["environment"][0]["title"], "Node");
    assert_eq!(ctx["relevant"][0]["kind"], "caution");
    assert_eq!(
        m.recall(&s, "alpha", None, Some(Kind::Attempt), 5).unwrap()[0].recalls,
        2,
        "the pnpm query above counted one"
    );
}

#[test]
fn get_reads_one_note_and_reaffirm_keeps_a_basis_until_correction() {
    let (_t, s) = state();
    let m = Memory::new();
    let (e, _) = m
        .remember(
            &s,
            "alpha",
            Kind::Lesson,
            "Run tests",
            "cargo test",
            None,
            None,
        )
        .unwrap();
    assert_eq!(m.get(&s, "alpha", &e.id).unwrap().body, "cargo test");
    assert_eq!(m.get(&s, "alpha", "zz").unwrap_err(), Error::InvalidRequest);
    assert_eq!(
        m.get(&s, "alpha", &"0".repeat(16)).unwrap_err(),
        Error::NotFound
    );
    let affirmed = m
        .reaffirm(&s, "alpha", &e.id, Some("ran it on this machine"))
        .unwrap();
    assert_eq!(affirmed.confirmations, 1);
    assert_eq!(affirmed.basis.as_deref(), Some("ran it on this machine"));
    let again = m.reaffirm(&s, "alpha", &e.id, None).unwrap();
    assert_eq!(again.confirmations, 2);
    assert_eq!(again.basis.as_deref(), Some("ran it on this machine"));
    assert_eq!(
        m.reaffirm(&s, "alpha", &e.id, Some("bad\u{7}"))
            .unwrap_err(),
        Error::InvalidRequest
    );
    let (fixed, what) = m
        .remember(
            &s,
            "alpha",
            Kind::Lesson,
            "Run tests",
            "cargo nextest run",
            None,
            None,
        )
        .unwrap();
    assert_eq!(what, Remembered::Corrected);
    assert!(fixed.basis.is_none(), "the basis was for the old text");
}

#[test]
fn reads_carry_a_verify_notice_after_a_week_without_confirmation() {
    let now = 1_000 * DAY_MS;
    let mut e = Entry {
        id: "0".repeat(16),
        kind: Kind::Lesson,
        title: "t".into(),
        body: "b".into(),
        pinned: false,
        confirmations: 0,
        recalls: 0,
        created_ms: 0,
        updated_ms: 0,
        confirmed_ms: now - 8 * DAY_MS,
        edited_by_operator: false,
        by: None,
        session: None,
        history: vec![],
        changes: vec![],
        proposed: None,
        basis: None,
    };
    let v = e.view(now);
    assert_eq!(v["unverified_days"], 8);
    assert!(
        v["notice"]
            .as_str()
            .unwrap()
            .contains("Not verified for 8 days")
    );
    e.confirmed_ms = now;
    let v = e.view(now);
    assert_eq!(v["unverified_days"], 0);
    assert!(v.get("notice").is_none());
}

#[test]
fn malformed_input_and_unknown_ids_are_refused() {
    let (_t, s) = state();
    let m = Memory::new();
    assert_eq!(
        m.remember(&s, "a", Kind::Lesson, " ", "b", None, None)
            .unwrap_err(),
        Error::InvalidRequest
    );
    assert_eq!(
        m.remember(&s, "a", Kind::Lesson, "t", "bad\u{7}", None, None)
            .unwrap_err(),
        Error::InvalidRequest
    );
    assert_eq!(m.forget(&s, "a", "zz").unwrap_err(), Error::InvalidRequest);
    assert_eq!(
        m.forget(&s, "a", &"0".repeat(16)).unwrap_err(),
        Error::NotFound
    );
    let (e, _) = m
        .remember(&s, "a", Kind::Lesson, "x", "1", None, None)
        .unwrap();
    m.remember(&s, "a", Kind::Lesson, "y", "2", None, None)
        .unwrap();
    assert_eq!(
        m.edit(&s, "a", &e.id, Some("Y"), None, None).unwrap_err(),
        Error::Conflict
    );
    assert_eq!(m.clear(&s, "a").unwrap(), 2);
}
