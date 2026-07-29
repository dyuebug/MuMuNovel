mod canonical_owner;
mod preferences_owner;
mod resolution_owner;
mod schema_owner;

pub use canonical_owner::{
    compute_role_model_policy_digest, normalize_role_model_policy, RoleModelPolicyError,
};
pub use preferences_owner::{read_role_model_policy, set_role_model_policy};
pub use resolution_owner::{resolve_role_model_policy, RoleModelResolutionInput};
pub use schema_owner::{
    GenerationRole, ModelSelectionSource, ResolvedRoleModelPolicyV1, RoleModelPolicyV1,
    RoleModelSelectionV1, ROLE_MODEL_POLICY_SCHEMA_VERSION,
};

#[cfg(test)]
mod tests;
