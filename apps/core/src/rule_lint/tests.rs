use super::*;
use crate::{Model, Phase, connection::Target, rule_templates::RuleTemplate};
use serde_json::{Value, json};

fn diagnostic() -> Value {
    json!({
        "code":"key.invalid", "severity":"error", "message":"The rule key is invalid.",
        "line":2, "column":1, "end_line":2, "end_column":4
    })
}

fn report(status: &str) -> Value {
    match status {
        "ok" => json!({
            "format":"precept-rules-lint", "version":1, "status":"ok",
            "summary":{"files":1,"errors":0,"warnings":0},
            "files":[{"file":".precept/rules/rust.md","status":"ok","diagnostics":[]}]
        }),
        "not_configured" => json!({
            "format":"precept-rules-lint", "version":1, "status":"not_configured",
            "summary":{"files":0,"errors":0,"warnings":0}, "files":[]
        }),
        "unavailable" => json!({
            "format":"precept-rules-lint", "version":1, "status":"unavailable",
            "summary":{"files":1,"errors":1,"warnings":0},
            "files":[{"file":".precept/rules","status":"unavailable","diagnostics":[{
                "code":"source.unavailable", "severity":"error",
                "message":"The rule source is unavailable.",
                "line":1,"column":1,"end_line":1,"end_column":1
            }]}]
        }),
        _ => json!({
            "format":"precept-rules-lint", "version":1, "status":"issues",
            "summary":{"files":2,"errors":1,"warnings":1},
            "files":[
                {"file":".precept/rules/a.md","status":"issues","diagnostics":[diagnostic()]},
                {"file":".precept/rules/b.md","status":"issues","diagnostics":[{
                    "code":"rule.duplicate_key","severity":"warning",
                    "message":"Another rule file uses the same key.",
                    "line":3,"column":1,"end_line":3,"end_column":4,
                    "related_file":".precept/rules/a.md"
                }]}
            ]
        }),
    }
}

fn live_model(target: Target) -> Model {
    let mut model = Model::fixture();
    model.live = true;
    model.connection.target = target;
    model.connection.runtime = std::env::temp_dir().to_string_lossy().into_owned();
    model.connection.endpoint = "synthetic".into();
    model.connection.distribution = "synthetic".into();
    model.connection.user = "operator".into();
    model.connection.helper = "/usr/bin/precept-bridge".into();
    model.connection.wsl_runtime = "/tmp/private".into();
    model.connection.wsl_endpoint = "/tmp/private/control.sock".into();
    model.select(0);
    model
}

#[test]
fn accepts_each_consistent_status_and_recomputes_the_summary() {
    for status in ["ok", "issues", "not_configured", "unavailable"] {
        let parsed = LintReport::parse(report(status)).unwrap();
        assert_eq!(parsed.validate(), Ok(()));
        let expected = parsed.files.iter().flat_map(|file| &file.diagnostics).fold(
            (0_u32, 0_u32),
            |(errors, warnings), diagnostic| match diagnostic.severity {
                Severity::Error => (errors + 1, warnings),
                Severity::Warning => (errors, warnings + 1),
            },
        );
        assert_eq!((parsed.summary.errors, parsed.summary.warnings), expected);
        assert_eq!(parsed.summary.files, parsed.files.len() as u32);
    }
}

#[test]
fn accepts_the_complete_daemon_diagnostic_vocabulary() {
    for (code, message, severity) in [
        ("id.invalid", "The rule file id is invalid.", "error"),
        (
            "encoding.invalid",
            "The rule file is not valid UTF-8.",
            "error",
        ),
        (
            "format.opening_marker",
            "The rule file must open with a +++ marker.",
            "error",
        ),
        (
            "format.closing_marker",
            "The front matter must end with a +++ marker.",
            "error",
        ),
        (
            "diagnostics.truncated",
            "Additional diagnostics were omitted by the report limit.",
            "warning",
        ),
    ] {
        let mut d = diagnostic();
        d["code"] = json!(code);
        d["message"] = json!(message);
        d["severity"] = json!(severity);
        let mut v = report("issues");
        v["files"][0]["diagnostics"] = json!([d]);
        v["files"].as_array_mut().unwrap().truncate(1);
        v["summary"] = json!({
            "files":1,
            "errors":if severity == "error" {1} else {0},
            "warnings":if severity == "warning" {1} else {0}
        });
        assert_eq!(LintReport::parse(v).unwrap().status, LintStatus::Issues);
    }
    let mut v = report("issues");
    v["files"][0]["diagnostics"][0]["code"] = json!("file.name_invalid");
    assert!(LintReport::parse(v).is_err());
}

