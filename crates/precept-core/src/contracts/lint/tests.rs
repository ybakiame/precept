use super::super::parser::{DIR, MAX_BYTES, parse, valid_id};
use super::report::{MAX_DIAGNOSTICS, display_file, report};
use super::*;
use serde_json::json;
use std::fs;

const VALID: &str = "+++\nkey = \"formatter\"\ntitle = \"Rust formatting\"\nallowed = [\"rustfmt\"]\nenforcement = \"verify\"\ntags = [\"Rust\"]\n[applies_to]\nlanguages = [\"Rust\"]\n[[check]]\nfile = \"rustfmt.toml\"\n+++\nRun `cargo fmt`.\n";

fn snapshot(raw: &[(&str, &str)]) -> (tempfile::TempDir, super::super::Snapshot) {
    let temp = tempfile::tempdir().unwrap();
    let dir = temp.path().join(DIR);
    fs::create_dir_all(&dir).unwrap();
    for (id, text) in raw {
        fs::write(dir.join(format!("{id}.md")), text).unwrap();
    }
    let snapshot = super::super::Snapshot::read(temp.path())
        .with_reviewed(Some(raw.iter().map(|(id, _)| (*id).to_owned()).collect()));
    (temp, snapshot)
}

#[test]
fn valid_rule_has_the_exact_serialized_shape() {
    assert!(lint("formatter", VALID.as_bytes()).is_empty());
    let (_temp, snapshot) = snapshot(&[("formatter", VALID)]);
    let report = snapshot.lint();
    assert_eq!(report.status, LintStatus::Ok);
    assert_eq!(report.summary.files, 1);
    assert_eq!(
        serde_json::to_value(report).unwrap(),
        json!({
            "format": "precept-rules-lint",
            "version": 1,
            "status": "ok",
            "summary": {"files": 1, "errors": 0, "warnings": 0},
            "files": [{
                "file": ".precept/rules/formatter.md",
                "status": "ok",
                "diagnostics": []
            }]
        })
    );
}

#[test]
fn toml_and_semantic_errors_have_stable_positions() {
    let raw = concat!(
        "+++\n",
        "key = \"x\"\n",
        "title = \"   \"\n",
        "allowed = [\"\"]\n",
        "tags = [\"bad/tag\"]\n",
        "priority = 101\n",
        "[applies_to]\n",
        "paths = [\"../private\"]\n",
        "[[check]]\n",
        "file = \"/private/path\"\n",
        "[[guard]]\n",
        "tool = \"shell\"\n",
        "+++\n",
        "body\n"
    );
    let diagnostics = lint("x", raw.as_bytes());
    let line = |code| {
        diagnostics
            .iter()
            .find(|diagnostic| diagnostic.code == code)
            .unwrap()
            .line
    };
    assert_eq!(line("title.invalid"), 3);
    assert_eq!(line("allowed.invalid"), 4);
    assert_eq!(line("tags.invalid"), 5);
    assert_eq!(line("priority.invalid"), 6);
    assert_eq!(line("applies_to.invalid"), 8);
    assert_eq!(line("check.invalid"), 10);
    assert_eq!(line("guard.invalid"), 11);
    assert_eq!(line("guard.requires_hard"), 11);
    assert!(diagnostics.iter().all(|diagnostic| {
        diagnostic.line >= 1
            && diagnostic.column >= 1
            && (diagnostic.end_line, diagnostic.end_column) >= (diagnostic.line, diagnostic.column)
    }));

    let malformed = "+++\nkey = \"x\"\npriority = \n+++\nbody\n";
    let diagnostics = lint("x", malformed.as_bytes());
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].code, "frontmatter.invalid");
    assert_eq!(diagnostics[0].line, 3);
    assert!(diagnostics[0].column >= 1);
    assert!(!diagnostics[0].message.contains("TOML"));
}

#[test]
fn nth_check_and_guard_tables_locate_the_offending_field() {
    let raw = concat!(
        "+++\n",
        "key = \"x\"\n",
        "enforcement = \"hard\"\n",
        "[[check]]\n",
        "file = \"ok\"\n",
        "[[check]]\n",
        "file = \"../private\"\n",
        "[[guard]]\n",
        "tool = \"write\"\n",
        "paths = [\"ok\"]\n",
        "[[guard]]\n",
        "tool = \"write\"\n",
        "paths = [\"../private\"]\n",
        "+++\n",
        "body\n"
    );
    let diagnostics = lint("x", raw.as_bytes());
    let check = diagnostics
        .iter()
        .find(|diagnostic| diagnostic.code == "check.invalid")
        .unwrap();
    assert_eq!((check.line, check.column), (7, 1));
    assert_eq!((check.end_line, check.end_column), (7, 5));
    let guard = diagnostics
        .iter()
        .find(|diagnostic| diagnostic.code == "guard.invalid")
        .unwrap();
    assert_eq!((guard.line, guard.column), (13, 1));
    assert_eq!((guard.end_line, guard.end_column), (13, 6));
}

