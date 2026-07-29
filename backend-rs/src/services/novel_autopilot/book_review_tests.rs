use chrono::NaiveDate;
use sea_orm::{
    ActiveModelTrait, ConnectionTrait, Database, DatabaseConnection, DbBackend, Schema, Set,
    Statement,
};
use serde_json::json;

use crate::{
    models::{
        chapter, novel_autopilot_run, novel_autopilot_step_run, outline, plot_analysis, project,
    },
    services::chapter_content_digest_service::chapter_content_digest,
};

use super::{
    book_review_repository::NovelAutopilotBookReviewCommit,
    book_review_service::load_book_review_summary,
    facts::enrich_novel_autopilot_completion_facts,
    repository::{
        CreateNovelAutopilotRun, CreateNovelAutopilotStepAttempt, NovelAutopilotRepository,
        PrepareAndClaimNovelAutopilotStep,
    },
    router::NovelAutopilotBusinessFacts,
    types::{
        NovelAutopilotPhase, NovelAutopilotRunConfig, NovelAutopilotRunStatus,
        NovelAutopilotStepStatus, NovelAutopilotStepType,
    },
};

const PROJECT_ID: &str = "project-book-review";
const USER_ID: &str = "owner-book-review";
const CHAPTER_ID: &str = "chapter-book-review-1";
const OUTLINE_ID: &str = "outline-book-review-1";
const STEP_KEY: &str = "completion:book_review";
const TASK_ID: &str = "book-review-task";
const INITIAL_CONTENT: &str = "第一章正文，风暴正在海面聚集。";
const REVISED_CONTENT: &str = "第一章修订正文，风暴掠过群岛并揭开失落王庭。";

async fn setup_db() -> DatabaseConnection {
    let db = Database::connect("sqlite::memory:")
        .await
        .expect("connect book review SQLite memory database");
    let builder = DbBackend::Sqlite;
    let schema = Schema::new(builder);
    for statement in [
        builder.build(&schema.create_table_from_entity(project::Entity)),
        builder.build(&schema.create_table_from_entity(outline::Entity)),
        builder.build(&schema.create_table_from_entity(chapter::Entity)),
        builder.build(&schema.create_table_from_entity(plot_analysis::Entity)),
        builder.build(&schema.create_table_from_entity(novel_autopilot_run::Entity)),
        builder.build(&schema.create_table_from_entity(novel_autopilot_step_run::Entity)),
    ] {
        db.execute(statement)
            .await
            .expect("create book review test table");
    }
    db.execute(Statement::from_string(
        builder,
        "CREATE UNIQUE INDEX uq_test_book_review_active_scope \
         ON novel_autopilot_runs (active_scope_key)"
            .to_string(),
    ))
    .await
    .expect("create book review active-run uniqueness index");
    db
}

fn test_time() -> chrono::NaiveDateTime {
    NaiveDate::from_ymd_opt(2026, 7, 19)
        .expect("valid date")
        .and_hms_opt(9, 0, 0)
        .expect("valid time")
}

async fn insert_business_facts(db: &DatabaseConnection) {
    let now = test_time();
    project::ActiveModel {
        id: Set(PROJECT_ID.to_string()),
        user_id: Set(USER_ID.to_string()),
        title: Set("全书审查测试".to_string()),
        description: Set(Some("群岛上的王庭遗迹逐渐苏醒。".to_string())),
        theme: Set(Some("秩序与自由".to_string())),
        genre: Set(Some("奇幻".to_string())),
        target_words: Set(100_000),
        current_words: Set(15),
        status: Set("writing".to_string()),
        wizard_status: Set("completed".to_string()),
        wizard_step: Set(4),
        outline_mode: Set("one-to-one".to_string()),
        character_count: Set(1),
        created_at: Set(now),
        updated_at: Set(Some(now)),
        ..Default::default()
    }
    .insert(db)
    .await
    .expect("insert book review project");
    outline::ActiveModel {
        id: Set(OUTLINE_ID.to_string()),
        project_id: Set(PROJECT_ID.to_string()),
        title: Set("第一章大纲".to_string()),
        content: Set(Some("风暴揭开王庭遗迹。".to_string())),
        structure: Set(None),
        order_index: Set(Some(1)),
        created_at: Set(now),
        updated_at: Set(Some(now)),
    }
    .insert(db)
    .await
    .expect("insert book review outline");
    chapter::ActiveModel {
        id: Set(CHAPTER_ID.to_string()),
        project_id: Set(PROJECT_ID.to_string()),
        chapter_number: Set(1),
        title: Set("风暴前夜".to_string()),
        content: Set(Some(INITIAL_CONTENT.to_string())),
        summary: Set(Some("风暴接近群岛。".to_string())),
        word_count: Set(15),
        status: Set("completed".to_string()),
        outline_id: Set(Some(OUTLINE_ID.to_string())),
        sub_index: Set(0),
        expansion_plan: Set(None),
        created_at: Set(now),
        updated_at: Set(Some(now)),
    }
    .insert(db)
    .await
    .expect("insert book review chapter");
    insert_analysis(
        db,
        "analysis-book-review-1",
        INITIAL_CONTENT,
        7.5,
        json!(["加强结尾钩子"]),
        now,
    )
    .await;
}

