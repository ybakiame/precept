use super::*;

impl Service {
    pub(super) fn query(
        &self,
        authority: &precept_ipc::peer::Peer,
        request: Request,
    ) -> precept_ipc::Result<Value> {
        let access = authority.access();
        let params = request.params;
        if request.method == "project.mcp" {
            if access != Access::Operator {
                return Err(Error::Unauthorized);
            }
            return self.mcp_launch(params);
        }
        if request.method.starts_with("profile.")
            || matches!(
                request.method.as_str(),
                "project.check"
                    | "project.add"
                    | "project.register"
                    | "project.remove"
                    | "project.agents"
                    | "project.propose"
                    | "project.gate"
            )
        {
            return self
                .profiles
                .handle(authority, &request.method, params, self);
        }
        // A work session becomes a Handoff: the operator's write, through the
        // ordinary reviewed write path, with the repository snapshot taken
        // here (never supplied by the caller).
        if request.method == "sessions.handoff" {
            return sessions::api::handoff(self, authority, params);
        }
        if matches!(access, Access::OperatorWrite { .. }) {
            return Err(Error::Unauthorized);
        }
        if request.method == "daemon.health" {
            let _: Empty = decode(params)?;
            let selected = self.profiles.selected();
            if !selected && access != Access::Operator {
                return Err(Error::Unauthorized);
            }
            let schema = if selected {
                format!(
                    "{:?}",
                    self.profiles
                        .read()?
                        .0
                        .schema()
                        .map_err(|_| Error::Unavailable)?
                )
            } else {
                "Unselected".into()
            };
            return Ok(
                json!({"api_version":precept_ipc::API_VERSION,"service":"preceptd","version":env!("CARGO_PKG_VERSION"),"schema":schema,"ready":true,"profile_selected":selected,"worktree_fingerprint":"on-demand-acp-worktree-v1"}),
            );
        }
        // The agent activity log is the daemon's own memory: no Profile needed.
        if request.method == "activity.list" {
            if access != Access::Operator {
                return Err(Error::Unauthorized);
            }
            let p: ActivityList = decode(params)?;
            if p.project
                .as_deref()
                .is_some_and(|n| !precept_ipc::valid_project(n))
            {
                return Err(Error::InvalidRequest);
            }
            let Some(project) = p.project else {
                return Ok(self.activity.list(None, p.before));
            };
            // A project's activity is its persisted evidence chain; refused
            // handshakes (no project known) stay in memory and are merged in.
            let state = self.profiles.state_dir();
            let (entries, more) = self
                .evidence
                .list(state, &project, p.before, activity::PAGE)?;
            let chain = self.evidence.status(state, &project)?;
            let memory = self.activity.list(Some(&project), None);
            let mut events: Vec<Value> = entries
                .into_iter()
                .map(|b| {
                    json!({"id":b.seq,"at_ms":b.at_ms,"kind":b.kind,"via":b.via,"project":project,
                        "client":b.client,"session":b.session,"outcome":b.outcome,
                        "duration_ms":b.duration_ms,"counts":b.counts,"rule":b.rule})
                })
                .collect();
            if p.before.is_none() {
                events.extend(
                    memory["events"]
                        .as_array()
                        .into_iter()
                        .flatten()
                        .filter(|e| e["project"].is_null())
                        .take(20)
                        .cloned(),
                );
                events.sort_by_key(|e| std::cmp::Reverse(e["at_ms"].as_u64().unwrap_or(0)));
            }
            return Ok(json!({
                "daemon_started_ms": memory["daemon_started_ms"],
                "events": events,
                "more": more,
                "chain": chain,
            }));
        }
        // The AGENTS.md pointer of a served project: status, and an explicit
        // add/remove guarded by the previewed file digest.
        if matches!(
            request.method.as_str(),
            "integration.pointer" | "integration.pointer.apply"
        ) {
            if access != Access::Operator {
                return Err(Error::Unauthorized);
            }
            #[derive(Deserialize)]
            #[serde(deny_unknown_fields)]
            struct Pointer {
                project: String,
                #[serde(default)]
                enabled: Option<bool>,
                #[serde(default)]
                expected_sha256: Option<String>,
            }
            let p: Pointer = decode(params)?;
            if !self.profiles.allowed(&p.project) {
                return Err(Error::NotFound);
            }
            let (_, roots) = self.profiles.read()?;
            let root = roots.get(&p.project).ok_or(Error::NotFound)?;
            return match (request.method.as_str(), p.enabled) {
                ("integration.pointer", None) if p.expected_sha256.is_none() => {
                    integration::status(root)
                }
                ("integration.pointer.apply", Some(enabled)) => {
                    if p.expected_sha256
                        .as_ref()
                        .is_some_and(|d| d.len() != 64 || !d.bytes().all(|b| b.is_ascii_hexdigit()))
                    {
                        return Err(Error::InvalidRequest);
                    }
                    integration::apply(root, enabled, p.expected_sha256.as_deref())
                }
                _ => Err(Error::InvalidRequest),
            };
        }
        // A confirmed project convention written as a repository rule file:
        // a preview, then an explicit write guarded by the previewed digest.
        if matches!(
            request.method.as_str(),
            "rules.export" | "rules.export.apply"
        ) {
            if access != Access::Operator {
                return Err(Error::Unauthorized);
            }
            #[derive(Deserialize)]
            #[serde(deny_unknown_fields)]
            struct Export {
                project: String,
                id: u64,
                #[serde(default)]
                enabled: Option<bool>,
                #[serde(default)]
                expected_sha256: Option<String>,
            }
            let p: Export = decode(params)?;
            if !self.profiles.allowed(&p.project) {
                return Err(Error::NotFound);
            }
            let (store, roots) = self.profiles.read()?;
            let root = roots.get(&p.project).ok_or(Error::NotFound)?;
            let prefs = precept_core::PreferenceRepository::preferences(&store, &p.project)
                .map_err(|_| Error::Unavailable)?;
            let convention = rule_export::convention(prefs, &p.project, p.id)?;
            if p.expected_sha256
                .as_ref()
                .is_some_and(|d| d.len() != 64 || !d.bytes().all(|b| b.is_ascii_hexdigit()))
            {
                return Err(Error::InvalidRequest);
            }
            return match (request.method.as_str(), p.enabled) {
                ("rules.export", None) if p.expected_sha256.is_none() => {
                    rule_export::preview(root, &convention)
                }
                ("rules.export.apply", Some(enabled)) => {
                    rule_export::apply(root, &convention, enabled, p.expected_sha256.as_deref())
                }
                _ => Err(Error::InvalidRequest),
            };
        }
        if matches!(
            request.method.as_str(),
            "integration.hooks" | "integration.hooks.apply"
        ) {
            if access != Access::Operator {
                return Err(Error::Unauthorized);
            }
            return self.hooks(&request.method, params);
        }
        // What the registered project's known agent rule files would suggest:
        // a bounded read in the served root; nothing is written here.
        if request.method == "import.scan" {
            if access != Access::Operator {
                return Err(Error::Unauthorized);
            }
            #[derive(Deserialize)]
            #[serde(deny_unknown_fields)]
            struct P {
                project: String,
            }
            let p: P = decode(params)?;
            if !self.profiles.allowed(&p.project) {
                return Err(Error::NotFound);
            }
            let (store, roots) = self.profiles.read()?;
            let root = roots.get(&p.project).ok_or(Error::NotFound)?;
            let (candidates, skipped) = importer::scan(root);
            let candidates: Vec<Value> = candidates
                .into_iter()
                .map(|c| {
                    // AGENTS.md sections are this project's conventions.
                    let scope = Scope::Project {
                        name: p.project.clone(),
                    };
                    // Already in the Profile (suggested or confirmed) as is.
                    let exists = store
                        .agent_suggestion_state(&scope, &c.key, &c.value)
                        .map(|(duplicate, _)| duplicate)
                        .unwrap_or(false);
                    let mut v = serde_json::to_value(&c).unwrap_or(Value::Null);
                    v["scope"] = serde_json::to_value(&scope).unwrap_or(Value::Null);
                    v["exists"] = json!(exists);
                    v
                })
                .collect();
            let reply = json!({"candidates": candidates, "skipped": skipped});
            if !serde_json::to_vec(&reply)
                .is_ok_and(|bytes| bytes.len() <= precept_core::import::MAX_REPLY_BYTES)
            {
                return Err(Error::Unavailable);
            }
            return Ok(reply);
        }
        if matches!(
            request.method.as_str(),
            "integration.mcp" | "integration.mcp.apply"
        ) {
            if access != Access::Operator {
                return Err(Error::Unauthorized);
            }
            return self.mcp_entries(&request.method, params);
        }
        // The daemon's own project set needs no Profile: an operator connecting
        // before creating or selecting one must still get a successful read.
        if request.method == "project.list" {
            let _: Empty = decode(params)?;
            if access != Access::Operator {
                return Err(Error::Unauthorized);
            }
            return Ok(self.profiles.list());
        }
        // Work sessions: an agent records its own goal and work state (needs
        // the operator's permission for agent writes); the operator lists and
        // deletes them.
        if matches!(
            request.method.as_str(),
            "session.start" | "session.get" | "session.update" | "session.close"
        ) {
            return sessions::api::handle(self, authority, &request.method, params);
        }
        // Project memory: agents write and recall their own notes (with the
        // operator's permission for agent writes); the operator reads, edits,
        // pins, deletes, restores and clears them.
        if request.method.starts_with("memory.") {
            let Access::Agent { project } = access else {
                return Err(Error::Unauthorized);
            };
            if !self.profiles.can_propose(&project) {
                return Err(Error::Unauthorized);
            }
            #[derive(Deserialize)]
            #[serde(deny_unknown_fields)]
            struct P {
                #[serde(default)]
                kind: Option<memory::Kind>,
                #[serde(default)]
                title: Option<String>,
                #[serde(default)]
                body: Option<String>,
                #[serde(default)]
                id: Option<String>,
                #[serde(default)]
                query: Option<String>,
                #[serde(default)]
                limit: Option<usize>,
                #[serde(default)]
                index: Option<bool>,
                #[serde(default)]
                basis: Option<String>,
                #[serde(default)]
                client: Option<String>,
            }
            let p: P = decode(params)?;
            let state = self.profiles.state_dir();
            let now = memory::now_ms();
            return match request.method.as_str() {
                "memory.remember" => {
                    let (kind, title, body) = match (p.kind, p.title, p.body) {
                        (Some(k), Some(t), Some(b)) => (k, t, b),
                        _ => return Err(Error::InvalidRequest),
                    };
                    let by = p.client.as_deref().and_then(activity::client_label);
                    let (entry, what) = self
                        .memory
                        .remember(state, &project, kind, &title, &body, by, None)?;
                    let entry = self.promote(&project, entry);
                    Ok(json!({"status": what.as_str(), "entry": entry.view(now)}))
                }
                "memory.reaffirm" => {
                    let id = p.id.ok_or(Error::InvalidRequest)?;
                    let entry = self
                        .memory
                        .reaffirm(state, &project, &id, p.basis.as_deref())?;
                    let entry = self.promote(&project, entry);
                    Ok(json!({"entry": entry.view(now)}))
                }
                "memory.forget" => {
                    let id = p.id.ok_or(Error::InvalidRequest)?;
                    self.memory.forget(state, &project, &id)?;
                    Ok(json!({"forgotten": id}))
                }
                "memory.recall" => {
                    let entries = self.memory.recall(
                        state,
                        &project,
                        p.query.as_deref(),
                        p.kind,
                        p.limit.unwrap_or(8),
                    )?;
                    if p.index.unwrap_or(false) {
                        let lines: Vec<_> = entries
                            .iter()
                            .map(|e| {
                                json!({
                                    "id": e.id, "kind": e.kind, "title": e.title,
                                    "weight": (e.weight(now) * 10.0).round() / 10.0,
                                })
                            })
                            .collect();
                        Ok(json!({"entries": lines, "index": true}))
                    } else {
                        Ok(
                            json!({"entries": entries.iter().map(|e| e.view(now)).collect::<Vec<_>>()}),
                        )
                    }
                }
                "memory.get" => {
                    let id = p.id.ok_or(Error::InvalidRequest)?;
                    let entry = self.memory.get(state, &project, &id)?;
                    Ok(json!({"entry": entry.view(now)}))
                }
                _ => Err(Error::Unsupported),
            };
        }
        if request.method.starts_with("memories.") {
            if access != Access::Operator {
                return Err(Error::Unauthorized);
            }
            #[derive(Deserialize)]
            #[serde(deny_unknown_fields)]
            struct P {
                project: String,
                #[serde(default)]
                id: Option<String>,
                #[serde(default)]
                title: Option<String>,
                #[serde(default)]
                body: Option<String>,
                #[serde(default)]
                pinned: Option<bool>,
                #[serde(default)]
                index: Option<usize>,
                #[serde(default)]
                entries: Option<Vec<ImportedNote>>,
            }
            /// A note carried by a Profile export.
            #[derive(Deserialize)]
            #[serde(deny_unknown_fields)]
            struct ImportedNote {
                kind: memory::Kind,
                title: String,
                body: String,
                #[serde(default)]
                pinned: bool,
            }
            let p: P = decode(params)?;
            if !precept_ipc::valid_project(&p.project) || !self.profiles.listed(&p.project) {
                return Err(Error::NotFound);
            }
            let state = self.profiles.state_dir();
            let now = memory::now_ms();
            let id = || p.id.clone().ok_or(Error::InvalidRequest);
            return match request.method.as_str() {
                "memories.list" => {
                    let entries = self.memory.list(state, &p.project)?;
                    Ok(json!({"entries": entries.iter().map(|e| e.view(now)).collect::<Vec<_>>()}))
                }
                "memories.edit" => Ok(json!({"entry": self
                    .memory
                    .edit(
                        state,
                        &p.project,
                        &id()?,
                        p.title.as_deref(),
                        p.body.as_deref(),
                        p.pinned
                    )?
                    .view(now)})),
                "memories.restore" => Ok(json!({"entry": self
                    .memory
                    .restore(state, &p.project, &id()?, p.index.ok_or(Error::InvalidRequest)?)?
                    .view(now)})),
                "memories.delete" => {
                    let id = id()?;
                    self.memory.forget(state, &p.project, &id)?;
                    Ok(json!({"deleted": id}))
                }
                "memories.clear" => Ok(json!({"cleared": self.memory.clear(state, &p.project)?})),
                // Notes from a Profile export: each kept as `remember` would
                // (the same kind and title updates that note), pins kept.
                "memories.import" => {
                    let entries = p.entries.ok_or(Error::InvalidRequest)?;
                    if entries.len() > memory::MAX_ENTRIES {
                        return Err(Error::InvalidRequest);
                    }
                    let mut counts = serde_json::Map::new();
                    for e in entries {
                        let (entry, what) = self.memory.remember(
                            state,
                            &p.project,
                            e.kind,
                            &e.title,
                            &e.body,
                            Some("import".into()),
                            None,
                        )?;
                        if e.pinned && !entry.pinned {
                            self.memory.edit(
                                state,
                                &p.project,
                                &entry.id,
                                None,
                                None,
                                Some(true),
                            )?;
                        }
                        let n = counts.entry(what.as_str()).or_insert(json!(0));
                        *n = json!(n.as_u64().unwrap_or(0) + 1);
                    }
                    Ok(Value::Object(counts))
                }
                _ => Err(Error::Unsupported),
            };
        }
        if matches!(request.method.as_str(), "sessions.list" | "sessions.delete") {
            if access != Access::Operator {
                return Err(Error::Unauthorized);
            }
            return sessions::api::handle_operator(self, &request.method, params);
        }
        // Where an agent capability's own project lives: precept-mcp asks
        // each published capability to find the one for its session's folder.
        if request.method == "project.root" {
            let Access::Agent { project } = access else {
                return Err(Error::Unauthorized);
            };
            let _: Empty = decode(params)?;
            if !self.profiles.allowed(&project) {
                return Err(Error::NotFound);
            }
            let (_, roots) = self.profiles.read()?;
            let root = roots.get(&project).ok_or(Error::NotFound)?;
            return Ok(json!({"project": project, "root": root}));
        }
        // The pre-tool gate: a harness hook asks whether a tool call breaks a
        // committed hard rule's guard. Off unless the operator turned it on.
        if request.method == "gate.check" {
            let Access::Agent { project } = access else {
                return Err(Error::Unauthorized);
            };
            #[derive(Deserialize)]
            #[serde(deny_unknown_fields)]
            struct Gate {
                tool: String,
                #[serde(default)]
                command: Option<String>,
                #[serde(default)]
                paths: Vec<String>,
                #[serde(default)]
                client: Option<String>,
            }
            let g: Gate = decode(params)?;
            let _ = g.client;
            use precept_core::contracts::gate::{ToolCall, check};
            let call = match (g.tool.as_str(), g.command) {
                ("shell", Some(c)) if g.paths.is_empty() => ToolCall::Shell(c),
                ("write", None) => ToolCall::Write(g.paths),
                _ => return Err(Error::InvalidRequest),
            };
            if !call.valid() {
                return Err(Error::InvalidRequest);
            }
            if !self.profiles.gated(&project) {
                return Ok(json!({"decision":"allow","gate":"off"}));
            }
            let (_, roots) = self.profiles.read()?;
            let root = roots.get(&project).ok_or(Error::NotFound)?.clone();
            let bound = precept_core::Project {
                name: project.clone(),
                root: root.clone(),
            };
            let snapshot = precept_core::contracts::Snapshot::read(&root).with_reviewed(
                precept_core::WorktreeInspector::reviewed(
                    &worktree::GitInspector,
                    &bound,
                    precept_core::contracts::DIR,
                ),
            );
            return Ok(match check(&snapshot, &call) {
                Some(d) => {
                    // This denial and the earlier ones of the same rule in
                    // the last day: a repeat tells the agent to stop trying
                    // variations.
                    let earlier = self
                        .evidence
                        .denials(
                            self.profiles.state_dir(),
                            &project,
                            activity::now().saturating_sub(REPEAT_WINDOW_MS),
                        )
                        .unwrap_or_default()
                        .into_iter()
                        .find(|x| x.rule == d.key)
                        .map_or(0, |x| x.count);
                    let repeat = earlier + 1;
                    let mut reason = d.reason.clone();
                    if repeat >= 2 {
                        reason.push_str(&format!(
                            "\n\nThis rule has now blocked {repeat} tool calls in this project in the last 24 hours. Do not retry variations of the same call: follow the rule, or stop and ask the developer."
                        ));
                    }
                    json!({"decision":"deny","reason":reason,"repeat":repeat,
                        "rule":{"id":d.id,"key":d.key,"file":d.file}})
                }
                None => json!({"decision":"allow","gate":"on"}),
            });
        }
        // Claims of committed rules about files an agent reads that do not
        // hold now: the read hook warns that such a document may be stale.
        if request.method == "doc.check" {
            let Access::Agent { project } = access else {
                return Err(Error::Unauthorized);
            };
            #[derive(Deserialize)]
            #[serde(deny_unknown_fields)]
            struct Doc {
                paths: Vec<String>,
                #[serde(default)]
                client: Option<String>,
            }
            let d: Doc = decode(params)?;
            let _ = d.client;
            if d.paths.is_empty()
                || d.paths.len() > 32
                || d.paths.iter().any(|p| {
                    p.is_empty()
                        || p.len() > 512
                        || p.starts_with('/')
                        || p.split('/').any(|c| c == ".." || c.is_empty())
                })
            {
                return Err(Error::InvalidRequest);
            }
            let (_, roots) = self.profiles.read()?;
            let root = roots.get(&project).ok_or(Error::NotFound)?.clone();
            let bound = precept_core::Project {
                name: project.clone(),
                root: root.clone(),
            };
            let snapshot = precept_core::contracts::Snapshot::read(&root).with_reviewed(
                precept_core::WorktreeInspector::reviewed(
                    &worktree::GitInspector,
                    &bound,
                    precept_core::contracts::DIR,
                ),
            );
            let stale = precept_core::contracts::gate::stale(&snapshot, &d.paths);
            return Ok(json!({"stale": stale}));
        }
        if request.method == "preference.suggest" {
            let Access::Agent { project } = access else {
                return Err(Error::Unauthorized);
            };
            let p: AgentSuggest = decode(params)?;
            let scope = match p.scope.as_str() {
                "project" => Scope::Project {
                    name: project.clone(),
                },
                "personal" => Scope::Global,
                _ => return Err(Error::InvalidRequest),
            };
            // An update of a confirmed rule keeps that rule's key, and its tags
            // and options unless the agent names new ones.
            let target = match p.replaces {
                None => None,
                Some(id) if id > 0 => Some(self.profiles.confirmed_record(&project, &scope, id)?),
                Some(_) => return Err(Error::InvalidRequest),
            };
            let key = target.as_ref().map_or(p.key, |t| t.key.clone());
            // Every record carries a tag; an untagged suggestion is filed under `agent`.
            let tags = match (&target, p.tags.is_empty()) {
                (Some(t), true) => t.tags.clone(),
                (None, true) => vec!["agent".to_owned()],
                _ => p.tags,
            };
            let tags = precept_ipc::operator::canonical_tags(&tags).ok_or(Error::InvalidRequest)?;
            let enforcement = match p.enforcement.as_deref() {
                None => target
                    .as_ref()
                    .map(|t| t.rule.enforcement)
                    .unwrap_or_default(),
                Some(e) => {
                    precept_core::operator::Enforcement::parse(e).ok_or(Error::InvalidRequest)?
                }
            };
            let applies_to = match &target {
                Some(t) if p.applies_to.is_empty() => t.rule.applies_to.clone(),
                _ => p.applies_to.canonical().ok_or(Error::InvalidRequest)?,
            };
            let rule = precept_core::operator::RuleOptions {
                applies_to,
                priority: target.as_ref().map_or(0, |t| t.rule.priority),
                enforcement,
            };
            if !rule.valid() {
                return Err(Error::InvalidRequest);
            }
            return self
                .profiles
                .agent_suggest(&project, scope, key, p.value, tags, rule);
        }
        let (store, roots) = self.profiles.read()?;
        let explaining = request.method == "context.explain";
        let (project, include, task, selector, key) = match (request.method.as_str(), access) {
            ("context.get" | "context.explain", Access::Operator) => {
                let p: OperatorContext = decode(params)?;
                (p.project, p.include_documents, None, p.selector, p.key)
            }
            ("context.get" | "context.explain", Access::Agent { project }) => {
                let p: AgentContext = decode(params)?;
                (project, p.include_documents, None, p.selector, p.key)
            }
            ("handoff.get", Access::Operator) => {
                let p: OperatorHandoff = decode(params)?;
                (p.project, false, Some(p.task_id), Default::default(), None)
            }
            ("handoff.get", Access::Agent { project }) => {
                let p: AgentHandoff = decode(params)?;
                (project, false, Some(p.task_id), Default::default(), None)
            }
            _ => return Err(Error::Unsupported),
        };
        if !self.profiles.allowed(&project) {
            return Err(Error::Unauthorized);
        }
        selector.validate().map_err(|_| Error::InvalidRequest)?;
        if task.as_ref().is_some_and(|s| s.chars().count() > 120)
            || explaining != key.is_some()
            || (explaining && include)
        {
            return Err(Error::InvalidRequest);
        }
        let root = store
            .read_project(&project)
            .ok()
            .and_then(|p| p.root.canonicalize().ok());
        if root.as_ref() != roots.get(&project) {
            return Err(Error::UnsafeRuntime);
        }
        let resolver = ContextResolver::new(
            &project,
            &store,
            &store,
            &store,
            &worktree::GitInspector,
            &FileDocuments,
        )
        .map_err(|_| Error::NotFound)?;
        let result = if let Some(task) = task {
            resolver.handoff(&task)
        } else if let Some(key) = key {
            // Why each rule of the key reaches this request or not; the same
            // projection agents receive.
            return resolver.explain(&key, &selector).map_err(|e| match e {
                precept_core::Error::InvalidArguments => Error::InvalidRequest,
                _ => Error::Unavailable,
            });
        } else {
            let prepared = resolver
                .prepare_context(include, &selector)
                .map_err(|_| Error::Unavailable)?;
            #[cfg(test)]
            process_tests::before_context_finish();
            // Recheck the selected/authorized root after assembly, then let the
            // core re-read the bounded source. Lease publication checks follow.
            let current = store
                .read_project(&project)
                .ok()
                .and_then(|p| p.root.canonicalize().ok());
            if current.as_ref() != roots.get(&project) {
                return Err(Error::UnsafeRuntime);
            }
            prepared.finish(current.as_ref().ok_or(Error::UnsafeRuntime)?)
        };
        let mut result = result.map_err(|e| match e {
            precept_core::Error::HandoffNotFound | precept_core::Error::UnknownProject => {
                Error::NotFound
            }
            _ => Error::Unavailable,
        })?;
        // Context carries the project's recent work sessions: where other
        // agents left the work (agents' own records, not reviewed rules).
        if request.method == "context.get" && result.is_object() {
            let work = self
                .sessions
                .for_context(self.profiles.state_dir(), &project);
            result["work_sessions"] = serde_json::to_value(work).unwrap_or(Value::Array(vec![]));
            // And the rules the gate denied lately, so the next agent does not
            // repeat what was blocked.
            result["gate_denials"] = serde_json::to_value(
                self.evidence
                    .denials(
                        self.profiles.state_dir(),
                        &project,
                        activity::now().saturating_sub(DENIALS_WINDOW_MS),
                    )
                    .unwrap_or_default()
                    .into_iter()
                    .take(5)
                    .collect::<Vec<_>>(),
            )
            .unwrap_or(Value::Array(vec![]));
            // And the project's memory: agents' notes, unreviewed, weighted.
            result["memory"] = self.memory.for_context(self.profiles.state_dir(), &project);
        }
        Ok(result)
    }
}
