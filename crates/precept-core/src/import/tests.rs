use super::*;

fn scan(source: &str, spec: SourceSpec, text: &str) -> Scan {
    let mut out = Scan::default();
    parse(source, spec, text, &mut out);
    out
}

fn spec(kind: SourceKind, frontmatter: Option<FrontmatterKind>) -> SourceSpec {
    SourceSpec {
        source: "rule.md",
        kind,
        sections: false,
        frontmatter,
    }
}

#[test]
fn agents_sections_keep_fences_and_skip_precepts_pointer() {
    let text = "# Demo\n\nIntro.\n\n<!-- precept:begin v1 -->\n## Precept\nCall tools.\n<!-- precept:end -->\n\n## Repository\n\nWrite English.\n\n```text\n## not a heading\n```\n\n## Engineering\n\n### Validation\n\nRun gates.\n\n### 安全\n\n不要记录密钥。\n\n## Empty\n";
    let out = scan("AGENTS.md", FILES[0], text);
    let keys: Vec<&str> = out.candidates.iter().map(|c| c.key.as_str()).collect();
    assert_eq!(
        keys,
        [
            "agents-repository",
            "agents-engineering-validation",
            "agents-engineering-section-4"
        ]
    );
    assert_eq!(out.candidates[0].kind, SourceKind::AgentsMd);
    assert!(out.candidates[0].value.contains("## not a heading"));
    assert!(
        !out.candidates
            .iter()
            .any(|c| c.value.contains("Call tools"))
    );
    assert_eq!(out.candidates[0].tags, ["agents-md", "imported"]);
    assert_eq!(out.candidates[1].title, "Engineering Validation");
    assert_eq!(out.candidates[2].value, "不要记录密钥。");
    assert_eq!(out.skipped[0].reason, "empty");
}

#[test]
fn claude_root_sections_use_a_distinct_namespace() {
    let out = scan("CLAUDE.md", FILES[1], "## Commits\n\nKeep them atomic.\n");
    assert_eq!(out.candidates[0].key, "claude-commits");
    assert_eq!(out.candidates[0].kind, SourceKind::ClaudeMd);
}

#[test]
fn cursor_frontmatter_preserves_description_and_literal_paths() {
    let text = "---\ndescription: API rules\nglobs:\n  - src/api/\n  - Cargo.toml\nalwaysApply: false\n---\n# API\n\nValidate inputs.\n";
    let out = scan(
        ".cursor/rules/api.mdc",
        spec(SourceKind::Cursor, Some(FrontmatterKind::Cursor)),
        text,
    );
    assert_eq!(out.candidates.len(), 1);
    assert_eq!(out.candidates[0].title, "API");
    assert_eq!(out.candidates[0].description.as_deref(), Some("API rules"));
    assert_eq!(
        out.candidates[0].rule.applies_to.paths,
        ["Cargo.toml", "src/api/"]
    );
    assert_eq!(out.candidates[0].tags, ["cursor", "imported"]);
    assert!(out.skipped.is_empty());
}

#[test]
fn bom_frontmatter_keeps_its_applicability() {
    let text = "\u{feff}---\npaths: src/\n---\nUse strict types.\n";
    let out = scan(
        ".claude/rules/bom.md",
        spec(SourceKind::Claude, Some(FrontmatterKind::Claude)),
        text,
    );
    assert_eq!(out.candidates.len(), 1);
    assert_eq!(out.candidates[0].rule.applies_to.paths, ["src/"]);
    assert_eq!(out.candidates[0].value, "Use strict types.");
}

#[test]
fn comma_separated_literal_selectors_are_split() {
    let text = "---\npaths: src/, tests/\n---\nUse strict types.\n";
    let out = scan(
        ".claude/rules/typescript.md",
        spec(SourceKind::Claude, Some(FrontmatterKind::Claude)),
        text,
    );
    assert_eq!(out.candidates[0].rule.applies_to.paths, ["src/", "tests/"]);
}