async fn insert_analysis(
    db: &DatabaseConnection,
    analysis_id: &str,
    content: &str,
    overall_score: f64,
    suggestions: serde_json::Value,
    created_at: chrono::NaiveDateTime,
) {
    plot_analysis::ActiveModel {
        id: Set(analysis_id.to_string()),
        project_id: Set(PROJECT_ID.to_string()),
        chapter_id: Set(CHAPTER_ID.to_string()),
        source_content_digest: Set(Some(chapter_content_digest(content))),
        hooks_count: Set(0),
        foreshadows_planted: Set(0),
        foreshadows_resolved: Set(0),
        plot_points_count: Set(0),
        overall_quality_score: Set(Some(overall_score)),
        pacing_score: Set(Some(8.0)),
        engagement_score: Set(Some(8.0)),
        coherence_score: Set(Some(8.0)),
        suggestions: Set(Some(suggestions)),
        created_at: Set(Some(created_at)),
        ..Default::default()
    }
    .insert(db)
    .await
    .expect("insert book review analysis");
}

async fn claim_review_step(
    db: &DatabaseConnection,
) -> super::repository::ClaimedNovelAutopilotStep {
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
    .expect("create book review run");
    let running = NovelAutopilotRepository::transition_owned(
        db,
        &created.run.id,
        USER_ID,
        created.run.version,
        NovelAutopilotRunStatus::Running,
    )
    .await
    .expect("start book review run");
    NovelAutopilotRepository::prepare_and_claim_step(
        db,
        PrepareAndClaimNovelAutopilotStep {
            attempt: CreateNovelAutopilotStepAttempt {
                run_id: running.id.clone(),
                user_id: USER_ID.to_string(),
                step_key: STEP_KEY.to_string(),
                step_type: NovelAutopilotStepType::BookReview,
                phase: NovelAutopilotPhase::BookReview,
                chapter_id: None,
                chapter_number: None,
                run_epoch: running.epoch,
                input_digest: "book-review-input-digest".to_string(),
            },
            expected_run_version: running.version,
            background_task_id: Some(TASK_ID.to_string()),
        },
    )
    .await
    .expect("claim book review step")
}

fn completed_business_facts() -> NovelAutopilotBusinessFacts {
    NovelAutopilotBusinessFacts {
        target_chapter_count: 1,
        completed_chapter_count: 1,
        ..NovelAutopilotBusinessFacts::default()
    }
}

#[tokio::test]
async fn book_review_commit_is_restart_safe_and_invalidates_after_business_change() {
    let db = setup_db().await;
    insert_business_facts(&db).await;
    let summary = load_book_review_summary(&db, PROJECT_ID, USER_ID, 1)
        .await
        .expect("load initial book review summary");
    assert!(summary.ready);
    assert_eq!(summary.analyzed_chapter_count, 1);
    assert_eq!(summary.pending_rewrites.len(), 1);
    assert_eq!(summary.pending_rewrites[0].chapter_id, CHAPTER_ID);
    assert_eq!(
        summary.pending_rewrites[0].analysis_id,
        "analysis-book-review-1"
    );

    let claimed = claim_review_step(&db).await;
    let committed = NovelAutopilotRepository::commit_book_review_step(
        &db,
        &claimed,
        USER_ID,
        STEP_KEY,
        Some(TASK_ID),
        NovelAutopilotBookReviewCommit {
            pending_rewrites: summary.pending_rewrites.clone(),
            result_digest: summary.result_digest.clone(),
        },
    )
    .await
    .expect("commit book review step");
    assert_eq!(
        committed.step.status,
        NovelAutopilotStepStatus::Completed.as_str()
    );
    assert_eq!(
        committed.step.result_digest.as_deref(),
        Some(summary.result_digest.as_str())
    );
    assert!(committed.run.current_step.is_none());
    assert!(committed.run.active_background_task_id.is_none());
    let persisted_rewrites = committed
        .run
        .pending_rewrites
        .as_array()
        .expect("pending rewrites array");
    assert_eq!(persisted_rewrites.len(), 1);
    assert!(persisted_rewrites[0].get("content").is_none());
    assert!(persisted_rewrites[0].get("prompt").is_none());
    assert!(persisted_rewrites[0].get("reasoning").is_none());

    let restarted_run = NovelAutopilotRepository::find_owned(&db, &committed.run.id, USER_ID)
        .await
        .expect("reload run after simulated restart");
    let mut facts = completed_business_facts();
    enrich_novel_autopilot_completion_facts(
        &db,
        &restarted_run,
        USER_ID,
        &NovelAutopilotRunConfig::default(),
        &mut facts,
    )
    .await
    .expect("load persisted completion facts");
    assert!(facts.book_review_completed);
    assert!(!facts.book_polish_completed);

    chapter::ActiveModel {
        id: Set(CHAPTER_ID.to_string()),
        content: Set(Some(REVISED_CONTENT.to_string())),
        word_count: Set(24),
        updated_at: Set(Some(test_time())),
        ..Default::default()
    }
    .update(&db)
    .await
    .expect("simulate manual chapter revision");
    insert_analysis(
        &db,
        "analysis-book-review-2",
        REVISED_CONTENT,
        8.8,
        json!([]),
        test_time() + chrono::Duration::seconds(1),
    )
    .await;

    let changed_summary = load_book_review_summary(&db, PROJECT_ID, USER_ID, 1)
        .await
        .expect("load changed book review summary");
    assert!(changed_summary.ready);
    assert_ne!(changed_summary.result_digest, summary.result_digest);
    assert!(changed_summary.pending_rewrites.is_empty());

    let mut changed_facts = completed_business_facts();
    enrich_novel_autopilot_completion_facts(
        &db,
        &restarted_run,
        USER_ID,
        &NovelAutopilotRunConfig::default(),
        &mut changed_facts,
    )
    .await
    .expect("reload changed completion facts");
    assert!(!changed_facts.book_review_completed);
    assert!(!changed_facts.book_polish_completed);
}