#[test]
fn accepts_synthetic_source_issues_but_not_synthetic_ok_files() {
    let mut v = report("issues");
    v["summary"] = json!({"files":1,"errors":1,"warnings":0});
    v["files"] = json!([{
        "file":".precept/rules","status":"issues","diagnostics":[{
            "code":"source.unsafe_path","severity":"error",
            "message":"The rule source contains an unsafe directory or link.",
            "line":1,"column":1,"end_line":1,"end_column":1
        }]
    }]);
    assert!(LintReport::parse(v).is_ok());

    let mut v = report("ok");
    v["files"][0]["file"] = json!(".precept/rules");
    assert!(LintReport::parse(v).is_err());
}

#[test]
fn refuses_unknown_shapes_enums_codes_and_transport_error_text() {
    let mut cases = Vec::new();
    let mut v = report("issues");
    v["format"] = json!("other");
    cases.push(v);
    let mut v = report("issues");
    v["version"] = json!(2);
    cases.push(v);
    let mut v = report("issues");
    v["unexpected"] = json!(true);
    cases.push(v);
    let mut v = report("issues");
    v["files"][1]["diagnostics"][0]["code"] = json!("private.key");
    cases.push(v);
    let mut v = report("issues");
    v["files"][1]["diagnostics"][0]["severity"] = json!("fatal");
    cases.push(v);
    let mut v = report("issues");
    v["files"][0]["diagnostics"][0]["message"] = json!("PRIVATE_SOURCE_65");
    cases.push(v);
    let mut v = report("issues");
    v["files"][0]["diagnostics"][0]["message"] =
        json!("The rule key is invalid.\nPRIVATE_SOURCE_65");
    cases.push(v);
    let mut v = report("issues");
    v["files"][0]["diagnostics"][0]["extra"] = json!("/private/root");
    cases.push(v);
    for value in cases {
        assert!(LintReport::parse(value).is_err());
    }
}

#[test]
fn refuses_unsafe_unsorted_duplicate_or_unbounded_files_and_counts() {
    for file in [
        "/absolute/private.md",
        "../private.md",
        ".precept//rules/a.md",
        ".precept/./rules/a.md",
        "C:/private.md",
        ".precept/rules/\\private.md",
        ".precept/rules/*.md",
        ".precept/rules/\u{202e}private.md",
        ".precept/rules/CON.md",
        ".precept/rules/private. ",
        ".precept/rules",
        ".precept/rules/nested/a.md",
        ".precept/rules/Uppercase.md",
    ] {
        let mut v = report("ok");
        v["files"][0]["file"] = json!(file);
        assert!(LintReport::parse(v).is_err(), "accepted {file}");
    }
    let mut v = report("issues");
    v["files"].as_array_mut().unwrap().reverse();
    assert!(LintReport::parse(v).is_err());
    let mut v = report("issues");
    v["files"][1]["file"] = v["files"][0]["file"].clone();
    assert!(LintReport::parse(v).is_err());
    let mut v = report("ok");
    v["summary"]["files"] = json!(u32::MAX);
    assert!(LintReport::parse(v).is_err());
    let mut v = report("ok");
    v["files"] = json!(
        (0..65)
            .map(|i| json!({
                "file":format!(".precept/rules/{i}.md"),"status":"ok","diagnostics":[]
            }))
            .collect::<Vec<_>>()
    );
    assert!(LintReport::parse(v).is_err());
    let mut v = report("issues");
    v["files"][0]["diagnostics"][0]["message"] = json!("x".repeat(768 * 1024));
    assert!(LintReport::parse(v).is_err());
    let mut v = report("issues");
    v["files"][0]["diagnostics"] = json!((0..33).map(|_| diagnostic()).collect::<Vec<_>>());
    assert!(LintReport::parse(v).is_err());
}

