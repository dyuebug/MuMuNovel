use chrono::NaiveDate;
use sea_orm::{
    ActiveModelTrait, ConnectionTrait, Database, DatabaseConnection, DbBackend, EntityTrait,
    IntoActiveModel, Schema, Set, Statement,
};
use serde_json::json;

use crate::{
    models::{
        chapter, novel_autopilot_run, novel_autopilot_step_run, outline, plot_analysis, project,
    },
    services::{
        chapter_content_digest_service::chapter_content_digest,
        novel_workflow_service::{get_state, NovelWorkflowPhase},
        project_export_service::{build_project_export_artifact, PROJECT_EXPORT_FORMAT_TXT},
    },
};

use super::{
    completion_gate_service::{
        advance_complete_book_workflow_once, evaluate_complete_book_completion_gate,
        NovelAutopilotCompletionGateDecision, NovelAutopilotCompletionGateError,
    },
    facts::enrich_novel_autopilot_completion_facts,
    repository::{
        CreateNovelAutopilotRun, NovelAutopilotRepository, NovelAutopilotRepositoryError,
    },
    router::NovelAutopilotBusinessFacts,
    types::{NovelAutopilotRunConfig, NovelAutopilotRunStatus},
};

const PROJECT_ID: &str = "project-completion-gate";
const USER_ID: &str = "owner-completion-gate";
const CHAPTER_ID: &str = "chapter-completion-gate-1";
const OUTLINE_ID: &str = "outline-completion-gate-1";
const INITIAL_CONTENT: &str = "第一章正文，风暴穿过群岛，失落王庭重新显现。";

async fn setup_db() -> DatabaseConnection {
    let db = Database::connect("sqlite::memory:")
        .await
        .expect("connect completion gate SQLite memory database");
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
            .expect("create completion gate test table");
    }
    db.execute(Statement::from_string(
        builder,
        "CREATE UNIQUE INDEX uq_test_completion_gate_active_scope \
         ON novel_autopilot_runs (active_scope_key)"
            .to_string(),
    ))
    .await
    .expect("create completion gate active-run uniqueness index");
    db
}

fn test_time() -> chrono::NaiveDateTime {
    NaiveDate::from_ymd_opt(2026, 7, 19)
        .expect("valid date")
        .and_hms_opt(18, 0, 0)
        .expect("valid time")
}