#[test]
fn toml_error_spans_use_original_lines_and_unicode_columns() {
    let raw = "\u{feff}+++\r\nkey = \"x\"\r\ntitle = \"é\" extra\r\n+++\r\nbody\r\n";
    let diagnostics = lint("x", raw.as_bytes());
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].code, "frontmatter.invalid");
    assert_eq!(diagnostics[0].line, 3);
    assert_eq!(diagnostics[0].column, 13);
    assert_eq!(diagnostics[0].end_line, diagnostics[0].line);
    assert!(diagnostics[0].end_column >= diagnostics[0].column);
    assert!(!diagnostics[0].message.contains("extra"));
}

#[test]
fn multiline_strings_do_not_hide_later_field_locations() {
    let raw = "+++\nkey = \"x\"\ntitle = \"\"\"é\"\"\"\"\npriority = 101\n+++\nbody\n";
    let diagnostics = lint("x", raw.as_bytes());
    assert_eq!(diagnostics.len(), 1, "{diagnostics:#?}");
    assert_eq!(diagnostics[0].code, "priority.invalid");
    assert_eq!((diagnostics[0].line, diagnostics[0].column), (4, 1));
}

#[test]
fn inline_tables_fall_back_to_their_containing_field() {
    let raw = "+++\nkey = \"x\"\ncheck = [{ file = \"../private\" }]\napplies_to = { paths = [\"../private\"] }\n+++\nbody\n";
    let diagnostics = lint("x", raw.as_bytes());
    let check = diagnostics
        .iter()
        .find(|diagnostic| diagnostic.code == "check.invalid")
        .unwrap();
    assert_eq!((check.line, check.column), (3, 1));
    let applies = diagnostics
        .iter()
        .find(|diagnostic| diagnostic.code == "applies_to.invalid")
        .unwrap();
    assert_eq!((applies.line, applies.column), (4, 1));
}

#[test]
fn bom_crlf_and_unicode_columns_are_counted_without_bom_width() {
    let raw = "\u{feff}+++\r\nkey = \"x\"\r\n+++\r\né\u{1}\r\n";
    let diagnostics = lint("x", raw.as_bytes());
    assert_eq!(diagnostics.len(), 1, "{diagnostics:#?}");
    assert_eq!(diagnostics[0].code, "body.control_characters");
    assert_eq!(
        (
            diagnostics[0].line,
            diagnostics[0].column,
            diagnostics[0].end_line,
            diagnostics[0].end_column
        ),
        (4, 2, 4, 3)
    );
    assert!(parse("x", raw.as_bytes()).is_err());
}

#[test]
fn diagnostics_never_expose_values_or_absolute_paths() {
    let secret = "TOPSECRET-value";
    let raw = format!(
        "+++\nkey = \"{secret}\"\ntitle = \"{secret}\"\nallowed = [\"{secret}\"]\n[applies_to]\npaths = [\"C:\\\\Users\\\\private\"]\n[[check]]\nfile = \"/home/private/{secret}\"\n+++\n{secret}\n"
    );
    let diagnostics = lint("x", raw.as_bytes());
    assert!(!diagnostics.is_empty());
    let encoded = serde_json::to_string(&diagnostics).unwrap();
    assert!(!encoded.contains("related_file"));
    for private in [secret, "C:", "Users", "/home/private", "rule key"] {
        assert!(!encoded.contains(private), "leaked {private}");
    }
    let (_temp, snapshot) = snapshot(&[("x", &raw)]);
    let encoded = serde_json::to_string(&snapshot.lint()).unwrap();
    assert!(!encoded.contains("TOPSECRET"));
    assert!(!encoded.contains("Users"));
    assert!(!encoded.contains("/home/private"));
}

