use sea_orm::DatabaseConnection;
use serde_json::{json, Value};

use crate::services::schema_migration_metadata_service::{
    build_schema_migration_metadata_contract, check_live_alembic_head,
    check_password_hash_storage_compatibility, LiveAlembicHeadCheck,
    PasswordHashStorageCompatibilityCheck,
};

#[derive(Debug, Clone)]
pub(crate) struct ProductionReadinessEvaluation {
    pub(crate) runtime_ready: bool,
    pub(crate) release_ready: bool,
    pub(crate) checks: Value,
}

impl ProductionReadinessEvaluation {
    pub(crate) fn runtime_payload(&self) -> Value {
        self.payload("runtime", self.runtime_ready)
    }

    pub(crate) fn release_payload(&self) -> Value {
        self.payload("production_release", self.release_ready)
    }

    pub(crate) fn release_exit_code(&self) -> i32 {
        if self.release_ready {
            0
        } else {
            1
        }
    }

    fn payload(&self, readiness_scope: &'static str, is_ready: bool) -> Value {
        json!({
            "status": if is_ready { "ready" } else { "not_ready" },
            "readiness_scope": readiness_scope,
            "runtime_ready": self.runtime_ready,
            "release_ready": self.release_ready,
            "checks": &self.checks,
        })
    }
}

pub(crate) async fn evaluate_production_readiness(
    db: Option<&DatabaseConnection>,
) -> ProductionReadinessEvaluation {
    let mut live_alembic_head_check = LiveAlembicHeadCheck::not_checked("database unavailable");
    let mut password_hash_storage_check =
        PasswordHashStorageCompatibilityCheck::not_checked_database_unavailable();
    let db_healthy = match db {
        Some(conn) => {
            let healthy = conn.ping().await.is_ok();
            if healthy {
                live_alembic_head_check = check_live_alembic_head(conn).await;
                password_hash_storage_check = check_password_hash_storage_compatibility(conn).await;
            }
            healthy
        }
        None => false,
    };

    build_evaluation(
        true,
        db_healthy,
        live_alembic_head_check,
        password_hash_storage_check,
    )
}

fn build_evaluation(
    startup_ready: bool,
    db_healthy: bool,
    live_alembic_head_check: LiveAlembicHeadCheck,
    password_hash_storage_check: PasswordHashStorageCompatibilityCheck,
) -> ProductionReadinessEvaluation {
    let runtime_ready = startup_ready
        && db_healthy
        && live_alembic_head_check.matches_catalog_head
        && password_hash_storage_check.allows_readiness;
    let release_ready =
        runtime_ready && password_hash_storage_check.matches_target_storage_contract == Some(true);

    ProductionReadinessEvaluation {
        runtime_ready,
        release_ready,
        checks: json!({
            "startup": {"ready": startup_ready},
            "database": {
                "healthy": db_healthy,
                "message": if db_healthy { "connected" } else { "unavailable" },
            },
            "schema_migration": build_schema_migration_metadata_contract(
                Some(&live_alembic_head_check),
                Some(&password_hash_storage_check),
            ),
        }),
    }
}

#[cfg(test)]
mod tests {
    use sea_orm::{ConnectionTrait, Database, DatabaseBackend, Statement};

    use super::{build_evaluation, evaluate_production_readiness};
    use crate::services::schema_migration_metadata_service::{
        LiveAlembicHeadCheck, PasswordHashStorageCompatibilityCheck,
    };

    fn matching_head() -> LiveAlembicHeadCheck {
        LiveAlembicHeadCheck {
            status: "head_matches",
            expected_head: "20260712_password_hash_phc_text",
            actual_head: Some("20260712_password_hash_phc_text".to_string()),
            matches_catalog_head: true,
            error: None,
        }
    }

    fn storage(
        data_type: &str,
        udt_name: &str,
        character_maximum_length: Option<i32>,
    ) -> PasswordHashStorageCompatibilityCheck {
        PasswordHashStorageCompatibilityCheck::from_column_metadata(
            data_type.to_string(),
            udt_name.to_string(),
            character_maximum_length,
        )
    }

    #[test]
    fn target_text_storage_is_runtime_and_release_ready() {
        let evaluation =
            build_evaluation(true, true, matching_head(), storage("text", "text", None));

        assert!(evaluation.runtime_ready);
        assert!(evaluation.release_ready);
        assert_eq!(evaluation.release_exit_code(), 0);
        assert_eq!(evaluation.release_payload()["status"], "ready");
        assert_eq!(
            evaluation.release_payload()["readiness_scope"],
            "production_release"
        );
    }

    #[test]
    fn compatible_bounded_varchar_is_runtime_ready_but_not_release_ready() {
        let evaluation = build_evaluation(
            true,
            true,
            matching_head(),
            storage("character varying", "varchar", Some(255)),
        );

        assert!(evaluation.runtime_ready);
        assert!(!evaluation.release_ready);
        assert_eq!(evaluation.release_exit_code(), 1);
        assert_eq!(evaluation.runtime_payload()["status"], "ready");
        assert_eq!(evaluation.release_payload()["status"], "not_ready");
    }

    #[test]
    fn legacy_varchar_64_fails_runtime_and_release_readiness() {
        let evaluation = build_evaluation(
            true,
            true,
            matching_head(),
            storage("character varying", "varchar", Some(64)),
        );

        assert!(!evaluation.runtime_ready);
        assert!(!evaluation.release_ready);
        assert_eq!(evaluation.release_exit_code(), 1);
    }

    #[test]
    fn migration_head_mismatch_fails_closed() {
        let live_head = LiveAlembicHeadCheck {
            status: "head_mismatch",
            expected_head: "20260712_password_hash_phc_text",
            actual_head: Some("old_revision".to_string()),
            matches_catalog_head: false,
            error: None,
        };
        let evaluation = build_evaluation(true, true, live_head, storage("text", "text", None));

        assert!(!evaluation.runtime_ready);
        assert!(!evaluation.release_ready);
    }

    #[tokio::test]
    async fn unavailable_database_fails_closed_with_structured_payload() {
        let evaluation = evaluate_production_readiness(None).await;
        let payload = evaluation.release_payload();

        assert!(!evaluation.runtime_ready);
        assert!(!evaluation.release_ready);
        assert_eq!(payload["status"], "not_ready");
        assert_eq!(payload["checks"]["database"]["healthy"], false);
        assert_eq!(
            payload["checks"]["schema_migration"]["live_database_head"]["status"],
            "not_checked"
        );
    }

    #[tokio::test]
    async fn sqlite_evidence_can_be_runtime_ready_but_never_release_ready() {
        let db = Database::connect("sqlite::memory:")
            .await
            .expect("connect sqlite readiness database");
        db.execute(Statement::from_string(
            DatabaseBackend::Sqlite,
            "CREATE TABLE alembic_version (version_num VARCHAR(255) NOT NULL)".to_string(),
        ))
        .await
        .expect("create alembic version table");
        db.execute(Statement::from_string(
            DatabaseBackend::Sqlite,
            "INSERT INTO alembic_version (version_num) VALUES ('20260712_password_hash_phc_text')"
                .to_string(),
        ))
        .await
        .expect("insert migration head");

        let evaluation = evaluate_production_readiness(Some(&db)).await;
        let payload = evaluation.release_payload();

        assert!(evaluation.runtime_ready);
        assert!(!evaluation.release_ready);
        assert_eq!(evaluation.release_exit_code(), 1);
        assert_eq!(
            payload["checks"]["schema_migration"]["auth_password_hash_storage"]["status"],
            "not_applicable_non_postgres"
        );
    }
}