async fn seed_ready_book(db: &DatabaseConnection, workflow_phase: NovelWorkflowPhase) {
    let now = test_time();
    project::ActiveModel {
        id: Set(PROJECT_ID.to_string()),
        user_id: Set(USER_ID.to_string()),
        title: Set("Durable Completion Gate 测试".to_string()),
        description: Set(Some("群岛与失落王庭的完整故事。".to_string())),
        theme: Set(Some("秩序与自由".to_string())),
        genre: Set(Some("奇幻".to_string())),
        target_words: Set(100_000),
        current_words: Set(24),
        status: Set(workflow_phase.as_str().to_string()),
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
    .expect("insert completion gate project");

    outline::ActiveModel {
        id: Set(OUTLINE_ID.to_string()),
        project_id: Set(PROJECT_ID.to_string()),
        title: Set("第一章大纲".to_string()),
        content: Set(Some("风暴揭开失落王庭。".to_string())),
        structure: Set(None),
        order_index: Set(Some(1)),
        created_at: Set(now),
        updated_at: Set(Some(now)),
    }
    .insert(db)
    .await
    .expect("insert completion gate outline");

    chapter::ActiveModel {
        id: Set(CHAPTER_ID.to_string()),
        project_id: Set(PROJECT_ID.to_string()),
        chapter_number: Set(1),
        title: Set("风暴王庭".to_string()),
        content: Set(Some(INITIAL_CONTENT.to_string())),
        summary: Set(Some("王庭在风暴中重现。".to_string())),
        word_count: Set(24),
        status: Set("completed".to_string()),
        outline_id: Set(Some(OUTLINE_ID.to_string())),
        sub_index: Set(0),
        expansion_plan: Set(None),
        created_at: Set(now),
        updated_at: Set(Some(now)),
    }
    .insert(db)
    .await
    .expect("insert completion gate chapter");

    plot_analysis::ActiveModel {
        id: Set("analysis-completion-gate-1".to_string()),
        project_id: Set(PROJECT_ID.to_string()),
        chapter_id: Set(CHAPTER_ID.to_string()),
        source_content_digest: Set(Some(chapter_content_digest(INITIAL_CONTENT))),
        hooks_count: Set(1),
        foreshadows_planted: Set(1),
        foreshadows_resolved: Set(1),
        plot_points_count: Set(2),
        overall_quality_score: Set(Some(8.5)),
        pacing_score: Set(Some(8.3)),
        engagement_score: Set(Some(8.6)),
        coherence_score: Set(Some(8.4)),
        suggestions: Set(Some(json!([]))),
        created_at: Set(Some(now)),
        ..Default::default()
    }
    .insert(db)
    .await
    .expect("insert completion gate analysis");
}

fn ready_facts() -> NovelAutopilotBusinessFacts {
    NovelAutopilotBusinessFacts {
        target_chapter_count: 1,
        completed_chapter_count: 1,
        book_review_completed: true,
        book_polish_completed: true,
        export_completed: true,
        ..NovelAutopilotBusinessFacts::default()
    }
}

async fn create_run_with_current_export(
    db: &DatabaseConnection,
) -> (novel_autopilot_run::Model, NovelAutopilotRunConfig) {
    let config = NovelAutopilotRunConfig::default();
    let created = NovelAutopilotRepository::create_or_get_active(
        db,
        CreateNovelAutopilotRun {
            project_id: PROJECT_ID.to_string(),
            user_id: USER_ID.to_string(),
            total_chapters: 1,
            config: config.clone(),
        },
    )
    .await
    .expect("create completion gate run");
    let artifact =
        build_project_export_artifact(db, PROJECT_ID, USER_ID, PROJECT_EXPORT_FORMAT_TXT)
            .await
            .expect("build current export artifact");
    let mut active = created.run.into_active_model();
    active.completed_chapters = Set(1);
    active.total_word_count = Set(24);
    active.final_export_ref = Set(Some(
        serde_json::to_string(&artifact.descriptor).expect("serialize export descriptor"),
    ));
    let run = active
        .update(db)
        .await
        .expect("persist completion gate run");
    (run, config)
}

#[tokio::test]
async fn ready_book_advances_exactly_one_workflow_phase_per_tick() {
    let db = setup_db().await;
    seed_ready_book(&db, NovelWorkflowPhase::Writing).await;
    let (run, config) = create_run_with_current_export(&db).await;
    let facts = ready_facts();

    let decision = evaluate_complete_book_completion_gate(&db, &run, USER_ID, &config, &facts)
        .await
        .expect("evaluate ready completion gate");
    let NovelAutopilotCompletionGateDecision::AdvanceWorkflow {
        report,
        expected,
        target,
    } = decision
    else {
        panic!("ready writing workflow must advance exactly one phase");
    };
    assert!(report.reason_codes.is_empty());
    assert_eq!(report.workflow_phase, NovelWorkflowPhase::Writing.as_str());
    assert_eq!(expected, NovelWorkflowPhase::Writing);
    assert_eq!(target, NovelWorkflowPhase::Reviewing);

    advance_complete_book_workflow_once(&db, &run, USER_ID, expected, target)
        .await
        .expect("advance writing to reviewing");
    let state = get_state(&db, PROJECT_ID, USER_ID)
        .await
        .expect("reload workflow state");
    assert_eq!(state.phase, NovelWorkflowPhase::Reviewing);

    let second = evaluate_complete_book_completion_gate(&db, &run, USER_ID, &config, &facts)
        .await
        .expect("evaluate next completion tick");
    assert!(matches!(
        second,
        NovelAutopilotCompletionGateDecision::AdvanceWorkflow {
            expected: NovelWorkflowPhase::Reviewing,
            target: NovelWorkflowPhase::Polishing,
            ..
        }
    ));
}

#[tokio::test]
async fn completed_workflow_is_the_only_ready_terminal_state() {
    let db = setup_db().await;
    seed_ready_book(&db, NovelWorkflowPhase::Completed).await;
    let (run, config) = create_run_with_current_export(&db).await;

    let decision =
        evaluate_complete_book_completion_gate(&db, &run, USER_ID, &config, &ready_facts())
            .await
            .expect("evaluate completed workflow");
    let NovelAutopilotCompletionGateDecision::Ready(report) = decision else {
        panic!("completed workflow with current facts must be terminal-ready");
    };
    assert!(report.ready);
    assert!(report.reason_codes.is_empty());
    assert_eq!(
        report.workflow_phase,
        NovelWorkflowPhase::Completed.as_str()
    );
}

#[tokio::test]
async fn chapter_change_after_export_forces_reroute() {
    let db = setup_db().await;
    seed_ready_book(&db, NovelWorkflowPhase::Completed).await;
    let (run, config) = create_run_with_current_export(&db).await;

    let chapter = chapter::Entity::find_by_id(CHAPTER_ID)
        .one(&db)
        .await
        .expect("load chapter before edit")
        .expect("chapter exists");
    let mut active = chapter.into_active_model();
    active.content = Set(Some("人工修改后的正文，导出描述符必须失效。".to_string()));
    active.word_count = Set(18);
    active
        .update(&db)
        .await
        .expect("update chapter after export");

    let decision =
        evaluate_complete_book_completion_gate(&db, &run, USER_ID, &config, &ready_facts())
            .await
            .expect("evaluate stale export");
    let NovelAutopilotCompletionGateDecision::Reroute(report) = decision else {
        panic!("stale export must reroute instead of completing");
    };
    assert!(!report.ready);
    assert!(report
        .reason_codes
        .iter()
        .any(|reason| reason == "final_export_not_current"));
}

#[tokio::test]
async fn malformed_pending_rewrite_item_is_rejected() {
    let db = setup_db().await;
    seed_ready_book(&db, NovelWorkflowPhase::Completed).await;
    let (run, config) = create_run_with_current_export(&db).await;
    let mut active = run.into_active_model();
    active.pending_rewrites = Set(json!([{ "chapter_id": CHAPTER_ID }]));
    let malformed = active
        .update(&db)
        .await
        .expect("persist malformed rewrite fixture");

    let error =
        evaluate_complete_book_completion_gate(&db, &malformed, USER_ID, &config, &ready_facts())
            .await
            .expect_err("malformed rewrite must not pass completion gate");
    assert_eq!(
        error,
        NovelAutopilotCompletionGateError::InvalidPendingRewrites
    );
}

#[tokio::test]
async fn completion_facts_invalidate_export_after_chapter_change() {
    let db = setup_db().await;
    seed_ready_book(&db, NovelWorkflowPhase::Completed).await;
    let (run, config) = create_run_with_current_export(&db).await;
    let mut facts = ready_facts();

    enrich_novel_autopilot_completion_facts(&db, &run, USER_ID, &config, &mut facts)
        .await
        .expect("enrich facts before chapter change");
    assert!(facts.export_completed);

    let chapter = chapter::Entity::find_by_id(CHAPTER_ID)
        .one(&db)
        .await
        .expect("load chapter for facts invalidation")
        .expect("chapter exists");
    let mut active = chapter.into_active_model();
    active.content = Set(Some("正文被人工更新，旧导出必须重新生成。".to_string()));
    active.word_count = Set(17);
    active
        .update(&db)
        .await
        .expect("update chapter for facts invalidation");

    let mut refreshed = ready_facts();
    enrich_novel_autopilot_completion_facts(&db, &run, USER_ID, &config, &mut refreshed)
        .await
        .expect("enrich facts after chapter change");
    assert!(!refreshed.export_completed);
}

#[tokio::test]
async fn malformed_final_export_ref_forces_reroute() {
    let db = setup_db().await;
    seed_ready_book(&db, NovelWorkflowPhase::Completed).await;
    let (run, config) = create_run_with_current_export(&db).await;
    let mut active = run.into_active_model();
    active.final_export_ref = Set(Some("{malformed-json".to_string()));
    let malformed = active
        .update(&db)
        .await
        .expect("persist malformed export descriptor");
    let mut facts = ready_facts();
    facts.export_completed = false;

    let decision =
        evaluate_complete_book_completion_gate(&db, &malformed, USER_ID, &config, &facts)
            .await
            .expect("evaluate malformed export descriptor");
    let NovelAutopilotCompletionGateDecision::Reroute(report) = decision else {
        panic!("malformed export descriptor must reroute");
    };
    assert!(report
        .reason_codes
        .iter()
        .any(|reason| reason == "final_export_ref_invalid"));
}

#[tokio::test]
async fn releasing_rerouted_tick_fences_stale_completion_cas() {
    let db = setup_db().await;
    seed_ready_book(&db, NovelWorkflowPhase::Completed).await;
    let (queued, _config) = create_run_with_current_export(&db).await;
    let running = NovelAutopilotRepository::transition_owned(
        &db,
        &queued.id,
        USER_ID,
        queued.version,
        NovelAutopilotRunStatus::Running,
    )
    .await
    .expect("start completion gate run");
    let bound = NovelAutopilotRepository::set_active_background_task_owned(
        &db,
        &running.id,
        USER_ID,
        running.version,
        running.epoch,
        Some("completion-gate-task"),
    )
    .await
    .expect("bind completion gate task");
    let released = NovelAutopilotRepository::set_active_background_task_owned(
        &db,
        &bound.id,
        USER_ID,
        bound.version,
        bound.epoch,
        None,
    )
    .await
    .expect("release rerouted completion gate task");
    assert_eq!(released.status, NovelAutopilotRunStatus::Running.as_str());
    assert!(released.active_background_task_id.is_none());
    assert!(released.version > bound.version);

    let stale = NovelAutopilotRepository::transition_owned(
        &db,
        &released.id,
        USER_ID,
        bound.version,
        NovelAutopilotRunStatus::Completed,
    )
    .await
    .expect_err("old tick must not complete after reroute release");
    assert_eq!(stale, NovelAutopilotRepositoryError::StaleVersion);
}

#[tokio::test]
async fn workflow_advance_keeps_run_running_until_terminal_gate_is_ready() {
    let db = setup_db().await;
    seed_ready_book(&db, NovelWorkflowPhase::Writing).await;
    let (queued, config) = create_run_with_current_export(&db).await;
    let running = NovelAutopilotRepository::transition_owned(
        &db,
        &queued.id,
        USER_ID,
        queued.version,
        NovelAutopilotRunStatus::Running,
    )
    .await
    .expect("start workflow completion run");
    let facts = ready_facts();

    let mut current = NovelWorkflowPhase::Writing;
    while current != NovelWorkflowPhase::Completed {
        let decision =
            evaluate_complete_book_completion_gate(&db, &running, USER_ID, &config, &facts)
                .await
                .expect("evaluate workflow completion phase");
        let NovelAutopilotCompletionGateDecision::AdvanceWorkflow {
            expected, target, ..
        } = decision
        else {
            panic!("non-terminal workflow must advance one phase");
        };
        assert_eq!(expected, current);
        advance_complete_book_workflow_once(&db, &running, USER_ID, expected, target)
            .await
            .expect("advance completion workflow");
        let stored_run = NovelAutopilotRepository::find_owned(&db, &running.id, USER_ID)
            .await
            .expect("reload running run");
        assert_eq!(stored_run.status, NovelAutopilotRunStatus::Running.as_str());
        current = target;
    }

    let decision = evaluate_complete_book_completion_gate(&db, &running, USER_ID, &config, &facts)
        .await
        .expect("evaluate terminal completion gate");
    assert!(matches!(
        decision,
        NovelAutopilotCompletionGateDecision::Ready(_)
    ));

    let completed = NovelAutopilotRepository::transition_owned(
        &db,
        &running.id,
        USER_ID,
        running.version,
        NovelAutopilotRunStatus::Completed,
    )
    .await
    .expect("complete run after terminal gate");
    assert_eq!(
        completed.status,
        NovelAutopilotRunStatus::Completed.as_str()
    );
    assert!(completed.active_scope_key.is_none());
}