#[test]
fn claude_project_file_is_a_known_section_source() {
    let out = scan(
        ".claude/CLAUDE.md",
        FILES[2],
        "## Commits\n\nKeep them atomic.\n",
    );
    assert_eq!(out.candidates[0].key, "claude-commits");
    assert_eq!(out.candidates[0].source, ".claude/CLAUDE.md");
}

#[test]
fn continue_name_and_inline_list_are_supported() {
    let text = "---\nname: TypeScript\nglobs: [src/, 'tests/']\n---\nUse strict types.\n";
    let out = scan(
        ".continue/rules/typescript.md",
        spec(SourceKind::Continue, Some(FrontmatterKind::Continue)),
        text,
    );
    assert_eq!(out.candidates[0].title, "TypeScript");
    assert_eq!(out.candidates[0].rule.applies_to.paths, ["src/", "tests/"]);
}

#[test]
fn copilot_and_claude_selectors_do_not_silently_accept_globs() {
    for (source, kind, field) in [
        (
            ".github/instructions/api.instructions.md",
            FrontmatterKind::Copilot,
            "applyTo",
        ),
        (".claude/rules/api.md", FrontmatterKind::Claude, "paths"),
    ] {
        let text = format!("---\n{field}: src/**/*.rs\n---\nUse the API.\n");
        let out = scan(source, spec(SourceKind::Copilot, Some(kind)), &text);
        assert!(out.candidates.is_empty());
        assert_eq!(out.skipped[0].reason, "unsupported_applicability");
        assert_eq!(out.skipped[0].detail.as_deref(), Some("src/**/*.rs"));
    }
}

#[test]
fn unsupported_activation_and_metadata_are_explicit() {
    let manual = "---\nalwaysApply: false\n---\nManual only.\n";
    let out = scan(
        ".cursor/rules/manual.mdc",
        spec(SourceKind::Cursor, Some(FrontmatterKind::Cursor)),
        manual,
    );
    assert_eq!(out.skipped[0].reason, "unsupported_applicability");
    assert_eq!(out.skipped[0].detail.as_deref(), Some("manual activation"));

    let described = "---\ndescription: Decide when to use this\n---\nManual only.\n";
    let out = scan(
        ".cursor/rules/described.mdc",
        spec(SourceKind::Cursor, Some(FrontmatterKind::Cursor)),
        described,
    );
    assert!(out.candidates.is_empty());
    assert_eq!(out.skipped[0].reason, "unsupported_applicability");

    let unknown = "---\ndescription: x\nfutureSelector: all\n---\nBody.\n";
    let out = scan(
        ".cursor/rules/future.mdc",
        spec(SourceKind::Cursor, Some(FrontmatterKind::Cursor)),
        unknown,
    );
    assert_eq!(out.skipped[0].reason, "unsupported_metadata");
    assert_eq!(out.skipped[0].detail.as_deref(), Some("futureSelector"));
}

#[test]
fn malformed_frontmatter_never_falls_back_to_body_import() {
    for text in [
        "---\ndescription: x\nBody without close.",
        "---\ndescription: x\ndescription: y\n---\nBody.",
        "---\ndescription: |\n  block\n---\nBody.",
        "---\nglobs:\n  nested: value\n---\nBody.",
        "---\npaths: [src/]\n  - tests/\n---\nBody.",
    ] {
        let out = scan(
            ".cursor/rules/bad.mdc",
            spec(SourceKind::Cursor, Some(FrontmatterKind::Cursor)),
            text,
        );
        assert!(out.candidates.is_empty());
        assert_eq!(out.skipped[0].reason, "malformed_frontmatter");
    }
}

#[test]
fn always_apply_wins_over_ignored_source_globs() {
    let text = "---\nalwaysApply: true\nglobs: src/**/*.rs\n---\nAlways.\n";
    let out = scan(
        ".cursor/rules/always.mdc",
        spec(SourceKind::Cursor, Some(FrontmatterKind::Cursor)),
        text,
    );
    assert_eq!(out.candidates.len(), 1);
    assert!(out.candidates[0].rule.applies_to.is_empty());
}

