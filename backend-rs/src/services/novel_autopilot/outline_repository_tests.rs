use chrono::NaiveDate;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, Database, DatabaseConnection, DbBackend,
    EntityTrait, IntoActiveModel, PaginatorTrait, QueryFilter, QueryOrder, Schema, Set, Statement,
};

use crate::models::{chapter, novel_autopilot_run, novel_autopilot_step_run, outline, project};

use super::{
    repository::{
        ClaimedNovelAutopilotStep, CreateNovelAutopilotRun, CreateNovelAutopilotStepAttempt,
        NovelAutopilotOutlineCommit, NovelAutopilotOutlineItemCommit,
        NovelAutopilotOutlineSnapshot, NovelAutopilotPendingChapterCommit,
        NovelAutopilotRepository, NovelAutopilotRepositoryError, PrepareAndClaimNovelAutopilotStep,
    },
    types::{
        NovelAutopilotPhase, NovelAutopilotRunConfig, NovelAutopilotRunStatus,
        NovelAutopilotStepStatus, NovelAutopilotStepType,
    },
};

const STEP_KEY: &str = "planning:outline";

async fn setup_repository_db() -> DatabaseConnection {
    let db = Database::connect("sqlite::memory:")
        .await
        .expect("connect outline repository SQLite memory database");
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
            .expect("create outline repository test table");
    }
    db.execute(Statement::from_string(
        builder,
        "CREATE UNIQUE INDEX uq_test_novel_autopilot_active_scope \
         ON novel_autopilot_runs (active_scope_key)"
            .to_string(),
    ))
    .await
    .expect("create active autopilot run uniqueness index");
    db
}

async fn insert_project(db: &DatabaseConnection, id: &str, user_id: &str) {
    let created_at = NaiveDate::from_ymd_opt(2026, 7, 19)
        .expect("valid date")
        .and_hms_opt(8, 0, 0)
        .expect("valid time");
    project::ActiveModel {
        id: Set(id.to_string()),
        user_id: Set(user_id.to_string()),
        title: Set(format!("Autopilot {id}")),
        target_words: Set(100_000),
        current_words: Set(0),
        status: Set("foundation".to_string()),
        wizard_status: Set("completed".to_string()),
        wizard_step: Set(0),
        outline_mode: Set("linear".to_string()),
        character_count: Set(0),
        created_at: Set(created_at),
        updated_at: Set(Some(created_at)),
        ..Default::default()
    }
    .insert(db)
    .await
    .expect("insert outline test project");
}

fn create_input(project_id: &str, user_id: &str) -> CreateNovelAutopilotRun {
    CreateNovelAutopilotRun {
        project_id: project_id.to_string(),
        user_id: user_id.to_string(),
        total_chapters: 10,
        config: NovelAutopilotRunConfig::default(),
    }
}

async fn claim_outline_step(
    db: &DatabaseConnection,
    project_id: &str,
    user_id: &str,
    task_id: &str,
) -> ClaimedNovelAutopilotStep {
    let created =
        NovelAutopilotRepository::create_or_get_active(db, create_input(project_id, user_id))
            .await
            .expect("create outline run");
    let running = NovelAutopilotRepository::transition_owned(
        db,
        &created.run.id,
        user_id,
        created.run.version,
        NovelAutopilotRunStatus::Running,
    )
    .await
    .expect("start outline run");
    NovelAutopilotRepository::prepare_and_claim_step(
        db,
        PrepareAndClaimNovelAutopilotStep {
            attempt: CreateNovelAutopilotStepAttempt {
                run_id: running.id.clone(),
                user_id: user_id.to_string(),
                step_key: "planning:outline".to_string(),
                step_type: NovelAutopilotStepType::Outline,
                phase: NovelAutopilotPhase::Outline,
                chapter_id: None,
                chapter_number: None,
                run_epoch: running.epoch,
                input_digest: "outline-input-digest".to_string(),
            },
            expected_run_version: running.version,
            background_task_id: Some(task_id.to_string()),
        },
    )
    .await
    .expect("claim outline step")
}

