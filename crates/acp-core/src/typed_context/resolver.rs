use super::types::*;
use std::collections::{BTreeMap, BTreeSet};

fn canonical<T: Ord>(values: &mut Vec<T>) {
    values.sort();
    values.dedup();
}
fn label(s: &str) -> bool {
    !s.is_empty() && s.len() <= 1024 && s.trim() == s && !s.chars().any(char::is_control)
}
fn value(s: &str) -> bool {
    !s.is_empty() && s.len() <= 4096 && !s.chars().any(char::is_control)
}
fn path(s: &str) -> bool {
    label(s)
        && !s.contains(['\\', ':', '*', '?', '[', ']', '{', '}'])
        && s.split('/').all(|part| !matches!(part, "" | "." | ".."))
}
fn binding(b: &ProjectBinding) -> bool {
    label(&b.project) && label(&b.root_id)
}
fn reference(r: &RuleRef) -> bool {
    label(&r.id) && r.revision > 0
}
fn values(v: &[String]) -> bool {
    !v.is_empty() && v.len() <= 64 && v.iter().all(|s| value(s))
}
fn normalize_source(source: &mut Source) {
    canonical(&mut source.provenance.references);
    canonical(&mut source.provenance.historical_validation);
}
fn source_valid(source: &Source) -> bool {
    label(&source.reference.id)
        && label(&source.reference.version)
        && source.reference.digest.as_deref().is_none_or(label)
        && source.provenance.references.iter().all(|s| label(s))
        && source
            .provenance
            .historical_validation
            .iter()
            .all(|c| label(&c.reference) && c.revision.as_deref().is_none_or(label))
}
fn normalize_selector(s: &mut Selector) {
    if let Some(p) = &mut s.projects {
        canonical(p);
    }
    for list in [
        &mut s.agents,
        &mut s.paths,
        &mut s.languages,
        &mut s.task_tags,
    ]
    .into_iter()
    .flatten()
    {
        canonical(list);
    }
}
fn normalize_rule(r: &mut Rule) {
    canonical(&mut r.allowed_values);
    canonical(&mut r.supersedes);
    normalize_selector(&mut r.selector);
    normalize_source(&mut r.source);
}
fn in_scope(scope: &Scope, query: &Query) -> bool {
    match scope {
        Scope::Global => true,
        Scope::Project { binding } => binding == &query.binding,
        Scope::Session {
            binding,
            session_id,
        } => binding == &query.binding && query.session_id.as_ref().is_none_or(|s| s == session_id),
    }
}
fn reviewed(r: &Rule) -> bool {
    r.review
        == match r.kind {
            RuleKind::Contract => ReviewState::Accepted,
            _ => ReviewState::Confirmed,
        }
}
fn shape(r: &Rule) -> bool {
    let kind_shape = match (&r.kind, &r.scope, &r.authority) {
        (RuleKind::Preference, Scope::Global | Scope::Project { .. }, Authority::Personal) => {
            r.allowed_values.len() == 1 && r.legacy_preference_id.is_some_and(|id| id > 0)
        }
        (RuleKind::Contract, Scope::Project { .. }, Authority::Repository) => {
            r.legacy_preference_id.is_none()
        }
        (RuleKind::Constraint, Scope::Session { session_id, .. }, Authority::SessionNarrowing) => {
            r.legacy_preference_id.is_none() && label(session_id)
        }
        _ => false,
    };
    kind_shape
        && label(&r.key)
        && values(&r.allowed_values)
        && r.supersedes
            .iter()
            .all(|s| reference(s) && s.id == r.reference.id && s.revision < r.reference.revision)
}
fn selector_valid(s: &Selector, registered: &[ProjectBinding]) -> bool {
    s.projects
        .as_ref()
        .is_none_or(|p| !p.is_empty() && p.iter().all(|b| registered.contains(b)))
        && [&s.agents, &s.languages, &s.task_tags]
            .into_iter()
            .all(|list| {
                list.as_ref()
                    .is_none_or(|v| !v.is_empty() && v.iter().all(|s| label(s)))
            })
        && s.paths
            .as_ref()
            .is_none_or(|v| !v.is_empty() && v.iter().all(|s| path(s)))
}
fn dimension<T: PartialEq>(wanted: &Option<Vec<T>>, actual: Option<&T>) -> Applicability {
    match (wanted, actual) {
        (None, _) => Applicability::Match,
        (Some(_), None) => Applicability::Unknown,
        (Some(v), Some(a)) if v.contains(a) => Applicability::Match,
        _ => Applicability::NoMatch,
    }
}
fn applicability(r: &Rule, q: &Query, registered: &[ProjectBinding]) -> Applicability {
    if !selector_valid(&r.selector, registered) {
        return Applicability::Invalid;
    }
    let mut states = vec![
        dimension(&r.selector.projects, Some(&q.binding)),
        dimension(&r.selector.agents, q.agent_id.as_ref()),
        dimension(&r.selector.paths, q.path.as_ref()),
        dimension(&r.selector.languages, q.language.as_ref()),
    ];
    states.push(match (&r.selector.task_tags, &q.task_tags) {
        (None, _) => Applicability::Match,
        (Some(_), None) => Applicability::Unknown,
        (Some(wanted), Some(actual)) if wanted.iter().any(|tag| actual.contains(tag)) => {
            Applicability::Match
        }
        _ => Applicability::NoMatch,
    });
    if matches!(r.scope, Scope::Session { .. }) && q.session_id.is_none() {
        states.push(Applicability::Unknown);
    }
    if states.contains(&Applicability::NoMatch) {
        Applicability::NoMatch
    } else if states.contains(&Applicability::Unknown) {
        Applicability::Unknown
    } else {
        Applicability::Match
    }
}
fn source_issue(s: &Source) -> Option<(Reason, ConflictCategory, ReviewAction)> {
    use {ConflictCategory as C, Reason as R, ReviewAction as A};
    if !source_valid(s) {
        return Some((R::InvalidSource, C::InvalidSourceSchema, A::RepairInput));
    }
    match s.state {
        SourceState::Fresh => None,
        SourceState::Stale => Some((R::StaleSource, C::StaleSourceSnapshot, A::RefreshSource)),
        SourceState::Unavailable => {
            Some((R::UnavailableSource, C::SourceUnavailable, A::RefreshSource))
        }
        SourceState::Unknown => Some((R::UnknownSource, C::SourceUnknown, A::RefreshSource)),
        _ => Some((R::InvalidSource, C::InvalidSourceSchema, A::RepairInput)),
    }
}
fn observation_issue(s: &Source) -> Option<(Reason, ConflictCategory, ReviewAction)> {
    source_issue(s).or_else(|| {
        matches!(
            s.provenance.origin,
            Origin::AgentReported | Origin::LegacyUnknown
        )
        .then_some((
            Reason::UnverifiedObservation,
            ConflictCategory::SourceUnknown,
            ReviewAction::InspectRepository,
        ))
    })
}
fn fact_issue(fact: &RepositoryFact) -> Option<(Reason, ConflictCategory, ReviewAction)> {
    if !label(&fact.key) || !value(&fact.value) {
        Some((
            Reason::InvalidSource,
            ConflictCategory::InvalidSourceSchema,
            ReviewAction::RepairInput,
        ))
    } else {
        observation_issue(&fact.source)
    }
}
fn policy_issue(policy: &PolicyBoundary) -> Option<(Reason, ConflictCategory, ReviewAction)> {
    if !label(&policy.key) || !values(&policy.allowed_values) {
        Some((
            Reason::InvalidSource,
            ConflictCategory::InvalidSourceSchema,
            ReviewAction::RepairInput,
        ))
    } else {
        observation_issue(&policy.source)
    }
}
fn add_conflict(
    rows: &[RuleExplanation],
    indices: &[usize],
    category: ConflictCategory,
    status: ConflictStatus,
    next_action: ReviewAction,
    extra: &[SourceRef],
    conflicts: &mut Vec<Conflict>,
) {
    if let Some(&first) = indices.first() {
        let mut rules = indices
            .iter()
            .map(|&i| rows[i].rule.reference.clone())
            .collect();
        let mut sources: Vec<_> = indices
            .iter()
            .map(|&i| rows[i].rule.source.reference.clone())
            .chain(extra.iter().cloned())
            .collect();
        canonical(&mut rules);
        canonical(&mut sources);
        conflicts.push(Conflict {
            key: rows[first].rule.key.clone(),
            category,
            status,
            rules,
            sources,
            next_action,
        });
    }
}
fn mask(row: &mut RuleExplanation, reason: Reason) {
    if row.outcome == Outcome::Selected {
        row.outcome = Outcome::Masked;
        row.reasons.push(reason);
    }
}
fn selected(rows: &[RuleExplanation], group: &[usize], kind: RuleKind) -> Vec<usize> {
    group
        .iter()
        .copied()
        .filter(|&i| rows[i].outcome == Outcome::Selected && rows[i].rule.kind == kind)
        .collect()
}
fn intersection(rows: &[RuleExplanation], indices: &[usize]) -> Option<BTreeSet<String>> {
    indices
        .iter()
        .map(|&i| {
            rows[i]
                .rule
                .allowed_values
                .iter()
                .cloned()
                .collect::<BTreeSet<_>>()
        })
        .reduce(|a, b| a.intersection(&b).cloned().collect())
}
fn subset(v: &[String], limit: &BTreeSet<String>) -> bool {
    v.iter().all(|s| limit.contains(s))
}

