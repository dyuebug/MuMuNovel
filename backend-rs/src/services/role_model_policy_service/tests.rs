use std::collections::BTreeMap;

use serde_json::{json, Value};

use super::*;
use crate::services::generation_contract_service::GenerationIntentKind;

fn policy_with(
    role: GenerationRole,
    provider: Option<&str>,
    model: Option<&str>,
) -> RoleModelPolicyV1 {
    RoleModelPolicyV1 {
        schema_version: ROLE_MODEL_POLICY_SCHEMA_VERSION.to_owned(),
        roles: BTreeMap::from([(
            role,
            RoleModelSelectionV1 {
                provider: provider.map(ToOwned::to_owned),
                model: model.map(ToOwned::to_owned),
            },
        )]),
    }
}

fn resolve(
    intent_kind: GenerationIntentKind,
    policy: &RoleModelPolicyV1,
    route_provider: Option<&str>,
    route_model: Option<&str>,
    global_provider: Option<&str>,
    global_model: Option<&str>,
) -> ResolvedRoleModelPolicyV1 {
    resolve_role_model_policy(
        RoleModelResolutionInput {
            intent_kind,
            policy,
            route_provider,
            route_model,
            global_provider,
            global_model,
            runtime_default_provider: "openai",
        },
        |provider| format!("default-{provider}"),
    )
    .expect("resolve role model policy")
}

#[test]
fn role_model_policy_maps_every_generation_intent_to_one_role() {
    let cases = [
        (
            GenerationIntentKind::OutlineGenerate,
            GenerationRole::Planner,
        ),
        (GenerationIntentKind::OutlineExpand, GenerationRole::Planner),
        (
            GenerationIntentKind::ChapterGenerate,
            GenerationRole::Writer,
        ),
        (
            GenerationIntentKind::BatchChapterGenerate,
            GenerationRole::Writer,
        ),
        (
            GenerationIntentKind::ChapterRegenerate,
            GenerationRole::Writer,
        ),
        (
            GenerationIntentKind::ChapterPartialRegenerate,
            GenerationRole::Writer,
        ),
        (GenerationIntentKind::ChapterRepair, GenerationRole::Writer),
        (GenerationIntentKind::BookPolish, GenerationRole::Writer),
        (
            GenerationIntentKind::ChapterReview,
            GenerationRole::Reviewer,
        ),
    ];

    for (intent_kind, expected_role) in cases {
        assert_eq!(GenerationRole::from_intent(intent_kind), expected_role);
    }
}

#[test]
fn role_model_policy_defaults_to_empty_versioned_policy() {
    let policy = read_role_model_policy(None).expect("default policy");
    assert_eq!(policy, RoleModelPolicyV1::default());
    assert_eq!(policy.schema_version, ROLE_MODEL_POLICY_SCHEMA_VERSION);
    assert!(policy.roles.is_empty());
}

#[test]
fn role_model_policy_preferences_merge_preserves_other_top_level_keys() {
    let preferences = json!({
        "api_presets": {"version": "1.0", "presets": [{"id": "preset-1"}]},
        "web_research": {"web_research_enabled": true},
        "future_key": {"kept": true}
    })
    .to_string();
    let policy = policy_with(
        GenerationRole::Writer,
        Some(" Anthropic "),
        Some(" claude-writer "),
    );

    let merged = set_role_model_policy(Some(&preferences), &policy).expect("merge policy");
    let merged_value: Value = serde_json::from_str(&merged).expect("parse merged preferences");
    assert_eq!(
        merged_value["api_presets"],
        json!({"version": "1.0", "presets": [{"id": "preset-1"}]})
    );
    assert_eq!(
        merged_value["web_research"],
        json!({"web_research_enabled": true})
    );
    assert_eq!(merged_value["future_key"], json!({"kept": true}));

    let restored = read_role_model_policy(Some(&merged)).expect("restore policy");
    assert_eq!(
        restored.roles[&GenerationRole::Writer],
        RoleModelSelectionV1 {
            provider: Some("anthropic".to_owned()),
            model: Some("claude-writer".to_owned()),
        }
    );
}

#[test]
fn role_model_policy_rejects_invalid_preferences_and_unknown_policy_fields() {
    assert!(matches!(
        read_role_model_policy(Some("[]")),
        Err(RoleModelPolicyError::InvalidPreferences(_))
    ));
    assert!(matches!(
        read_role_model_policy(Some(
            r#"{"role_model_policy":{"schema_version":"role-model-policy/v1","roles":{"editor":{}}}}"#
        )),
        Err(RoleModelPolicyError::InvalidPolicy(_))
    ));
    assert!(matches!(
        read_role_model_policy(Some(
            r#"{"role_model_policy":{"schema_version":"role-model-policy/v1","roles":{"writer":{"api_key":"secret"}}}}"#
        )),
        Err(RoleModelPolicyError::InvalidPolicy(_))
    ));
    assert!(matches!(
        read_role_model_policy(Some(
            r#"{"role_model_policy":{"schema_version":"role-model-policy/v2","roles":{}}}"#
        )),
        Err(RoleModelPolicyError::UnsupportedSchemaVersion(_))
    ));
}

