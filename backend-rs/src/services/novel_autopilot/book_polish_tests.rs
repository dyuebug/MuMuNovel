use chrono::NaiveDate;
use sea_orm::{
    ActiveModelTrait, ConnectionTrait, Database, DatabaseConnection, DbBackend, EntityTrait,
    Schema, Set, Statement,
};
use serde_json::json;

use crate::{
    models::{
        career, chapter, character, novel_autopilot_run, novel_autopilot_step_run, organization,
        outline, plot_analysis, project,
    },
    services::chapter_content_digest_service::chapter_content_digest,
};

use super::{
    book_polish_repository::NovelAutopilotBookPolishCommit,
    book_review_service::BookReviewRewriteReference,
    chapter_repository::ChapterBusinessSnapshot,
    facts::{
        enrich_novel_autopilot_completion_facts, load_novel_autopilot_business_facts,
        NovelAutopilotFactsError, NovelAutopilotQualityFactScope,
    },
    repository::{
        ClaimedNovelAutopilotStep, CreateNovelAutopilotRun, CreateNovelAutopilotStepAttempt,
        NovelAutopilotRepository, NovelAutopilotRepositoryError, PrepareAndClaimNovelAutopilotStep,
    },
    router::NovelAutopilotBusinessFacts,
    types::{
        NovelAutopilotPhase, NovelAutopilotQualityDecision, NovelAutopilotRunConfig,
        NovelAutopilotRunStatus, NovelAutopilotStepStatus, NovelAutopilotStepType,
    },
};

const PROJECT_ID: &str = "project-book-polish";
const USER_ID: &str = "owner-book-polish";
const CHAPTER_ID: &str = "chapter-book-polish-1";
const OUTLINE_ID: &str = "outline-book-polish-1";
const ANALYSIS_ID: &str = "analysis-book-polish-1";
const STEP_KEY: &str = "completion:book_polish:chapter:0001:chapter-book-polish-1";
const TASK_ID: &str = "book-polish-task";
const INITIAL_CONTENT: &str = "第一章正文，风暴正在海面聚集。";
const POLISHED_CONTENT: &str = "第一章润色正文，风暴越过群岛，并揭开失落王庭的第一道门。";
const MANUAL_CONTENT: &str = "用户手工改写后的正文，不允许迟到的模型结果覆盖。";

async fn setup_db() -> DatabaseConnection {
    let db = Database::connect("sqlite::memory:")
        .await
        .expect("connect book polish SQLite memory database");
    let builder = DbBackend::Sqlite;
    let schema = Schema::new(builder);
    for statement in [
        builder.build(&schema.create_table_from_entity(project::Entity)),
        builder.build(&schema.create_table_from_entity(career::Entity)),
        builder.build(&schema.create_table_from_entity(character::Entity)),
        builder.build(&schema.create_table_from_entity(organization::Entity)),
        builder.build(&schema.create_table_from_entity(outline::Entity)),
        builder.build(&schema.create_table_from_entity(chapter::Entity)),
        builder.build(&schema.create_table_from_entity(plot_analysis::Entity)),
        builder.build(&schema.create_table_from_entity(novel_autopilot_run::Entity)),
        builder.build(&schema.create_table_from_entity(novel_autopilot_step_run::Entity)),
    ] {
        db.execute(statement)
            .await
            .expect("create book polish test table");
    }
    db.execute(Statement::from_string(
        builder,
        "CREATE UNIQUE INDEX uq_test_book_polish_active_scope \
         ON novel_autopilot_runs (active_scope_key)"
            .to_string(),
    ))
    .await
    .expect("create book polish active-run uniqueness index");
    db
}

fn test_time() -> chrono::NaiveDateTime {
    NaiveDate::from_ymd_opt(2026, 7, 19)
        .expect("valid date")
        .and_hms_opt(10, 0, 0)
        .expect("valid time")
}

fn expected_rewrite() -> BookReviewRewriteReference {
    BookReviewRewriteReference {
        chapter_id: CHAPTER_ID.to_string(),
        chapter_number: 1,
        analysis_id: ANALYSIS_ID.to_string(),
        source_content_digest: chapter_content_digest(INITIAL_CONTENT),
        reason_code: "quality_below_target".to_string(),
        attempt: 1,
    }
}

fn following_rewrite() -> BookReviewRewriteReference {
    BookReviewRewriteReference {
        chapter_id: "chapter-book-polish-2".to_string(),
        chapter_number: 2,
        analysis_id: "analysis-book-polish-2".to_string(),
        source_content_digest: chapter_content_digest("第二章旧正文"),
        reason_code: "suggestions_present".to_string(),
        attempt: 1,
    }
}

