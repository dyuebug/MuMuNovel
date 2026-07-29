use chrono::NaiveDate;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, Database, DatabaseConnection, DbBackend,
    EntityTrait, IntoActiveModel, PaginatorTrait, QueryFilter, QueryOrder, Schema, Set, Statement,
};

use crate::models::{chapter, novel_autopilot_run, novel_autopilot_step_run, outline, project};

use super::{
    repository::{
        ClaimedNovelAutopilotStep, CreateNovelAutopilotRun, CreateNovelAutopilotStepAttempt,
        NovelAutopilotExpandedChapterCommit, NovelAutopilotOutlineExpansionCommit,
        NovelAutopilotOutlineSnapshot, NovelAutopilotRepository, NovelAutopilotRepositoryError,
        PrepareAndClaimNovelAutopilotStep,
    },
    types::{
        NovelAutopilotPhase, NovelAutopilotRunConfig, NovelAutopilotRunStatus,
        NovelAutopilotStepStatus, NovelAutopilotStepType,
    },
};

const PROJECT_ID: &str = "project-outline-expand";
const USER_ID: &str = "owner-1";
const OUTLINE_ID: &str = "outline-1";
const STEP_KEY: &str = "planning:outline_expand:0001:outline-1";
const TASK_ID: &str = "outline-expand-task";

async fn setup_repository_db() -> DatabaseConnection {
    let db = Database::connect("sqlite::memory:")
        .await
        .expect("connect outline expansion repository SQLite memory database");
    let builder = DbBackend::Sqlite;
    let schema = Schema::new(builder);

    for statement in [
        builder.build(&schema.create_table_from_entity(project::Entity)),
        builder.build(&schema.create_table_from_entity(outline::Entity)),
        builder.build(&schema.create_table_from_entity(chapter::Entity)),
        builder.build(&schema.create_table_from_entity(novel_autopilot_run::Entity)),
        builder.build(&schema.create_table_from_entity(novel_autopilot_step_run::Entity)),
    ] {
        db.execute(statement)
            .await
            .expect("create outline expansion repository test table");
    }
    db.execute(Statement::from_string(
        builder,
        "CREATE UNIQUE INDEX uq_test_outline_expand_active_scope \
         ON novel_autopilot_runs (active_scope_key)"
            .to_string(),
    ))
    .await
    .expect("create active autopilot run uniqueness index");
    db
}

fn test_time() -> chrono::NaiveDateTime {
    NaiveDate::from_ymd_opt(2026, 7, 19)
        .expect("valid date")
        .and_hms_opt(8, 0, 0)
        .expect("valid time")
}

