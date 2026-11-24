use precept_core::contracts::{CheckCache, MAX_CHECKED_BYTES, Snapshot, evaluate_cached, parse};
use precept_core::{
    ContextResolver, DocumentRepository, Error, Handoff, HandoffRepository, Preference,
    PreferenceRepository, Project, ProjectRegistry, Worktree, WorktreeInspector,
    briefing::{Moment, render},
};
use serde_json::{Value, json};
use std::{
    collections::BTreeSet,
    fs,
    path::PathBuf,
    time::{Duration, Instant},
};

const RULES: usize = 64;
const CHECKS: usize = 16;
const SAMPLES: usize = 25;

struct Registry(PathBuf);
impl ProjectRegistry for Registry {
    fn project(&self, name: &str) -> precept_core::Result<Project> {
        (name == "benchmark")
            .then(|| Project {
                name: name.into(),
                root: self.0.clone(),
            })
            .ok_or(Error::UnknownProject)
    }
}
struct Preferences;
impl PreferenceRepository for Preferences {
    fn preferences(&self, _project: &str) -> precept_core::Result<Vec<Preference>> {
        Ok(vec![])
    }
}
struct Handoffs;
impl HandoffRepository for Handoffs {
    fn handoff(&self, _project: &str, _task_id: &str) -> precept_core::Result<Handoff> {
        Err(Error::HandoffNotFound)
    }
}
struct Worktrees(BTreeSet<String>);
impl WorktreeInspector for Worktrees {
    fn inspect(&self, _project: &Project, _fingerprint: bool) -> precept_core::Result<Worktree> {
        Ok(Worktree {
            dirty: Some(false),
            ..Default::default()
        })
    }
    fn reviewed(&self, _project: &Project, _dir: &str) -> Option<BTreeSet<String>> {
        Some(self.0.clone())
    }
}
struct Documents;
impl DocumentRepository for Documents {
    fn metadata(
        &self,
        _project: &Project,
        _include_content: bool,
    ) -> precept_core::Result<Vec<Value>> {
        Ok(vec![])
    }
}

fn fixture() -> (tempfile::TempDir, Vec<(String, Vec<u8>)>) {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();
    let dir = root.join(".precept/rules");
    fs::create_dir_all(&dir).unwrap();
    let cargo = format!(
        "[package]\nname = \"benchmark\"\nedition = \"2024\"\n{}",
        "# padding\n".repeat(180)
    );
    let readme = format!("{}benchmark-readme", "documentation ".repeat(8192));
    let package = format!(
        "{{\"padding\":\"{}\",\"engine\":\"node\"}}",
        "p".repeat(60_000)
    );
    let large = format!("{}benchmark-large", "x".repeat(MAX_CHECKED_BYTES - 15));
    fs::write(root.join("Cargo.toml"), &cargo).unwrap();
    fs::write(root.join("README.md"), &readme).unwrap();
    fs::write(root.join("package.json"), &package).unwrap();
    fs::write(root.join("large.txt"), &large).unwrap();
    let mut sources = Vec::new();
    for index in 0..RULES {
        let mut source = format!(
            "+++\nkey = \"benchmark-{index}\"\ntitle = \"Benchmark rule {index}\"\ntags = [\"benchmark\"]\n"
        );
        for check in 0..CHECKS {
            let extra = match check % 4 {
                0 => "toml = \"package.edition\"\nequals = \"2024\"\n",
                1 => "contains = \"benchmark-readme\"\n",
                2 => "json = \"engine\"\nequals = \"node\"\n",
                _ => "contains = \"benchmark-large\"\n",
            };
            let file = ["Cargo.toml", "README.md", "package.json", "large.txt"][check % 4];
            source.push_str(&format!("\n[[check]]\nfile = \"{file}\"\n{extra}"));
        }
        source.push_str(&format!(
            "+++\n\n# Benchmark rule {index}\n\n{}\n",
            "Follow the reviewed benchmark contract. ".repeat(24)
        ));
        let id = format!("rule-{index:02}");
        fs::write(dir.join(format!("{id}.md")), &source).unwrap();
        sources.push((id, source.into_bytes()));
    }
    (temp, sources)
}