fn accepted_commit() -> NovelAutopilotBookPolishCommit {
    NovelAutopilotBookPolishCommit {
        content: POLISHED_CONTENT.to_string(),
        word_count: 28,
        content_digest: chapter_content_digest(POLISHED_CONTENT),
        result_digest: "book-polish-result-digest".to_string(),
    }
}

async fn insert_business_facts(db: &DatabaseConnection) {
    let now = test_time();
    project::ActiveModel {
        id: Set(PROJECT_ID.to_string()),
        user_id: Set(USER_ID.to_string()),
        title: Set("全书润色测试".to_string()),
        description: Set(Some("群岛王庭在风暴中苏醒。".to_string())),
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
    .expect("insert book polish project");
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
    .expect("insert book polish outline");
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
    .expect("insert book polish chapter");
    plot_analysis::ActiveModel {
        id: Set(ANALYSIS_ID.to_string()),
        project_id: Set(PROJECT_ID.to_string()),
        chapter_id: Set(CHAPTER_ID.to_string()),
        source_content_digest: Set(Some(chapter_content_digest(INITIAL_CONTENT))),
        hooks_count: Set(0),
        foreshadows_planted: Set(0),
        foreshadows_resolved: Set(0),
        plot_points_count: Set(0),
        overall_quality_score: Set(Some(7.5)),
        pacing_score: Set(Some(8.0)),
        engagement_score: Set(Some(8.0)),
        coherence_score: Set(Some(8.0)),
        suggestions: Set(Some(json!(["加强结尾钩子"]))),
        created_at: Set(Some(now)),
        ..Default::default()
    }
    .insert(db)
    .await
    .expect("insert book polish analysis");
}

async fn claim_polish_step(
    db: &DatabaseConnection,
    rewrites: Vec<BookReviewRewriteReference>,
) -> ClaimedNovelAutopilotStep {
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
    .expect("create book polish run");
    let running = NovelAutopilotRepository::transition_owned(
        db,
        &created.run.id,
        USER_ID,
        created.run.version,
        NovelAutopilotRunStatus::Running,
    )
    .await
    .expect("start book polish run");
    let running = novel_autopilot_run::ActiveModel {
        id: Set(running.id),
        completed_chapters: Set(1),
        total_word_count: Set(15),
        pending_rewrites: Set(serde_json::to_value(rewrites).expect("serialize rewrites")),
        ..Default::default()
    }
    .update(db)
    .await
    .expect("seed pending polish rewrites");

    NovelAutopilotRepository::prepare_and_claim_step(
        db,
        PrepareAndClaimNovelAutopilotStep {
            attempt: CreateNovelAutopilotStepAttempt {
                run_id: running.id.clone(),
                user_id: USER_ID.to_string(),
                step_key: STEP_KEY.to_string(),
                step_type: NovelAutopilotStepType::BookPolish,
                phase: NovelAutopilotPhase::BookPolish,
                chapter_id: Some(CHAPTER_ID.to_string()),
                chapter_number: Some(1),
                run_epoch: running.epoch,
                input_digest: "book-polish-input-digest".to_string(),
            },
            expected_run_version: running.version,
            background_task_id: Some(TASK_ID.to_string()),
        },
    )
    .await
    .expect("claim book polish step")
}

async fn commit(
    db: &DatabaseConnection,
    claimed: &ClaimedNovelAutopilotStep,
    expected_chapter: &ChapterBusinessSnapshot,
    rewrite: &BookReviewRewriteReference,
    value: NovelAutopilotBookPolishCommit,
) -> Result<ClaimedNovelAutopilotStep, NovelAutopilotRepositoryError> {
    NovelAutopilotRepository::commit_book_polish_step(
        db,
        &claimed.step.id,
        USER_ID,
        claimed.run.version,
        claimed.run.epoch,
        STEP_KEY,
        Some(TASK_ID),
        expected_chapter,
        rewrite,
        value,
    )
    .await
}

#[tokio::test]
async fn book_polish_commit_is_atomic_restart_safe_and_requeues_analysis() {
    let db = setup_db().await;
    insert_business_facts(&db).await;
    let rewrite = expected_rewrite();
    let next_rewrite = following_rewrite();
    let claimed = claim_polish_step(&db, vec![rewrite.clone(), next_rewrite.clone()]).await;
    let expected_chapter = ChapterBusinessSnapshot::load(&db, PROJECT_ID, CHAPTER_ID)
        .await
        .expect("load expected chapter snapshot");

    let committed = commit(
        &db,
        &claimed,
        &expected_chapter,
        &rewrite,
        accepted_commit(),
    )
    .await
    .expect("commit book polish step");

    assert_eq!(
        committed.step.status,
        NovelAutopilotStepStatus::Completed.as_str()
    );
    assert_eq!(
        committed.step.quality_decision.as_deref(),
        Some(NovelAutopilotQualityDecision::Accept.as_str())
    );
    assert_eq!(
        committed.step.result_digest.as_deref(),
        Some("book-polish-result-digest")
    );
    assert!(committed.run.current_step.is_none());
    assert!(committed.run.active_background_task_id.is_none());
    assert_eq!(committed.run.completed_chapters, 1);
    assert_eq!(committed.run.total_word_count, 28);
    assert_eq!(
        serde_json::from_value::<Vec<BookReviewRewriteReference>>(
            committed.run.pending_rewrites.clone()
        )
        .expect("deserialize remaining rewrites"),
        vec![next_rewrite]
    );

    let chapter = chapter::Entity::find_by_id(CHAPTER_ID)
        .one(&db)
        .await
        .expect("load polished chapter")
        .expect("polished chapter exists");
    assert_eq!(chapter.content.as_deref(), Some(POLISHED_CONTENT));
    assert_eq!(chapter.word_count, 28);
    assert_eq!(chapter.status, "completed");

    let restarted_run = NovelAutopilotRepository::find_owned(&db, &committed.run.id, USER_ID)
        .await
        .expect("reload run after simulated restart");
    let facts = load_novel_autopilot_business_facts(
        &db,
        PROJECT_ID,
        USER_ID,
        1,
        1,
        NovelAutopilotQualityFactScope::AllChapters,
    )
    .await
    .expect("reload business facts after polish");
    assert_eq!(
        facts.pending_analysis_chapter_id.as_deref(),
        Some(CHAPTER_ID)
    );
    assert_eq!(facts.pending_analysis_chapter_number, Some(1));
    assert!(facts.pending_repair_chapter_id.is_none());
    assert_eq!(restarted_run.total_word_count, 28);

    assert_eq!(
        commit(
            &db,
            &claimed,
            &expected_chapter,
            &rewrite,
            accepted_commit(),
        )
        .await
        .unwrap_err(),
        NovelAutopilotRepositoryError::StaleVersion
    );
}

#[tokio::test]
async fn book_polish_rejects_late_result_after_manual_chapter_change() {
    let db = setup_db().await;
    insert_business_facts(&db).await;
    let rewrite = expected_rewrite();
    let claimed = claim_polish_step(&db, vec![rewrite.clone()]).await;
    let expected_chapter = ChapterBusinessSnapshot::load(&db, PROJECT_ID, CHAPTER_ID)
        .await
        .expect("load expected chapter snapshot");

    chapter::ActiveModel {
        id: Set(CHAPTER_ID.to_string()),
        content: Set(Some(MANUAL_CONTENT.to_string())),
        word_count: Set(23),
        updated_at: Set(Some(test_time() + chrono::Duration::seconds(1))),
        ..Default::default()
    }
    .update(&db)
    .await
    .expect("simulate manual chapter change");

    assert_eq!(
        commit(
            &db,
            &claimed,
            &expected_chapter,
            &rewrite,
            accepted_commit(),
        )
        .await
        .unwrap_err(),
        NovelAutopilotRepositoryError::BusinessDataChanged
    );
    let chapter = chapter::Entity::find_by_id(CHAPTER_ID)
        .one(&db)
        .await
        .expect("reload manually changed chapter")
        .expect("chapter exists");
    assert_eq!(chapter.content.as_deref(), Some(MANUAL_CONTENT));
    let run = NovelAutopilotRepository::find_owned(&db, &claimed.run.id, USER_ID)
        .await
        .expect("reload run after rejected late result");
    assert_eq!(
        serde_json::from_value::<Vec<BookReviewRewriteReference>>(run.pending_rewrites)
            .expect("deserialize preserved rewrite queue"),
        vec![rewrite]
    );
}

#[tokio::test]
async fn book_polish_repository_validates_digest_and_analysis_fence() {
    let db = setup_db().await;
    insert_business_facts(&db).await;
    let rewrite = expected_rewrite();
    let claimed = claim_polish_step(&db, vec![rewrite.clone()]).await;
    let expected_chapter = ChapterBusinessSnapshot::load(&db, PROJECT_ID, CHAPTER_ID)
        .await
        .expect("load expected chapter snapshot");

    let mut mismatched = accepted_commit();
    mismatched.content_digest = chapter_content_digest("另一份正文");
    assert!(matches!(
        commit(&db, &claimed, &expected_chapter, &rewrite, mismatched)
            .await
            .unwrap_err(),
        NovelAutopilotRepositoryError::InvalidConfig {
            field: "content_digest",
            ..
        }
    ));

    let unchanged = NovelAutopilotBookPolishCommit {
        content: INITIAL_CONTENT.to_string(),
        word_count: 15,
        content_digest: chapter_content_digest(INITIAL_CONTENT),
        result_digest: "unchanged-result-digest".to_string(),
    };
    assert!(matches!(
        commit(&db, &claimed, &expected_chapter, &rewrite, unchanged)
            .await
            .unwrap_err(),
        NovelAutopilotRepositoryError::InvalidConfig {
            field: "content_unchanged",
            ..
        }
    ));

    plot_analysis::ActiveModel {
        id: Set(ANALYSIS_ID.to_string()),
        source_content_digest: Set(Some(chapter_content_digest("被替换的分析正文"))),
        ..Default::default()
    }
    .update(&db)
    .await
    .expect("invalidate expected analysis digest");
    assert_eq!(
        commit(
            &db,
            &claimed,
            &expected_chapter,
            &rewrite,
            accepted_commit(),
        )
        .await
        .unwrap_err(),
        NovelAutopilotRepositoryError::BusinessDataChanged
    );
}

#[tokio::test]
async fn book_polish_failure_marks_step_failed_and_clears_run_cursor() {
    let db = setup_db().await;
    insert_business_facts(&db).await;
    let rewrite = expected_rewrite();
    let claimed = claim_polish_step(&db, vec![rewrite.clone()]).await;

    let terminal = NovelAutopilotRepository::finish_book_polish_failure(
        &db,
        &claimed.step.id,
        USER_ID,
        claimed.run.version,
        claimed.run.epoch,
        STEP_KEY,
        Some(TASK_ID),
        "book_polish_provider_failed",
        true,
        true,
        NovelAutopilotQualityDecision::ManualReview,
    )
    .await
    .expect("finish book polish failure");

    assert_eq!(
        terminal.run.status,
        NovelAutopilotRunStatus::WaitingHuman.as_str()
    );
    assert!(terminal.run.current_step.is_none());
    assert!(terminal.run.active_background_task_id.is_none());
    assert_eq!(
        terminal.run.last_error_code.as_deref(),
        Some("book_polish_provider_failed")
    );
    assert_eq!(terminal.run.consecutive_provider_failures, 1);
    assert_eq!(terminal.run.consecutive_quality_failures, 0);
    assert_eq!(
        serde_json::from_value::<Vec<BookReviewRewriteReference>>(
            terminal.run.pending_rewrites.clone()
        )
        .expect("deserialize preserved rewrite queue"),
        vec![rewrite]
    );
    assert_eq!(
        terminal.step.status,
        NovelAutopilotStepStatus::Failed.as_str()
    );
    assert_eq!(
        terminal.step.quality_decision.as_deref(),
        Some(NovelAutopilotQualityDecision::ManualReview.as_str())
    );
    assert_eq!(
        terminal.step.error_code.as_deref(),
        Some("book_polish_provider_failed")
    );
    assert!(terminal.step.completed_at.is_some());
}

#[tokio::test]
async fn malformed_pending_rewrites_fail_closed_in_completion_facts() {
    let db = setup_db().await;
    insert_business_facts(&db).await;
    let claimed = claim_polish_step(&db, vec![expected_rewrite()]).await;
    let run = novel_autopilot_run::ActiveModel {
        id: Set(claimed.run.id),
        pending_rewrites: Set(json!({"malformed": true})),
        ..Default::default()
    }
    .update(&db)
    .await
    .expect("persist malformed rewrite payload");
    let mut facts = NovelAutopilotBusinessFacts {
        target_chapter_count: 1,
        completed_chapter_count: 1,
        ..NovelAutopilotBusinessFacts::default()
    };

    assert_eq!(
        enrich_novel_autopilot_completion_facts(
            &db,
            &run,
            USER_ID,
            &NovelAutopilotRunConfig::default(),
            &mut facts,
        )
        .await
        .unwrap_err(),
        NovelAutopilotFactsError::InvalidPendingRewrites
    );
    assert!(!facts.book_polish_completed);
}