/// Resolve only explicit values. Caller authorization and source inspection stay upstream.
pub fn resolve(input: &ResolutionInputV1) -> Result<EffectiveContextV1, InputError> {
    if input.schema_version != SCHEMA_VERSION {
        return Err(InputError::UnsupportedSchema);
    }
    let mut registered = input.registered_projects.clone();
    canonical(&mut registered);
    if registered.iter().any(|b| !binding(b))
        || registered.windows(2).any(|b| b[0].project == b[1].project)
    {
        return Err(InputError::InvalidRegistration);
    }
    let q = &input.query;
    if !registered.contains(&q.binding) {
        return Err(InputError::UnregisteredProject);
    }
    if [
        q.agent_id.as_deref(),
        q.language.as_deref(),
        q.session_id.as_deref(),
    ]
    .into_iter()
    .flatten()
    .any(|s| !label(s))
        || q.path.as_deref().is_some_and(|s| !path(s))
        || q.task_tags
            .as_ref()
            .is_some_and(|tags| tags.iter().any(|s| !label(s)))
    {
        return Err(InputError::InvalidQuery);
    }

    let mut rules: Vec<_> = input
        .rules
        .iter()
        .filter(|r| in_scope(&r.scope, q))
        .cloned()
        .collect();
    for rule in &mut rules {
        normalize_rule(rule);
    }
    canonical(&mut rules);
    let mut identity_counts = BTreeMap::new();
    for r in &rules {
        *identity_counts.entry(r.reference.clone()).or_insert(0usize) += 1;
    }
    // Sorting is explicit rather than derived field order (Rule starts with schema_version).
    rules.sort_by(|a, b| (&a.key, &a.reference, a).cmp(&(&b.key, &b.reference, b)));
    let mut rows = Vec::new();
    let mut conflicts = Vec::new();
    for rule in rules {
        let app = applicability(&rule, q, &registered);
        let invalid = if rule.schema_version != SCHEMA_VERSION {
            Some(Reason::InvalidSchema)
        } else if !reference(&rule.reference) || identity_counts[&rule.reference] > 1 {
            Some(Reason::InvalidIdentity)
        } else if !shape(&rule) {
            Some(Reason::InvalidRuleShape)
        } else if app == Applicability::Invalid {
            Some(Reason::InvalidSelector)
        } else {
            None
        };
        let mut row = RuleExplanation {
            rule,
            applicability: app,
            outcome: Outcome::Selected,
            reasons: vec![],
        };
        let issue = if !reviewed(&row.rule) {
            row.outcome = Outcome::Excluded;
            row.reasons.push(Reason::ReviewRequired);
            None
        } else if app == Applicability::NoMatch {
            row.outcome = Outcome::Excluded;
            row.reasons.push(Reason::SelectorMismatch);
            if let Some(reason) = invalid {
                row.reasons.push(reason);
            }
            None
        } else if let Some(reason) = invalid {
            row.outcome = Outcome::Excluded;
            Some((
                reason,
                ConflictCategory::InvalidSourceSchema,
                ReviewAction::RepairInput,
            ))
        } else if app == Applicability::Unknown {
            row.outcome = Outcome::Unknown;
            Some((
                Reason::MissingQuery,
                ConflictCategory::UnknownApplicability,
                ReviewAction::SupplyQuery,
            ))
        } else {
            source_issue(&row.rule.source)
        };
        if let Some((reason, category, action)) = issue {
            if category == ConflictCategory::InvalidSourceSchema {
                row.outcome = Outcome::Excluded;
            } else {
                row.outcome = Outcome::Unknown;
            }
            row.reasons.push(reason);
            rows.push(row);
            add_conflict(
                &rows,
                &[rows.len() - 1],
                category,
                ConflictStatus::Unresolved,
                action,
                &[],
                &mut conflicts,
            );
        } else {
            rows.push(row);
        }
        // Missing query evidence must not hide a separately stale/invalid source.
        if app == Applicability::Unknown && invalid.is_none() {
            let i = rows.len() - 1;
            if reviewed(&rows[i].rule)
                && let Some((reason, category, action)) = source_issue(&rows[i].rule.source)
            {
                rows[i].reasons.push(reason);
                if category == ConflictCategory::InvalidSourceSchema {
                    rows[i].outcome = Outcome::Excluded;
                }
                add_conflict(
                    &rows,
                    &[i],
                    category,
                    ConflictStatus::Unresolved,
                    action,
                    &[],
                    &mut conflicts,
                );
            }
        }
    }

    // An explicit new revision can retire an old one only within the same domain/source.
    let mut superseded = BTreeSet::new();
    let mut invalid_successors = BTreeSet::new();
    for (i, row) in rows
        .iter()
        .enumerate()
        .filter(|(_, r)| r.outcome == Outcome::Selected)
    {
        for target in &row.rule.supersedes {
            for (j, old) in rows
                .iter()
                .enumerate()
                .filter(|(_, r)| &r.rule.reference == target)
            {
                if old.rule.key != row.rule.key
                    || old.rule.kind != row.rule.kind
                    || old.rule.scope != row.rule.scope
                    || old.rule.authority != row.rule.authority
                    || old.rule.source.reference.id != row.rule.source.reference.id
                {
                    invalid_successors.insert(i);
                } else {
                    superseded.insert((i, j));
                }
            }
        }
    }
    for i in invalid_successors.iter().copied() {
        rows[i].outcome = Outcome::Excluded;
        rows[i].reasons.push(Reason::InvalidIdentity);
        add_conflict(
            &rows,
            &[i],
            ConflictCategory::InvalidSourceSchema,
            ConflictStatus::Unresolved,
            ReviewAction::RepairInput,
            &[],
            &mut conflicts,
        );
    }
    for (i, j) in superseded {
        if !invalid_successors.contains(&i) {
            mask(&mut rows[j], Reason::Superseded);
        }
    }

    let mut facts: Vec<_> = input
        .repository_facts
        .iter()
        .filter(|f| f.binding == q.binding)
        .cloned()
        .collect();
    for f in &mut facts {
        normalize_source(&mut f.source);
    }
    canonical(&mut facts);
    let mut boundaries: Vec<_> = input
        .policy_boundaries
        .iter()
        .filter(|p| p.binding == q.binding)
        .cloned()
        .collect();
    for p in &mut boundaries {
        normalize_source(&mut p.source);
        canonical(&mut p.allowed_values);
    }
    canonical(&mut boundaries);
    let mut groups: BTreeMap<String, Vec<usize>> = BTreeMap::new();
    for (i, row) in rows.iter().enumerate() {
        groups.entry(row.rule.key.clone()).or_default().push(i);
    }
    let observation_issues = facts
        .iter()
        .filter_map(|f| fact_issue(f).map(|issue| (&f.key, &f.source.reference, issue)))
        .chain(
            boundaries
                .iter()
                .filter_map(|p| policy_issue(p).map(|issue| (&p.key, &p.source.reference, issue))),
        );
    for (key, source, (_, category, action)) in observation_issues {
        if let Some(group) = groups.get(key) {
            add_conflict(
                &rows,
                group,
                category,
                ConflictStatus::Unresolved,
                action,
                std::slice::from_ref(source),
                &mut conflicts,
            );
        } else {
            conflicts.push(Conflict {
                key: key.clone(),
                category,
                status: ConflictStatus::Unresolved,
                rules: vec![],
                sources: vec![source.clone()],
                next_action: action,
            });
        }
    }
    let mut effective = Vec::new();
    for (key, group) in groups {
        // Competing revisions must be resolved explicitly, before the legacy preference tie-break.
        let mut revisions: BTreeMap<String, Vec<usize>> = BTreeMap::new();
        for &i in &group {
            if rows[i].outcome == Outcome::Selected {
                revisions
                    .entry(rows[i].rule.reference.id.clone())
                    .or_default()
                    .push(i);
            }
        }
        for same in revisions.values().filter(|v| v.len() > 1) {
            add_conflict(
                &rows,
                same,
                ConflictCategory::RuleVsRule,
                ConflictStatus::Unresolved,
                ReviewAction::ReviewRules,
                &[],
                &mut conflicts,
            );
        }
        let prefs = selected(&rows, &group, RuleKind::Preference);
        let has_project = prefs
            .iter()
            .any(|&i| matches!(rows[i].rule.scope, Scope::Project { .. }));
        for i in prefs {
            if has_project && rows[i].rule.scope == Scope::Global {
                mask(&mut rows[i], Reason::ProjectPreference);
            }
        }
        let prefs = selected(&rows, &group, RuleKind::Preference);
        let newest = prefs
            .iter()
            .filter_map(|&i| rows[i].rule.legacy_preference_id)
            .max();
        for i in prefs {
            if rows[i].rule.legacy_preference_id != newest {
                mask(&mut rows[i], Reason::NewerPreference);
            }
        }
        let prefs = selected(&rows, &group, RuleKind::Preference);
        if intersection(&rows, &prefs).is_some_and(|s| s.is_empty()) {
            add_conflict(
                &rows,
                &prefs,
                ConflictCategory::RuleVsRule,
                ConflictStatus::Unresolved,
                ReviewAction::ReviewRules,
                &[],
                &mut conflicts,
            );
        }
        let contracts = selected(&rows, &group, RuleKind::Contract);
        let contract_values = intersection(&rows, &contracts);
        if contract_values.as_ref().is_some_and(BTreeSet::is_empty) {
            add_conflict(
                &rows,
                &contracts,
                ConflictCategory::RuleVsRule,
                ConflictStatus::Unresolved,
                ReviewAction::ReviewRules,
                &[],
                &mut conflicts,
            );
        }
        let constraints = selected(&rows, &group, RuleKind::Constraint);
        for &i in &constraints {
            if contract_values
                .as_ref()
                .is_some_and(|limit| !subset(&rows[i].rule.allowed_values, limit))
            {
                let affected: Vec<_> = contracts.iter().copied().chain([i]).collect();
                add_conflict(
                    &rows,
                    &affected,
                    ConflictCategory::SessionVsContract,
                    ConflictStatus::Unresolved,
                    ReviewAction::ReviewConstraint,
                    &[],
                    &mut conflicts,
                );
            }
        }
        for policy in boundaries.iter().filter(|p| p.key == key) {
            if policy_issue(policy).is_none() {
                let limit: BTreeSet<_> = policy.allowed_values.iter().cloned().collect();
                for &i in &constraints {
                    if !subset(&rows[i].rule.allowed_values, &limit) {
                        add_conflict(
                            &rows,
                            &[i],
                            ConflictCategory::SessionVsPolicy,
                            ConflictStatus::Unresolved,
                            ReviewAction::ReviewConstraint,
                            std::slice::from_ref(&policy.source.reference),
                            &mut conflicts,
                        );
                    }
                }
            }
        }
        let constraint_values = intersection(&rows, &constraints);
        if constraint_values.as_ref().is_some_and(BTreeSet::is_empty) {
            add_conflict(
                &rows,
                &constraints,
                ConflictCategory::RuleVsRule,
                ConflictStatus::Unresolved,
                ReviewAction::ReviewRules,
                &[],
                &mut conflicts,
            );
        }
        let mut observed_values = BTreeSet::new();
        let mut observed_sources = Vec::new();
        for fact in facts.iter().filter(|f| f.key == key) {
            if fact_issue(fact).is_some() {
                continue;
            }
            observed_values.insert(fact.value.clone());
            observed_sources.push(fact.source.reference.clone());
            let normative: Vec<_> = contracts
                .iter()
                .chain(&constraints)
                .copied()
                .filter(|&i| !rows[i].rule.allowed_values.contains(&fact.value))
                .collect();
            if !normative.is_empty() {
                add_conflict(
                    &rows,
                    &normative,
                    ConflictCategory::RuleVsRepository,
                    ConflictStatus::Unresolved,
                    ReviewAction::InspectRepository,
                    std::slice::from_ref(&fact.source.reference),
                    &mut conflicts,
                );
            }
        }
        if observed_values.len() > 1 {
            add_conflict(
                &rows,
                &group,
                ConflictCategory::RuleVsRepository,
                ConflictStatus::Unresolved,
                ReviewAction::InspectRepository,
                &observed_sources,
                &mut conflicts,
            );
        }
        // Explanations must not claim resolution using authorities that are themselves
        // unresolved. Inspect the complete contract/constraint/policy/observation set
        // before masking any remaining preference; retain independent legacy fallback.
        if !conflicts
            .iter()
            .any(|c| c.key == key && c.status == ConflictStatus::Unresolved)
        {
            if let Some(limit) = contract_values.as_ref().filter(|v| !v.is_empty()) {
                for i in selected(&rows, &group, RuleKind::Preference) {
                    if !subset(&rows[i].rule.allowed_values, limit) {
                        mask(&mut rows[i], Reason::RepositoryContract);
                        let affected: Vec<_> = contracts.iter().copied().chain([i]).collect();
                        add_conflict(
                            &rows,
                            &affected,
                            ConflictCategory::PreferenceVsContract,
                            ConflictStatus::ResolvedByAuthority,
                            ReviewAction::InspectPreference,
                            &[],
                            &mut conflicts,
                        );
                    }
                }
            }
            if let Some(limit) = constraint_values.as_ref().filter(|v| !v.is_empty()) {
                for i in selected(&rows, &group, RuleKind::Preference) {
                    if !subset(&rows[i].rule.allowed_values, limit) {
                        mask(&mut rows[i], Reason::SessionConstraint);
                    }
                }
            }
            // No unresolved observation disagreement/drift/unknown remains here.
            if let Some(observed) = observed_values.first() {
                for i in selected(&rows, &group, RuleKind::Preference) {
                    if !rows[i].rule.allowed_values.contains(observed) {
                        mask(&mut rows[i], Reason::RepositoryObservation);
                        add_conflict(
                            &rows,
                            &[i],
                            ConflictCategory::RuleVsRepository,
                            ConflictStatus::ResolvedByAuthority,
                            ReviewAction::InspectPreference,
                            &observed_sources,
                            &mut conflicts,
                        );
                    }
                }
            }
        }
        let active: Vec<_> = group
            .iter()
            .copied()
            .filter(|&i| rows[i].outcome == Outcome::Selected)
            .collect();
        let combined = intersection(&rows, &active);
        if combined.as_ref().is_some_and(BTreeSet::is_empty) {
            add_conflict(
                &rows,
                &active,
                ConflictCategory::RuleVsRule,
                ConflictStatus::Unresolved,
                ReviewAction::ReviewRules,
                &[],
                &mut conflicts,
            );
        }
        if conflicts
            .iter()
            .any(|c| c.key == key && c.status == ConflictStatus::Unresolved)
        {
            for i in active {
                rows[i].outcome = Outcome::Conflicted;
                rows[i].reasons.push(Reason::UnresolvedKey);
            }
        } else if let Some(allowed_values) = combined {
            let mut rules = active
                .iter()
                .map(|&i| rows[i].rule.reference.clone())
                .collect();
            let mut sources = active
                .iter()
                .map(|&i| rows[i].rule.source.reference.clone())
                .collect();
            canonical(&mut rules);
            canonical(&mut sources);
            effective.push(EffectiveValue {
                key,
                allowed_values: allowed_values.into_iter().collect(),
                rules,
                sources,
            });
        }
    }
    for row in &mut rows {
        canonical(&mut row.reasons);
    }
    canonical(&mut conflicts);
    Ok(EffectiveContextV1 {
        schema_version: SCHEMA_VERSION,
        binding: q.binding.clone(),
        explanations: rows,
        effective,
        conflicts,
        repository_facts: facts,
        policy_boundaries: boundaries,
    })
}