fn generated_outline_commit() -> NovelAutopilotOutlineCommit {
    NovelAutopilotOutlineCommit {
        outlines: vec![
            NovelAutopilotOutlineItemCommit {
                title: "星门异动".to_string(),
                content: "林舟发现星门异常，并决定进入遗迹。".to_string(),
                structure: r#"{"chapter_number":1,"summary":"发现星门异常"}"#.to_string(),
                order_index: 1,
            },
            NovelAutopilotOutlineItemCommit {
                title: "遗迹代价".to_string(),
                content: "队伍进入遗迹，第一次直面规则代价。".to_string(),
                structure: r#"{"chapter_number":2,"summary":"进入遗迹"}"#.to_string(),
                order_index: 2,
            },
        ],
        pending_chapters: vec![
            NovelAutopilotPendingChapterCommit {
                chapter_number: 1,
                title: "星门异动".to_string(),
                summary: "发现星门异常".to_string(),
                outline_index: 0,
            },
            NovelAutopilotPendingChapterCommit {
                chapter_number: 2,
                title: "遗迹代价".to_string(),
                summary: "进入遗迹".to_string(),
                outline_index: 1,
            },
        ],
        outline_mode: "one-to-one".to_string(),
        narrative_perspective: Some("第三人称限知".to_string()),
        target_words: 3_000,
        result_digest: "outline-result-digest".to_string(),
    }
}