#[test]
fn role_model_policy_normalizes_empty_selection_fields_to_absent() {
    let policy = policy_with(GenerationRole::Writer, Some("  "), Some("\t"));
    let normalized = normalize_role_model_policy(&policy).expect("normalize policy");
    assert!(!normalized.roles.contains_key(&GenerationRole::Writer));
    assert_eq!(
        compute_role_model_policy_digest(&normalized).expect("normalized digest"),
        compute_role_model_policy_digest(&RoleModelPolicyV1::default()).expect("default digest")
    );
}

#[test]
fn role_model_policy_digest_is_stable_for_normalized_selection() {
    let left = policy_with(
        GenerationRole::Writer,
        Some(" Anthropic "),
        Some(" claude-writer "),
    );
    let right = policy_with(
        GenerationRole::Writer,
        Some("anthropic"),
        Some("claude-writer"),
    );
    let changed = policy_with(
        GenerationRole::Writer,
        Some("anthropic"),
        Some("claude-reviewer"),
    );

    let left_digest = compute_role_model_policy_digest(&left).expect("left digest");
    let right_digest = compute_role_model_policy_digest(&right).expect("right digest");
    let changed_digest = compute_role_model_policy_digest(&changed).expect("changed digest");
    assert_eq!(left_digest, right_digest);
    assert_ne!(left_digest, changed_digest);
    assert!(left_digest.starts_with("sha256:"));
    assert_eq!(left_digest.len(), "sha256:".len() + 64);
}

#[test]
fn role_model_policy_resolution_follows_field_level_precedence() {
    let policy = policy_with(
        GenerationRole::Writer,
        Some("anthropic"),
        Some("claude-writer"),
    );

    let route = resolve(
        GenerationIntentKind::ChapterGenerate,
        &policy,
        Some(" Gemini "),
        Some("gemini-route"),
        Some("openai"),
        Some("global-openai"),
    );
    assert_eq!(route.resolved_provider, "gemini");
    assert_eq!(route.resolved_model, "gemini-route");
    assert_eq!(route.provider_source, ModelSelectionSource::RouteOverride);
    assert_eq!(route.model_source, ModelSelectionSource::RouteOverride);
    assert_eq!(route.requested_provider.as_deref(), Some("gemini"));
    assert_eq!(route.requested_model.as_deref(), Some("gemini-route"));

    let role = resolve(
        GenerationIntentKind::ChapterGenerate,
        &policy,
        None,
        None,
        Some("openai"),
        Some("global-openai"),
    );
    assert_eq!(role.resolved_provider, "anthropic");
    assert_eq!(role.resolved_model, "claude-writer");
    assert_eq!(role.provider_source, ModelSelectionSource::RoleOverride);
    assert_eq!(role.model_source, ModelSelectionSource::RoleOverride);

    let global = resolve(
        GenerationIntentKind::ChapterReview,
        &RoleModelPolicyV1::default(),
        None,
        None,
        Some("Gemini"),
        Some("gemini-global"),
    );
    assert_eq!(global.resolved_provider, "gemini");
    assert_eq!(global.resolved_model, "gemini-global");
    assert_eq!(global.provider_source, ModelSelectionSource::GlobalSettings);
    assert_eq!(global.model_source, ModelSelectionSource::GlobalSettings);
}

#[test]
fn role_model_policy_provider_switch_does_not_reuse_other_provider_model() {
    let policy = policy_with(
        GenerationRole::Writer,
        Some("openai"),
        Some("openai-role-model"),
    );
    let resolved = resolve(
        GenerationIntentKind::ChapterGenerate,
        &policy,
        Some("gemini"),
        None,
        Some("openai"),
        Some("openai-global-model"),
    );

    assert_eq!(resolved.resolved_provider, "gemini");
    assert_eq!(resolved.resolved_model, "default-gemini");
    assert_eq!(resolved.model_source, ModelSelectionSource::ProviderDefault);
}

#[test]
fn role_model_policy_model_only_override_applies_to_resolved_provider() {
    let policy = policy_with(GenerationRole::Writer, None, Some("writer-model"));
    let resolved = resolve(
        GenerationIntentKind::ChapterGenerate,
        &policy,
        Some("gemini"),
        None,
        Some("openai"),
        Some("openai-global-model"),
    );

    assert_eq!(resolved.resolved_provider, "gemini");
    assert_eq!(resolved.resolved_model, "writer-model");
    assert_eq!(resolved.model_source, ModelSelectionSource::RoleOverride);
}
