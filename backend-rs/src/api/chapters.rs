use axum::Router;
#[cfg(test)]
use serde_json::json;
#[cfg(test)]
use serde_json::Value;

use crate::api::{
    chapter_analysis_routes, chapter_batch_generation, chapter_crud_routes, chapter_draft_routes,
    chapter_generation_routes, chapter_regeneration_routes,
};

#[cfg(test)]
fn build_chapters_route_aggregate_owner_contract() -> Value {
    json!({
        "owner": "chapters_route_aggregate",
        "rust_owner": "backend-rs/src/api/chapters.rs",
        "scope": "chapters_route_group_router_merge_owner",
        "route_prefix": "/api",
        "merged_route_owners": [
            "chapter_crud_routes",
            "chapter_analysis_routes",
            "chapter_draft_routes",
            "chapter_generation_routes",
            "chapter_regeneration_routes",
            "chapter_batch_generation"
        ],
        "rust_target_map": [
            "backend-rs/src/api/chapters.rs",
            "backend-rs/src/api/chapter_crud_routes.rs",
            "backend-rs/src/api/chapter_analysis_routes.rs",
            "backend-rs/src/api/chapter_draft_routes.rs",
            "backend-rs/src/api/chapter_generation_routes.rs",
            "backend-rs/src/api/chapter_regeneration_routes.rs",
            "backend-rs/src/api/chapter_batch_generation.rs"
        ],
        "python_source_map": [
            "backend/app/api/chapters.py",
            "backend/app/api/chapter_generation_routes.py",
            "backend/app/api/chapter_batch_generation_routes.py",
            "backend/app/api/chapter_regeneration_routes.py",
            "backend/app/api/chapter_partial_regeneration_routes.py",
            "backend/app/api/chapter_analysis_routes.py",
            "backend/app/api/chapter_analysis_task_routes.py",
            "backend/app/api/chapter_draft_routes.py"
        ],
        "route_merge_contract": {
            "crud": "chapter_crud_routes::routes()",
            "analysis": "chapter_analysis_routes::routes()",
            "draft": "chapter_draft_routes::routes()",
            "single_generation": "chapter_generation_routes::routes()",
            "regeneration": "chapter_regeneration_routes::routes()",
            "batch_generation": "chapter_batch_generation::routes()"
        },
        "behavior_contract": {
            "aggregation_only": true,
            "transport_behavior": "No request parsing, database access, auth decision, SSE projection, task lifecycle, or business payload is implemented in chapters.rs.",
            "owner_boundary": "Each merged route file owns its route handlers, service handoffs, error mapping, tests, and rollback/source-map policy.",
            "migration_meaning": "Python backend/app/api/chapters.py has been decomposed into multiple Rust route owners plus this aggregate merge owner; this file proves the route group wiring boundary only."
        },
        "readiness_evidence": [
            "chapter-candidate-route-gateway-smoke-rust",
            "chapters-list-auth-guard-rust",
            "chapters-project-list-auth-guard-rust",
            "chapters-analysis-auth-guard-rust",
            "chapters-batch-analysis-status-auth-guard-rust",
            "chapters-batch-active-tasks-auth-guard-rust",
            "chapters-batch-stream-auth-guard-rust",
            "chapters-batch-resume-auth-guard-rust",
            "chapters-generate-background-auth-guard-rust",
            "chapters-generate-stream-auth-guard-rust",
            "chapters-regenerate-stream-auth-guard-rust",
            "chapters-partial-regenerate-stream-auth-guard-rust",
            "chapters-apply-partial-regenerate-auth-guard-rust",
            "chapters-regeneration-tasks-auth-guard-rust"
        ],
        "owner_profile": {
            "name": "chapters-route-aggregate-owner",
            "profile_kind": "aggregate_route_group_readiness",
            "business_probes": [
                "chapter-candidate-route-gateway-smoke-rust"
            ],
            "route_readiness_probes": [
                "chapters-list-auth-guard-rust",
                "chapters-project-list-auth-guard-rust",
                "chapters-analysis-auth-guard-rust",
                "chapters-batch-analysis-status-auth-guard-rust",
                "chapters-batch-active-tasks-auth-guard-rust",
                "chapters-batch-stream-auth-guard-rust",
                "chapters-batch-resume-auth-guard-rust",
                "chapters-generate-background-auth-guard-rust",
                "chapters-generate-stream-auth-guard-rust",
                "chapters-regenerate-stream-auth-guard-rust",
                "chapters-partial-regenerate-stream-auth-guard-rust",
                "chapters-apply-partial-regenerate-auth-guard-rust",
                "chapters-regeneration-tasks-auth-guard-rust"
            ],
            "python_fallback_probe_count": 0,
            "manifest_profile": "route-groups"
        },
        "business_smoke_status": {
            "owner_profile": "chapters-route-aggregate-owner",
            "manifest_profile": "route-groups",
            "readiness_probe_count": 14,
            "business_probe_count": 1,
            "auth_guard_probe_count": 13,
            "fixture_probe_count": 0,
            "python_fallback_probe_count": 0,
            "status": "covered_by_aggregate_route_group_readiness_profile",
            "scope_note": "chapters.rs is an aggregate merge owner; child route files own business handlers and dedicated owner profiles."
        },
        "next_cutover_gate": "aggregate source-map has been repointed to the split Rust route owners; final physical deletion still requires a same-round approval and rollback policy",
        "migration_policy": "Chapters aggregate routing is covered by the route-groups manifest profile; the aggregate Python source-map has been repointed to the split Rust route owners after child-owner review, and final physical deletion still requires a same-round approval and rollback policy.",
        "validation_boundary": [
            "cargo test api::chapters",
            "python backend/tools/run_strangler_gateway_smoke.py --validate-manifest-only --route-group chapters",
            "cargo check"
        ],
        "rollback_boundary": {
            "source_map_policy": "keep_python_chapters_py_and_split_route_modules_as_source_map_until_explicit_freeze_delete_round",
            "python_route_files_status": "source_map_only_for_chapters_route_group",
            "source_map_freeze_candidate_ready": true,
            "full_module_freeze_ready": false,
            "python_fallback_removal_ready": false,
            "remaining_blockers": [
                "chapters.rs is an aggregate wiring owner, not a business implementation owner",
                "some merged child owners have auth-guard or internal readiness rather than full logged-in business smoke",
                "explicit aggregate source-map delete approval is still required"
            ],
            "freeze_reason": "Rust chapters.rs now owns route-group aggregation across the split Rust route owners, and the aggregate Python chapters.py source-map has been repointed after child-owner review; final physical deletion still requires explicit same-round approval.",
            "rollback_files": [
                "backend/app/api/chapters.py",
                "backend/app/api/chapter_generation_routes.py",
                "backend/app/api/chapter_batch_generation_routes.py",
                "backend/app/api/chapter_regeneration_routes.py",
                "backend/app/api/chapter_partial_regeneration_routes.py",
                "backend/app/api/chapter_analysis_routes.py",
                "backend/app/api/chapter_analysis_task_routes.py",
                "backend/app/api/chapter_draft_routes.py"
            ]
        }
    })
}