async fn insert_manual_outline(db: &DatabaseConnection, project_id: &str) {
    let now = NaiveDate::from_ymd_opt(2026, 7, 19)
        .expect("valid date")
        .and_hms_opt(10, 0, 0)
        .expect("valid time");
    outline::ActiveModel {
        id: Set(format!("manual-outline-{project_id}")),
        project_id: Set(project_id.to_string()),
        title: Set("人工大纲".to_string()),
        content: Set(Some("人工编辑内容".to_string())),
        structure: Set(Some(r#"{"source":"human"}"#.to_string())),
        order_index: Set(Some(1)),
        created_at: Set(now),
        updated_at: Set(Some(now)),
    }
    .insert(db)
    .await
    .expect("insert manual outline");
}

async fn insert_manual_chapter(db: &DatabaseConnection, project_id: &str) {
    let now = NaiveDate::from_ymd_opt(2026, 7, 19)
        .expect("valid date")
        .and_hms_opt(10, 0, 0)
        .expect("valid time");
    chapter::ActiveModel {
        id: Set(format!("manual-chapter-{project_id}")),
        project_id: Set(project_id.to_string()),
        chapter_number: Set(1),
        title: Set("人工章节".to_string()),
        content: Set(Some("人工正文".to_string())),
        summary: Set(Some("人工摘要".to_string())),
        word_count: Set(4),
        status: Set("draft".to_string()),
        outline_id: Set(None),
        sub_index: Set(0),
        expansion_plan: Set(None),
        created_at: Set(now),
        updated_at: Set(Some(now)),
    }
    .insert(db)
    .await
    .expect("insert manual chapter");
}

#[tokio::test]
async fn outline_commit_persists_project_outlines_chapters_and_terminals_atomically() {
    let db = setup_repository_db().await;
    insert_project(&db, "project-outline", "owner-1").await;
    let claimed = claim_outline_step(&db, "project-outline", "owner-1", "outline-task").await;
    let expected = NovelAutopilotOutlineSnapshot::load(&db, "project-outline")
        .await
        .expect("load outline snapshot");
    assert!(expected.is_blank());

    let committed = NovelAutopilotRepository::commit_outline_design_step(
        &db,
        &claimed.step.id,
        "owner-1",
        claimed.run.version,
        claimed.run.epoch,
        "planning:outline",
        Some("outline-task"),
        &expected,
        generated_outline_commit(),
    )
    .await
    .expect("commit outline design");

    let outlines = outline::Entity::find()
        .filter(outline::Column::ProjectId.eq("project-outline"))
        .order_by_asc(outline::Column::OrderIndex)
        .all(&db)
        .await
        .expect("load generated outlines");
    let chapters = chapter::Entity::find()
        .filter(chapter::Column::ProjectId.eq("project-outline"))
        .order_by_asc(chapter::Column::ChapterNumber)
        .all(&db)
        .await
        .expect("load generated chapters");
    assert_eq!(outlines.len(), 2);
    assert_eq!(chapters.len(), 2);
    assert_eq!(
        chapters[0].outline_id.as_deref(),
        Some(outlines[0].id.as_str())
    );
    assert_eq!(
        chapters[1].outline_id.as_deref(),
        Some(outlines[1].id.as_str())
    );
    assert!(chapters.iter().all(|chapter| chapter.status == "pending"));

    let project_after = project::Entity::find_by_id("project-outline")
        .one(&db)
        .await
        .expect("reload outline project")
        .expect("outline project exists");
    assert_eq!(project_after.chapter_count, Some(2));
    assert_eq!(project_after.outline_mode, "one-to-one");
    assert_eq!(
        project_after.narrative_perspective.as_deref(),
        Some("第三人称限知")
    );
    assert_eq!(project_after.target_words, 3_000);
    assert_eq!(project_after.status, "writing");
    assert_eq!(project_after.wizard_status, "completed");
    assert_eq!(project_after.wizard_step, 4);

    assert_eq!(committed.run.version, claimed.run.version + 1);
    assert!(committed.run.current_step.is_none());
    assert!(committed.run.active_background_task_id.is_none());
    assert_eq!(
        committed.step.status,
        NovelAutopilotStepStatus::Completed.as_str()
    );
    assert_eq!(
        committed.step.result_digest.as_deref(),
        Some("outline-result-digest")
    );
}

#[tokio::test]
async fn outline_commit_rejects_human_outline_added_during_generation_without_partial_writes() {
    let db = setup_repository_db().await;
    insert_project(&db, "project-outline-race", "owner-1").await;
    let claimed =
        claim_outline_step(&db, "project-outline-race", "owner-1", "outline-task-race").await;
    let expected = NovelAutopilotOutlineSnapshot::load(&db, "project-outline-race")
        .await
        .expect("load initial outline snapshot");
    insert_manual_outline(&db, "project-outline-race").await;

    assert_eq!(
        NovelAutopilotRepository::commit_outline_design_step(
            &db,
            &claimed.step.id,
            "owner-1",
            claimed.run.version,
            claimed.run.epoch,
            "planning:outline",
            Some("outline-task-race"),
            &expected,
            generated_outline_commit(),
        )
        .await
        .unwrap_err(),
        NovelAutopilotRepositoryError::BusinessDataChanged
    );

    let outlines = outline::Entity::find()
        .filter(outline::Column::ProjectId.eq("project-outline-race"))
        .all(&db)
        .await
        .expect("load outlines after conflict");
    assert_eq!(outlines.len(), 1);
    assert_eq!(outlines[0].title, "人工大纲");
    assert_eq!(
        chapter::Entity::find()
            .filter(chapter::Column::ProjectId.eq("project-outline-race"))
            .count(&db)
            .await
            .expect("count chapters after conflict"),
        0
    );
    let run_after = NovelAutopilotRepository::find_owned(&db, &claimed.run.id, "owner-1")
        .await
        .expect("reload outline run after conflict");
    assert_eq!(run_after.version, claimed.run.version);
    assert_eq!(run_after.current_step.as_deref(), Some("planning:outline"));
    let step_after = novel_autopilot_step_run::Entity::find_by_id(&claimed.step.id)
        .one(&db)
        .await
        .expect("reload outline step")
        .expect("outline step exists");
    assert_eq!(
        step_after.status,
        NovelAutopilotStepStatus::Running.as_str()
    );
}

#[tokio::test]
async fn outline_commit_rejects_human_chapter_added_during_generation() {
    let db = setup_repository_db().await;
    insert_project(&db, "project-outline-chapter-race", "owner-1").await;
    let claimed = claim_outline_step(
        &db,
        "project-outline-chapter-race",
        "owner-1",
        "outline-chapter-task-race",
    )
    .await;
    let expected = NovelAutopilotOutlineSnapshot::load(&db, "project-outline-chapter-race")
        .await
        .expect("load initial outline snapshot");
    insert_manual_chapter(&db, "project-outline-chapter-race").await;

    assert_eq!(
        NovelAutopilotRepository::commit_outline_design_step(
            &db,
            &claimed.step.id,
            "owner-1",
            claimed.run.version,
            claimed.run.epoch,
            "planning:outline",
            Some("outline-chapter-task-race"),
            &expected,
            generated_outline_commit(),
        )
        .await
        .unwrap_err(),
        NovelAutopilotRepositoryError::BusinessDataChanged
    );
    assert_eq!(
        outline::Entity::find()
            .filter(outline::Column::ProjectId.eq("project-outline-chapter-race"))
            .count(&db)
            .await
            .expect("count outlines after chapter conflict"),
        0
    );
    assert_eq!(
        chapter::Entity::find()
            .filter(chapter::Column::ProjectId.eq("project-outline-chapter-race"))
            .count(&db)
            .await
            .expect("count chapters after chapter conflict"),
        1
    );
}

#[tokio::test]
async fn outline_commit_reports_project_edit_as_business_data_changed_before_workflow_error() {
    let db = setup_repository_db().await;
    insert_project(&db, "project-outline-project-race", "owner-1").await;
    let claimed = claim_outline_step(
        &db,
        "project-outline-project-race",
        "owner-1",
        "outline-project-task-race",
    )
    .await;
    let expected = NovelAutopilotOutlineSnapshot::load(&db, "project-outline-project-race")
        .await
        .expect("load initial outline snapshot");
    let project_model = project::Entity::find_by_id("project-outline-project-race")
        .one(&db)
        .await
        .expect("load outline project")
        .expect("outline project exists");
    let mut project_active = project_model.into_active_model();
    project_active.status = Set("completed".to_string());
    project_active
        .update(&db)
        .await
        .expect("edit project status");

    assert_eq!(
        NovelAutopilotRepository::commit_outline_design_step(
            &db,
            &claimed.step.id,
            "owner-1",
            claimed.run.version,
            claimed.run.epoch,
            "planning:outline",
            Some("outline-project-task-race"),
            &expected,
            generated_outline_commit(),
        )
        .await
        .unwrap_err(),
        NovelAutopilotRepositoryError::BusinessDataChanged
    );
    assert_eq!(
        outline::Entity::find()
            .filter(outline::Column::ProjectId.eq("project-outline-project-race"))
            .count(&db)
            .await
            .expect("count outlines after project conflict"),
        0
    );
    assert_eq!(
        chapter::Entity::find()
            .filter(chapter::Column::ProjectId.eq("project-outline-project-race"))
            .count(&db)
            .await
            .expect("count chapters after project conflict"),
        0
    );
}

async fn assert_outline_commit_not_applied(
    db: &DatabaseConnection,
    claimed: &ClaimedNovelAutopilotStep,
    project_id: &str,
    user_id: &str,
) {
    assert_eq!(
        outline::Entity::find()
            .filter(outline::Column::ProjectId.eq(project_id))
            .count(db)
            .await
            .expect("count outlines after rejected commit"),
        0
    );
    assert_eq!(
        chapter::Entity::find()
            .filter(chapter::Column::ProjectId.eq(project_id))
            .count(db)
            .await
            .expect("count chapters after rejected commit"),
        0
    );

    let project_after = project::Entity::find_by_id(project_id)
        .one(db)
        .await
        .expect("reload project after rejected commit")
        .expect("project still exists");
    assert_eq!(project_after.status, "foundation");
    assert_eq!(project_after.outline_mode, "linear");
    assert_eq!(project_after.target_words, 100_000);
    assert_eq!(project_after.wizard_step, 0);

    let run_after = NovelAutopilotRepository::find_owned(db, &claimed.run.id, user_id)
        .await
        .expect("reload run after rejected commit");
    assert_eq!(run_after.version, claimed.run.version);
    assert_eq!(run_after.epoch, claimed.run.epoch);
    assert_eq!(run_after.current_step.as_deref(), Some(STEP_KEY));
    assert_eq!(
        run_after.active_background_task_id,
        claimed.run.active_background_task_id
    );

    let step_after = novel_autopilot_step_run::Entity::find_by_id(&claimed.step.id)
        .one(db)
        .await
        .expect("reload step after rejected commit")
        .expect("step still exists");
    assert_eq!(
        step_after.status,
        NovelAutopilotStepStatus::Running.as_str()
    );
    assert_eq!(step_after.run_epoch, claimed.step.run_epoch);
    assert_eq!(
        step_after.background_task_id,
        claimed.step.background_task_id
    );
}

#[tokio::test]
async fn outline_commit_rejects_stale_version_epoch_and_task_without_business_writes() {
    let db = setup_repository_db().await;
    let project_id = "project-outline-fencing";
    let user_id = "owner-1";
    let task_id = "outline-fencing-task";
    insert_project(&db, project_id, user_id).await;
    let claimed = claim_outline_step(&db, project_id, user_id, task_id).await;
    let expected = NovelAutopilotOutlineSnapshot::load(&db, project_id)
        .await
        .expect("load initial outline snapshot");

    let stale_version = NovelAutopilotRepository::commit_outline_design_step(
        &db,
        &claimed.step.id,
        user_id,
        claimed.run.version + 1,
        claimed.run.epoch,
        STEP_KEY,
        Some(task_id),
        &expected,
        generated_outline_commit(),
    )
    .await
    .expect_err("stale version must reject outline commit");
    assert_eq!(stale_version, NovelAutopilotRepositoryError::StaleVersion);
    assert_outline_commit_not_applied(&db, &claimed, project_id, user_id).await;

    let stale_epoch = NovelAutopilotRepository::commit_outline_design_step(
        &db,
        &claimed.step.id,
        user_id,
        claimed.run.version,
        claimed.run.epoch + 1,
        STEP_KEY,
        Some(task_id),
        &expected,
        generated_outline_commit(),
    )
    .await
    .expect_err("stale epoch must reject outline commit");
    assert_eq!(stale_epoch, NovelAutopilotRepositoryError::StaleEpoch);
    assert_outline_commit_not_applied(&db, &claimed, project_id, user_id).await;

    let invalid_task = NovelAutopilotRepository::commit_outline_design_step(
        &db,
        &claimed.step.id,
        user_id,
        claimed.run.version,
        claimed.run.epoch,
        STEP_KEY,
        Some("different-outline-task"),
        &expected,
        generated_outline_commit(),
    )
    .await
    .expect_err("different task must reject outline commit");
    assert_eq!(
        invalid_task,
        NovelAutopilotRepositoryError::InvalidTransition
    );
    assert_outline_commit_not_applied(&db, &claimed, project_id, user_id).await;
}

#[tokio::test]
async fn outline_terminal_commit_cannot_be_replayed_with_stale_cursor() {
    let db = setup_repository_db().await;
    let project_id = "project-outline-terminal-replay";
    let user_id = "owner-1";
    let task_id = "outline-terminal-task";
    insert_project(&db, project_id, user_id).await;
    let claimed = claim_outline_step(&db, project_id, user_id, task_id).await;
    let expected = NovelAutopilotOutlineSnapshot::load(&db, project_id)
        .await
        .expect("load initial outline snapshot");

    let committed = NovelAutopilotRepository::commit_outline_design_step(
        &db,
        &claimed.step.id,
        user_id,
        claimed.run.version,
        claimed.run.epoch,
        STEP_KEY,
        Some(task_id),
        &expected,
        generated_outline_commit(),
    )
    .await
    .expect("commit outline terminal state");
    assert_eq!(committed.run.version, claimed.run.version + 1);
    assert_eq!(committed.run.current_step, None);
    assert_eq!(committed.run.active_background_task_id, None);
    assert_eq!(
        committed.step.status,
        NovelAutopilotStepStatus::Completed.as_str()
    );
    assert_eq!(
        committed.step.result_digest.as_deref(),
        Some("outline-result-digest")
    );

    let replay = NovelAutopilotRepository::commit_outline_design_step(
        &db,
        &claimed.step.id,
        user_id,
        claimed.run.version,
        claimed.run.epoch,
        STEP_KEY,
        Some(task_id),
        &expected,
        generated_outline_commit(),
    )
    .await
    .expect_err("terminal outline commit must not be replayable");
    assert_eq!(replay, NovelAutopilotRepositoryError::StaleVersion);
    assert_eq!(
        outline::Entity::find()
            .filter(outline::Column::ProjectId.eq(project_id))
            .count(&db)
            .await
            .expect("count outlines after replay"),
        2
    );
    assert_eq!(
        chapter::Entity::find()
            .filter(chapter::Column::ProjectId.eq(project_id))
            .count(&db)
            .await
            .expect("count chapters after replay"),
        2
    );
}
