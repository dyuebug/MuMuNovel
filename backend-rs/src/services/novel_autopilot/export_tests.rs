use chrono::NaiveDate;
use sea_orm::{
    ActiveModelTrait, ConnectionTrait, Database, DatabaseConnection, DbBackend, EntityTrait,
    IntoActiveModel, Schema, Set, Statement,
};

use crate::{
    models::{chapter, novel_autopilot_run, novel_autopilot_step_run, project},
    services::project_export_service::{
        build_project_export_artifact, ProjectExportArtifactDescriptorV1,
        ProjectExportServiceError, PROJECT_EXPORT_FORMAT_TXT,
    },
};

use super::{
    export_repository::NovelAutopilotExportCommit,
    repository::{
        ClaimedNovelAutopilotStep, CreateNovelAutopilotRun, CreateNovelAutopilotStepAttempt,
        NovelAutopilotRepository, NovelAutopilotRepositoryError, PrepareAndClaimNovelAutopilotStep,
    },
    types::{
        NovelAutopilotPhase, NovelAutopilotRunConfig, NovelAutopilotRunStatus,
        NovelAutopilotStepStatus, NovelAutopilotStepType,
    },
};

const PROJECT_ID: &str = "project-export-step";
const USER_ID: &str = "owner-export-step";
const CHAPTER_ID: &str = "chapter-export-step-1";
const STEP_KEY: &str = "completion:export";
const TASK_ID: &str = "export-step-task";
const INITIAL_CONTENT: &str = "第一章正文，风暴正在海面聚集。";

async fn setup_db() -> DatabaseConnection {
    let db = Database::connect("sqlite::memory:")
        .await
        .expect("connect export SQLite memory database");
    let builder = DbBackend::Sqlite;
    let schema = Schema::new(builder);
    for statement in [
        builder.build(&schema.create_table_from_entity(project::Entity)),
        builder.build(&schema.create_table_from_entity(chapter::Entity)),
        builder.build(&schema.create_table_from_entity(novel_autopilot_run::Entity)),
        builder.build(&schema.create_table_from_entity(novel_autopilot_step_run::Entity)),
    ] {
        db.execute(statement)
            .await
            .expect("create export test table");
    }
    db.execute(Statement::from_string(
        builder,
        "CREATE UNIQUE INDEX uq_test_export_active_scope          ON novel_autopilot_runs (active_scope_key)"
            .to_string(),
    ))
    .await
    .expect("create export active-run uniqueness index");
    db
}

fn test_time() -> chrono::NaiveDateTime {
    NaiveDate::from_ymd_opt(2026, 7, 19)
        .expect("valid date")
        .and_hms_opt(16, 0, 0)
        .expect("valid time")
}

async fn insert_project_and_chapter(db: &DatabaseConnection) {
    let now = test_time();
    project::ActiveModel {
        id: Set(PROJECT_ID.to_string()),
        user_id: Set(USER_ID.to_string()),
        title: Set("Durable Export 测试".to_string()),
        description: Set(Some("导出链路测试项目".to_string())),
        theme: Set(Some("恢复与完成".to_string())),
        genre: Set(Some("奇幻".to_string())),
        target_words: Set(100_000),
        current_words: Set(15),
        status: Set("writing".to_string()),
        wizard_status: Set("completed".to_string()),
        wizard_step: Set(6),
        outline_mode: Set("one-to-one".to_string()),
        character_count: Set(1),
        created_at: Set(now),
        updated_at: Set(Some(now)),
        ..Default::default()
    }
    .insert(db)
    .await
    .expect("insert export project");
    chapter::ActiveModel {
        id: Set(CHAPTER_ID.to_string()),
        project_id: Set(PROJECT_ID.to_string()),
        chapter_number: Set(1),
        title: Set("风暴前夜".to_string()),
        content: Set(Some(INITIAL_CONTENT.to_string())),
        summary: Set(Some("风暴接近群岛。".to_string())),
        word_count: Set(15),
        status: Set("completed".to_string()),
        outline_id: Set(None),
        sub_index: Set(0),
        expansion_plan: Set(None),
        created_at: Set(now),
        updated_at: Set(Some(now)),
    }
    .insert(db)
    .await
    .expect("insert export chapter");
}

async fn claim_export_step(db: &DatabaseConnection) -> ClaimedNovelAutopilotStep {
    let created = NovelAutopilotRepository::create_or_get_active(
        db,
        CreateNovelAutopilotRun {
            project_id: PROJECT_ID.to_string(),
            user_id: USER_ID.to_string(),
            total_chapters: 1,
            config: NovelAutopilotRunConfig::default(),
        },
    )
    .await
    .expect("create export run");
    let running = NovelAutopilotRepository::transition_owned(
        db,
        &created.run.id,
        USER_ID,
        created.run.version,
        NovelAutopilotRunStatus::Running,
    )
    .await
    .expect("start export run");
    let running = novel_autopilot_run::ActiveModel {
        id: Set(running.id),
        completed_chapters: Set(1),
        total_word_count: Set(15),
        ..Default::default()
    }
    .update(db)
    .await
    .expect("seed completed export run");

    NovelAutopilotRepository::prepare_and_claim_step(
        db,
        PrepareAndClaimNovelAutopilotStep {
            attempt: CreateNovelAutopilotStepAttempt {
                run_id: running.id.clone(),
                user_id: USER_ID.to_string(),
                step_key: STEP_KEY.to_string(),
                step_type: NovelAutopilotStepType::Export,
                phase: NovelAutopilotPhase::Export,
                chapter_id: None,
                chapter_number: None,
                run_epoch: running.epoch,
                input_digest: "export-input-digest".to_string(),
            },
            expected_run_version: running.version,
            background_task_id: Some(TASK_ID.to_string()),
        },
    )
    .await
    .expect("claim export step")
}