async fn insert_project_and_outline(db: &DatabaseConnection) {
    let now = test_time();
    project::ActiveModel {
        id: Set(PROJECT_ID.to_string()),
        user_id: Set(USER_ID.to_string()),
        title: Set("Outline expansion project".to_string()),
        target_words: Set(100_000),
        current_words: Set(0),
        status: Set("outline".to_string()),
        wizard_status: Set("completed".to_string()),
        wizard_step: Set(0),
        outline_mode: Set("one-to-many".to_string()),
        character_count: Set(0),
        created_at: Set(now),
        updated_at: Set(Some(now)),
        ..Default::default()
    }
    .insert(db)
    .await
    .expect("insert outline expansion project");

    outline::ActiveModel {
        id: Set(OUTLINE_ID.to_string()),
        project_id: Set(PROJECT_ID.to_string()),
        title: Set("遗迹开端".to_string()),
        content: Set(Some("主角进入遗迹并遭遇第一轮规则冲突。".to_string())),
        structure: Set(Some(r#"{"source":"autopilot"}"#.to_string())),
        order_index: Set(Some(1)),
        created_at: Set(now),
        updated_at: Set(Some(now)),
    }
    .insert(db)
    .await
    .expect("insert expandable outline");
}

async fn claim_outline_expansion_step(db: &DatabaseConnection) -> ClaimedNovelAutopilotStep {
    let created = NovelAutopilotRepository::create_or_get_active(
        db,
        CreateNovelAutopilotRun {
            project_id: PROJECT_ID.to_string(),
            user_id: USER_ID.to_string(),
            total_chapters: 2,
            config: NovelAutopilotRunConfig::default(),
        },
    )
    .await
    .expect("create outline expansion run");
    let running = NovelAutopilotRepository::transition_owned(
        db,
        &created.run.id,
        USER_ID,
        created.run.version,
        NovelAutopilotRunStatus::Running,
    )
    .await
    .expect("start outline expansion run");

    NovelAutopilotRepository::prepare_and_claim_step(
        db,
        PrepareAndClaimNovelAutopilotStep {
            attempt: CreateNovelAutopilotStepAttempt {
                run_id: running.id.clone(),
                user_id: USER_ID.to_string(),
                step_key: STEP_KEY.to_string(),
                step_type: NovelAutopilotStepType::OutlineExpand,
                phase: NovelAutopilotPhase::Outline,
                chapter_id: None,
                chapter_number: None,
                run_epoch: running.epoch,
                input_digest: "outline-expand-input-digest".to_string(),
            },
            expected_run_version: running.version,
            background_task_id: Some(TASK_ID.to_string()),
        },
    )
    .await
    .expect("claim outline expansion step")
}

fn expansion_commit() -> NovelAutopilotOutlineExpansionCommit {
    NovelAutopilotOutlineExpansionCommit {
        outline_id: OUTLINE_ID.to_string(),
        chapters: vec![
            NovelAutopilotExpandedChapterCommit {
                title: "进入遗迹".to_string(),
                summary: "主角进入遗迹并确认规则。".to_string(),
                sub_index: 1,
                expansion_plan: r#"{"outline_id":"outline-1","sub_index":1,"title":"进入遗迹","plot_summary":"主角进入遗迹并确认规则。"}"#.to_string(),
            },
            NovelAutopilotExpandedChapterCommit {
                title: "规则代价".to_string(),
                summary: "队伍首次为触犯规则付出代价。".to_string(),
                sub_index: 2,
                expansion_plan: r#"{"outline_id":"outline-1","sub_index":2,"title":"规则代价","plot_summary":"队伍首次为触犯规则付出代价。"}"#.to_string(),
            },
        ],
        result_digest: "sha256:outline-expand-result".to_string(),
    }
}

async fn load_snapshot(db: &DatabaseConnection) -> NovelAutopilotOutlineSnapshot {
    NovelAutopilotOutlineSnapshot::load(db, PROJECT_ID)
        .await
        .expect("load outline expansion snapshot")
}

async fn assert_no_chapters(db: &DatabaseConnection) {
    assert_eq!(
        chapter::Entity::find()
            .filter(chapter::Column::ProjectId.eq(PROJECT_ID))
            .count(db)
            .await
            .expect("count chapters"),
        0
    );
}

#[tokio::test]
async fn outline_expansion_commit_creates_pending_chapters_and_completes_step_atomically() {
    let db = setup_repository_db().await;
    insert_project_and_outline(&db).await;
    let claimed = claim_outline_expansion_step(&db).await;
    let expected = load_snapshot(&db).await;

    let committed = NovelAutopilotRepository::commit_outline_expansion_step(
        &db,
        &claimed.step.id,
        USER_ID,
        claimed.run.version,
        claimed.run.epoch,
        STEP_KEY,
        Some(TASK_ID),
        &expected,
        expansion_commit(),
    )
    .await
    .expect("commit outline expansion");

    assert_eq!(
        committed.run.status,
        NovelAutopilotRunStatus::Running.as_str()
    );
    assert_eq!(committed.run.current_step, None);
    assert_eq!(committed.run.active_background_task_id, None);
    assert_eq!(
        committed.step.status,
        NovelAutopilotStepStatus::Completed.as_str()
    );
    assert_eq!(
        committed.step.result_digest.as_deref(),
        Some("sha256:outline-expand-result")
    );

    let chapters = chapter::Entity::find()
        .filter(chapter::Column::ProjectId.eq(PROJECT_ID))
        .order_by_asc(chapter::Column::ChapterNumber)
        .all(&db)
        .await
        .expect("load expanded chapters");
    assert_eq!(chapters.len(), 2);
    assert_eq!(chapters[0].chapter_number, 1);
    assert_eq!(chapters[0].title, "进入遗迹");
    assert_eq!(chapters[0].content.as_deref(), Some(""));
    assert_eq!(
        chapters[0].summary.as_deref(),
        Some("主角进入遗迹并确认规则。")
    );
    assert_eq!(chapters[0].status, "pending");
    assert_eq!(chapters[0].outline_id.as_deref(), Some(OUTLINE_ID));
    assert_eq!(chapters[0].sub_index, 1);
    assert!(chapters[0].expansion_plan.is_some());
    assert_eq!(chapters[1].chapter_number, 2);
    assert_eq!(chapters[1].sub_index, 2);
}

#[tokio::test]
async fn outline_expansion_commit_rejects_stale_epoch_and_task_without_writes() {
    let db = setup_repository_db().await;
    insert_project_and_outline(&db).await;
    let claimed = claim_outline_expansion_step(&db).await;
    let expected = load_snapshot(&db).await;

    let stale_epoch = NovelAutopilotRepository::commit_outline_expansion_step(
        &db,
        &claimed.step.id,
        USER_ID,
        claimed.run.version,
        claimed.run.epoch + 1,
        STEP_KEY,
        Some(TASK_ID),
        &expected,
        expansion_commit(),
    )
    .await
    .expect_err("stale epoch must reject outline expansion");
    assert_eq!(stale_epoch, NovelAutopilotRepositoryError::StaleEpoch);
    assert_no_chapters(&db).await;

    let stale_task = NovelAutopilotRepository::commit_outline_expansion_step(
        &db,
        &claimed.step.id,
        USER_ID,
        claimed.run.version,
        claimed.run.epoch,
        STEP_KEY,
        Some("other-outline-expand-task"),
        &expected,
        expansion_commit(),
    )
    .await
    .expect_err("different task must reject outline expansion");
    assert_eq!(stale_task, NovelAutopilotRepositoryError::InvalidTransition);
    assert_no_chapters(&db).await;
}

#[tokio::test]
async fn outline_expansion_commit_rejects_business_snapshot_changes_without_writes() {
    let db = setup_repository_db().await;
    insert_project_and_outline(&db).await;
    let claimed = claim_outline_expansion_step(&db).await;
    let expected = load_snapshot(&db).await;

    let outline_model = outline::Entity::find_by_id(OUTLINE_ID)
        .one(&db)
        .await
        .expect("load outline for edit")
        .expect("outline exists");
    let mut active = outline_model.into_active_model();
    active.title = Set("人工修改后的遗迹开端".to_string());
    active
        .update(&db)
        .await
        .expect("edit outline during generation");

    let error = NovelAutopilotRepository::commit_outline_expansion_step(
        &db,
        &claimed.step.id,
        USER_ID,
        claimed.run.version,
        claimed.run.epoch,
        STEP_KEY,
        Some(TASK_ID),
        &expected,
        expansion_commit(),
    )
    .await
    .expect_err("business snapshot change must reject outline expansion");
    assert_eq!(error, NovelAutopilotRepositoryError::BusinessDataChanged);
    assert_no_chapters(&db).await;
}

#[tokio::test]
async fn outline_expansion_retry_does_not_create_duplicate_chapters() {
    let db = setup_repository_db().await;
    insert_project_and_outline(&db).await;
    let claimed = claim_outline_expansion_step(&db).await;
    let expected = load_snapshot(&db).await;

    NovelAutopilotRepository::commit_outline_expansion_step(
        &db,
        &claimed.step.id,
        USER_ID,
        claimed.run.version,
        claimed.run.epoch,
        STEP_KEY,
        Some(TASK_ID),
        &expected,
        expansion_commit(),
    )
    .await
    .expect("first outline expansion commit");

    let retry_error = NovelAutopilotRepository::commit_outline_expansion_step(
        &db,
        &claimed.step.id,
        USER_ID,
        claimed.run.version,
        claimed.run.epoch,
        STEP_KEY,
        Some(TASK_ID),
        &expected,
        expansion_commit(),
    )
    .await
    .expect_err("replayed commit must be rejected");
    assert!(matches!(
        retry_error,
        NovelAutopilotRepositoryError::StaleVersion
            | NovelAutopilotRepositoryError::InvalidTransition
            | NovelAutopilotRepositoryError::BusinessDataChanged
    ));
    assert_eq!(
        chapter::Entity::find()
            .filter(chapter::Column::ProjectId.eq(PROJECT_ID))
            .count(&db)
            .await
            .expect("count chapters after replay"),
        2
    );
}