#[test]
fn refuses_bad_positions_related_paths_severity_and_status_consistency() {
    for (line, column, end_line, end_column) in [
        (0, 1, 1, 1),
        (1, 0, 1, 1),
        (2, 2, 2, 1),
        (1, 1, 0, 1),
        (1_000_001, 1, 1_000_001, 1),
    ] {
        let mut v = report("issues");
        let d = &mut v["files"][0]["diagnostics"][0];
        d["line"] = json!(line);
        d["column"] = json!(column);
        d["end_line"] = json!(end_line);
        d["end_column"] = json!(end_column);
        assert!(LintReport::parse(v).is_err());
    }
    let mut v = report("issues");
    v["files"][1]["diagnostics"][0]["related_file"] = json!("../../private.md");
    assert!(LintReport::parse(v).is_err());
    let mut v = report("issues");
    v["files"][1]["diagnostics"][0]["related_file"] = json!(".precept/rules/not-reported.md");
    assert!(LintReport::parse(v).is_err());
    let mut v = report("issues");
    v["files"][0]["diagnostics"][0]["severity"] = json!("warning");
    assert!(LintReport::parse(v).is_err());
    let mut v = report("ok");
    v["files"][0]["status"] = json!("issues");
    assert!(LintReport::parse(v).is_err());
    let mut v = report("issues");
    v["status"] = json!("ok");
    assert!(LintReport::parse(v).is_err());
    let mut v = report("not_configured");
    v["files"] = report("ok")["files"].clone();
    assert!(LintReport::parse(v).is_err());
    let mut v = report("unavailable");
    v["files"][0]["status"] = json!("issues");
    assert!(LintReport::parse(v).is_err());
    let mut v = report("unavailable");
    v["files"]
        .as_array_mut()
        .unwrap()
        .push(report("issues")["files"][0].clone());
    v["summary"] = json!({"files":2,"errors":2,"warnings":0});
    assert!(LintReport::parse(v).is_err());
    let mut v = report("ok");
    v["files"] = json!([]);
    v["summary"]["files"] = json!(0);
    assert_eq!(LintReport::parse(v).unwrap().status, LintStatus::Ok);
    let mut v = report("issues");
    v["files"][0]["diagnostics"][0]["code"] = json!("source.unavailable");
    v["files"][0]["diagnostics"][0]["message"] = json!("The rule source is unavailable.");
    assert!(LintReport::parse(v).is_err());
}

#[test]
fn plain_text_is_bounded_english_and_contains_no_unvalidated_data() {
    let parsed = LintReport::parse(report("issues")).unwrap();
    let text = parsed.plain_text();
    assert!(text.starts_with("Precept rule diagnostics\nStatus: issues found"));
    assert!(text.contains("[Error] key.invalid 2:1-2:4"));
    assert!(text.contains("Related file: .precept/rules/a.md"));
    assert!(!text.contains("PRIVATE_SOURCE"));
    assert!(text.chars().count() <= 24 * 1024);

    let files: Vec<_> = ["a", "b", "c"]
        .into_iter()
        .map(|id| {
            json!({
                "file":format!(".precept/rules/{id}.md"),"status":"issues",
                "diagnostics":(0..32).map(|_| diagnostic()).collect::<Vec<_>>()
            })
        })
        .collect();
    let v = json!({
        "format":"precept-rules-lint", "version":1, "status":"issues",
        "summary":{"files":3,"errors":96,"warnings":0},
        "files":files
    });
    let parsed = LintReport::parse(v).unwrap();
    let text = parsed.plain_text();
    assert!(
        text.contains("Copy includes 80 of 96 diagnostics; 16 omitted."),
        "{text}"
    );
    assert!(text.chars().count() <= 24 * 1024);
}

#[test]
fn copy_size_truncation_names_the_limit_and_actual_complete_count() {
    let first = format!(".precept/rules/{}.md", "a".repeat(120));
    let diagnostics: Vec<_> = (0..2)
        .map(|_| Diagnostic {
            code: DiagnosticCode::SelectorPathNotPresent,
            severity: Severity::Warning,
            message: DiagnosticCode::SelectorPathNotPresent.message().to_owned(),
            line: 1_000_000,
            column: 999_999,
            end_line: 1_000_000,
            end_column: 1_000_000,
            related_file: Some(first.clone()),
        })
        .collect();
    let mut files = Vec::new();
    for index in 0..64 {
        let file = if index == 0 {
            first.clone()
        } else {
            format!(".precept/rules/b{index:02}-{}.md", "c".repeat(116))
        };
        let count = if index < 16 { 2 } else { 1 };
        files.push(LintFile {
            file,
            status: FileStatus::Issues,
            diagnostics: diagnostics[..count].to_vec(),
        });
    }
    let report = LintReport {
        format: "precept-rules-lint".to_owned(),
        version: 1,
        status: LintStatus::Issues,
        summary: LintSummary {
            files: 64,
            errors: 0,
            warnings: 80,
        },
        files,
    };
    let text = report.plain_text();
    assert!(
        text.contains("Copy truncated at the 24,576-character limit."),
        "{} chars: {}",
        text.chars().count(),
        text
    );
    assert!(text.contains("Copy includes"));
    assert!(text.contains("of 80 diagnostics;"));
    assert!(text.chars().count() <= 24 * 1024);
}

