use crate::services::generation_contract_service::GenerationIntentKind;

use super::canonical_owner::{
    compute_role_model_policy_digest, normalize_model, normalize_provider,
    normalize_role_model_policy, RoleModelPolicyError,
};
use super::schema_owner::{
    GenerationRole, ModelSelectionSource, ResolvedRoleModelPolicyV1, RoleModelPolicyV1,
};

#[derive(Debug, Clone, Copy)]
pub struct RoleModelResolutionInput<'a> {
    pub intent_kind: GenerationIntentKind,
    pub policy: &'a RoleModelPolicyV1,
    pub route_provider: Option<&'a str>,
    pub route_model: Option<&'a str>,
    pub global_provider: Option<&'a str>,
    pub global_model: Option<&'a str>,
    pub runtime_default_provider: &'a str,
}

pub fn resolve_role_model_policy<F>(
    input: RoleModelResolutionInput<'_>,
    provider_default_model: F,
) -> Result<ResolvedRoleModelPolicyV1, RoleModelPolicyError>
where
    F: Fn(&str) -> String,
{
    let policy = normalize_role_model_policy(input.policy)?;
    let role = GenerationRole::from_intent(input.intent_kind);
    let role_selection = policy.roles.get(&role).cloned().unwrap_or_default();

    let requested_provider = normalize_provider(input.route_provider);
    let requested_model = normalize_model(input.route_model);
    let role_provider = role_selection.provider;
    let role_model = role_selection.model;
    let global_provider = normalize_provider(input.global_provider);
    let global_model = normalize_model(input.global_model);
    let runtime_default_provider = normalize_provider(Some(input.runtime_default_provider))
        .ok_or_else(|| {
            RoleModelPolicyError::InvalidPolicy(
                "runtime default provider must not be empty".to_owned(),
            )
        })?;

    let (resolved_provider, provider_source) = requested_provider
        .clone()
        .map(|provider| (provider, ModelSelectionSource::RouteOverride))
        .or_else(|| {
            role_provider
                .clone()
                .map(|provider| (provider, ModelSelectionSource::RoleOverride))
        })
        .or_else(|| {
            global_provider
                .clone()
                .map(|provider| (provider, ModelSelectionSource::GlobalSettings))
        })
        .unwrap_or((
            runtime_default_provider,
            ModelSelectionSource::ProviderDefault,
        ));

    let (resolved_model, model_source) = requested_model
        .clone()
        .map(|model| (model, ModelSelectionSource::RouteOverride))
        .or_else(|| {
            compatible_model(
                role_model.clone(),
                role_provider.as_deref(),
                &resolved_provider,
            )
            .map(|model| (model, ModelSelectionSource::RoleOverride))
        })
        .or_else(|| {
            compatible_model(
                global_model.clone(),
                global_provider.as_deref(),
                &resolved_provider,
            )
            .map(|model| (model, ModelSelectionSource::GlobalSettings))
        })
        .unwrap_or_else(|| {
            (
                provider_default_model(&resolved_provider),
                ModelSelectionSource::ProviderDefault,
            )
        });
    let resolved_model = normalize_model(Some(&resolved_model)).ok_or_else(|| {
        RoleModelPolicyError::InvalidPolicy(format!(
            "provider default model must not be empty for {resolved_provider}"
        ))
    })?;

    Ok(ResolvedRoleModelPolicyV1 {
        role,
        policy_schema_version: policy.schema_version.clone(),
        policy_digest: compute_role_model_policy_digest(&policy)?,
        requested_provider,
        requested_model,
        resolved_provider,
        resolved_model,
        provider_source,
        model_source,
    })
}

fn compatible_model(
    model: Option<String>,
    associated_provider: Option<&str>,
    resolved_provider: &str,
) -> Option<String> {
    model.filter(|_| {
        associated_provider
            .map(|provider| provider == resolved_provider)
            .unwrap_or(true)
    })
}