#[test]
fn duplicate_and_priority_warnings_reference_the_first_file() {
    let first = "+++\nkey = \"shared\"\npriority = 5\n[applies_to]\nlanguages = [\"Rust\"]\npaths = [\"src/\", \"README.md\"]\n+++\nfirst\n";
    let second = "+++\nkey = \"shared\"\npriority = 5\n[applies_to]\nlanguages = [\"rust\"]\npaths = [\"README.md\", \"src/\"]\n+++\nsecond\n";
    let third = "+++\nkey = \"shared\"\npriority = 6\n[applies_to]\nlanguages = [\"rust\"]\npaths = [\"src/\", \"README.md\"]\n+++\nthird\n";
    let (temp, snapshot) = snapshot(&[("c", third), ("a", first), ("b", second)]);
    fs::create_dir(temp.path().join("src")).unwrap();
    fs::write(temp.path().join("README.md"), "present").unwrap();
    let report = snapshot.lint();
    assert_eq!(report.status, LintStatus::Issues);
    assert_eq!(report.summary.warnings, 3);
    assert!(report.files[0].diagnostics.is_empty());
    let codes: Vec<_> = report.files[1]
        .diagnostics
        .iter()
        .map(|diagnostic| diagnostic.code.as_str())
        .collect();
    assert_eq!(codes, ["rule.duplicate_key", "rule.priority_tie"]);
    assert!(
        report.files[1]
            .diagnostics
            .iter()
            .all(|diagnostic| diagnostic.related_file.as_deref() == Some(".precept/rules/a.md"))
    );
    assert_eq!(report.files[2].diagnostics.len(), 1);
    assert_eq!(report.files[2].diagnostics[0].code, "rule.duplicate_key");
    assert!(
        !serde_json::to_string(&report)
            .unwrap()
            .contains("contradict")
    );
}

#[test]
fn cross_file_warnings_survive_unrelated_errors_but_path_warnings_do_not() {
    let first =
        "+++\nkey = \"shared\"\npriority = 5\n[applies_to]\npaths = [\"missing\"]\n+++\nfirst\n";
    let second = "+++\nkey = \"shared\"\npriority = 5\ntitle = \"\"\n[applies_to]\npaths = [\"missing\"]\n+++\nsecond\n";
    let (_temp, snapshot) = snapshot(&[("a", first), ("b", second)]);
    let report = snapshot.lint();
    assert_eq!(report.files[0].diagnostics.len(), 1);
    assert_eq!(
        report.files[0].diagnostics[0].code,
        "selector.path_not_present"
    );
    let codes: Vec<_> = report.files[1]
        .diagnostics
        .iter()
        .map(|diagnostic| diagnostic.code.as_str())
        .collect();
    assert_eq!(
        codes,
        ["rule.duplicate_key", "rule.priority_tie", "title.invalid"]
    );
    assert!(!codes.contains(&"selector.path_not_present"));
}

#[test]
fn missing_selectors_warn_about_future_files_without_naming_paths() {
    let raw = "+++\nkey = \"x\"\n[applies_to]\npaths = [\"present.txt\", \"present-dir/\", \"missing.txt\", \"missing-dir/\"]\n+++\nbody\n";
    let (temp, snapshot) = snapshot(&[("x", raw)]);
    fs::write(temp.path().join("present.txt"), "present").unwrap();
    fs::create_dir(temp.path().join("present-dir")).unwrap();
    let report = snapshot.lint();
    let diagnostics = &report.files[0].diagnostics;
    assert_eq!(diagnostics.len(), 2);
    assert!(
        diagnostics
            .iter()
            .all(|diagnostic| diagnostic.code == "selector.path_not_present")
    );
    assert!(
        diagnostics
            .iter()
            .all(|diagnostic| diagnostic.message.contains("future files may still match"))
    );
    assert!(
        diagnostics
            .iter()
            .all(|diagnostic| diagnostic.line == 4 && diagnostic.column == 1)
    );
    let encoded = serde_json::to_string(&report).unwrap();
    assert!(!encoded.contains("missing.txt"));
    assert!(!encoded.contains("missing-dir"));
}

#[test]
fn verify_without_checks_is_only_a_warning() {
    let raw = "+++\nkey = \"x\"\nenforcement = \"verify\"\n+++\nbody\n";
    let diagnostics = lint("x", raw.as_bytes());
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].code, "enforcement.no_verification");
    assert_eq!(diagnostics[0].severity, Severity::Warning);
    assert!(parse("x", raw.as_bytes()).is_ok());
}

