use chrono::{NaiveDate, Utc};
use sea_orm::{
    ActiveModelTrait, ConnectionTrait, Database, DatabaseConnection, DbBackend, EntityTrait,
    IntoActiveModel, PaginatorTrait, Schema, Set, Statement,
};
use serde_json::json;

use crate::models::{
    chapter, chapter_draft_attempt, generation_history, novel_autopilot_run,
    novel_autopilot_step_run, plot_analysis, project,
};
use crate::services::chapter_content_digest_service::chapter_content_digest;

use super::{
    chapter_analysis_repository::NovelAutopilotChapterAnalysisCommit,
    chapter_repair_repository::{
        NovelAutopilotChapterRepairCommit, NovelAutopilotChapterRepairFailureEvidence,
    },
    chapter_repository::NovelAutopilotChapterGenerateRetryCandidate,
    repository::{
        ChapterBusinessSnapshot, ClaimedNovelAutopilotStep, CreateNovelAutopilotRun,
        CreateNovelAutopilotStepAttempt, NovelAutopilotChapterGenerateCommit,
        NovelAutopilotManualReviewCandidate, NovelAutopilotRepository,
        NovelAutopilotRepositoryError, PrepareAndClaimNovelAutopilotStep,
    },
    types::{
        NovelAutopilotFailureCounterKind, NovelAutopilotPhase, NovelAutopilotQualityDecision,
        NovelAutopilotRunConfig, NovelAutopilotRunStatus, NovelAutopilotStepStatus,
        NovelAutopilotStepType,
    },
};

const PROJECT_ID: &str = "project-chapter";
const USER_ID: &str = "owner-chapter";
const CHAPTER_ID: &str = "chapter-1";
const CHAPTER_NUMBER: i32 = 1;
const STEP_KEY: &str = "chapter:1:generate";
const TASK_ID: &str = "chapter-generate-task";
const ANALYSIS_STEP_KEY: &str = "chapter:1:analyze";
const ANALYSIS_TASK_ID: &str = "chapter-analysis-task";
const REPAIR_STEP_KEY: &str = "chapter:1:repair";
const REPAIR_TASK_ID: &str = "chapter-repair-task";
const INITIAL_CONTENT: &str = "旧正文，等待生成。";
const INITIAL_WORD_COUNT: i32 = 8;