#[test]
fn stale_requests_are_rejected_after_every_scope_or_epoch_change() {
    for target in [Target::Local, Target::Wsl] {
        let mut model = live_model(target);
        assert!(model.rule_lint_ready());
        let request = model.request_rule_lint().unwrap();
        model.select(1);
        assert!(!model.complete_rule_lint(&request, Ok(report("ok"))));

        let request = model.request_rule_lint().unwrap();
        model.begin_read().unwrap();
        assert!(!model.complete_rule_lint(&request, Ok(report("ok"))));

        model.receive(Ok(crate::Snapshot::fixture()));
        model.select(0);
        let request = model.request_rule_lint().unwrap();
        model.connection_draft = model.connection.clone();
        assert!(model.apply_connection());
        assert!(!model.complete_rule_lint(&request, Ok(report("ok"))));

        model.receive(Ok(crate::Snapshot::fixture()));
        model.select(0);
        let request = model.request_rule_lint().unwrap();
        model.connection.endpoint = "changed".into();
        assert!(!model.complete_rule_lint(&request, Ok(report("ok"))));
    }
}

#[test]
fn profile_scope_invalidation_rejects_a_delayed_same_project_completion() {
    let mut model = live_model(Target::Local);
    let request = model.request_rule_lint().unwrap();
    model.rule_lint.invalidate();
    assert!(!model.complete_rule_lint(&request, Ok(report("issues"))));
    assert!(model.rule_lint.report.is_none());
}

#[test]
fn newest_lint_request_wins_and_failures_keep_only_fixed_reasons() {
    let mut model = live_model(Target::Local);
    let older = model.request_rule_lint().unwrap();
    let newer = model.request_rule_lint().unwrap();
    assert!(model.complete_rule_lint(&newer, Ok(report("issues"))));
    assert!(!model.complete_rule_lint(&older, Ok(report("ok"))));
    assert_eq!(
        model.rule_lint.report.as_ref().unwrap().status,
        LintStatus::Issues
    );

    let request = model.request_rule_lint().unwrap();
    assert!(model.complete_rule_lint(&request, Err("PRIVATE raw transport error")));
    assert_eq!(model.rule_lint.error, Some("Unavailable"));
    assert!(!format!("{:?}", model.rule_lint).contains("PRIVATE"));

    let request = model.request_rule_lint().unwrap();
    assert!(model.complete_rule_lint(&request, Ok(report("issues"))));
    assert!(model.rule_lint.report.is_some());
}

#[test]
fn offline_synthetic_missing_and_invalid_projects_cannot_start_io() {
    let mut model = Model::fixture();
    model.select(0);
    assert_eq!(model.request_rule_lint(), Err("Unavailable"));
    model.live = true;
    model.connection.target = Target::Synthetic;
    assert_eq!(model.request_rule_lint(), Err("InvalidRequest"));
    model = live_model(Target::Local);
    model.selected_project = Some("../private".into());
    assert_eq!(model.request_rule_lint(), Err("InvalidRequest"));
    model = live_model(Target::Local);
    model.phase = Phase::Loading;
    assert_eq!(model.request_rule_lint(), Err("InvalidRequest"));
}

#[test]
fn template_dialog_state_is_copy_only_and_invalidation_closes_it() {
    let mut model = live_model(Target::Local);
    model.rule_lint.open_templates();
    assert_eq!(model.rule_lint.selected_template, Some(RuleTemplate::Rust));
    model.rule_lint.selected_template = Some(RuleTemplate::Security);
    model.rule_lint.close_templates();
    assert!(model.rule_lint.selected_template.is_none());
    model.rule_lint.open_templates();
    model.select(1);
    assert!(model.rule_lint.selected_template.is_none());
    assert!(!model.rule_lint.busy);
}