fn percentile(mut samples: Vec<Duration>, numerator: usize, denominator: usize) -> f64 {
    samples.sort_unstable();
    let index = samples.len() * numerator / denominator;
    samples[index.min(samples.len() - 1)].as_secs_f64() * 1000.0
}

fn summary(samples: &[Duration]) -> Value {
    json!({
        "samples": samples.len(),
        "p50_ms": percentile(samples.to_vec(), 1, 2),
        "p95_ms": percentile(samples.to_vec(), 95, 100),
    })
}

#[test]
#[ignore = "opt-in performance measurement; timing is not a CI assertion"]
fn many_rules_checks_repeated_files_and_large_files_report_stage_costs() {
    let (_temp, sources) = fixture();
    let root = _temp.path();
    let reviewed: BTreeSet<_> = sources.iter().map(|(id, _)| id.clone()).collect();
    let registry = Registry(root.to_path_buf());
    let preferences = Preferences;
    let handoffs = Handoffs;
    let worktrees = Worktrees(reviewed);
    let documents = Documents;
    let resolver = ContextResolver::new(
        "benchmark",
        &registry,
        &preferences,
        &handoffs,
        &worktrees,
        &documents,
    )
    .unwrap();
    let mut context_samples = Vec::new();
    let mut hook_samples = Vec::new();
    for sample in 0..SAMPLES + 2 {
        let started = Instant::now();
        let context = resolver.context(false).unwrap();
        let context_ms = started.elapsed();
        assert_eq!(context["contract_resolution"]["status"], "ready");
        assert_eq!(context["repository_rules"].as_array().unwrap().len(), RULES);
        let started = Instant::now();
        let briefing = render("benchmark", &context, Moment::Start);
        let hook_ms = context_ms + started.elapsed();
        assert!(briefing.len() <= 15 * 1024);
        assert_eq!(
            briefing
                .lines()
                .filter(|line| line.starts_with("- `") && line.contains("text:MCP"))
                .count(),
            RULES
        );
        assert!((0..RULES).all(|index| briefing.contains(&format!("benchmark-{index}"))));
        assert!(!briefing.contains("Follow the reviewed benchmark contract."));
        if sample >= 2 {
            context_samples.push(context_ms);
            hook_samples.push(hook_ms);
        }
    }

    let started = Instant::now();
    let snapshot = Snapshot::read(root);
    let read_ms = started.elapsed();
    assert_eq!(snapshot.status(), "ready");
    let started = Instant::now();
    let parsed: Vec<_> = sources
        .iter()
        .map(|(id, raw)| parse(id, raw).unwrap())
        .collect();
    let parse_ms = started.elapsed();
    let started = Instant::now();
    let mut cache = CheckCache::new(root);
    for rule in &parsed {
        assert_eq!(
            evaluate_cached(&rule.checks, &mut cache)["status"],
            "verified"
        );
    }
    let check_ms = started.elapsed();
    let stage_io = cache.stats();
    let context = resolver.context(false).unwrap();
    let started = Instant::now();
    let briefing = render("benchmark", &context, Moment::Start);
    let briefing_ms = started.elapsed();
    assert!(!briefing.is_empty());

    println!(
        "{}",
        json!({
            "fixture": {"rules":RULES,"checks_per_rule":CHECKS,"unique_checked_files":4,"largest_checked_bytes":MAX_CHECKED_BYTES},
            "context":summary(&context_samples),
            "hook":summary(&hook_samples),
            "stages_once": {"rule_read_ms":read_ms.as_secs_f64()*1000.0,"parse_ms":parse_ms.as_secs_f64()*1000.0,"checks_ms":check_ms.as_secs_f64()*1000.0,"briefing_ms":briefing_ms.as_secs_f64()*1000.0},
            "check_io": {"context":context["contract_resolution"]["check_io"],"isolated_checks":stage_io}
        })
    );
}