#[test]
fn duplicate_values_keep_the_first_source_and_disclose_it() {
    let mut out = Scan::default();
    parse(
        "AGENTS.md",
        FILES[0],
        "## Same\n\nUse one style.\n",
        &mut out,
    );
    parse(
        ".cursor/rules/same.mdc",
        SourceSpec {
            source: ".cursor/rules/same.mdc",
            kind: SourceKind::Cursor,
            sections: false,
            frontmatter: Some(FrontmatterKind::Cursor),
        },
        "---\nalwaysApply: true\n---\nUse one style.\n",
        &mut out,
    );
    assert_eq!(out.candidates.len(), 1);
    assert_eq!(out.skipped[0].reason, "duplicate");
    assert_eq!(out.skipped[0].detail.as_deref(), Some("AGENTS.md"));
}

#[test]
fn equal_values_with_different_applicability_remain_distinct() {
    let mut out = Scan::default();
    parse(
        "AGENTS.md",
        FILES[0],
        "## Same\n\nUse one style.\n",
        &mut out,
    );
    parse(
        ".claude/rules/same.md",
        SourceSpec {
            source: ".claude/rules/same.md",
            kind: SourceKind::Claude,
            sections: false,
            frontmatter: Some(FrontmatterKind::Claude),
        },
        "---\npaths: src/\n---\nUse one style.\n",
        &mut out,
    );
    assert_eq!(out.candidates.len(), 2);
    assert!(out.skipped.is_empty());
}

#[test]
fn keys_are_unique_bounded_and_stable() {
    let mut out = Scan::default();
    for index in 0..3 {
        let source = format!(".claude/rules/{}.md", "x".repeat(index + 1));
        parse(
            &source,
            SourceSpec {
                source: ".claude/rules/rule.md",
                kind: SourceKind::Claude,
                sections: false,
                frontmatter: Some(FrontmatterKind::Claude),
            },
            &format!("## Same\n\nBody {index}."),
            &mut out,
        );
    }
    assert_eq!(out.candidates.len(), 3);
    let keys: std::collections::HashSet<&str> =
        out.candidates.iter().map(|c| c.key.as_str()).collect();
    assert_eq!(keys.len(), 3);
    assert!(out.candidates.iter().all(|c| c.key.len() <= MAX_KEY));
}

#[test]
fn value_and_reply_budgets_are_explicit() {
    let long = format!("## Big\n\n{}", "x".repeat(MAX_VALUE + 1));
    let out = scan("AGENTS.md", FILES[0], &long);
    assert_eq!(out.skipped[0].reason, "too_long");

    let mut text = String::new();
    for index in 0..(MAX_CANDIDATES + 2) {
        text.push_str(&format!(
            "## Rule {index}\n\n{index}:{}\n",
            "y".repeat(MAX_VALUE - 30)
        ));
    }
    let out = scan("AGENTS.md", FILES[0], &text);
    assert!(out.candidates.len() <= MAX_CANDIDATES);
    assert!(
        out.skipped.iter().any(|s| s.reason == "budget_exhausted"),
        "candidates={} skipped={} bytes={}",
        out.candidates.len(),
        out.skipped.len(),
        out.bytes
    );
    assert!(serde_json::to_vec(&out.candidates).unwrap().len() <= MAX_REPLY_BYTES);
    let mut candidates = serde_json::to_value(&out.candidates).unwrap();
    for candidate in candidates.as_array_mut().unwrap() {
        candidate["scope"] = serde_json::json!({"kind":"project","name":"p".repeat(64)});
        candidate["exists"] = serde_json::json!(false);
    }
    let reply = serde_json::json!({"candidates":candidates,"skipped":out.skipped});
    assert!(serde_json::to_vec(&reply).unwrap().len() <= MAX_REPLY_BYTES);
}