async fn build_commit(db: &DatabaseConnection) -> (String, ProjectExportArtifactDescriptorV1) {
    let artifact =
        build_project_export_artifact(db, PROJECT_ID, USER_ID, PROJECT_EXPORT_FORMAT_TXT)
            .await
            .expect("build export artifact");
    let descriptor_json = artifact.descriptor_json().expect("serialize descriptor");
    (descriptor_json, artifact.descriptor)
}

#[tokio::test]
async fn export_commit_is_atomic_and_persists_only_safe_descriptor() {
    let db = setup_db().await;
    insert_project_and_chapter(&db).await;
    let claimed = claim_export_step(&db).await;
    let (descriptor_json, descriptor) = build_commit(&db).await;

    let committed = NovelAutopilotRepository::commit_export_step(
        &db,
        &claimed,
        USER_ID,
        STEP_KEY,
        Some(TASK_ID),
        NovelAutopilotExportCommit {
            descriptor_json,
            descriptor: descriptor.clone(),
        },
    )
    .await
    .expect("commit export step");

    assert_eq!(committed.run.version, claimed.run.version + 1);
    assert!(committed.run.current_step.is_none());
    assert!(committed.run.active_background_task_id.is_none());
    let persisted_ref = committed
        .run
        .final_export_ref
        .as_deref()
        .expect("persist final export ref");
    assert!(!persisted_ref.contains(INITIAL_CONTENT));
    let persisted: ProjectExportArtifactDescriptorV1 =
        serde_json::from_str(persisted_ref).expect("parse export descriptor");
    assert_eq!(persisted, descriptor);
    assert_eq!(persisted.project_id, PROJECT_ID);
    assert_eq!(persisted.chapter_count, 1);
    assert_eq!(
        committed.step.status,
        NovelAutopilotStepStatus::Completed.as_str()
    );
    assert_eq!(
        committed.step.result_digest.as_deref(),
        Some(persisted.content_digest.as_str())
    );
    assert_eq!(committed.step.quality_decision.as_deref(), Some("accept"));
}

#[tokio::test]
async fn repeated_export_commit_is_rejected_by_run_version_fencing() {
    let db = setup_db().await;
    insert_project_and_chapter(&db).await;
    let claimed = claim_export_step(&db).await;
    let (descriptor_json, descriptor) = build_commit(&db).await;
    let commit = NovelAutopilotExportCommit {
        descriptor_json,
        descriptor,
    };

    NovelAutopilotRepository::commit_export_step(
        &db,
        &claimed,
        USER_ID,
        STEP_KEY,
        Some(TASK_ID),
        commit.clone(),
    )
    .await
    .expect("first export commit");
    let error = NovelAutopilotRepository::commit_export_step(
        &db,
        &claimed,
        USER_ID,
        STEP_KEY,
        Some(TASK_ID),
        commit,
    )
    .await
    .expect_err("stale export commit must fail");

    assert!(matches!(error, NovelAutopilotRepositoryError::StaleVersion));
}

#[tokio::test]
async fn export_service_enforces_owner_and_digest_changes_with_content() {
    let db = setup_db().await;
    insert_project_and_chapter(&db).await;
    let owner_artifact =
        build_project_export_artifact(&db, PROJECT_ID, USER_ID, PROJECT_EXPORT_FORMAT_TXT)
            .await
            .expect("owner builds artifact");
    let error =
        build_project_export_artifact(&db, PROJECT_ID, "different-user", PROJECT_EXPORT_FORMAT_TXT)
            .await
            .expect_err("non-owner must not export");
    assert_eq!(error, ProjectExportServiceError::NotFoundOrAccessDenied);

    let mut chapter = chapter::Entity::find_by_id(CHAPTER_ID)
        .one(&db)
        .await
        .expect("load chapter")
        .expect("chapter exists")
        .into_active_model();
    chapter.content = Set(Some("正文已由人工修改。".to_string()));
    chapter.word_count = Set(9);
    chapter.update(&db).await.expect("update chapter content");
    let changed_artifact =
        build_project_export_artifact(&db, PROJECT_ID, USER_ID, PROJECT_EXPORT_FORMAT_TXT)
            .await
            .expect("rebuild artifact");

    assert_ne!(
        owner_artifact.descriptor.content_digest,
        changed_artifact.descriptor.content_digest
    );
}