async fn setup_repository_db() -> DatabaseConnection {
    let db = Database::connect("sqlite::memory:")
        .await
        .expect("connect chapter repository SQLite memory database");
    let builder = DbBackend::Sqlite;
    let schema = Schema::new(builder);

    for statement in [
        builder.build(&schema.create_table_from_entity(project::Entity)),
        builder.build(&schema.create_table_from_entity(chapter::Entity)),
        builder.build(&schema.create_table_from_entity(chapter_draft_attempt::Entity)),
        builder.build(&schema.create_table_from_entity(generation_history::Entity)),
        builder.build(&schema.create_table_from_entity(plot_analysis::Entity)),
        builder.build(&schema.create_table_from_entity(novel_autopilot_run::Entity)),
        builder.build(&schema.create_table_from_entity(novel_autopilot_step_run::Entity)),
    ] {
        db.execute(statement)
            .await
            .expect("create chapter repository test table");
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

fn test_time() -> chrono::NaiveDateTime {
    NaiveDate::from_ymd_opt(2026, 7, 19)
        .expect("valid date")
        .and_hms_opt(8, 0, 0)
        .expect("valid time")
}

async fn insert_project(db: &DatabaseConnection) {
    let now = test_time();
    project::ActiveModel {
        id: Set(PROJECT_ID.to_string()),
        user_id: Set(USER_ID.to_string()),
        title: Set("章节生成测试项目".to_string()),
        target_words: Set(100_000),
        current_words: Set(0),
        status: Set("writing".to_string()),
        wizard_status: Set("completed".to_string()),
        wizard_step: Set(4),
        outline_mode: Set("one-to-one".to_string()),
        character_count: Set(0),
        created_at: Set(now),
        updated_at: Set(Some(now)),
        ..Default::default()
    }
    .insert(db)
    .await
    .expect("insert chapter test project");
}

async fn insert_chapter(db: &DatabaseConnection) {
    let now = test_time();
    chapter::ActiveModel {
        id: Set(CHAPTER_ID.to_string()),
        project_id: Set(PROJECT_ID.to_string()),
        chapter_number: Set(CHAPTER_NUMBER),
        title: Set("星门初启".to_string()),
        content: Set(Some(INITIAL_CONTENT.to_string())),
        summary: Set(Some("主角发现沉睡的星门".to_string())),
        word_count: Set(INITIAL_WORD_COUNT),
        status: Set("pending".to_string()),
        outline_id: Set(Some("outline-1".to_string())),
        sub_index: Set(0),
        expansion_plan: Set(Some("保持悬念并埋下伏笔".to_string())),
        created_at: Set(now),
        updated_at: Set(Some(now)),
    }
    .insert(db)
    .await
    .expect("insert chapter test chapter");
}

fn create_input() -> CreateNovelAutopilotRun {
    CreateNovelAutopilotRun {
        project_id: PROJECT_ID.to_string(),
        user_id: USER_ID.to_string(),
        total_chapters: 3,
        config: NovelAutopilotRunConfig::default(),
    }
}

async fn claim_chapter_step(
    db: &DatabaseConnection,
    step_type: NovelAutopilotStepType,
    task_id: &str,
) -> ClaimedNovelAutopilotStep {
    claim_chapter_step_with_key(
        db,
        STEP_KEY,
        step_type,
        task_id,
        "chapter-generate-input-digest",
    )
    .await
}

async fn claim_chapter_analysis_step(db: &DatabaseConnection) -> ClaimedNovelAutopilotStep {
    claim_chapter_step_with_key(
        db,
        ANALYSIS_STEP_KEY,
        NovelAutopilotStepType::ChapterAnalyze,
        ANALYSIS_TASK_ID,
        "chapter-analysis-input-digest",
    )
    .await
}

async fn claim_chapter_repair_step(db: &DatabaseConnection) -> ClaimedNovelAutopilotStep {
    claim_chapter_step_with_key(
        db,
        REPAIR_STEP_KEY,
        NovelAutopilotStepType::ChapterRepair,
        REPAIR_TASK_ID,
        "chapter-repair-input-digest",
    )
    .await
}

async fn claim_chapter_step_with_key(
    db: &DatabaseConnection,
    step_key: &str,
    step_type: NovelAutopilotStepType,
    task_id: &str,
    input_digest: &str,
) -> ClaimedNovelAutopilotStep {
    let created = NovelAutopilotRepository::create_or_get_active(db, create_input())
        .await
        .expect("create chapter-generation run");
    let running = NovelAutopilotRepository::transition_owned(
        db,
        &created.run.id,
        USER_ID,
        created.run.version,
        NovelAutopilotRunStatus::Running,
    )
    .await
    .expect("start chapter-generation run");

    NovelAutopilotRepository::prepare_and_claim_step(
        db,
        PrepareAndClaimNovelAutopilotStep {
            attempt: CreateNovelAutopilotStepAttempt {
                run_id: running.id.clone(),
                user_id: USER_ID.to_string(),
                step_key: step_key.to_string(),
                step_type,
                phase: NovelAutopilotPhase::ChapterLoop,
                chapter_id: Some(CHAPTER_ID.to_string()),
                chapter_number: Some(CHAPTER_NUMBER as u32),
                run_epoch: running.epoch,
                input_digest: input_digest.to_string(),
            },
            expected_run_version: running.version,
            background_task_id: Some(task_id.to_string()),
        },
    )
    .await
    .expect("claim chapter-generation step")
}

fn accepted_analysis_commit() -> NovelAutopilotChapterAnalysisCommit {
    NovelAutopilotChapterAnalysisCommit {
        payload: json!({
            "plot_stage": "opening",
            "conflict": {"level": 6, "types": ["external"]},
            "emotional_arc": {"tone": "tense", "intensity": 7.0, "curve": []},
            "hooks": [],
            "foreshadows": [],
            "plot_points": [],
            "character_states": [],
            "scenes": [],
            "pacing": "balanced",
            "scores": {
                "overall": 8.6,
                "pacing": 8.2,
                "engagement": 8.8,
                "coherence": 8.5
            },
            "suggestions": ["保持悬念"]
        }),
        result_digest: "analysis-result-digest".to_string(),
        quality_decision: NovelAutopilotQualityDecision::Accept,
        waiting_human: false,
    }
}

async fn analysis_count(db: &DatabaseConnection) -> u64 {
    plot_analysis::Entity::find()
        .count(db)
        .await
        .expect("count chapter analyses")
}

async fn assert_analysis_not_applied(db: &DatabaseConnection, claimed: &ClaimedNovelAutopilotStep) {
    assert_eq!(analysis_count(db).await, 0);
    let run = NovelAutopilotRepository::find_owned(db, &claimed.run.id, USER_ID)
        .await
        .expect("reload analysis run");
    assert_eq!(run.version, claimed.run.version);
    assert_eq!(run.current_step.as_deref(), Some(ANALYSIS_STEP_KEY));
    assert_eq!(
        run.active_background_task_id.as_deref(),
        Some(ANALYSIS_TASK_ID)
    );
    let step = novel_autopilot_step_run::Entity::find_by_id(&claimed.step.id)
        .one(db)
        .await
        .expect("reload analysis step")
        .expect("analysis step exists");
    assert_eq!(step.status, NovelAutopilotStepStatus::Running.as_str());
    assert!(step.result_digest.is_none());
    assert!(step.quality_decision.is_none());
    assert!(step.error_code.is_none());
}

fn accepted_chapter_commit() -> NovelAutopilotChapterGenerateCommit {
    NovelAutopilotChapterGenerateCommit {
        content: "星门在雨夜开启，林舟听见遗迹深处传来的召唤。".to_string(),
        word_count: 20,
        status: "completed".to_string(),
        result_digest: "chapter-generate-result-digest".to_string(),
        quality_decision: NovelAutopilotQualityDecision::Accept.as_str().to_string(),
    }
}

fn accepted_repair_commit() -> NovelAutopilotChapterRepairCommit {
    NovelAutopilotChapterRepairCommit {
        content: "星门在暴雨中轰然开启，林舟循着古老呼唤踏入遗迹。".to_string(),
        word_count: 24,
        status: "completed".to_string(),
        result_digest: "chapter-repair-result-digest".to_string(),
    }
}

fn repair_failure_evidence(
    claimed: &ClaimedNovelAutopilotStep,
    expected: &ChapterBusinessSnapshot,
    content: &str,
) -> NovelAutopilotChapterRepairFailureEvidence {
    let result_digest = chapter_content_digest(content);
    NovelAutopilotChapterRepairFailureEvidence {
        expected_chapter: expected.clone(),
        draft_attempt: chapter_draft_attempt::Model {
            id: claimed.step.id.clone(),
            project_id: expected.project_id.clone(),
            chapter_id: Some(expected.chapter_id.clone()),
            batch_task_id: None,
            source: "novel_autopilot_chapter_repair".to_string(),
            attempt_state: "retry".to_string(),
            quality_gate_action: Some("retry".to_string()),
            quality_gate_decision: Some("retry".to_string()),
            word_count: i32::try_from(content.chars().count()).expect("word count fits i32"),
            summary_preview: Some(content.chars().take(220).collect()),
            content_preview: Some(content.chars().take(4000).collect()),
            quality_metrics: Some(json!({
                "overall_score": 6.4,
                "quality_gate": {"failed_metrics": ["outline_alignment_rate"]}
            })),
            repair_payload: Some(json!({
                "previous_content": INITIAL_CONTENT,
                "previous_word_count": INITIAL_WORD_COUNT,
                "candidate_full_content": content,
                "content_complete": true,
                "run_id": claimed.run.id,
                "run_epoch": claimed.run.epoch,
                "source_content_digest": expected.content_digest().expect("source digest"),
                "analysis_id": "repair-analysis-1",
                "candidate_content_digest": result_digest,
                "quality_gate_message": "需要继续修复章节转折",
            })),
            created_at: Some(test_time()),
        },
        result_digest,
    }
}

async fn assert_repair_failure_not_applied(
    db: &DatabaseConnection,
    claimed: &ClaimedNovelAutopilotStep,
) {
    let run = NovelAutopilotRepository::find_owned(db, &claimed.run.id, USER_ID)
        .await
        .expect("reload repair run");
    assert_eq!(run.version, claimed.run.version);
    assert_eq!(run.current_step.as_deref(), Some(REPAIR_STEP_KEY));
    assert_eq!(
        run.active_background_task_id.as_deref(),
        Some(REPAIR_TASK_ID)
    );
    let step = novel_autopilot_step_run::Entity::find_by_id(&claimed.step.id)
        .one(db)
        .await
        .expect("reload repair step")
        .expect("repair step exists");
    assert_eq!(step.status, NovelAutopilotStepStatus::Running.as_str());
    assert!(step.result_digest.is_none());
    assert!(step.quality_decision.is_none());
}

async fn load_chapter(db: &DatabaseConnection) -> chapter::Model {
    chapter::Entity::find_by_id(CHAPTER_ID)
        .one(db)
        .await
        .expect("reload chapter")
        .expect("chapter exists")
}

async fn assert_commit_not_applied(db: &DatabaseConnection, claimed: &ClaimedNovelAutopilotStep) {
    let chapter_after = load_chapter(db).await;
    assert_eq!(chapter_after.content.as_deref(), Some(INITIAL_CONTENT));
    assert_eq!(chapter_after.word_count, INITIAL_WORD_COUNT);
    assert_eq!(chapter_after.status, "pending");

    let run_after = NovelAutopilotRepository::find_owned(db, &claimed.run.id, USER_ID)
        .await
        .expect("reload run after rejected chapter commit");
    assert_eq!(run_after.version, claimed.run.version);
    assert_eq!(run_after.epoch, claimed.run.epoch);
    assert_eq!(run_after.current_step.as_deref(), Some(STEP_KEY));
    assert_eq!(
        run_after.active_background_task_id,
        claimed.run.active_background_task_id
    );
    assert_eq!(run_after.completed_chapters, claimed.run.completed_chapters);
    assert_eq!(run_after.total_word_count, claimed.run.total_word_count);

    let step_after = novel_autopilot_step_run::Entity::find_by_id(&claimed.step.id)
        .one(db)
        .await
        .expect("reload step after rejected chapter commit")
        .expect("step still exists");
    assert_eq!(
        step_after.status,
        NovelAutopilotStepStatus::Running.as_str()
    );
    assert_eq!(step_after.result_digest, None);
    assert_eq!(step_after.quality_decision, None);
}

fn manual_review_candidate(content: &str) -> NovelAutopilotManualReviewCandidate {
    NovelAutopilotManualReviewCandidate {
        content: content.to_string(),
        word_count: i32::try_from(content.chars().count()).expect("candidate word count fits i32"),
        result_digest: chapter_content_digest(content),
        quality_metrics: Some(json!({"overall_score": 6.8})),
        quality_gate_action: Some("manual_review".to_string()),
        quality_gate_message: Some("需要人工确认候选质量".to_string()),
    }
}

fn generate_retry_candidate(content: &str) -> NovelAutopilotChapterGenerateRetryCandidate {
    NovelAutopilotChapterGenerateRetryCandidate {
        content: content.to_string(),
        word_count: i32::try_from(content.chars().count()).expect("retry word count fits i32"),
        result_digest: chapter_content_digest(content),
        quality_diagnostic: json!({
            "overall_score": 66.1,
            "quality_gate_action": "auto_repair",
            "failed_metrics": [{"key": "pacing", "label": "节奏"}],
            "repair_targets": ["压缩说明段"]
        }),
        quality_gate_action: Some("auto_repair".to_string()),
        quality_gate_message: Some("继续修复节奏与转折".to_string()),
    }
}

#[tokio::test]
async fn chapter_generate_quality_retry_atomically_persists_scoped_candidate() {
    let db = setup_repository_db().await;
    insert_project(&db).await;
    insert_chapter(&db).await;
    let claimed = claim_chapter_step(&db, NovelAutopilotStepType::ChapterGenerate, TASK_ID).await;
    let expected = ChapterBusinessSnapshot::load(&db, PROJECT_ID, CHAPTER_ID)
        .await
        .expect("load chapter snapshot");
    let candidate_content = "星门在暴雨中开启，林舟听见遗迹深处的回声。";
    let candidate = generate_retry_candidate(candidate_content);

    let terminal = NovelAutopilotRepository::persist_chapter_generate_retry_candidate(
        &db,
        &claimed.step.id,
        USER_ID,
        claimed.run.version,
        claimed.run.epoch,
        STEP_KEY,
        Some(TASK_ID),
        &expected,
        NovelAutopilotQualityDecision::AutoRepair,
        "chapter_quality_auto_repair",
        candidate.clone(),
    )
    .await
    .expect("persist generate retry candidate");

    assert_eq!(
        terminal.run.status,
        NovelAutopilotRunStatus::Running.as_str()
    );
    assert_eq!(terminal.run.version, claimed.run.version + 1);
    assert!(terminal.run.current_step.is_none());
    assert!(terminal.run.active_background_task_id.is_none());
    assert_eq!(
        terminal.run.last_error_code.as_deref(),
        Some("chapter_quality_auto_repair")
    );
    assert_eq!(terminal.run.consecutive_quality_failures, 1);
    assert_eq!(
        terminal.step.status,
        NovelAutopilotStepStatus::Failed.as_str()
    );
    assert_eq!(
        terminal.step.result_digest.as_deref(),
        Some(candidate.result_digest.as_str())
    );
    assert_eq!(
        terminal.step.quality_decision.as_deref(),
        Some(NovelAutopilotQualityDecision::AutoRepair.as_str())
    );

    let attempt = chapter_draft_attempt::Entity::find_by_id(&claimed.step.id)
        .one(&db)
        .await
        .expect("load retry evidence")
        .expect("retry evidence exists");
    assert_eq!(attempt.source, "novel_autopilot_chapter_generate");
    assert_eq!(attempt.attempt_state, "retry");
    assert_eq!(attempt.word_count, candidate.word_count);
    assert_eq!(attempt.quality_metrics, Some(candidate.quality_diagnostic));
    let payload = attempt.repair_payload.expect("retry payload");
    assert_eq!(payload["run_id"], claimed.run.id);
    assert_eq!(payload["run_epoch"], claimed.run.epoch);
    assert_eq!(payload["step_attempt"], claimed.step.attempt);
    assert_eq!(
        payload["source_chapter_snapshot_digest"],
        expected.snapshot_digest()
    );
    assert_eq!(payload["candidate_full_content"], candidate_content);
    assert_eq!(payload["candidate_content_digest"], candidate.result_digest);
    assert_eq!(payload["content_complete"], true);
    assert_eq!(
        load_chapter(&db).await.content.as_deref(),
        Some(INITIAL_CONTENT)
    );
}

#[tokio::test]
async fn chapter_generate_quality_retry_rolls_back_when_chapter_snapshot_changes() {
    let db = setup_repository_db().await;
    insert_project(&db).await;
    insert_chapter(&db).await;
    let claimed = claim_chapter_step(&db, NovelAutopilotStepType::ChapterGenerate, TASK_ID).await;
    let expected = ChapterBusinessSnapshot::load(&db, PROJECT_ID, CHAPTER_ID)
        .await
        .expect("load chapter snapshot");
    let mut changed = load_chapter(&db).await.into_active_model();
    changed.title = Set("人工修改后的标题".to_string());
    changed
        .update(&db)
        .await
        .expect("modify chapter after snapshot");

    let error = NovelAutopilotRepository::persist_chapter_generate_retry_candidate(
        &db,
        &claimed.step.id,
        USER_ID,
        claimed.run.version,
        claimed.run.epoch,
        STEP_KEY,
        Some(TASK_ID),
        &expected,
        NovelAutopilotQualityDecision::Retry,
        "chapter_quality_retry",
        generate_retry_candidate("这份候选不能覆盖人工修改。"),
    )
    .await
    .expect_err("stale chapter snapshot must reject retry evidence");

    assert_eq!(error, NovelAutopilotRepositoryError::BusinessDataChanged);
    assert_eq!(
        chapter_draft_attempt::Entity::find()
            .count(&db)
            .await
            .expect("count retry evidence"),
        0
    );
    assert_commit_not_applied(&db, &claimed).await;
}

#[tokio::test]
async fn chapter_generate_quality_retry_rejects_invalid_digest_before_writes() {
    let db = setup_repository_db().await;
    insert_project(&db).await;
    insert_chapter(&db).await;
    let claimed = claim_chapter_step(&db, NovelAutopilotStepType::ChapterGenerate, TASK_ID).await;
    let expected = ChapterBusinessSnapshot::load(&db, PROJECT_ID, CHAPTER_ID)
        .await
        .expect("load chapter snapshot");
    let mut candidate = generate_retry_candidate("摘要错误的候选正文。");
    candidate.result_digest = "sha256:tampered".to_string();

    let error = NovelAutopilotRepository::persist_chapter_generate_retry_candidate(
        &db,
        &claimed.step.id,
        USER_ID,
        claimed.run.version,
        claimed.run.epoch,
        STEP_KEY,
        Some(TASK_ID),
        &expected,
        NovelAutopilotQualityDecision::Retry,
        "chapter_quality_retry",
        candidate,
    )
    .await
    .expect_err("invalid digest must reject retry evidence");

    assert!(matches!(
        error,
        NovelAutopilotRepositoryError::InvalidConfig {
            field: "retry_candidate_result_digest",
            ..
        }
    ));
    assert_eq!(
        chapter_draft_attempt::Entity::find()
            .count(&db)
            .await
            .expect("count retry evidence"),
        0
    );
    assert_commit_not_applied(&db, &claimed).await;
}

async fn resume_manual_candidate_for_accept(
    db: &DatabaseConnection,
    waiting: &novel_autopilot_run::Model,
    task_id: &str,
) -> novel_autopilot_run::Model {
    let queued = NovelAutopilotRepository::transition_owned(
        db,
        &waiting.id,
        USER_ID,
        waiting.version,
        NovelAutopilotRunStatus::Queued,
    )
    .await
    .expect("queue human decision tick");
    let bound = NovelAutopilotRepository::set_active_background_task_owned(
        db,
        &queued.id,
        USER_ID,
        queued.version,
        queued.epoch,
        Some(task_id),
    )
    .await
    .expect("bind human decision task");
    NovelAutopilotRepository::transition_owned(
        db,
        &bound.id,
        USER_ID,
        bound.version,
        NovelAutopilotRunStatus::Running,
    )
    .await
    .expect("start human decision tick")
}

#[tokio::test]
async fn manual_review_candidate_persists_content_outside_durable_run_and_step() {
    let db = setup_repository_db().await;
    insert_project(&db).await;
    insert_chapter(&db).await;
    let claimed = claim_chapter_step(&db, NovelAutopilotStepType::ChapterGenerate, TASK_ID).await;
    let expected = ChapterBusinessSnapshot::load(&db, PROJECT_ID, CHAPTER_ID)
        .await
        .expect("load chapter business snapshot");
    let candidate_content = "候选正文保存在业务草稿表，等待人工接受。";

    let terminal = NovelAutopilotRepository::persist_chapter_manual_review_candidate(
        &db,
        &claimed.step.id,
        USER_ID,
        claimed.run.version,
        claimed.run.epoch,
        STEP_KEY,
        NovelAutopilotStepType::ChapterGenerate,
        Some(TASK_ID),
        &expected,
        NovelAutopilotStepStatus::Skipped,
        "chapter_quality_manual_review",
        manual_review_candidate(candidate_content),
    )
    .await
    .expect("persist chapter manual-review candidate");

    assert_eq!(
        terminal.run.status,
        NovelAutopilotRunStatus::WaitingHuman.as_str()
    );
    assert!(terminal.run.current_step.is_none());
    assert!(terminal.run.active_background_task_id.is_none());
    assert_eq!(terminal.step.id, claimed.step.id);
    assert_eq!(
        terminal.step.status,
        NovelAutopilotStepStatus::Skipped.as_str()
    );
    assert_eq!(
        terminal.step.error_code.as_deref(),
        Some("chapter_quality_manual_review")
    );
    assert_eq!(
        terminal.step.quality_decision.as_deref(),
        Some(NovelAutopilotQualityDecision::ManualReview.as_str())
    );
    assert!(!terminal
        .run
        .config_snapshot
        .to_string()
        .contains(candidate_content));

    let candidate = chapter_draft_attempt::Entity::find_by_id(&claimed.step.id)
        .one(&db)
        .await
        .expect("load manual-review candidate")
        .expect("manual-review candidate exists");
    assert_eq!(candidate.source, "novel_book_autopilot");
    assert_eq!(candidate.attempt_state, "waiting_human");
    assert_eq!(candidate.batch_task_id, None);
    assert_eq!(
        candidate
            .repair_payload
            .as_ref()
            .and_then(|payload| payload.get("quality_gate_message"))
            .and_then(serde_json::Value::as_str),
        Some("需要人工确认候选质量")
    );
    assert_eq!(
        candidate
            .repair_payload
            .as_ref()
            .and_then(|payload| payload.get("candidate_full_content"))
            .and_then(serde_json::Value::as_str),
        Some(candidate_content)
    );
    assert_eq!(
        candidate
            .repair_payload
            .as_ref()
            .and_then(|payload| payload.get("candidate_chapter_status"))
            .and_then(serde_json::Value::as_str),
        Some("completed")
    );
    assert_eq!(
        load_chapter(&db).await.content.as_deref(),
        Some(INITIAL_CONTENT)
    );
}

#[tokio::test]
async fn active_task_execution_failure_atomically_finishes_step_and_updates_run_time() {
    let db = setup_repository_db().await;
    insert_project(&db).await;
    insert_chapter(&db).await;
    let claimed = claim_chapter_step(&db, NovelAutopilotStepType::ChapterGenerate, TASK_ID).await;
    let previous_updated_at = claimed.run.updated_at;

    let waiting = NovelAutopilotRepository::fail_active_task_and_wait_owned(
        &db,
        &claimed.run.id,
        USER_ID,
        claimed.run.epoch,
        TASK_ID,
        "novel_autopilot_execution_failed",
    )
    .await
    .expect("converge execution failure");

    assert_eq!(
        waiting.status,
        NovelAutopilotRunStatus::WaitingHuman.as_str()
    );
    assert_eq!(
        waiting.last_error_code.as_deref(),
        Some("novel_autopilot_execution_failed")
    );
    assert!(waiting.current_step.is_none());
    assert!(waiting.active_background_task_id.is_none());
    assert!(waiting.updated_at >= previous_updated_at);

    let terminal_step = novel_autopilot_step_run::Entity::find_by_id(&claimed.step.id)
        .one(&db)
        .await
        .expect("load terminal step")
        .expect("terminal step exists");
    assert_eq!(
        terminal_step.status,
        NovelAutopilotStepStatus::Failed.as_str()
    );
    assert_eq!(
        terminal_step.error_code.as_deref(),
        Some("novel_autopilot_execution_failed")
    );
    assert!(terminal_step.completed_at.is_some());
    assert_eq!(
        chapter_draft_attempt::Entity::find()
            .count(&db)
            .await
            .expect("count execution failure candidates"),
        0
    );
}

#[tokio::test]
async fn manual_review_candidate_rejects_mismatched_content_digest_without_writes() {
    let db = setup_repository_db().await;
    insert_project(&db).await;
    insert_chapter(&db).await;
    let claimed = claim_chapter_step(&db, NovelAutopilotStepType::ChapterGenerate, TASK_ID).await;
    let expected = ChapterBusinessSnapshot::load(&db, PROJECT_ID, CHAPTER_ID)
        .await
        .expect("load chapter business snapshot");
    let mut candidate = manual_review_candidate("摘要必须绑定完整候选正文。");
    candidate.result_digest = chapter_content_digest("另一份正文");

    let error = NovelAutopilotRepository::persist_chapter_manual_review_candidate(
        &db,
        &claimed.step.id,
        USER_ID,
        claimed.run.version,
        claimed.run.epoch,
        STEP_KEY,
        NovelAutopilotStepType::ChapterGenerate,
        Some(TASK_ID),
        &expected,
        NovelAutopilotStepStatus::Skipped,
        "chapter_quality_manual_review",
        candidate,
    )
    .await
    .expect_err("mismatched candidate digest must be rejected");

    assert!(matches!(
        error,
        NovelAutopilotRepositoryError::InvalidConfig {
            field: "candidate_result_digest",
            ..
        }
    ));
    assert_eq!(
        chapter_draft_attempt::Entity::find()
            .count(&db)
            .await
            .expect("count draft attempts"),
        0
    );
    let run = NovelAutopilotRepository::find_owned(&db, &claimed.run.id, USER_ID)
        .await
        .expect("reload run");
    assert_eq!(run.version, claimed.run.version);
    assert_eq!(run.current_step.as_deref(), Some(STEP_KEY));
}

#[tokio::test]
async fn accepting_manual_review_candidate_rejects_tampered_digest_without_chapter_write() {
    let db = setup_repository_db().await;
    insert_project(&db).await;
    insert_chapter(&db).await;
    let claimed = claim_chapter_step(&db, NovelAutopilotStepType::ChapterGenerate, TASK_ID).await;
    let expected = ChapterBusinessSnapshot::load(&db, PROJECT_ID, CHAPTER_ID)
        .await
        .expect("load chapter business snapshot");
    let waiting = NovelAutopilotRepository::persist_chapter_manual_review_candidate(
        &db,
        &claimed.step.id,
        USER_ID,
        claimed.run.version,
        claimed.run.epoch,
        STEP_KEY,
        NovelAutopilotStepType::ChapterGenerate,
        Some(TASK_ID),
        &expected,
        NovelAutopilotStepStatus::Skipped,
        "chapter_quality_manual_review",
        manual_review_candidate("候选正文摘要被篡改后不得接受。"),
    )
    .await
    .expect("persist candidate");
    let candidate = chapter_draft_attempt::Entity::find_by_id(&claimed.step.id)
        .one(&db)
        .await
        .expect("load candidate")
        .expect("candidate exists");
    let mut candidate = candidate.into_active_model();
    let mut repair_payload = candidate
        .repair_payload
        .take()
        .expect("repair payload is set")
        .expect("repair payload exists");
    repair_payload["candidate_content_digest"] = json!(chapter_content_digest("被替换的正文"));
    candidate.repair_payload = Set(Some(repair_payload));
    candidate
        .update(&db)
        .await
        .expect("tamper candidate digest");

    let decision_task_id = "chapter-candidate-tampered-accept-task";
    let running = resume_manual_candidate_for_accept(&db, &waiting.run, decision_task_id).await;
    let error = NovelAutopilotRepository::accept_chapter_manual_review_candidate(
        &db,
        &running.id,
        &claimed.step.id,
        USER_ID,
        running.version,
        running.epoch,
        Some(decision_task_id),
    )
    .await
    .expect_err("tampered digest must reject accept");

    assert!(matches!(
        error,
        NovelAutopilotRepositoryError::InvalidConfig {
            field: "candidate_result_digest",
            ..
        }
    ));
    assert_eq!(
        load_chapter(&db).await.content.as_deref(),
        Some(INITIAL_CONTENT)
    );
}

#[tokio::test]
async fn accepting_manual_review_candidate_commits_chapter_history_and_run_progress() {
    let db = setup_repository_db().await;
    insert_project(&db).await;
    insert_chapter(&db).await;
    let claimed = claim_chapter_step(&db, NovelAutopilotStepType::ChapterGenerate, TASK_ID).await;
    let expected = ChapterBusinessSnapshot::load(&db, PROJECT_ID, CHAPTER_ID)
        .await
        .expect("load chapter business snapshot");
    let candidate_content = "人工确认后的候选正文已经安全提交到章节。";
    let waiting = NovelAutopilotRepository::persist_chapter_manual_review_candidate(
        &db,
        &claimed.step.id,
        USER_ID,
        claimed.run.version,
        claimed.run.epoch,
        STEP_KEY,
        NovelAutopilotStepType::ChapterGenerate,
        Some(TASK_ID),
        &expected,
        NovelAutopilotStepStatus::Skipped,
        "chapter_quality_manual_review",
        manual_review_candidate(candidate_content),
    )
    .await
    .expect("persist chapter manual-review candidate");
    let decision_task_id = "chapter-candidate-accept-task";
    let running = resume_manual_candidate_for_accept(&db, &waiting.run, decision_task_id).await;

    let accepted = NovelAutopilotRepository::accept_chapter_manual_review_candidate(
        &db,
        &running.id,
        &claimed.step.id,
        USER_ID,
        running.version,
        running.epoch,
        Some(decision_task_id),
    )
    .await
    .expect("accept chapter manual-review candidate");

    let expected_word_count = i32::try_from(candidate_content.chars().count()).unwrap();
    let chapter_after = load_chapter(&db).await;
    assert_eq!(chapter_after.content.as_deref(), Some(candidate_content));
    assert_eq!(chapter_after.word_count, expected_word_count);
    assert_eq!(
        chapter_after.status,
        NovelAutopilotStepStatus::Completed.as_str()
    );
    assert_eq!(accepted.candidate_id, claimed.step.id);
    assert_eq!(accepted.word_count, expected_word_count);
    assert_eq!(
        accepted.run.completed_chapters,
        claimed.run.completed_chapters + 1
    );
    assert_eq!(
        accepted.run.total_word_count,
        claimed.run.total_word_count + i64::from(expected_word_count)
    );
    assert!(accepted.run.active_background_task_id.is_none());

    let candidate = chapter_draft_attempt::Entity::find_by_id(&claimed.step.id)
        .one(&db)
        .await
        .expect("load accepted candidate")
        .expect("accepted candidate exists");
    assert_eq!(candidate.attempt_state, "accepted");
    assert_eq!(
        generation_history::Entity::find()
            .count(&db)
            .await
            .expect("count candidate apply history"),
        1
    );
}

#[tokio::test]
async fn accepting_generation_attempts_exhausted_candidate_commits_chapter_history_and_run_progress(
) {
    let db = setup_repository_db().await;
    insert_project(&db).await;
    insert_chapter(&db).await;
    let claimed = claim_chapter_step(&db, NovelAutopilotStepType::ChapterGenerate, TASK_ID).await;
    let expected = ChapterBusinessSnapshot::load(&db, PROJECT_ID, CHAPTER_ID)
        .await
        .expect("load chapter business snapshot");
    let candidate_content = "质量重试耗尽后保存的候选正文依然可供人工接受。";
    let waiting = NovelAutopilotRepository::persist_chapter_manual_review_candidate(
        &db,
        &claimed.step.id,
        USER_ID,
        claimed.run.version,
        claimed.run.epoch,
        STEP_KEY,
        NovelAutopilotStepType::ChapterGenerate,
        Some(TASK_ID),
        &expected,
        NovelAutopilotStepStatus::Skipped,
        "chapter_generation_attempts_exhausted",
        manual_review_candidate(candidate_content),
    )
    .await
    .expect("persist exhausted candidate");
    let decision_task_id = "chapter-generation-exhausted-accept-task";
    let running = resume_manual_candidate_for_accept(&db, &waiting.run, decision_task_id).await;

    let accepted = NovelAutopilotRepository::accept_chapter_manual_review_candidate(
        &db,
        &running.id,
        &claimed.step.id,
        USER_ID,
        running.version,
        running.epoch,
        Some(decision_task_id),
    )
    .await
    .expect("accept exhausted candidate");

    let expected_word_count = i32::try_from(candidate_content.chars().count()).unwrap();
    let chapter_after = load_chapter(&db).await;
    assert_eq!(chapter_after.content.as_deref(), Some(candidate_content));
    assert_eq!(chapter_after.word_count, expected_word_count);
    assert_eq!(
        chapter_after.status,
        NovelAutopilotStepStatus::Completed.as_str()
    );
    assert_eq!(accepted.candidate_id, claimed.step.id);
    assert_eq!(accepted.word_count, expected_word_count);
    assert_eq!(
        accepted.run.completed_chapters,
        claimed.run.completed_chapters + 1
    );
    assert_eq!(
        accepted.run.total_word_count,
        claimed.run.total_word_count + i64::from(expected_word_count)
    );
    assert!(accepted.run.active_background_task_id.is_none());

    let candidate = chapter_draft_attempt::Entity::find_by_id(&claimed.step.id)
        .one(&db)
        .await
        .expect("load exhausted candidate")
        .expect("exhausted candidate exists");
    assert_eq!(candidate.attempt_state, "accepted");
}

#[tokio::test]
async fn accepting_repair_manual_review_candidate_updates_only_word_count_delta() {
    let db = setup_repository_db().await;
    insert_project(&db).await;
    insert_chapter(&db).await;
    let claimed = claim_chapter_repair_step(&db).await;
    let expected = ChapterBusinessSnapshot::load(&db, PROJECT_ID, CHAPTER_ID)
        .await
        .expect("load chapter business snapshot");
    let candidate_content = "修复后的正文保留原有情节，并补充星门开启后的行动与冲突。";
    let expected_word_count =
        i32::try_from(candidate_content.chars().count()).expect("candidate word count fits i32");
    let waiting = NovelAutopilotRepository::persist_chapter_manual_review_candidate(
        &db,
        &claimed.step.id,
        USER_ID,
        claimed.run.version,
        claimed.run.epoch,
        REPAIR_STEP_KEY,
        NovelAutopilotStepType::ChapterRepair,
        Some(REPAIR_TASK_ID),
        &expected,
        NovelAutopilotStepStatus::Failed,
        "chapter_repair_manual_review",
        manual_review_candidate(candidate_content),
    )
    .await
    .expect("persist repair manual-review candidate");
    let decision_task_id = "chapter-repair-candidate-accept-task";
    let running = resume_manual_candidate_for_accept(&db, &waiting.run, decision_task_id).await;

    let accepted = NovelAutopilotRepository::accept_chapter_manual_review_candidate(
        &db,
        &running.id,
        &claimed.step.id,
        USER_ID,
        running.version,
        running.epoch,
        Some(decision_task_id),
    )
    .await
    .expect("accept repair manual-review candidate");

    let chapter = load_chapter(&db).await;
    assert_eq!(chapter.content.as_deref(), Some(candidate_content));
    assert_eq!(chapter.word_count, expected_word_count);
    assert_eq!(chapter.status, NovelAutopilotStepStatus::Completed.as_str());
    assert_eq!(
        accepted.run.completed_chapters,
        claimed.run.completed_chapters
    );
    assert_eq!(
        accepted.run.total_word_count,
        claimed.run.total_word_count + i64::from(expected_word_count)
            - i64::from(INITIAL_WORD_COUNT)
    );
    assert!(accepted.run.active_background_task_id.is_none());

    let candidate = chapter_draft_attempt::Entity::find_by_id(&claimed.step.id)
        .one(&db)
        .await
        .expect("load accepted repair candidate")
        .expect("accepted repair candidate exists");
    assert_eq!(candidate.attempt_state, "accepted");
    assert_eq!(
        generation_history::Entity::find()
            .count(&db)
            .await
            .expect("count repair candidate apply history"),
        1
    );
}

#[tokio::test]
async fn accepting_manual_review_candidate_rejects_stale_chapter_snapshot() {
    let db = setup_repository_db().await;
    insert_project(&db).await;
    insert_chapter(&db).await;
    let claimed = claim_chapter_step(&db, NovelAutopilotStepType::ChapterGenerate, TASK_ID).await;
    let expected = ChapterBusinessSnapshot::load(&db, PROJECT_ID, CHAPTER_ID)
        .await
        .expect("load chapter business snapshot");
    let candidate_content = "该候选不应覆盖人工修改后的章节正文。";
    let waiting = NovelAutopilotRepository::persist_chapter_manual_review_candidate(
        &db,
        &claimed.step.id,
        USER_ID,
        claimed.run.version,
        claimed.run.epoch,
        STEP_KEY,
        NovelAutopilotStepType::ChapterGenerate,
        Some(TASK_ID),
        &expected,
        NovelAutopilotStepStatus::Skipped,
        "chapter_quality_manual_review",
        manual_review_candidate(candidate_content),
    )
    .await
    .expect("persist chapter manual-review candidate");

    let mut edited = load_chapter(&db).await.into_active_model();
    edited.content = Set(Some("人工已修改正文，必须保留。".to_string()));
    edited.word_count = Set(13);
    edited
        .update(&db)
        .await
        .expect("simulate manual chapter edit");
    let decision_task_id = "chapter-candidate-stale-task";
    let running = resume_manual_candidate_for_accept(&db, &waiting.run, decision_task_id).await;

    let error = NovelAutopilotRepository::accept_chapter_manual_review_candidate(
        &db,
        &running.id,
        &claimed.step.id,
        USER_ID,
        running.version,
        running.epoch,
        Some(decision_task_id),
    )
    .await
    .expect_err("stale candidate must not overwrite chapter");
    assert_eq!(error, NovelAutopilotRepositoryError::BusinessDataChanged);
    assert_eq!(
        load_chapter(&db).await.content.as_deref(),
        Some("人工已修改正文，必须保留。")
    );
    let candidate = chapter_draft_attempt::Entity::find_by_id(&claimed.step.id)
        .one(&db)
        .await
        .expect("reload stale candidate")
        .expect("stale candidate remains available");
    assert_eq!(candidate.attempt_state, "waiting_human");
}

#[tokio::test]
async fn chapter_generate_commit_enters_waiting_human_in_same_transaction() {
    let db = setup_repository_db().await;
    insert_project(&db).await;
    insert_chapter(&db).await;
    let claimed = claim_chapter_step(&db, NovelAutopilotStepType::ChapterGenerate, TASK_ID).await;
    let expected = ChapterBusinessSnapshot::load(&db, PROJECT_ID, CHAPTER_ID)
        .await
        .expect("load chapter business snapshot");

    let chapter_commit = accepted_chapter_commit();
    let expected_content = chapter_commit.content.clone();
    let expected_word_count = chapter_commit.word_count;
    let committed = NovelAutopilotRepository::commit_chapter_generate_step(
        &db,
        &claimed.step.id,
        USER_ID,
        claimed.run.version,
        claimed.run.epoch,
        STEP_KEY,
        Some(TASK_ID),
        &expected,
        NovelAutopilotRunStatus::WaitingHuman,
        chapter_commit,
    )
    .await
    .expect("commit accepted chapter behind human gate");

    assert_eq!(
        committed.run.status,
        NovelAutopilotRunStatus::WaitingHuman.as_str()
    );
    assert_eq!(committed.run.completed_chapters, 1);
    assert!(committed.run.active_background_task_id.is_none());
    assert!(committed.run.current_step.is_none());
    assert_eq!(
        committed.step.status,
        NovelAutopilotStepStatus::Completed.as_str()
    );
    let chapter = load_chapter(&db).await;
    assert_eq!(chapter.content.as_deref(), Some(expected_content.as_str()));
    assert_eq!(chapter.word_count, expected_word_count);
    assert_eq!(chapter.status, NovelAutopilotStepStatus::Completed.as_str());
}

#[tokio::test]
async fn chapter_generate_commit_updates_chapter_progress_cursor_and_step_atomically() {
    let db = setup_repository_db().await;
    insert_project(&db).await;
    insert_chapter(&db).await;
    let claimed = claim_chapter_step(&db, NovelAutopilotStepType::ChapterGenerate, TASK_ID).await;
    let expected = ChapterBusinessSnapshot::load(&db, PROJECT_ID, CHAPTER_ID)
        .await
        .expect("load chapter business snapshot");
    let chapter_commit = accepted_chapter_commit();

    let committed = NovelAutopilotRepository::commit_chapter_generate_step(
        &db,
        &claimed.step.id,
        USER_ID,
        claimed.run.version,
        claimed.run.epoch,
        STEP_KEY,
        Some(TASK_ID),
        &expected,
        NovelAutopilotRunStatus::Running,
        chapter_commit.clone(),
    )
    .await
    .expect("commit accepted chapter generation");

    let chapter_after = load_chapter(&db).await;
    assert_eq!(
        chapter_after.content.as_deref(),
        Some(chapter_commit.content.as_str())
    );
    assert_eq!(chapter_after.word_count, chapter_commit.word_count);
    assert_eq!(chapter_after.status, "completed");
    assert_eq!(chapter_after.summary.as_deref(), Some("主角发现沉睡的星门"));
    assert_eq!(chapter_after.outline_id.as_deref(), Some("outline-1"));

    assert_eq!(committed.run.version, claimed.run.version + 1);
    assert!(committed.run.current_step.is_none());
    assert!(committed.run.active_background_task_id.is_none());
    assert_eq!(
        committed.run.completed_chapters,
        claimed.run.completed_chapters + 1
    );
    assert_eq!(
        committed.run.total_word_count,
        claimed.run.total_word_count + i64::from(chapter_commit.word_count)
    );
    assert_eq!(
        committed.run.current_chapter_id.as_deref(),
        Some(CHAPTER_ID)
    );
    assert_eq!(committed.run.current_chapter_number, Some(CHAPTER_NUMBER));
    assert_eq!(
        committed.step.status,
        NovelAutopilotStepStatus::Completed.as_str()
    );
    assert_eq!(
        committed.step.result_digest.as_deref(),
        Some("chapter-generate-result-digest")
    );
    assert_eq!(
        committed.step.quality_decision.as_deref(),
        Some(NovelAutopilotQualityDecision::Accept.as_str())
    );
    assert_eq!(committed.step.error_code, None);
}

#[tokio::test]
async fn chapter_generate_commit_returns_business_data_changed_without_terminalizing_step() {
    let db = setup_repository_db().await;
    insert_project(&db).await;
    insert_chapter(&db).await;
    let claimed = claim_chapter_step(&db, NovelAutopilotStepType::ChapterGenerate, TASK_ID).await;
    let expected = ChapterBusinessSnapshot::load(&db, PROJECT_ID, CHAPTER_ID)
        .await
        .expect("load expected chapter snapshot");

    let chapter_model = load_chapter(&db).await;
    let mut human_edit = chapter_model.into_active_model();
    human_edit.content = Set(Some("人工修改后的正文".to_string()));
    human_edit.word_count = Set(9);
    human_edit.updated_at = Set(Some(test_time()));
    human_edit
        .update(&db)
        .await
        .expect("apply human chapter edit");

    let error = NovelAutopilotRepository::commit_chapter_generate_step(
        &db,
        &claimed.step.id,
        USER_ID,
        claimed.run.version,
        claimed.run.epoch,
        STEP_KEY,
        Some(TASK_ID),
        &expected,
        NovelAutopilotRunStatus::Running,
        accepted_chapter_commit(),
    )
    .await
    .expect_err("human chapter edit must reject stale generated result");
    assert_eq!(error, NovelAutopilotRepositoryError::BusinessDataChanged);

    let chapter_after = load_chapter(&db).await;
    assert_eq!(chapter_after.content.as_deref(), Some("人工修改后的正文"));
    assert_eq!(chapter_after.word_count, 9);
    assert_eq!(chapter_after.status, "pending");

    let run_after = NovelAutopilotRepository::find_owned(&db, &claimed.run.id, USER_ID)
        .await
        .expect("reload run after business conflict");
    assert_eq!(run_after.version, claimed.run.version);
    assert_eq!(run_after.current_step.as_deref(), Some(STEP_KEY));
    let step_after = novel_autopilot_step_run::Entity::find_by_id(&claimed.step.id)
        .one(&db)
        .await
        .expect("reload step after business conflict")
        .expect("step exists");
    assert_eq!(
        step_after.status,
        NovelAutopilotStepStatus::Running.as_str()
    );
}

#[tokio::test]
async fn chapter_generate_commit_rejects_stale_version_epoch_and_task_without_writes() {
    let db = setup_repository_db().await;
    insert_project(&db).await;
    insert_chapter(&db).await;
    let claimed = claim_chapter_step(&db, NovelAutopilotStepType::ChapterGenerate, TASK_ID).await;
    let expected = ChapterBusinessSnapshot::load(&db, PROJECT_ID, CHAPTER_ID)
        .await
        .expect("load expected chapter snapshot");

    let stale_version = NovelAutopilotRepository::commit_chapter_generate_step(
        &db,
        &claimed.step.id,
        USER_ID,
        claimed.run.version + 1,
        claimed.run.epoch,
        STEP_KEY,
        Some(TASK_ID),
        &expected,
        NovelAutopilotRunStatus::Running,
        accepted_chapter_commit(),
    )
    .await
    .expect_err("stale version must reject chapter commit");
    assert_eq!(stale_version, NovelAutopilotRepositoryError::StaleVersion);
    assert_commit_not_applied(&db, &claimed).await;

    let stale_epoch = NovelAutopilotRepository::commit_chapter_generate_step(
        &db,
        &claimed.step.id,
        USER_ID,
        claimed.run.version,
        claimed.run.epoch + 1,
        STEP_KEY,
        Some(TASK_ID),
        &expected,
        NovelAutopilotRunStatus::Running,
        accepted_chapter_commit(),
    )
    .await
    .expect_err("stale epoch must reject chapter commit");
    assert_eq!(stale_epoch, NovelAutopilotRepositoryError::StaleEpoch);
    assert_commit_not_applied(&db, &claimed).await;

    let invalid_task = NovelAutopilotRepository::commit_chapter_generate_step(
        &db,
        &claimed.step.id,
        USER_ID,
        claimed.run.version,
        claimed.run.epoch,
        STEP_KEY,
        Some("other-chapter-task"),
        &expected,
        NovelAutopilotRunStatus::Running,
        accepted_chapter_commit(),
    )
    .await
    .expect_err("different task must reject chapter commit");
    assert_eq!(
        invalid_task,
        NovelAutopilotRepositoryError::InvalidTransition
    );
    assert_commit_not_applied(&db, &claimed).await;
}

#[tokio::test]
async fn chapter_generate_commit_rejects_non_generation_step_type_without_writes() {
    let db = setup_repository_db().await;
    insert_project(&db).await;
    insert_chapter(&db).await;
    let claimed = claim_chapter_step(&db, NovelAutopilotStepType::ChapterAnalyze, TASK_ID).await;
    let expected = ChapterBusinessSnapshot::load(&db, PROJECT_ID, CHAPTER_ID)
        .await
        .expect("load expected chapter snapshot");

    let error = NovelAutopilotRepository::commit_chapter_generate_step(
        &db,
        &claimed.step.id,
        USER_ID,
        claimed.run.version,
        claimed.run.epoch,
        STEP_KEY,
        Some(TASK_ID),
        &expected,
        NovelAutopilotRunStatus::Running,
        accepted_chapter_commit(),
    )
    .await
    .expect_err("non-generation step type must reject chapter commit");
    assert_eq!(error, NovelAutopilotRepositoryError::InvalidTransition);
    assert_commit_not_applied(&db, &claimed).await;
}

#[tokio::test]
async fn chapter_generate_commit_rejects_non_accept_or_non_completed_payload_without_writes() {
    let db = setup_repository_db().await;
    insert_project(&db).await;
    insert_chapter(&db).await;
    let claimed = claim_chapter_step(&db, NovelAutopilotStepType::ChapterGenerate, TASK_ID).await;
    let expected = ChapterBusinessSnapshot::load(&db, PROJECT_ID, CHAPTER_ID)
        .await
        .expect("load expected chapter snapshot");

    let mut non_accept = accepted_chapter_commit();
    non_accept.quality_decision = NovelAutopilotQualityDecision::ManualReview
        .as_str()
        .to_string();
    let non_accept_error = NovelAutopilotRepository::commit_chapter_generate_step(
        &db,
        &claimed.step.id,
        USER_ID,
        claimed.run.version,
        claimed.run.epoch,
        STEP_KEY,
        Some(TASK_ID),
        &expected,
        NovelAutopilotRunStatus::Running,
        non_accept,
    )
    .await
    .expect_err("manual review result must not complete chapter generate step");
    assert_eq!(
        non_accept_error,
        NovelAutopilotRepositoryError::InvalidConfig {
            field: "quality_decision",
            code: "invalid",
        }
    );
    assert_commit_not_applied(&db, &claimed).await;

    let mut non_completed = accepted_chapter_commit();
    non_completed.status = "draft".to_string();
    let non_completed_error = NovelAutopilotRepository::commit_chapter_generate_step(
        &db,
        &claimed.step.id,
        USER_ID,
        claimed.run.version,
        claimed.run.epoch,
        STEP_KEY,
        Some(TASK_ID),
        &expected,
        NovelAutopilotRunStatus::Running,
        non_completed,
    )
    .await
    .expect_err("non-completed chapter status must reject commit");
    assert_eq!(
        non_completed_error,
        NovelAutopilotRepositoryError::InvalidConfig {
            field: "status",
            code: "invalid",
        }
    );
    assert_commit_not_applied(&db, &claimed).await;
}

#[tokio::test]
async fn chapter_generate_commit_rolls_back_chapter_write_when_run_cas_loses_race() {
    let db = setup_repository_db().await;
    insert_project(&db).await;
    insert_chapter(&db).await;
    let claimed = claim_chapter_step(&db, NovelAutopilotStepType::ChapterGenerate, TASK_ID).await;
    let expected = ChapterBusinessSnapshot::load(&db, PROJECT_ID, CHAPTER_ID)
        .await
        .expect("load expected chapter snapshot");

    db.execute(Statement::from_string(
        DbBackend::Sqlite,
        format!(
            "CREATE TRIGGER chapter_generate_test_force_run_cas_loss \
             AFTER UPDATE OF content ON chapters \
             WHEN NEW.id = '{CHAPTER_ID}' \
             BEGIN \
               UPDATE novel_autopilot_runs SET version = version + 1 WHERE id = '{}'; \
             END",
            claimed.run.id
        ),
    ))
    .await
    .expect("create transaction rollback trigger");

    let error = NovelAutopilotRepository::commit_chapter_generate_step(
        &db,
        &claimed.step.id,
        USER_ID,
        claimed.run.version,
        claimed.run.epoch,
        STEP_KEY,
        Some(TASK_ID),
        &expected,
        NovelAutopilotRunStatus::Running,
        accepted_chapter_commit(),
    )
    .await
    .expect_err("run CAS loss after chapter update must rollback transaction");
    assert_eq!(error, NovelAutopilotRepositoryError::StaleEpoch);
    assert_commit_not_applied(&db, &claimed).await;
}

#[tokio::test]
async fn chapter_analysis_commit_persists_analysis_and_completes_step_atomically() {
    let db = setup_repository_db().await;
    insert_project(&db).await;
    insert_chapter(&db).await;
    let claimed = claim_chapter_analysis_step(&db).await;
    let expected = ChapterBusinessSnapshot::load(&db, PROJECT_ID, CHAPTER_ID)
        .await
        .expect("load expected chapter snapshot");

    let committed = NovelAutopilotRepository::commit_chapter_analysis_step(
        &db,
        &claimed.step.id,
        USER_ID,
        claimed.run.version,
        claimed.run.epoch,
        ANALYSIS_STEP_KEY,
        Some(ANALYSIS_TASK_ID),
        &expected,
        accepted_analysis_commit(),
    )
    .await
    .expect("commit chapter analysis");

    assert_eq!(analysis_count(&db).await, 1);
    let analysis = plot_analysis::Entity::find()
        .one(&db)
        .await
        .expect("load analysis")
        .expect("analysis exists");
    assert_eq!(analysis.project_id, PROJECT_ID);
    assert_eq!(analysis.chapter_id, CHAPTER_ID);
    assert_eq!(analysis.source_content_digest, expected.content_digest());
    assert_eq!(analysis.overall_quality_score, Some(8.6));
    assert_eq!(
        analysis.analysis_report.as_deref(),
        Some(
            "剧情阶段：opening
改进建议：保持悬念"
        )
    );

    assert_eq!(
        committed.run.status,
        NovelAutopilotRunStatus::Running.as_str()
    );
    assert_eq!(committed.run.version, claimed.run.version + 1);
    assert!(committed.run.current_step.is_none());
    assert!(committed.run.active_background_task_id.is_none());
    assert_eq!(committed.run.consecutive_provider_failures, 0);
    assert_eq!(committed.run.consecutive_quality_failures, 0);
    assert!(committed.run.last_error_code.is_none());
    assert_eq!(
        committed.step.status,
        NovelAutopilotStepStatus::Completed.as_str()
    );
    assert_eq!(
        committed.step.result_digest.as_deref(),
        Some("analysis-result-digest")
    );
    assert_eq!(
        committed.step.quality_decision.as_deref(),
        Some(NovelAutopilotQualityDecision::Accept.as_str())
    );
    assert!(committed.step.error_code.is_none());
}

#[tokio::test]
async fn chapter_analysis_manual_review_enters_human_gate_and_tracks_quality_failure() {
    let db = setup_repository_db().await;
    insert_project(&db).await;
    insert_chapter(&db).await;
    let claimed = claim_chapter_analysis_step(&db).await;
    let expected = ChapterBusinessSnapshot::load(&db, PROJECT_ID, CHAPTER_ID)
        .await
        .expect("load expected chapter snapshot");
    let mut commit = accepted_analysis_commit();
    commit.quality_decision = NovelAutopilotQualityDecision::ManualReview;
    commit.waiting_human = true;

    let committed = NovelAutopilotRepository::commit_chapter_analysis_step(
        &db,
        &claimed.step.id,
        USER_ID,
        claimed.run.version,
        claimed.run.epoch,
        ANALYSIS_STEP_KEY,
        Some(ANALYSIS_TASK_ID),
        &expected,
        commit,
    )
    .await
    .expect("commit manual-review analysis");

    assert_eq!(analysis_count(&db).await, 1);
    assert_eq!(
        committed.run.status,
        NovelAutopilotRunStatus::WaitingHuman.as_str()
    );
    assert_eq!(committed.run.consecutive_quality_failures, 1);
    assert_eq!(
        committed.run.last_error_code.as_deref(),
        Some("chapter_analysis_manual_review")
    );
    assert_eq!(
        committed.step.status,
        NovelAutopilotStepStatus::Completed.as_str()
    );
    assert_eq!(
        committed.step.quality_decision.as_deref(),
        Some(NovelAutopilotQualityDecision::ManualReview.as_str())
    );
    assert_eq!(
        committed.step.error_code.as_deref(),
        Some("chapter_analysis_manual_review")
    );
}

#[tokio::test]
async fn chapter_analysis_commit_rejects_stale_fences_without_writes() {
    for case in ["version", "epoch", "task"] {
        let db = setup_repository_db().await;
        insert_project(&db).await;
        insert_chapter(&db).await;
        let claimed = claim_chapter_analysis_step(&db).await;
        let expected = ChapterBusinessSnapshot::load(&db, PROJECT_ID, CHAPTER_ID)
            .await
            .expect("load expected chapter snapshot");
        let expected_version = if case == "version" {
            claimed.run.version + 1
        } else {
            claimed.run.version
        };
        let expected_epoch = if case == "epoch" {
            claimed.run.epoch + 1
        } else {
            claimed.run.epoch
        };
        let expected_task = if case == "task" {
            Some("other-analysis-task")
        } else {
            Some(ANALYSIS_TASK_ID)
        };

        let error = NovelAutopilotRepository::commit_chapter_analysis_step(
            &db,
            &claimed.step.id,
            USER_ID,
            expected_version,
            expected_epoch,
            ANALYSIS_STEP_KEY,
            expected_task,
            &expected,
            accepted_analysis_commit(),
        )
        .await
        .expect_err("stale analysis fence must reject commit");

        let expected_error = match case {
            "version" => NovelAutopilotRepositoryError::StaleVersion,
            "epoch" => NovelAutopilotRepositoryError::StaleEpoch,
            _ => NovelAutopilotRepositoryError::InvalidTransition,
        };
        assert_eq!(error, expected_error);
        assert_analysis_not_applied(&db, &claimed).await;
    }
}

#[tokio::test]
async fn chapter_analysis_commit_rejects_human_chapter_edit_and_existing_analysis() {
    let db = setup_repository_db().await;
    insert_project(&db).await;
    insert_chapter(&db).await;
    let claimed = claim_chapter_analysis_step(&db).await;
    let expected = ChapterBusinessSnapshot::load(&db, PROJECT_ID, CHAPTER_ID)
        .await
        .expect("load expected chapter snapshot");
    let mut chapter_model = load_chapter(&db).await.into_active_model();
    chapter_model.content = Set(Some("人工修改后的正文".to_string()));
    chapter_model.word_count = Set(9);
    chapter_model
        .update(&db)
        .await
        .expect("apply concurrent human chapter edit");

    let edit_error = NovelAutopilotRepository::commit_chapter_analysis_step(
        &db,
        &claimed.step.id,
        USER_ID,
        claimed.run.version,
        claimed.run.epoch,
        ANALYSIS_STEP_KEY,
        Some(ANALYSIS_TASK_ID),
        &expected,
        accepted_analysis_commit(),
    )
    .await
    .expect_err("human chapter edit must reject stale analysis");
    assert_eq!(
        edit_error,
        NovelAutopilotRepositoryError::BusinessDataChanged
    );
    assert_analysis_not_applied(&db, &claimed).await;

    let db = setup_repository_db().await;
    insert_project(&db).await;
    insert_chapter(&db).await;
    let claimed = claim_chapter_analysis_step(&db).await;
    let expected = ChapterBusinessSnapshot::load(&db, PROJECT_ID, CHAPTER_ID)
        .await
        .expect("load expected chapter snapshot");
    crate::services::chapter_analysis_runtime_service::persistence_owner::build_plot_analysis_active_model(
        &load_chapter(&db).await,
        &accepted_analysis_commit().payload,
        test_time(),
    )
    .insert(&db)
    .await
    .expect("insert concurrent manual analysis");

    let duplicate_error = NovelAutopilotRepository::commit_chapter_analysis_step(
        &db,
        &claimed.step.id,
        USER_ID,
        claimed.run.version,
        claimed.run.epoch,
        ANALYSIS_STEP_KEY,
        Some(ANALYSIS_TASK_ID),
        &expected,
        accepted_analysis_commit(),
    )
    .await
    .expect_err("existing analysis must reject duplicate durable commit");
    assert_eq!(
        duplicate_error,
        NovelAutopilotRepositoryError::BusinessDataChanged
    );
    assert_eq!(analysis_count(&db).await, 1);
    let run = NovelAutopilotRepository::find_owned(&db, &claimed.run.id, USER_ID)
        .await
        .expect("reload duplicate-analysis run");
    assert_eq!(run.version, claimed.run.version);
}

#[tokio::test]
async fn chapter_analysis_commit_replaces_analysis_for_stale_content_digest() {
    let db = setup_repository_db().await;
    insert_project(&db).await;
    insert_chapter(&db).await;
    let claimed = claim_chapter_analysis_step(&db).await;
    let expected = ChapterBusinessSnapshot::load(&db, PROJECT_ID, CHAPTER_ID)
        .await
        .expect("load expected chapter snapshot");
    let mut stale = crate::services::chapter_analysis_runtime_service::persistence_owner::build_plot_analysis_active_model(
        &load_chapter(&db).await,
        &accepted_analysis_commit().payload,
        test_time(),
    );
    stale.source_content_digest = Set(Some("sha256:stale-content".to_string()));
    let stale = stale
        .insert(&db)
        .await
        .expect("insert stale chapter analysis");

    NovelAutopilotRepository::commit_chapter_analysis_step(
        &db,
        &claimed.step.id,
        USER_ID,
        claimed.run.version,
        claimed.run.epoch,
        ANALYSIS_STEP_KEY,
        Some(ANALYSIS_TASK_ID),
        &expected,
        accepted_analysis_commit(),
    )
    .await
    .expect("replace stale chapter analysis");

    assert_eq!(analysis_count(&db).await, 1);
    let current = plot_analysis::Entity::find()
        .one(&db)
        .await
        .expect("load replacement analysis")
        .expect("replacement analysis exists");
    assert_ne!(current.id, stale.id);
    assert_eq!(current.source_content_digest, expected.content_digest());
}

#[tokio::test]
async fn chapter_analysis_provider_failure_updates_budget_without_persisting_analysis() {
    for waiting_human in [false, true] {
        let db = setup_repository_db().await;
        insert_project(&db).await;
        insert_chapter(&db).await;
        let claimed = claim_chapter_analysis_step(&db).await;
        let failure_started_at = Utc::now().naive_utc();

        let terminal = NovelAutopilotRepository::finish_chapter_analysis_failure(
            &db,
            &claimed.step.id,
            USER_ID,
            claimed.run.version,
            claimed.run.epoch,
            ANALYSIS_STEP_KEY,
            Some(ANALYSIS_TASK_ID),
            "chapter_analysis_provider_failed",
            NovelAutopilotFailureCounterKind::Provider,
            Some(120),
            waiting_human,
        )
        .await
        .expect("finish chapter analysis provider failure");

        assert_eq!(analysis_count(&db).await, 0);
        assert_eq!(terminal.run.consecutive_provider_failures, 1);
        assert_eq!(terminal.run.consecutive_quality_failures, 0);
        assert_eq!(
            terminal.run.status,
            if waiting_human {
                NovelAutopilotRunStatus::WaitingHuman.as_str()
            } else {
                NovelAutopilotRunStatus::Running.as_str()
            }
        );
        assert!(terminal.run.current_step.is_none());
        assert!(terminal.run.active_background_task_id.is_none());
        assert_eq!(
            terminal.run.last_error_code.as_deref(),
            Some("chapter_analysis_provider_failed")
        );
        assert_eq!(terminal.run.next_attempt_at.is_some(), !waiting_human);
        if let Some(next_attempt_at) = terminal.run.next_attempt_at {
            assert!(next_attempt_at > claimed.run.updated_at);
            assert!((next_attempt_at - failure_started_at).num_seconds() >= 120);
        }
        assert_eq!(
            terminal.step.status,
            NovelAutopilotStepStatus::Failed.as_str()
        );
        assert!(terminal.step.result_digest.is_none());
        assert!(terminal.step.quality_decision.is_none());
        assert_eq!(
            terminal.step.error_code.as_deref(),
            Some("chapter_analysis_provider_failed")
        );
    }
}

#[tokio::test]
async fn chapter_repair_commit_updates_content_by_delta_without_advancing_chapter_cursor() {
    let db = setup_repository_db().await;
    insert_project(&db).await;
    insert_chapter(&db).await;
    let claimed = claim_chapter_repair_step(&db).await;
    let expected = ChapterBusinessSnapshot::load(&db, PROJECT_ID, CHAPTER_ID)
        .await
        .expect("load repair chapter snapshot");

    let run = NovelAutopilotRepository::find_owned(&db, &claimed.run.id, USER_ID)
        .await
        .expect("load repair run before commit");
    let mut run = run.into_active_model();
    run.total_word_count = Set(i64::from(INITIAL_WORD_COUNT));
    run.completed_chapters = Set(1);
    run.consecutive_provider_failures = Set(2);
    run.consecutive_quality_failures = Set(1);
    run.update(&db).await.expect("seed repair run counters");

    let committed = NovelAutopilotRepository::commit_chapter_repair_step(
        &db,
        &claimed.step.id,
        USER_ID,
        claimed.run.version,
        claimed.run.epoch,
        REPAIR_STEP_KEY,
        Some(REPAIR_TASK_ID),
        &expected,
        accepted_repair_commit(),
    )
    .await
    .expect("commit durable chapter repair");

    let chapter = load_chapter(&db).await;
    assert_eq!(chapter.content, Some(accepted_repair_commit().content));
    assert_eq!(chapter.word_count, accepted_repair_commit().word_count);
    assert_eq!(chapter.status, "completed");
    assert_eq!(committed.run.total_word_count, 24);
    assert_eq!(committed.run.completed_chapters, 1);
    assert_eq!(
        committed.run.current_chapter_id.as_deref(),
        Some(CHAPTER_ID)
    );
    assert_eq!(committed.run.current_chapter_number, Some(CHAPTER_NUMBER));
    assert_eq!(committed.run.consecutive_provider_failures, 0);
    assert_eq!(committed.run.consecutive_quality_failures, 0);
    assert!(committed.run.current_step.is_none());
    assert!(committed.run.active_background_task_id.is_none());
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
        Some("chapter-repair-result-digest")
    );
}

#[tokio::test]
async fn chapter_repair_commit_rejects_manual_edit_and_rolls_back_run_and_step() {
    let db = setup_repository_db().await;
    insert_project(&db).await;
    insert_chapter(&db).await;
    let claimed = claim_chapter_repair_step(&db).await;
    let expected = ChapterBusinessSnapshot::load(&db, PROJECT_ID, CHAPTER_ID)
        .await
        .expect("load repair snapshot before manual edit");

    let chapter = load_chapter(&db).await;
    let mut chapter = chapter.into_active_model();
    chapter.content = Set(Some("人工编辑后的正文".to_string()));
    chapter.word_count = Set(9);
    chapter
        .update(&db)
        .await
        .expect("simulate manual chapter edit");

    let error = NovelAutopilotRepository::commit_chapter_repair_step(
        &db,
        &claimed.step.id,
        USER_ID,
        claimed.run.version,
        claimed.run.epoch,
        REPAIR_STEP_KEY,
        Some(REPAIR_TASK_ID),
        &expected,
        accepted_repair_commit(),
    )
    .await
    .expect_err("manual edit must reject repair commit");
    assert_eq!(error, NovelAutopilotRepositoryError::BusinessDataChanged);

    let chapter = load_chapter(&db).await;
    assert_eq!(chapter.content.as_deref(), Some("人工编辑后的正文"));
    assert_eq!(chapter.word_count, 9);
    let run = NovelAutopilotRepository::find_owned(&db, &claimed.run.id, USER_ID)
        .await
        .expect("reload run after rejected repair commit");
    assert_eq!(run.version, claimed.run.version);
    assert_eq!(run.current_step.as_deref(), Some(REPAIR_STEP_KEY));
    assert_eq!(
        run.active_background_task_id.as_deref(),
        Some(REPAIR_TASK_ID)
    );
    let step = novel_autopilot_step_run::Entity::find_by_id(&claimed.step.id)
        .one(&db)
        .await
        .expect("reload repair step")
        .expect("repair step exists");
    assert_eq!(step.status, NovelAutopilotStepStatus::Running.as_str());
    assert!(step.result_digest.is_none());
    assert!(step.quality_decision.is_none());
}

#[tokio::test]
async fn chapter_repair_failure_tracks_provider_and_quality_budgets_independently() {
    for (counter_kind, waiting_human, decision, provider_failures, quality_failures) in [
        (
            NovelAutopilotFailureCounterKind::Provider,
            false,
            NovelAutopilotQualityDecision::Retry,
            1,
            0,
        ),
        (
            NovelAutopilotFailureCounterKind::Quality,
            true,
            NovelAutopilotQualityDecision::ManualReview,
            0,
            1,
        ),
        (
            NovelAutopilotFailureCounterKind::None,
            true,
            NovelAutopilotQualityDecision::ManualReview,
            0,
            0,
        ),
    ] {
        let db = setup_repository_db().await;
        insert_project(&db).await;
        insert_chapter(&db).await;
        let claimed = claim_chapter_repair_step(&db).await;
        let failure_started_at = Utc::now().naive_utc();
        let retry_after_seconds =
            (counter_kind == NovelAutopilotFailureCounterKind::Provider).then_some(120);

        let terminal = NovelAutopilotRepository::finish_chapter_repair_failure(
            &db,
            &claimed.step.id,
            USER_ID,
            claimed.run.version,
            claimed.run.epoch,
            REPAIR_STEP_KEY,
            Some(REPAIR_TASK_ID),
            "chapter_repair_failed",
            counter_kind,
            retry_after_seconds,
            waiting_human,
            decision,
            None,
        )
        .await
        .expect("finish durable chapter repair failure");

        assert_eq!(
            terminal.run.consecutive_provider_failures,
            provider_failures
        );
        assert_eq!(terminal.run.consecutive_quality_failures, quality_failures);
        assert_eq!(
            terminal.run.status,
            if waiting_human {
                NovelAutopilotRunStatus::WaitingHuman.as_str()
            } else {
                NovelAutopilotRunStatus::Running.as_str()
            }
        );
        assert!(terminal.run.current_step.is_none());
        assert!(terminal.run.active_background_task_id.is_none());
        assert_eq!(
            terminal.run.next_attempt_at.is_some(),
            counter_kind == NovelAutopilotFailureCounterKind::Provider && !waiting_human
        );
        if let Some(next_attempt_at) = terminal.run.next_attempt_at {
            assert!(next_attempt_at > claimed.run.updated_at);
            assert!((next_attempt_at - failure_started_at).num_seconds() >= 120);
        }
        assert_eq!(
            terminal.step.status,
            NovelAutopilotStepStatus::Failed.as_str()
        );
        assert_eq!(
            terminal.step.quality_decision.as_deref(),
            Some(decision.as_str())
        );
        assert_eq!(
            terminal.step.error_code.as_deref(),
            Some("chapter_repair_failed")
        );
    }
}

#[tokio::test]
async fn chapter_repair_quality_failure_atomically_persists_retry_candidate_and_step_digest() {
    let db = setup_repository_db().await;
    insert_project(&db).await;
    insert_chapter(&db).await;
    let claimed = claim_chapter_repair_step(&db).await;
    let expected = ChapterBusinessSnapshot::load(&db, PROJECT_ID, CHAPTER_ID)
        .await
        .expect("load repair snapshot");
    let content = "第一轮返修候选保留原剧情，并补足星门开启后的因果。";
    let evidence = repair_failure_evidence(&claimed, &expected, content);
    let expected_digest = evidence.result_digest.clone();

    let terminal = NovelAutopilotRepository::finish_chapter_repair_failure(
        &db,
        &claimed.step.id,
        USER_ID,
        claimed.run.version,
        claimed.run.epoch,
        REPAIR_STEP_KEY,
        Some(REPAIR_TASK_ID),
        "chapter_repair_quality_retry",
        NovelAutopilotFailureCounterKind::Quality,
        None,
        false,
        NovelAutopilotQualityDecision::Retry,
        Some(evidence),
    )
    .await
    .expect("persist retry evidence and terminal step");

    assert_eq!(
        terminal.run.status,
        NovelAutopilotRunStatus::Running.as_str()
    );
    assert_eq!(terminal.run.consecutive_quality_failures, 1);
    assert!(terminal.run.current_step.is_none());
    assert_eq!(
        terminal.step.result_digest.as_deref(),
        Some(expected_digest.as_str())
    );
    assert_eq!(
        terminal.step.quality_decision.as_deref(),
        Some(NovelAutopilotQualityDecision::Retry.as_str())
    );
    let draft = chapter_draft_attempt::Entity::find_by_id(&claimed.step.id)
        .one(&db)
        .await
        .expect("load retry draft")
        .expect("retry draft exists");
    assert_eq!(draft.source, "novel_autopilot_chapter_repair");
    assert_eq!(draft.attempt_state, "retry");
    assert_eq!(draft.batch_task_id, None);
    assert_eq!(
        draft
            .repair_payload
            .as_ref()
            .and_then(|payload| payload.get("run_id"))
            .and_then(serde_json::Value::as_str),
        Some(claimed.run.id.as_str())
    );
    assert_eq!(
        load_chapter(&db).await.content.as_deref(),
        Some(INITIAL_CONTENT)
    );
}

#[tokio::test]
async fn chapter_repair_retry_candidate_insert_failure_rolls_back_run_and_step() {
    let db = setup_repository_db().await;
    insert_project(&db).await;
    insert_chapter(&db).await;
    let claimed = claim_chapter_repair_step(&db).await;
    let expected = ChapterBusinessSnapshot::load(&db, PROJECT_ID, CHAPTER_ID)
        .await
        .expect("load repair snapshot");
    let evidence = repair_failure_evidence(&claimed, &expected, "待持久化的返修候选正文");
    evidence
        .draft_attempt
        .clone()
        .into_active_model()
        .insert(&db)
        .await
        .expect("seed conflicting draft id");

    let error = NovelAutopilotRepository::finish_chapter_repair_failure(
        &db,
        &claimed.step.id,
        USER_ID,
        claimed.run.version,
        claimed.run.epoch,
        REPAIR_STEP_KEY,
        Some(REPAIR_TASK_ID),
        "chapter_repair_quality_retry",
        NovelAutopilotFailureCounterKind::Quality,
        None,
        false,
        NovelAutopilotQualityDecision::Retry,
        Some(evidence),
    )
    .await
    .expect_err("duplicate candidate id must roll back transaction");

    assert!(matches!(error, NovelAutopilotRepositoryError::Database(_)));
    assert_repair_failure_not_applied(&db, &claimed).await;
    assert_eq!(
        chapter_draft_attempt::Entity::find()
            .count(&db)
            .await
            .expect("count draft attempts"),
        1
    );
}

#[tokio::test]
async fn chapter_repair_retry_candidate_rejects_stale_chapter_and_invalid_scope() {
    for invalid_scope in [false, true] {
        let db = setup_repository_db().await;
        insert_project(&db).await;
        insert_chapter(&db).await;
        let claimed = claim_chapter_repair_step(&db).await;
        let expected = ChapterBusinessSnapshot::load(&db, PROJECT_ID, CHAPTER_ID)
            .await
            .expect("load repair snapshot");
        let mut evidence =
            repair_failure_evidence(&claimed, &expected, "作用域受保护的返修候选正文");
        if invalid_scope {
            evidence
                .draft_attempt
                .repair_payload
                .as_mut()
                .and_then(serde_json::Value::as_object_mut)
                .expect("repair payload object")
                .insert("run_id".to_string(), json!("other-run"));
        } else {
            let mut edited = load_chapter(&db).await.into_active_model();
            edited.content = Set(Some("人工编辑后的正文必须保留".to_string()));
            edited.word_count = Set(11);
            edited.update(&db).await.expect("simulate manual edit");
        }

        let error = NovelAutopilotRepository::finish_chapter_repair_failure(
            &db,
            &claimed.step.id,
            USER_ID,
            claimed.run.version,
            claimed.run.epoch,
            REPAIR_STEP_KEY,
            Some(REPAIR_TASK_ID),
            "chapter_repair_quality_retry",
            NovelAutopilotFailureCounterKind::Quality,
            None,
            false,
            NovelAutopilotQualityDecision::Retry,
            Some(evidence),
        )
        .await
        .expect_err("stale chapter or invalid scope must reject retry evidence");

        assert_eq!(
            error,
            if invalid_scope {
                NovelAutopilotRepositoryError::InvalidTransition
            } else {
                NovelAutopilotRepositoryError::BusinessDataChanged
            }
        );
        assert_repair_failure_not_applied(&db, &claimed).await;
        assert_eq!(
            chapter_draft_attempt::Entity::find()
                .count(&db)
                .await
                .expect("count rejected retry drafts"),
            0
        );
    }
}
