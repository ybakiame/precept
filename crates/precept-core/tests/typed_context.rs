#[path = "typed_context/selectors.rs"]
mod selectors;

#[path = "typed_context/precedence.rs"]
mod precedence;

#[path = "typed_context/sources.rs"]
mod sources;

#[path = "typed_context/determinism.rs"]
mod determinism;

#[path = "typed_context/review.rs"]
mod review;

#[path = "typed_context/support.rs"]
mod support;

#[test]
fn selectors_are_and_across_or_within_exact_dimensions() {
    selectors::selectors_are_and_across_or_within_exact_dimensions();
}

#[test]
fn absent_missing_known_empty_and_false_unknown_have_distinct_meanings() {
    selectors::absent_missing_known_empty_and_false_unknown_have_distinct_meanings();
}

#[test]
fn malformed_selectors_paths_and_queries_never_broaden_applicability() {
    selectors::malformed_selectors_paths_and_queries_never_broaden_applicability();
}

#[test]
fn reviewed_project_fallback_and_legacy_newest_id_are_preserved() {
    precedence::reviewed_project_fallback_and_legacy_newest_id_are_preserved();
}

#[test]
fn repository_contract_masks_incompatible_preference_with_both_source_refs() {
    precedence::repository_contract_masks_incompatible_preference_with_both_source_refs();
}

#[test]
fn equal_authority_total_intersection_conflicts_have_no_winner() {
    precedence::equal_authority_total_intersection_conflicts_have_no_winner();
}

#[test]
fn session_constraints_only_narrow_contract_and_supplied_policy() {
    precedence::session_constraints_only_narrow_contract_and_supplied_policy();
}

#[test]
fn live_repository_drift_remains_unresolved_and_claims_are_not_promoted() {
    precedence::live_repository_drift_remains_unresolved_and_claims_are_not_promoted();
}

#[test]
fn sources_unknown_and_invalid_categories_block_only_affected_key() {
    sources::sources_unknown_and_invalid_categories_block_only_affected_key();
}

#[test]
fn invalid_versions_shapes_sources_and_duplicate_identity_are_explained() {
    sources::invalid_versions_shapes_sources_and_duplicate_identity_are_explained();
}

#[test]
fn only_explicit_eligible_same_domain_supersession_can_retire_old_revision() {
    sources::only_explicit_eligible_same_domain_supersession_can_retire_old_revision();
}

#[test]
fn project_registration_and_scope_cannot_choose_new_roots_or_disclose_foreign_sources() {
    sources::project_registration_and_scope_cannot_choose_new_roots_or_disclose_foreign_sources();
}

#[test]
fn normalization_and_input_permutations_produce_identical_serialized_output() {
    determinism::normalization_and_input_permutations_produce_identical_serialized_output();
}

#[test]
fn malformed_serialized_input_and_unknown_fields_are_not_silently_dropped() {
    determinism::malformed_serialized_input_and_unknown_fields_are_not_silently_dropped();
}

#[test]
fn no_match_invalid_metadata_does_not_block_and_unmatched_observation_issues_stay_visible() {
    determinism::no_match_invalid_metadata_does_not_block_and_unmatched_observation_issues_stay_visible();
}

#[test]
fn review_unresolved_contracts_cannot_claim_a_resolved_preference_mask() {
    review::review_unresolved_contracts_cannot_claim_a_resolved_preference_mask();
}

#[test]
fn review_unresolved_constraints_cannot_claim_a_resolved_preference_mask() {
    review::review_unresolved_constraints_cannot_claim_a_resolved_preference_mask();
}

#[test]
fn review_drift_or_disagreeing_facts_cannot_claim_a_resolved_preference_mask() {
    review::review_drift_or_disagreeing_facts_cannot_claim_a_resolved_preference_mask();
}