#[test]
fn absent_unavailable_and_empty_sources_are_distinct() {
    let temp = tempfile::tempdir().unwrap();
    let absent = super::super::Snapshot::read(temp.path()).lint();
    assert_eq!(absent.status, LintStatus::NotConfigured);
    assert!(absent.files.is_empty());
    assert_eq!(absent.summary.files, 0);

    fs::create_dir_all(temp.path().join(DIR)).unwrap();
    let empty = super::super::Snapshot::read(temp.path()).lint();
    assert_eq!(empty.status, LintStatus::Ok);
    assert!(empty.files.is_empty());

    let unavailable = super::super::Snapshot::read(&temp.path().join("does-not-exist")).lint();
    assert_eq!(unavailable.status, LintStatus::Unavailable);
    assert_eq!(unavailable.files[0].status, "unavailable");
    assert_eq!(
        unavailable.files[0].diagnostics[0].code,
        "source.unavailable"
    );
    assert!(
        !serde_json::to_string(&unavailable)
            .unwrap()
            .contains("does-not-exist")
    );
}

#[test]
fn lint_acceptance_matches_the_strict_parser() {
    let cases = [
        VALID,
        "\u{feff}+++\r\nkey = \"x\"\r\n+++\r\nbody\r\n",
        "+++\nkey = \" padded\"\n+++\nbody",
        "+++\nkey = \"x\"\ntitle = \"\"\n+++\nbody",
        "+++\nkey = \"x\"\nallowed = [\"\"]\n+++\nbody",
        "+++\nkey = \"x\"\ntags = [\"bad/tag\"]\n+++\nbody",
        "+++\nkey = \"x\"\npriority = 101\n+++\nbody",
        "+++\nkey = \"x\"\n[applies_to]\npaths = [\"../x\"]\n+++\nbody",
        "+++\nkey = \"x\"\n[[check]]\nfile = \"a\"\ncontains = \"b\"\nabsent = \"c\"\n+++\nbody",
        "+++\nkey = \"x\"\n[[check]]\nfile = \"a\"\ntoml = \"b\"\n+++\nbody",
        "+++\nkey = \"x\"\n[[guard]]\ntool = \"write\"\n+++\nbody",
        "+++\nkey = \"x\"\n[[guard]]\ntool = \"shell\"\ndeny = \"rm\"\n+++\nbody",
        "+++\nkey = \"x\"\nunknown = true\n+++\nbody",
        "+++\nkey = \"x\"\n+++\n   ",
        "+++\nkey = \"x\"\n+++\nbody\u{1}",
        "key = \"x\"\n+++\nbody",
        "+++\nkey = \"x\"\n",
    ];
    for raw in cases {
        let parsed = parse("x", raw.as_bytes());
        let has_error = lint("x", raw.as_bytes())
            .iter()
            .any(|diagnostic| diagnostic.severity == Severity::Error);
        assert_eq!(parsed.is_ok(), !has_error, "acceptance changed for {raw:?}");
    }
    assert!(
        lint("Bad Name", VALID.as_bytes())
            .iter()
            .any(|diagnostic| diagnostic.code == "id.invalid")
    );
    assert!(
        lint("x", &vec![b'a'; MAX_BYTES + 1])
            .iter()
            .any(|diagnostic| diagnostic.code == "file.too_large")
    );
    assert!(
        lint("x", &[0xff, 0xfe])
            .iter()
            .any(|diagnostic| diagnostic.code == "encoding.invalid")
    );
}

#[test]
fn reports_are_deterministic_and_related_files_are_bounded() {
    let raw = "+++\nkey = \"x\"\n[applies_to]\npaths = [\"missing\"]\n+++\nbody\n";
    let (_temp, snapshot) = snapshot(&[("b", raw), ("a", raw)]);
    let first = serde_json::to_string(&snapshot.lint()).unwrap();
    let second = serde_json::to_string(&snapshot.lint()).unwrap();
    assert_eq!(first, second);
    let report = snapshot.lint();
    assert_eq!(report.files[0].file, ".precept/rules/a.md");
    assert_eq!(report.files[1].file, ".precept/rules/b.md");
    let related = report.files[1]
        .diagnostics
        .iter()
        .filter_map(|diagnostic| diagnostic.related_file.as_deref())
        .collect::<Vec<_>>();
    assert!(
        related
            .iter()
            .all(|file| file.starts_with(".precept/rules/"))
    );
    assert!(related.iter().all(|file| file.len() <= 138));
}