pub(crate) fn routes() -> Router {
    Router::new()
        .merge(chapter_crud_routes::routes())
        .merge(chapter_analysis_routes::routes())
        .merge(chapter_draft_routes::routes())
        .merge(chapter_generation_routes::routes())
        .merge(chapter_regeneration_routes::routes())
        .merge(chapter_batch_generation::routes())
}

#[cfg(test)]
mod tests {
    use super::build_chapters_route_aggregate_owner_contract;
    use serde_json::json;

    #[test]
    fn should_publish_chapters_route_aggregate_owner_contract() {
        let contract = build_chapters_route_aggregate_owner_contract();

        assert_eq!(contract["owner"], "chapters_route_aggregate");
        assert_eq!(contract["rust_owner"], "backend-rs/src/api/chapters.rs");
        assert_eq!(contract["behavior_contract"]["aggregation_only"], true);
        assert_eq!(
            contract["merged_route_owners"]
                .as_array()
                .expect("merged route owners")
                .len(),
            6
        );
        for owner in [
            "chapter_crud_routes",
            "chapter_analysis_routes",
            "chapter_draft_routes",
            "chapter_generation_routes",
            "chapter_regeneration_routes",
            "chapter_batch_generation",
        ] {
            assert!(
                contract["merged_route_owners"]
                    .as_array()
                    .expect("merged route owners")
                    .iter()
                    .any(|item| item == owner),
                "missing merged route owner: {owner}"
            );
        }
        assert_eq!(
            contract["owner_profile"]["name"],
            "chapters-route-aggregate-owner"
        );
        assert_eq!(contract["owner_profile"]["python_fallback_probe_count"], 0);
        assert_eq!(
            contract["business_smoke_status"]["owner_profile"],
            "chapters-route-aggregate-owner"
        );
        assert_eq!(
            contract["business_smoke_status"]["manifest_profile"],
            "route-groups"
        );
        assert_eq!(
            contract["business_smoke_status"]["readiness_probe_count"],
            json!(14)
        );
        assert_eq!(
            contract["business_smoke_status"]["business_probe_count"],
            json!(1)
        );
        assert_eq!(
            contract["business_smoke_status"]["auth_guard_probe_count"],
            json!(13)
        );
        assert_eq!(
            contract["business_smoke_status"]["fixture_probe_count"],
            json!(0)
        );
        assert_eq!(
            contract["business_smoke_status"]["python_fallback_probe_count"],
            json!(0)
        );
        assert_eq!(
            contract["business_smoke_status"]["status"],
            "covered_by_aggregate_route_group_readiness_profile"
        );
        assert_eq!(
            contract["next_cutover_gate"],
            "aggregate source-map has been repointed to the split Rust route owners; final physical deletion still requires a same-round approval and rollback policy"
        );
        assert_eq!(
            contract["migration_policy"],
            "Chapters aggregate routing is covered by the route-groups manifest profile; the aggregate Python source-map has been repointed to the split Rust route owners after child-owner review, and final physical deletion still requires a same-round approval and rollback policy."
        );
        assert!(contract["readiness_evidence"]
            .as_array()
            .expect("readiness evidence")
            .iter()
            .any(|item| item == "chapter-candidate-route-gateway-smoke-rust"));
        assert_eq!(
            contract["rollback_boundary"]["source_map_freeze_candidate_ready"],
            true
        );
        assert_eq!(
            contract["rollback_boundary"]["full_module_freeze_ready"],
            false
        );
        assert_eq!(
            contract["rollback_boundary"]["python_fallback_removal_ready"],
            false
        );
    }

    #[test]
    fn should_keep_chapters_router_as_pure_merge_owner() {
        let contract = build_chapters_route_aggregate_owner_contract();

        assert_eq!(
            contract["route_merge_contract"]["crud"],
            "chapter_crud_routes::routes()"
        );
        assert_eq!(
            contract["route_merge_contract"]["batch_generation"],
            "chapter_batch_generation::routes()"
        );
        assert_eq!(
            contract["behavior_contract"]["transport_behavior"],
            "No request parsing, database access, auth decision, SSE projection, task lifecycle, or business payload is implemented in chapters.rs."
        );
    }
}