#[test]
fn diagnostic_truncation_is_explicit_and_bounded() {
    let mut raw = String::from("+++\nkey = \"x\"\n");
    for _ in 0..(MAX_DIAGNOSTICS + 8) {
        raw.push_str("[[check]]\nfile = \"../private\"\n");
    }
    raw.push_str("+++\nbody\n");
    let diagnostics = lint("x", raw.as_bytes());
    assert_eq!(diagnostics.len(), MAX_DIAGNOSTICS);
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "diagnostics.truncated")
    );
    assert!(
        diagnostics
            .iter()
            .all(|diagnostic| !diagnostic.message.contains("../private"))
    );
}

#[test]
fn cross_file_findings_do_not_duplicate_the_truncation_marker() {
    let mut raw =
        String::from("+++\nkey = \"shared\"\npriority = 5\n[applies_to]\npaths = [\"missing\"]\n");
    for _ in 0..(MAX_DIAGNOSTICS + 8) {
        raw.push_str("[[check]]\nfile = \"../private\"\n");
    }
    raw.push_str("+++\nbody\n");
    let first =
        "+++\nkey = \"shared\"\npriority = 5\n[applies_to]\npaths = [\"missing\"]\n+++\nfirst\n";
    let (_temp, snapshot) = snapshot(&[("a", first), ("b", &raw)]);
    let report = snapshot.lint();
    let diagnostics = &report.files[1].diagnostics;
    assert_eq!(diagnostics.len(), MAX_DIAGNOSTICS);
    assert_eq!(
        diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.code == "diagnostics.truncated")
            .count(),
        1
    );
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "rule.duplicate_key")
    );
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "rule.priority_tie")
    );
}

#[test]
fn invalid_file_display_names_are_safe_and_unique() {
    let first = display_file("../Private\\Rule.md");
    let second = display_file("..\\Private\\Rule.md");
    assert_ne!(first, second);
    for file in [&first, &second] {
        assert!(file.starts_with(".precept/rules/"));
        assert!(!file.contains(".."));
        assert!(!file.contains('\\'));
        assert!(!file.contains("Private"));
        assert!(!file.contains("Rule.md"));
        let id = file
            .strip_prefix(".precept/rules/")
            .and_then(|name| name.strip_suffix(".md"));
        assert!(id.is_some_and(valid_id));
    }
}

#[test]
fn invalid_file_display_names_cannot_collide_with_real_rule_paths() {
    let invalid = display_file("Bad");
    let valid = invalid
        .strip_prefix(".precept/rules/")
        .and_then(|name| name.strip_suffix(".md"))
        .unwrap();
    assert!(valid_id(valid));
    let sources = [
        ("Bad", b"".as_slice(), None),
        (valid, VALID.as_bytes(), None),
    ];
    let report = super::lint_files(sources.into_iter(), None);
    let paths: std::collections::BTreeSet<_> =
        report.files.iter().map(|file| file.file.as_str()).collect();
    assert_eq!(report.files.len(), 2);
    assert_eq!(paths.len(), 2);
    assert!(paths.contains(format!("{DIR}/{valid}.md").as_str()));
    assert!(paths.contains(format!("{DIR}/{valid}-2.md").as_str()));
    assert!(
        report
            .files
            .windows(2)
            .all(|pair| pair[0].file < pair[1].file)
    );
    assert_eq!(
        report
            .files
            .iter()
            .flat_map(|file| &file.diagnostics)
            .filter(|diagnostic| diagnostic.code == "id.invalid")
            .count(),
        1
    );
}

#[test]
fn worst_case_serialized_report_stays_below_the_daemon_limit() {
    let file = format!(".precept/rules/{}.md", "a".repeat(120));
    let related = format!(".precept/rules/{}.md", "b".repeat(120));
    let diagnostics: Vec<Diagnostic> = (0..MAX_DIAGNOSTICS)
        .map(|index| Diagnostic {
            code: "selector.path_not_present".to_owned(),
            severity: Severity::Warning,
            message: super::report::message("selector.path_not_present").to_owned(),
            line: u32::try_from(index + 1).unwrap(),
            column: 1,
            end_line: u32::try_from(index + 1).unwrap(),
            end_column: 1,
            related_file: Some(related.clone()),
        })
        .collect();
    let files = (0..64)
        .map(|_| LintFile {
            file: file.clone(),
            status: "issues".to_owned(),
            diagnostics: diagnostics.clone(),
        })
        .collect();
    let encoded = serde_json::to_vec(&report(LintStatus::Issues, files)).unwrap();
    assert!(encoded.len() < 768 * 1024, "{} bytes", encoded.len());
}
