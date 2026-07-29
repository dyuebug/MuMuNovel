use super::{
    router::{
        route_next_step, NovelAutopilotBusinessFacts, NovelAutopilotRouteDecision,
        NovelAutopilotRouteSnapshot,
    },
    types::{
        NovelAutopilotExecutionScope, NovelAutopilotHumanGateMode, NovelAutopilotPhase,
        NovelAutopilotRunConfig, NovelAutopilotRunStatus, NovelAutopilotStepType,
    },
};

fn ready_planning_facts() -> NovelAutopilotBusinessFacts {
    NovelAutopilotBusinessFacts {
        foundation_ready: true,
        world_ready: true,
        careers_ready: true,
        characters_ready: true,
        organizations_ready: true,
        outline_ready: true,
        ..Default::default()
    }
}

#[test]
fn run_status_transitions_are_explicit_and_terminal_states_are_closed() {
    assert!(NovelAutopilotRunStatus::Queued.can_transition_to(NovelAutopilotRunStatus::Running));
    assert!(NovelAutopilotRunStatus::Running.can_transition_to(NovelAutopilotRunStatus::Paused));
    assert!(NovelAutopilotRunStatus::Paused.can_transition_to(NovelAutopilotRunStatus::Queued));
    assert!(!NovelAutopilotRunStatus::Paused.can_transition_to(NovelAutopilotRunStatus::Completed));
    assert!(NovelAutopilotRunStatus::Completed.is_terminal());
    assert!(!NovelAutopilotRunStatus::Completed.can_schedule());
}

#[test]
fn config_rejects_missing_next_n_count_and_invalid_gate_interval() {
    let next_n = NovelAutopilotRunConfig {
        execution_scope: NovelAutopilotExecutionScope::NextNChapters,
        ..Default::default()
    };
    assert_eq!(next_n.validate().unwrap_err().field, "next_chapter_count");

    let gate = NovelAutopilotRunConfig {
        human_gate_mode: NovelAutopilotHumanGateMode::EveryNChapters,
        gate_interval: 0,
        ..Default::default()
    };
    assert_eq!(gate.validate().unwrap_err().field, "gate_interval");
}

#[test]
fn router_plans_missing_foundation_before_other_materials() {
    let snapshot = NovelAutopilotRouteSnapshot {
        status: NovelAutopilotRunStatus::Running,
        config: NovelAutopilotRunConfig::default(),
        facts: NovelAutopilotBusinessFacts::default(),
    };

    let NovelAutopilotRouteDecision::Execute(plan) = route_next_step(&snapshot) else {
        panic!("expected an executable plan");
    };
    assert_eq!(plan.step_key, "planning:foundation");
    assert_eq!(plan.step_type, NovelAutopilotStepType::Foundation);
}

#[test]
fn planning_only_completes_after_outline() {
    let snapshot = NovelAutopilotRouteSnapshot {
        status: NovelAutopilotRunStatus::Running,
        config: NovelAutopilotRunConfig {
            execution_scope: NovelAutopilotExecutionScope::PlanningOnly,
            ..Default::default()
        },
        facts: ready_planning_facts(),
    };

    assert_eq!(
        route_next_step(&snapshot),
        NovelAutopilotRouteDecision::Complete(NovelAutopilotPhase::Completed)
    );
}

#[test]
fn one_to_many_planning_routes_one_outline_expansion_per_tick() {
    let mut facts = ready_planning_facts();
    facts.outline_mode = "one-to-many".to_string();
    facts.target_chapter_count = 5;
    facts.current_chapter_count = 0;
    facts.next_unexpanded_outline_id = Some("outline-1".to_string());
    facts.next_unexpanded_outline_order = Some(1);
    facts.remaining_unexpanded_outline_count = 2;

    let snapshot = NovelAutopilotRouteSnapshot {
        status: NovelAutopilotRunStatus::Running,
        config: NovelAutopilotRunConfig {
            execution_scope: NovelAutopilotExecutionScope::PlanningOnly,
            ..Default::default()
        },
        facts,
    };

    let NovelAutopilotRouteDecision::Execute(first) = route_next_step(&snapshot) else {
        panic!("one-to-many planning must expand the first outline");
    };
    assert_eq!(first.step_type, NovelAutopilotStepType::OutlineExpand);
    assert_eq!(first.step_key, "planning:outline_expand:0001:outline-1");
    assert_eq!(first.outline_id.as_deref(), Some("outline-1"));
    assert_eq!(first.target_chapter_count, Some(3));

    let mut next_facts = snapshot.facts.clone();
    next_facts.current_chapter_count = 3;
    next_facts.next_unexpanded_outline_id = Some("outline-2".to_string());
    next_facts.next_unexpanded_outline_order = Some(2);
    next_facts.remaining_unexpanded_outline_count = 1;
    let next_snapshot = NovelAutopilotRouteSnapshot {
        facts: next_facts,
        ..snapshot
    };
    let NovelAutopilotRouteDecision::Execute(second) = route_next_step(&next_snapshot) else {
        panic!("the next tick must expand only the next outline");
    };
    assert_eq!(second.step_key, "planning:outline_expand:0002:outline-2");
    assert_eq!(second.outline_id.as_deref(), Some("outline-2"));
    assert_eq!(second.target_chapter_count, Some(2));
}

#[test]
fn one_to_many_planning_only_completes_after_all_outlines_are_expanded() {
    let mut facts = ready_planning_facts();
    facts.outline_mode = "one-to-many".to_string();
    facts.target_chapter_count = 5;
    facts.current_chapter_count = 5;
    facts.remaining_unexpanded_outline_count = 0;

    let snapshot = NovelAutopilotRouteSnapshot {
        status: NovelAutopilotRunStatus::Running,
        config: NovelAutopilotRunConfig {
            execution_scope: NovelAutopilotExecutionScope::PlanningOnly,
            ..Default::default()
        },
        facts,
    };

    assert_eq!(
        route_next_step(&snapshot),
        NovelAutopilotRouteDecision::Complete(NovelAutopilotPhase::Completed)
    );
}

#[test]
fn next_n_chapters_stops_at_requested_count() {
    let mut facts = ready_planning_facts();
    facts.target_chapter_count = 20;
    facts.completed_chapter_count = 7;
    facts.chapters_completed_in_run = 3;
    facts.next_incomplete_chapter_id = Some("chapter-8".to_string());
    facts.next_incomplete_chapter_number = Some(8);

    let snapshot = NovelAutopilotRouteSnapshot {
        status: NovelAutopilotRunStatus::Running,
        config: NovelAutopilotRunConfig {
            execution_scope: NovelAutopilotExecutionScope::NextNChapters,
            next_chapter_count: Some(3),
            ..Default::default()
        },
        facts,
    };

    assert_eq!(
        route_next_step(&snapshot),
        NovelAutopilotRouteDecision::Complete(NovelAutopilotPhase::Completed)
    );
}

#[test]
fn continue_from_current_starts_at_first_incomplete_chapter_and_stops_at_outline_end() {
    let mut facts = ready_planning_facts();
    facts.target_chapter_count = 3;
    facts.current_chapter_count = 3;
    facts.completed_chapter_count = 1;
    facts.next_incomplete_chapter_id = Some("chapter-2".to_string());
    facts.next_incomplete_chapter_number = Some(2);

    let snapshot = NovelAutopilotRouteSnapshot {
        status: NovelAutopilotRunStatus::Running,
        config: NovelAutopilotRunConfig {
            execution_scope: NovelAutopilotExecutionScope::ContinueFromCurrent,
            ..Default::default()
        },
        facts,
    };

    let NovelAutopilotRouteDecision::Execute(plan) = route_next_step(&snapshot) else {
        panic!("continue_from_current must start at the first incomplete chapter");
    };
    assert_eq!(plan.step_type, NovelAutopilotStepType::ChapterGenerate);
    assert_eq!(plan.chapter_id.as_deref(), Some("chapter-2"));
    assert_eq!(plan.chapter_number, Some(2));
    assert_eq!(plan.step_key, "chapter:0002:generate");

    let mut completed_facts = snapshot.facts.clone();
    completed_facts.completed_chapter_count = 3;
    completed_facts.next_incomplete_chapter_id = None;
    completed_facts.next_incomplete_chapter_number = None;
    let completed = NovelAutopilotRouteSnapshot {
        facts: completed_facts,
        ..snapshot
    };
    assert_eq!(
        route_next_step(&completed),
        NovelAutopilotRouteDecision::Complete(NovelAutopilotPhase::Completed)
    );
}

#[test]
fn complete_book_routes_chapter_then_review_polish_and_export() {
    let mut facts = ready_planning_facts();
    facts.target_chapter_count = 3;
    facts.completed_chapter_count = 2;
    facts.next_incomplete_chapter_id = Some("chapter-3".to_string());
    facts.next_incomplete_chapter_number = Some(3);

    let snapshot = NovelAutopilotRouteSnapshot {
        status: NovelAutopilotRunStatus::Running,
        config: NovelAutopilotRunConfig::default(),
        facts,
    };
    let NovelAutopilotRouteDecision::Execute(plan) = route_next_step(&snapshot) else {
        panic!("expected chapter plan");
    };
    assert_eq!(plan.step_key, "chapter:0003:generate");

    let mut facts = snapshot.facts.clone();
    facts.completed_chapter_count = 3;
    facts.next_incomplete_chapter_id = None;
    facts.next_incomplete_chapter_number = None;
    let review = NovelAutopilotRouteSnapshot { facts, ..snapshot };
    let NovelAutopilotRouteDecision::Execute(plan) = route_next_step(&review) else {
        panic!("expected review plan");
    };
    assert_eq!(plan.step_key, "completion:book_review");
}

#[test]
fn complete_book_routes_pending_rewrite_to_chapter_bound_polish() {
    let mut facts = ready_planning_facts();
    facts.target_chapter_count = 3;
    facts.completed_chapter_count = 3;
    facts.book_review_completed = true;
    facts.book_polish_completed = false;
    facts.pending_polish_chapter_id = Some("chapter-2".to_string());
    facts.pending_polish_chapter_number = Some(2);

    let snapshot = NovelAutopilotRouteSnapshot {
        status: NovelAutopilotRunStatus::Running,
        config: NovelAutopilotRunConfig::default(),
        facts,
    };
    let NovelAutopilotRouteDecision::Execute(plan) = route_next_step(&snapshot) else {
        panic!("expected chapter-bound book polish plan");
    };
    assert_eq!(
        plan.step_key,
        "completion:book_polish:chapter:0002:chapter-2"
    );
    assert_eq!(plan.step_type, NovelAutopilotStepType::BookPolish);
    assert_eq!(plan.phase, NovelAutopilotPhase::BookPolish);
    assert_eq!(plan.chapter_id.as_deref(), Some("chapter-2"));
    assert_eq!(plan.chapter_number, Some(2));
}

#[test]
fn complete_book_rejects_polish_without_a_pending_chapter_reference() {
    let mut facts = ready_planning_facts();
    facts.target_chapter_count = 1;
    facts.completed_chapter_count = 1;
    facts.book_review_completed = true;
    facts.book_polish_completed = false;

    let snapshot = NovelAutopilotRouteSnapshot {
        status: NovelAutopilotRunStatus::Running,
        config: NovelAutopilotRunConfig::default(),
        facts,
    };
    assert_eq!(
        route_next_step(&snapshot),
        NovelAutopilotRouteDecision::InvalidFacts("book_polish_rewrite_missing")
    );
}

#[test]
fn complete_book_can_skip_polish_and_route_to_export() {
    let mut facts = ready_planning_facts();
    facts.target_chapter_count = 1;
    facts.completed_chapter_count = 1;
    facts.book_review_completed = true;
    facts.book_polish_completed = false;
    facts.pending_polish_chapter_id = Some("chapter-1".to_string());
    facts.pending_polish_chapter_number = Some(1);

    let snapshot = NovelAutopilotRouteSnapshot {
        status: NovelAutopilotRunStatus::Running,
        config: NovelAutopilotRunConfig {
            run_book_polish: false,
            ..NovelAutopilotRunConfig::default()
        },
        facts,
    };
    let NovelAutopilotRouteDecision::Execute(plan) = route_next_step(&snapshot) else {
        panic!("expected export plan when book polish is disabled");
    };
    assert_eq!(plan.step_key, "completion:export");
    assert_eq!(plan.step_type, NovelAutopilotStepType::Export);
}

#[test]
fn paused_waiting_and_terminal_runs_do_not_schedule() {
    for status in [
        NovelAutopilotRunStatus::Paused,
        NovelAutopilotRunStatus::WaitingHuman,
        NovelAutopilotRunStatus::Completed,
        NovelAutopilotRunStatus::Failed,
        NovelAutopilotRunStatus::Cancelled,
    ] {
        let snapshot = NovelAutopilotRouteSnapshot {
            status,
            config: NovelAutopilotRunConfig::default(),
            facts: NovelAutopilotBusinessFacts::default(),
        };
        assert_eq!(
            route_next_step(&snapshot),
            NovelAutopilotRouteDecision::Idle
        );
    }
}

#[cfg(test)]
mod repository_tests {
    use chrono::NaiveDate;
    use sea_orm::{
        ActiveModelTrait, ColumnTrait, ConnectionTrait, Database, DatabaseConnection, DbBackend,
        EntityTrait, IntoActiveModel, PaginatorTrait, QueryFilter, Schema, Set, Statement,
    };

    use crate::models::{
        career, character, novel_autopilot_run, novel_autopilot_step_run, organization,
        organization_member, project, relationship,
    };

    use super::super::{
        repository::{
            CreateNovelAutopilotRun, CreateNovelAutopilotStepAttempt, NovelAutopilotCareerCommit,
            NovelAutopilotCareerItemCommit, NovelAutopilotCareerSnapshot,
            NovelAutopilotFoundationCommit, NovelAutopilotFoundationSnapshot,
            NovelAutopilotOrganizationCommit, NovelAutopilotOrganizationMemberCommit,
            NovelAutopilotOrganizationRelationshipCommit, NovelAutopilotOrganizationSnapshot,
            NovelAutopilotRepository, NovelAutopilotRepositoryError,
            NovelAutopilotStepTerminalPatch, NovelAutopilotWorldCommit,
            NovelAutopilotWorldSnapshot, PrepareAndClaimNovelAutopilotStep,
        },
        types::{
            NovelAutopilotPhase, NovelAutopilotPrivateSnapshot, NovelAutopilotRunConfig,
            NovelAutopilotRunStatus, NovelAutopilotStepStatus, NovelAutopilotStepType,
        },
    };

    async fn setup_repository_db() -> DatabaseConnection {
        let db = Database::connect("sqlite::memory:")
            .await
            .expect("connect repository sqlite memory db");
        let builder = DbBackend::Sqlite;
        let schema = Schema::new(builder);
        db.execute(builder.build(&schema.create_table_from_entity(project::Entity)))
            .await
            .expect("create projects table");
        db.execute(builder.build(&schema.create_table_from_entity(career::Entity)))
            .await
            .expect("create careers table");
        db.execute(builder.build(&schema.create_table_from_entity(character::Entity)))
            .await
            .expect("create characters table");
        db.execute(builder.build(&schema.create_table_from_entity(organization::Entity)))
            .await
            .expect("create organizations table");
        db.execute(builder.build(&schema.create_table_from_entity(organization_member::Entity)))
            .await
            .expect("create organization members table");
        db.execute(builder.build(&schema.create_table_from_entity(relationship::Entity)))
            .await
            .expect("create character relationships table");
        db.execute(builder.build(&schema.create_table_from_entity(novel_autopilot_run::Entity)))
            .await
            .expect("create novel autopilot runs table");
        db.execute(
            builder.build(&schema.create_table_from_entity(novel_autopilot_step_run::Entity)),
        )
        .await
        .expect("create novel autopilot step runs table");
        db.execute(Statement::from_string(
            builder,
            "CREATE UNIQUE INDEX uq_test_novel_autopilot_active_scope ON novel_autopilot_runs (active_scope_key)".to_string(),
        ))
        .await
        .expect("create active run uniqueness index");
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
        .expect("insert project");
    }

    async fn insert_character(
        db: &DatabaseConnection,
        project_id: &str,
        id: &str,
        name: &str,
        is_organization: bool,
    ) {
        let now = NaiveDate::from_ymd_opt(2026, 7, 19)
            .expect("valid date")
            .and_hms_opt(9, 0, 0)
            .expect("valid time");
        character::ActiveModel {
            id: Set(id.to_string()),
            project_id: Set(project_id.to_string()),
            name: Set(name.to_string()),
            age: Set(None),
            gender: Set(None),
            is_organization: Set(is_organization),
            role_type: Set(Some("supporting".to_string())),
            personality: Set(None),
            background: Set(None),
            appearance: Set(None),
            relationships: Set(None),
            organization_type: Set(is_organization.then(|| "其他".to_string())),
            organization_purpose: Set(None),
            organization_members: Set(None),
            status: Set("active".to_string()),
            status_changed_chapter: Set(None),
            current_state: Set(None),
            state_updated_chapter: Set(None),
            main_career_id: Set(None),
            main_career_stage: Set(None),
            sub_careers: Set(None),
            avatar_url: Set(None),
            traits: Set(None),
            created_at: Set(now),
            updated_at: Set(Some(now)),
        }
        .insert(db)
        .await
        .expect("insert character");
    }

    fn create_input(project_id: &str, user_id: &str) -> CreateNovelAutopilotRun {
        CreateNovelAutopilotRun {
            project_id: project_id.to_string(),
            user_id: user_id.to_string(),
            total_chapters: 10,
            config: NovelAutopilotRunConfig::default(),
        }
    }

    #[tokio::test]
    async fn duplicate_create_returns_the_same_active_run() {
        let db = setup_repository_db().await;
        insert_project(&db, "project-1", "owner-1").await;

        let first = NovelAutopilotRepository::create_or_get_active(
            &db,
            create_input("project-1", "owner-1"),
        )
        .await
        .expect("create first run");
        let second = NovelAutopilotRepository::create_or_get_active(
            &db,
            create_input("project-1", "owner-1"),
        )
        .await
        .expect("return active run");

        assert!(first.created);
        assert!(!second.created);
        assert_eq!(first.run.id, second.run.id);
        assert_eq!(
            NovelAutopilotRepository::list_owned(&db, "project-1", "owner-1")
                .await
                .expect("list owned runs")
                .len(),
            1
        );
    }

    #[tokio::test]
    async fn another_user_cannot_read_or_list_the_run() {
        let db = setup_repository_db().await;
        insert_project(&db, "project-1", "owner-1").await;
        let created = NovelAutopilotRepository::create_or_get_active(
            &db,
            create_input("project-1", "owner-1"),
        )
        .await
        .expect("create run");

        assert_eq!(
            NovelAutopilotRepository::find_owned(&db, &created.run.id, "owner-2")
                .await
                .unwrap_err(),
            NovelAutopilotRepositoryError::NotFoundOrAccessDenied
        );
        assert_eq!(
            NovelAutopilotRepository::list_owned(&db, "project-1", "owner-2")
                .await
                .unwrap_err(),
            NovelAutopilotRepositoryError::NotFoundOrAccessDenied
        );
    }

    #[tokio::test]
    async fn pause_increments_epoch_and_version_and_rejects_stale_version() {
        let db = setup_repository_db().await;
        insert_project(&db, "project-1", "owner-1").await;
        let created = NovelAutopilotRepository::create_or_get_active(
            &db,
            create_input("project-1", "owner-1"),
        )
        .await
        .expect("create run");
        let running = NovelAutopilotRepository::transition_owned(
            &db,
            &created.run.id,
            "owner-1",
            0,
            NovelAutopilotRunStatus::Running,
        )
        .await
        .expect("start run");
        let paused = NovelAutopilotRepository::transition_owned(
            &db,
            &created.run.id,
            "owner-1",
            running.version,
            NovelAutopilotRunStatus::Paused,
        )
        .await
        .expect("pause run");

        assert_eq!(paused.epoch, running.epoch + 1);
        assert_eq!(paused.version, running.version + 1);
        assert_eq!(
            NovelAutopilotRepository::transition_owned(
                &db,
                &created.run.id,
                "owner-1",
                running.version,
                NovelAutopilotRunStatus::Queued,
            )
            .await
            .unwrap_err(),
            NovelAutopilotRepositoryError::StaleVersion
        );
    }

    #[tokio::test]
    async fn paused_guidance_update_upgrades_legacy_snapshot_and_advances_fence() {
        let db = setup_repository_db().await;
        insert_project(&db, "project-guidance-paused", "owner-1").await;
        let created = NovelAutopilotRepository::create_or_get_active(
            &db,
            create_input("project-guidance-paused", "owner-1"),
        )
        .await
        .expect("create guidance run");
        let running = NovelAutopilotRepository::transition_owned(
            &db,
            &created.run.id,
            "owner-1",
            created.run.version,
            NovelAutopilotRunStatus::Running,
        )
        .await
        .expect("start guidance run");
        let paused = NovelAutopilotRepository::transition_owned(
            &db,
            &created.run.id,
            "owner-1",
            running.version,
            NovelAutopilotRunStatus::Paused,
        )
        .await
        .expect("pause guidance run");

        let legacy_config = NovelAutopilotPrivateSnapshot::decode(&paused.config_snapshot)
            .expect("decode initial private snapshot")
            .config;
        let mut legacy_run = paused.clone().into_active_model();
        legacy_run.config_snapshot =
            Set(serde_json::to_value(legacy_config)
                .expect("serialize legacy direct config snapshot"));
        let legacy_run = legacy_run
            .update(&db)
            .await
            .expect("persist legacy snapshot");

        let guidance = "后续章节增强人物冲突，并保留已有伏笔";
        let guidance_digest = "a".repeat(64);
        let updated = NovelAutopilotRepository::update_guidance(
            &db,
            &legacy_run.id,
            "owner-1",
            legacy_run.version,
            guidance,
            &guidance_digest,
        )
        .await
        .expect("update paused guidance");

        assert_eq!(updated.version, legacy_run.version + 1);
        assert_eq!(updated.epoch, legacy_run.epoch + 1);
        assert_eq!(
            updated.guidance_digest.as_deref(),
            Some(guidance_digest.as_str())
        );
        let private_snapshot = NovelAutopilotPrivateSnapshot::decode(&updated.config_snapshot)
            .expect("decode upgraded private snapshot");
        assert_eq!(private_snapshot.guidance.as_deref(), Some(guidance));
        assert_eq!(private_snapshot.config, NovelAutopilotRunConfig::default());
        assert!(updated.config_snapshot.get("config").is_some());
        assert!(updated.config_snapshot.get("guidance").is_some());
    }

    #[tokio::test]
    async fn waiting_human_guidance_update_is_owned_and_rejects_stale_version() {
        let db = setup_repository_db().await;
        insert_project(&db, "project-guidance-human", "owner-1").await;
        let created = NovelAutopilotRepository::create_or_get_active(
            &db,
            create_input("project-guidance-human", "owner-1"),
        )
        .await
        .expect("create guidance run");
        let running = NovelAutopilotRepository::transition_owned(
            &db,
            &created.run.id,
            "owner-1",
            created.run.version,
            NovelAutopilotRunStatus::Running,
        )
        .await
        .expect("start guidance run");
        let waiting = NovelAutopilotRepository::transition_owned(
            &db,
            &created.run.id,
            "owner-1",
            running.version,
            NovelAutopilotRunStatus::WaitingHuman,
        )
        .await
        .expect("enter human gate");

        assert_eq!(
            NovelAutopilotRepository::update_guidance(
                &db,
                &waiting.id,
                "owner-2",
                waiting.version,
                "错误用户指导",
                &"b".repeat(64),
            )
            .await
            .unwrap_err(),
            NovelAutopilotRepositoryError::NotFoundOrAccessDenied
        );

        let updated = NovelAutopilotRepository::update_guidance(
            &db,
            &waiting.id,
            "owner-1",
            waiting.version,
            "下一阶段加强悬念",
            &"c".repeat(64),
        )
        .await
        .expect("update waiting-human guidance");
        assert_eq!(updated.version, waiting.version + 1);
        assert_eq!(updated.epoch, waiting.epoch + 1);

        assert_eq!(
            NovelAutopilotRepository::update_guidance(
                &db,
                &waiting.id,
                "owner-1",
                waiting.version,
                "过期版本指导",
                &"d".repeat(64),
            )
            .await
            .unwrap_err(),
            NovelAutopilotRepositoryError::StaleVersion
        );
    }

    #[tokio::test]
    async fn guidance_update_rejects_running_and_terminal_runs() {
        let db = setup_repository_db().await;
        insert_project(&db, "project-guidance-state", "owner-1").await;
        let created = NovelAutopilotRepository::create_or_get_active(
            &db,
            create_input("project-guidance-state", "owner-1"),
        )
        .await
        .expect("create guidance run");
        let running = NovelAutopilotRepository::transition_owned(
            &db,
            &created.run.id,
            "owner-1",
            created.run.version,
            NovelAutopilotRunStatus::Running,
        )
        .await
        .expect("start guidance run");

        assert_eq!(
            NovelAutopilotRepository::update_guidance(
                &db,
                &running.id,
                "owner-1",
                running.version,
                "运行中不允许更新",
                &"e".repeat(64),
            )
            .await
            .unwrap_err(),
            NovelAutopilotRepositoryError::InvalidTransition
        );

        let completed = NovelAutopilotRepository::transition_owned(
            &db,
            &running.id,
            "owner-1",
            running.version,
            NovelAutopilotRunStatus::Completed,
        )
        .await
        .expect("complete guidance run");
        assert_eq!(
            NovelAutopilotRepository::update_guidance(
                &db,
                &completed.id,
                "owner-1",
                completed.version,
                "终态不允许更新",
                &"f".repeat(64),
            )
            .await
            .unwrap_err(),
            NovelAutopilotRepositoryError::InvalidTransition
        );
    }

    #[tokio::test]
    async fn paused_run_rejects_late_step_terminal_commit() {
        let db = setup_repository_db().await;
        insert_project(&db, "project-1", "owner-1").await;
        let created = NovelAutopilotRepository::create_or_get_active(
            &db,
            create_input("project-1", "owner-1"),
        )
        .await
        .expect("create run");
        let step = NovelAutopilotRepository::create_step_attempt(
            &db,
            CreateNovelAutopilotStepAttempt {
                run_id: created.run.id.clone(),
                user_id: "owner-1".to_string(),
                step_key: "planning:foundation".to_string(),
                step_type: NovelAutopilotStepType::Foundation,
                phase: NovelAutopilotPhase::Foundation,
                chapter_id: None,
                chapter_number: None,
                run_epoch: 0,
                input_digest: "digest-1".to_string(),
            },
        )
        .await
        .expect("create step");
        let claimed = NovelAutopilotRepository::claim_step(
            &db,
            &step.id,
            "owner-1",
            created.run.version,
            created.run.epoch,
            Some("background-1"),
        )
        .await
        .expect("claim step");
        let paused = NovelAutopilotRepository::transition_owned(
            &db,
            &created.run.id,
            "owner-1",
            claimed.run.version,
            NovelAutopilotRunStatus::Paused,
        )
        .await
        .expect("pause claimed run");

        assert_eq!(
            NovelAutopilotRepository::complete_step(
                &db,
                &step.id,
                "owner-1",
                paused.version,
                0,
                "planning:foundation",
                Some("background-1"),
                NovelAutopilotStepStatus::Completed,
                NovelAutopilotStepTerminalPatch::default(),
            )
            .await
            .unwrap_err(),
            NovelAutopilotRepositoryError::StaleEpoch
        );
        assert!(paused.current_step.is_none());
        let paused_step =
            NovelAutopilotRepository::list_steps_owned(&db, &created.run.id, "owner-1")
                .await
                .expect("list paused step")
                .pop()
                .expect("paused step exists");
        assert_eq!(paused_step.status, NovelAutopilotStepStatus::Stale.as_str());
        assert_eq!(paused_step.error_code.as_deref(), Some("run_paused"));

        let resumed = NovelAutopilotRepository::transition_owned(
            &db,
            &created.run.id,
            "owner-1",
            paused.version,
            NovelAutopilotRunStatus::Queued,
        )
        .await
        .expect("resume paused run");
        let reclaimed = NovelAutopilotRepository::prepare_and_claim_step(
            &db,
            PrepareAndClaimNovelAutopilotStep {
                attempt: CreateNovelAutopilotStepAttempt {
                    run_id: created.run.id.clone(),
                    user_id: "owner-1".to_string(),
                    step_key: "planning:foundation".to_string(),
                    step_type: NovelAutopilotStepType::Foundation,
                    phase: NovelAutopilotPhase::Foundation,
                    chapter_id: None,
                    chapter_number: None,
                    run_epoch: resumed.epoch,
                    input_digest: "digest-2".to_string(),
                },
                expected_run_version: resumed.version,
                background_task_id: Some("background-2".to_string()),
            },
        )
        .await
        .expect("resume can claim a new attempt");
        assert_eq!(reclaimed.step.attempt, 2);
        assert_eq!(
            reclaimed.step.status,
            NovelAutopilotStepStatus::Running.as_str()
        );
    }

    #[tokio::test]
    async fn prepare_and_claim_is_atomic_and_owns_the_active_task_cursor() {
        let db = setup_repository_db().await;
        insert_project(&db, "project-1", "owner-1").await;
        let created = NovelAutopilotRepository::create_or_get_active(
            &db,
            create_input("project-1", "owner-1"),
        )
        .await
        .expect("create run");

        let claimed = NovelAutopilotRepository::prepare_and_claim_step(
            &db,
            PrepareAndClaimNovelAutopilotStep {
                attempt: CreateNovelAutopilotStepAttempt {
                    run_id: created.run.id.clone(),
                    user_id: "owner-1".to_string(),
                    step_key: "planning:foundation".to_string(),
                    step_type: NovelAutopilotStepType::Foundation,
                    phase: NovelAutopilotPhase::Foundation,
                    chapter_id: None,
                    chapter_number: None,
                    run_epoch: created.run.epoch,
                    input_digest: "digest-1".to_string(),
                },
                expected_run_version: created.run.version,
                background_task_id: Some("background-1".to_string()),
            },
        )
        .await
        .expect("atomically claim step");

        assert_eq!(claimed.step.status, "running");
        assert_eq!(
            claimed.run.current_step.as_deref(),
            Some("planning:foundation")
        );
        assert_eq!(
            claimed.run.active_background_task_id.as_deref(),
            Some("background-1")
        );
        assert_eq!(
            NovelAutopilotRepository::complete_step(
                &db,
                &claimed.step.id,
                "owner-1",
                claimed.run.version,
                claimed.run.epoch,
                "planning:foundation",
                Some("background-other"),
                NovelAutopilotStepStatus::Completed,
                NovelAutopilotStepTerminalPatch::default(),
            )
            .await
            .unwrap_err(),
            NovelAutopilotRepositoryError::InvalidTransition
        );

        assert_eq!(
            NovelAutopilotRepository::complete_step(
                &db,
                &claimed.step.id,
                "owner-1",
                claimed.run.version,
                claimed.run.epoch,
                "planning:other",
                Some("background-1"),
                NovelAutopilotStepStatus::Completed,
                NovelAutopilotStepTerminalPatch::default(),
            )
            .await
            .unwrap_err(),
            NovelAutopilotRepositoryError::InvalidTransition
        );

        assert_eq!(
            NovelAutopilotRepository::prepare_and_claim_step(
                &db,
                PrepareAndClaimNovelAutopilotStep {
                    attempt: CreateNovelAutopilotStepAttempt {
                        run_id: created.run.id.clone(),
                        user_id: "owner-1".to_string(),
                        step_key: "planning:foundation".to_string(),
                        step_type: NovelAutopilotStepType::Foundation,
                        phase: NovelAutopilotPhase::Foundation,
                        chapter_id: None,
                        chapter_number: None,
                        run_epoch: created.run.epoch,
                        input_digest: "digest-2".to_string(),
                    },
                    expected_run_version: claimed.run.version,
                    background_task_id: Some("background-2".to_string()),
                },
            )
            .await
            .unwrap_err(),
            NovelAutopilotRepositoryError::InvalidTransition
        );

        let completed = NovelAutopilotRepository::complete_step(
            &db,
            &claimed.step.id,
            "owner-1",
            claimed.run.version,
            claimed.run.epoch,
            "planning:foundation",
            Some("background-1"),
            NovelAutopilotStepStatus::Completed,
            NovelAutopilotStepTerminalPatch::default(),
        )
        .await
        .expect("complete claimed step");
        assert!(completed.run.active_background_task_id.is_none());
        assert!(completed.run.current_step.is_none());
    }

    async fn claim_foundation_step(
        db: &DatabaseConnection,
        project_id: &str,
        user_id: &str,
        task_id: &str,
    ) -> super::super::repository::ClaimedNovelAutopilotStep {
        let created =
            NovelAutopilotRepository::create_or_get_active(db, create_input(project_id, user_id))
                .await
                .expect("create foundation run");
        let running = NovelAutopilotRepository::transition_owned(
            db,
            &created.run.id,
            user_id,
            created.run.version,
            NovelAutopilotRunStatus::Running,
        )
        .await
        .expect("start foundation run");
        NovelAutopilotRepository::prepare_and_claim_step(
            db,
            PrepareAndClaimNovelAutopilotStep {
                attempt: CreateNovelAutopilotStepAttempt {
                    run_id: running.id.clone(),
                    user_id: user_id.to_string(),
                    step_key: "planning:foundation".to_string(),
                    step_type: NovelAutopilotStepType::Foundation,
                    phase: NovelAutopilotPhase::Foundation,
                    chapter_id: None,
                    chapter_number: None,
                    run_epoch: running.epoch,
                    input_digest: "foundation-input-digest".to_string(),
                },
                expected_run_version: running.version,
                background_task_id: Some(task_id.to_string()),
            },
        )
        .await
        .expect("claim foundation step")
    }

    fn generated_foundation_commit() -> NovelAutopilotFoundationCommit {
        NovelAutopilotFoundationCommit {
            title: "苍穹残卷".to_string(),
            description: "群岛文明在失落天幕下争夺旧时代遗产。".to_string(),
            theme: "秩序与自由的代价".to_string(),
            genre: "奇幻冒险".to_string(),
            narrative_perspective: "第三人称限知".to_string(),
            result_digest: "foundation-result-digest".to_string(),
        }
    }

    async fn claim_world_step(
        db: &DatabaseConnection,
        project_id: &str,
        user_id: &str,
        task_id: &str,
    ) -> super::super::repository::ClaimedNovelAutopilotStep {
        let created =
            NovelAutopilotRepository::create_or_get_active(db, create_input(project_id, user_id))
                .await
                .expect("create world run");
        let running = NovelAutopilotRepository::transition_owned(
            db,
            &created.run.id,
            user_id,
            created.run.version,
            NovelAutopilotRunStatus::Running,
        )
        .await
        .expect("start world run");
        NovelAutopilotRepository::prepare_and_claim_step(
            db,
            PrepareAndClaimNovelAutopilotStep {
                attempt: CreateNovelAutopilotStepAttempt {
                    run_id: running.id.clone(),
                    user_id: user_id.to_string(),
                    step_key: "planning:world_building".to_string(),
                    step_type: NovelAutopilotStepType::WorldBuilding,
                    phase: NovelAutopilotPhase::WorldBuilding,
                    chapter_id: None,
                    chapter_number: None,
                    run_epoch: running.epoch,
                    input_digest: "world-input-digest".to_string(),
                },
                expected_run_version: running.version,
                background_task_id: Some(task_id.to_string()),
            },
        )
        .await
        .expect("claim world step")
    }

    async fn claim_career_step(
        db: &DatabaseConnection,
        project_id: &str,
        user_id: &str,
        task_id: &str,
    ) -> super::super::repository::ClaimedNovelAutopilotStep {
        let created =
            NovelAutopilotRepository::create_or_get_active(db, create_input(project_id, user_id))
                .await
                .expect("create career run");
        let running = NovelAutopilotRepository::transition_owned(
            db,
            &created.run.id,
            user_id,
            created.run.version,
            NovelAutopilotRunStatus::Running,
        )
        .await
        .expect("start career run");
        NovelAutopilotRepository::prepare_and_claim_step(
            db,
            PrepareAndClaimNovelAutopilotStep {
                attempt: CreateNovelAutopilotStepAttempt {
                    run_id: running.id.clone(),
                    user_id: user_id.to_string(),
                    step_key: "planning:career_design".to_string(),
                    step_type: NovelAutopilotStepType::CareerDesign,
                    phase: NovelAutopilotPhase::CareerDesign,
                    chapter_id: None,
                    chapter_number: None,
                    run_epoch: running.epoch,
                    input_digest: "career-input-digest".to_string(),
                },
                expected_run_version: running.version,
                background_task_id: Some(task_id.to_string()),
            },
        )
        .await
        .expect("claim career step")
    }

    async fn claim_organization_step(
        db: &DatabaseConnection,
        project_id: &str,
        user_id: &str,
        task_id: &str,
    ) -> super::super::repository::ClaimedNovelAutopilotStep {
        let created =
            NovelAutopilotRepository::create_or_get_active(db, create_input(project_id, user_id))
                .await
                .expect("create organization run");
        let running = NovelAutopilotRepository::transition_owned(
            db,
            &created.run.id,
            user_id,
            created.run.version,
            NovelAutopilotRunStatus::Running,
        )
        .await
        .expect("start organization run");
        NovelAutopilotRepository::prepare_and_claim_step(
            db,
            PrepareAndClaimNovelAutopilotStep {
                attempt: CreateNovelAutopilotStepAttempt {
                    run_id: running.id.clone(),
                    user_id: user_id.to_string(),
                    step_key: "planning:organization_design".to_string(),
                    step_type: NovelAutopilotStepType::OrganizationDesign,
                    phase: NovelAutopilotPhase::OrganizationDesign,
                    chapter_id: None,
                    chapter_number: None,
                    run_epoch: running.epoch,
                    input_digest: "organization-input-digest".to_string(),
                },
                expected_run_version: running.version,
                background_task_id: Some(task_id.to_string()),
            },
        )
        .await
        .expect("claim organization step")
    }

    fn generated_organization_commit(
        member_character_id: Option<&str>,
        target_organization_character_id: Option<&str>,
    ) -> NovelAutopilotOrganizationCommit {
        NovelAutopilotOrganizationCommit {
            name: "浮港议会".to_string(),
            organization_type: "政治".to_string(),
            personality: Some("谨慎而务实".to_string()),
            background: Some("由港口商会联合成立".to_string()),
            appearance: Some("悬浮港口的圆形议事厅".to_string()),
            organization_purpose: Some("维持航路秩序".to_string()),
            traits: r#"["公开议事","优先贸易"]"#.to_string(),
            power_level: 70,
            location: Some("浮港".to_string()),
            motto: Some("让航路保持开放".to_string()),
            color: Some("青铜".to_string()),
            members: member_character_id
                .map(|character_id| {
                    vec![NovelAutopilotOrganizationMemberCommit {
                        character_id: character_id.to_string(),
                        position: "议员".to_string(),
                        rank: 2,
                        status: "active".to_string(),
                        joined_at: Some("2026-07-19".to_string()),
                        loyalty: 80,
                    }]
                })
                .unwrap_or_default(),
            relationships: target_organization_character_id
                .map(|character_id| {
                    vec![NovelAutopilotOrganizationRelationshipCommit {
                        target_organization_character_id: character_id.to_string(),
                        relationship_name: Some("联盟".to_string()),
                        description: Some("共同维护航路".to_string()),
                    }]
                })
                .unwrap_or_default(),
            result_digest: "organization-result-digest".to_string(),
        }
    }

    fn generated_career_commit() -> NovelAutopilotCareerCommit {
        NovelAutopilotCareerCommit {
            careers: vec![
                NovelAutopilotCareerItemCommit {
                    name: "巡界师".to_string(),
                    career_type: "main".to_string(),
                    description: Some("维护浮空航路".to_string()),
                    category: Some("战斗".to_string()),
                    stages: r#"[{"name":"见习"}]"#.to_string(),
                    max_stage: 10,
                    requirements: Some("感知稳定".to_string()),
                    special_abilities: Some("锚定航路".to_string()),
                    worldview_rules: Some("记忆燃料会反噬".to_string()),
                    attribute_bonuses: Some(r#"{"perception":2}"#.to_string()),
                },
                NovelAutopilotCareerItemCommit {
                    name: "记忆修补匠".to_string(),
                    career_type: "sub".to_string(),
                    description: Some("修复受损记忆".to_string()),
                    category: Some("辅助".to_string()),
                    stages: r#"[{"name":"学徒"}]"#.to_string(),
                    max_stage: 5,
                    requirements: None,
                    special_abilities: None,
                    worldview_rules: None,
                    attribute_bonuses: None,
                },
            ],
            result_digest: "career-result-digest".to_string(),
        }
    }

    fn generated_world_commit() -> NovelAutopilotWorldCommit {
        NovelAutopilotWorldCommit {
            time_period: "蒸汽纪元".to_string(),
            location: "浮空群岛".to_string(),
            atmosphere: "工业与秘术并存".to_string(),
            rules: "记忆可以作为燃料".to_string(),
            result_digest: "world-result-digest".to_string(),
        }
    }

    #[tokio::test]
    async fn world_commit_atomically_updates_project_and_completes_step() {
        let db = setup_repository_db().await;
        insert_project(&db, "project-world", "owner-1").await;
        let claimed = claim_world_step(&db, "project-world", "owner-1", "world-task").await;
        let project_before = project::Entity::find_by_id("project-world")
            .one(&db)
            .await
            .expect("load project before world commit")
            .expect("project exists");
        let expected_world = NovelAutopilotWorldSnapshot::from_project(&project_before);
        assert!(expected_world.is_blank());

        let committed = NovelAutopilotRepository::commit_world_building_step(
            &db,
            &claimed.step.id,
            "owner-1",
            claimed.run.version,
            claimed.run.epoch,
            "planning:world_building",
            Some("world-task"),
            &expected_world,
            generated_world_commit(),
        )
        .await
        .expect("commit generated world");

        let project_after = project::Entity::find_by_id("project-world")
            .one(&db)
            .await
            .expect("load project after world commit")
            .expect("project exists");
        assert_eq!(project_after.world_time_period.as_deref(), Some("蒸汽纪元"));
        assert_eq!(project_after.world_location.as_deref(), Some("浮空群岛"));
        assert_eq!(
            project_after.world_atmosphere.as_deref(),
            Some("工业与秘术并存")
        );
        assert_eq!(
            project_after.world_rules.as_deref(),
            Some("记忆可以作为燃料")
        );
        assert_eq!(committed.run.version, claimed.run.version + 1);
        assert!(committed.run.current_step.is_none());
        assert!(committed.run.active_background_task_id.is_none());
        assert_eq!(
            committed.step.status,
            NovelAutopilotStepStatus::Completed.as_str()
        );
        assert_eq!(
            committed.step.result_digest.as_deref(),
            Some("world-result-digest")
        );
    }

    #[tokio::test]
    async fn world_commit_does_not_overwrite_user_edits_made_during_generation() {
        let db = setup_repository_db().await;
        insert_project(&db, "project-world-race", "owner-1").await;
        let claimed =
            claim_world_step(&db, "project-world-race", "owner-1", "world-task-race").await;
        let project_before = project::Entity::find_by_id("project-world-race")
            .one(&db)
            .await
            .expect("load initial project")
            .expect("project exists");
        let expected_world = NovelAutopilotWorldSnapshot::from_project(&project_before);

        let mut manual_edit = project_before.into_active_model();
        manual_edit.world_time_period = Set(Some("用户手工纪元".to_string()));
        manual_edit
            .update(&db)
            .await
            .expect("save manual world edit");

        assert_eq!(
            NovelAutopilotRepository::commit_world_building_step(
                &db,
                &claimed.step.id,
                "owner-1",
                claimed.run.version,
                claimed.run.epoch,
                "planning:world_building",
                Some("world-task-race"),
                &expected_world,
                generated_world_commit(),
            )
            .await
            .unwrap_err(),
            NovelAutopilotRepositoryError::BusinessDataChanged
        );

        let project_after = project::Entity::find_by_id("project-world-race")
            .one(&db)
            .await
            .expect("reload project")
            .expect("project exists");
        assert_eq!(
            project_after.world_time_period.as_deref(),
            Some("用户手工纪元")
        );
        assert!(project_after.world_location.is_none());
        let run_after = NovelAutopilotRepository::find_owned(&db, &claimed.run.id, "owner-1")
            .await
            .expect("reload run after conflict");
        assert_eq!(
            run_after.current_step.as_deref(),
            Some("planning:world_building")
        );
        let step_after = novel_autopilot_step_run::Entity::find_by_id(&claimed.step.id)
            .one(&db)
            .await
            .expect("reload step after conflict")
            .expect("step exists");
        assert_eq!(
            step_after.status,
            NovelAutopilotStepStatus::Running.as_str()
        );
    }

    #[tokio::test]
    async fn paused_world_step_rejects_late_business_commit_without_project_write() {
        let db = setup_repository_db().await;
        insert_project(&db, "project-world-paused", "owner-1").await;
        let claimed =
            claim_world_step(&db, "project-world-paused", "owner-1", "world-task-paused").await;
        let project_before = project::Entity::find_by_id("project-world-paused")
            .one(&db)
            .await
            .expect("load initial project")
            .expect("project exists");
        let expected_world = NovelAutopilotWorldSnapshot::from_project(&project_before);
        let paused = NovelAutopilotRepository::transition_owned(
            &db,
            &claimed.run.id,
            "owner-1",
            claimed.run.version,
            NovelAutopilotRunStatus::Paused,
        )
        .await
        .expect("pause world run");

        assert_eq!(
            NovelAutopilotRepository::commit_world_building_step(
                &db,
                &claimed.step.id,
                "owner-1",
                paused.version,
                claimed.run.epoch,
                "planning:world_building",
                Some("world-task-paused"),
                &expected_world,
                generated_world_commit(),
            )
            .await
            .unwrap_err(),
            NovelAutopilotRepositoryError::StaleEpoch
        );
        let project_after = project::Entity::find_by_id("project-world-paused")
            .one(&db)
            .await
            .expect("reload project")
            .expect("project exists");
        assert!(NovelAutopilotWorldSnapshot::from_project(&project_after).is_blank());
    }

    #[tokio::test]
    async fn startup_recovery_fences_running_step_and_resets_run_for_scheduling() {
        let db = setup_repository_db().await;
        insert_project(&db, "project-1", "owner-1").await;
        let created = NovelAutopilotRepository::create_or_get_active(
            &db,
            create_input("project-1", "owner-1"),
        )
        .await
        .expect("create run");
        let running = NovelAutopilotRepository::transition_owned(
            &db,
            &created.run.id,
            "owner-1",
            created.run.version,
            NovelAutopilotRunStatus::Running,
        )
        .await
        .expect("start run");
        let claimed = NovelAutopilotRepository::prepare_and_claim_step(
            &db,
            PrepareAndClaimNovelAutopilotStep {
                attempt: CreateNovelAutopilotStepAttempt {
                    run_id: running.id.clone(),
                    user_id: "owner-1".to_string(),
                    step_key: "planning:foundation".to_string(),
                    step_type: NovelAutopilotStepType::Foundation,
                    phase: NovelAutopilotPhase::Foundation,
                    chapter_id: None,
                    chapter_number: None,
                    run_epoch: running.epoch,
                    input_digest: "digest-restart".to_string(),
                },
                expected_run_version: running.version,
                background_task_id: Some("orphan-task".to_string()),
            },
        )
        .await
        .expect("claim running step");

        let recoverable = NovelAutopilotRepository::list_startup_recoverable(&db)
            .await
            .expect("list recoverable runs");
        assert_eq!(recoverable.len(), 1);
        assert_eq!(recoverable[0].id, claimed.run.id);

        let recovered = NovelAutopilotRepository::prepare_startup_recovery(
            &db,
            &claimed.run.id,
            claimed.run.version,
            claimed.run.epoch,
        )
        .await
        .expect("prepare startup recovery");
        assert_eq!(recovered.status, "queued");
        assert_eq!(recovered.epoch, claimed.run.epoch + 1);
        assert_eq!(recovered.version, claimed.run.version + 1);
        assert!(recovered.current_step.is_none());
        assert!(recovered.active_background_task_id.is_none());

        let step = novel_autopilot_step_run::Entity::find_by_id(&claimed.step.id)
            .one(&db)
            .await
            .expect("load stale step")
            .expect("stale step exists");
        assert_eq!(step.status, "stale");
        assert_eq!(step.error_code.as_deref(), Some("service_restarted"));

        assert_eq!(
            NovelAutopilotRepository::prepare_startup_recovery(
                &db,
                &claimed.run.id,
                claimed.run.version,
                claimed.run.epoch,
            )
            .await
            .unwrap_err(),
            NovelAutopilotRepositoryError::StaleVersion
        );
    }

    #[tokio::test]
    async fn terminal_run_releases_project_scope_for_a_new_run() {
        let db = setup_repository_db().await;
        insert_project(&db, "project-1", "owner-1").await;
        let first = NovelAutopilotRepository::create_or_get_active(
            &db,
            create_input("project-1", "owner-1"),
        )
        .await
        .expect("create first run");
        let cancelled = NovelAutopilotRepository::transition_owned(
            &db,
            &first.run.id,
            "owner-1",
            first.run.version,
            NovelAutopilotRunStatus::Cancelled,
        )
        .await
        .expect("cancel queued run");
        assert!(cancelled.active_scope_key.is_none());

        let second = NovelAutopilotRepository::create_or_get_active(
            &db,
            create_input("project-1", "owner-1"),
        )
        .await
        .expect("create replacement run");
        assert!(second.created);
        assert_ne!(first.run.id, second.run.id);
    }

    #[tokio::test]
    async fn career_commit_inserts_all_items_and_completes_step_atomically() {
        let db = setup_repository_db().await;
        insert_project(&db, "project-career", "owner-1").await;
        let claimed = claim_career_step(&db, "project-career", "owner-1", "career-task").await;
        let expected_careers = NovelAutopilotCareerSnapshot::load(&db, "project-career")
            .await
            .expect("load career snapshot");
        assert!(expected_careers.is_blank());

        let committed = NovelAutopilotRepository::commit_career_design_step(
            &db,
            &claimed.step.id,
            "owner-1",
            claimed.run.version,
            claimed.run.epoch,
            "planning:career_design",
            Some("career-task"),
            &expected_careers,
            generated_career_commit(),
        )
        .await
        .expect("commit generated careers");

        let careers = career::Entity::find()
            .filter(career::Column::ProjectId.eq("project-career"))
            .all(&db)
            .await
            .expect("load committed careers");
        assert_eq!(careers.len(), 2);
        assert!(careers.iter().all(|career| career.source == "ai"));
        assert_eq!(committed.run.version, claimed.run.version + 1);
        assert!(committed.run.current_step.is_none());
        assert!(committed.run.active_background_task_id.is_none());
        assert_eq!(
            committed.step.status,
            NovelAutopilotStepStatus::Completed.as_str()
        );
        assert_eq!(
            committed.step.result_digest.as_deref(),
            Some("career-result-digest")
        );
    }

    #[tokio::test]
    async fn career_commit_rejects_manual_career_added_during_generation() {
        let db = setup_repository_db().await;
        insert_project(&db, "project-career-race", "owner-1").await;
        let claimed =
            claim_career_step(&db, "project-career-race", "owner-1", "career-task-race").await;
        let expected_careers = NovelAutopilotCareerSnapshot::load(&db, "project-career-race")
            .await
            .expect("load initial career snapshot");

        let now = NaiveDate::from_ymd_opt(2026, 7, 19)
            .expect("valid date")
            .and_hms_opt(9, 0, 0)
            .expect("valid time");
        career::ActiveModel {
            id: Set("manual-career".to_string()),
            project_id: Set("project-career-race".to_string()),
            name: Set("用户自建职业".to_string()),
            career_type: Set("main".to_string()),
            description: Set(None),
            category: Set(None),
            stages: Set("[]".to_string()),
            max_stage: Set(10),
            requirements: Set(None),
            special_abilities: Set(None),
            worldview_rules: Set(None),
            attribute_bonuses: Set(None),
            source: Set("manual".to_string()),
            created_at: Set(now),
            updated_at: Set(Some(now)),
        }
        .insert(&db)
        .await
        .expect("insert manual career");

        assert_eq!(
            NovelAutopilotRepository::commit_career_design_step(
                &db,
                &claimed.step.id,
                "owner-1",
                claimed.run.version,
                claimed.run.epoch,
                "planning:career_design",
                Some("career-task-race"),
                &expected_careers,
                generated_career_commit(),
            )
            .await
            .unwrap_err(),
            NovelAutopilotRepositoryError::BusinessDataChanged
        );

        let careers = career::Entity::find()
            .filter(career::Column::ProjectId.eq("project-career-race"))
            .all(&db)
            .await
            .expect("load careers after conflict");
        assert_eq!(careers.len(), 1);
        assert_eq!(careers[0].id, "manual-career");
        let run_after = NovelAutopilotRepository::find_owned(&db, &claimed.run.id, "owner-1")
            .await
            .expect("reload run after career conflict");
        assert_eq!(
            run_after.current_step.as_deref(),
            Some("planning:career_design")
        );
        let step_after = novel_autopilot_step_run::Entity::find_by_id(&claimed.step.id)
            .one(&db)
            .await
            .expect("reload career step")
            .expect("career step exists");
        assert_eq!(
            step_after.status,
            NovelAutopilotStepStatus::Running.as_str()
        );
    }

    #[tokio::test]
    async fn organization_commit_inserts_typed_business_rows_and_completes_step_atomically() {
        let db = setup_repository_db().await;
        insert_project(&db, "project-organization", "owner-1").await;
        insert_character(
            &db,
            "project-organization",
            "member-character",
            "岚舟",
            false,
        )
        .await;
        insert_character(
            &db,
            "project-organization",
            "target-organization-character",
            "旧港同盟",
            true,
        )
        .await;
        let claimed =
            claim_organization_step(&db, "project-organization", "owner-1", "organization-task")
                .await;
        let expected = NovelAutopilotOrganizationSnapshot::load(&db, "project-organization")
            .await
            .expect("load organization snapshot");

        let committed = NovelAutopilotRepository::commit_organization_design_step(
            &db,
            &claimed.step.id,
            "owner-1",
            claimed.run.version,
            claimed.run.epoch,
            "planning:organization_design",
            Some("organization-task"),
            &expected,
            generated_organization_commit(
                Some("member-character"),
                Some("target-organization-character"),
            ),
        )
        .await
        .expect("commit generated organization");

        let generated_character = character::Entity::find()
            .filter(character::Column::ProjectId.eq("project-organization"))
            .filter(character::Column::Name.eq("浮港议会"))
            .one(&db)
            .await
            .expect("load generated organization character")
            .expect("generated organization character exists");
        assert!(generated_character.is_organization);
        assert_eq!(
            generated_character.organization_type.as_deref(),
            Some("政治")
        );
        assert_eq!(
            generated_character.traits.as_deref(),
            Some(r#"["公开议事","优先贸易"]"#)
        );

        let generated_organization = organization::Entity::find()
            .filter(organization::Column::CharacterId.eq(&generated_character.id))
            .one(&db)
            .await
            .expect("load generated organization")
            .expect("generated organization exists");
        assert_eq!(generated_organization.member_count, 1);
        assert_eq!(generated_organization.power_level, 70);

        let members = organization_member::Entity::find()
            .filter(organization_member::Column::OrganizationId.eq(&generated_organization.id))
            .all(&db)
            .await
            .expect("load generated organization members");
        assert_eq!(members.len(), 1);
        assert_eq!(members[0].character_id, "member-character");
        assert_eq!(members[0].source, "novel_autopilot");

        let relationships = relationship::Entity::find()
            .filter(relationship::Column::CharacterFromId.eq(&generated_character.id))
            .all(&db)
            .await
            .expect("load generated organization relationships");
        assert_eq!(relationships.len(), 1);
        assert_eq!(
            relationships[0].character_to_id,
            "target-organization-character"
        );
        assert_eq!(relationships[0].relationship_name.as_deref(), Some("联盟"));

        assert_eq!(committed.run.version, claimed.run.version + 1);
        assert!(committed.run.current_step.is_none());
        assert!(committed.run.active_background_task_id.is_none());
        assert_eq!(
            committed.step.status,
            NovelAutopilotStepStatus::Completed.as_str()
        );
        assert_eq!(
            committed.step.result_digest.as_deref(),
            Some("organization-result-digest")
        );
    }

    #[tokio::test]
    async fn organization_commit_rejects_human_organization_added_during_generation() {
        let db = setup_repository_db().await;
        insert_project(&db, "project-organization-race", "owner-1").await;
        let claimed = claim_organization_step(
            &db,
            "project-organization-race",
            "owner-1",
            "organization-task-race",
        )
        .await;
        let expected = NovelAutopilotOrganizationSnapshot::load(&db, "project-organization-race")
            .await
            .expect("load initial organization snapshot");
        assert!(expected.is_blank());

        insert_character(
            &db,
            "project-organization-race",
            "manual-organization-character",
            "人工组织",
            true,
        )
        .await;

        assert_eq!(
            NovelAutopilotRepository::commit_organization_design_step(
                &db,
                &claimed.step.id,
                "owner-1",
                claimed.run.version,
                claimed.run.epoch,
                "planning:organization_design",
                Some("organization-task-race"),
                &expected,
                generated_organization_commit(None, None),
            )
            .await
            .unwrap_err(),
            NovelAutopilotRepositoryError::BusinessDataChanged
        );

        let organization_count = organization::Entity::find()
            .filter(organization::Column::ProjectId.eq("project-organization-race"))
            .count(&db)
            .await
            .expect("count organizations after conflict");
        assert_eq!(organization_count, 0);
        let generated_character_count = character::Entity::find()
            .filter(character::Column::ProjectId.eq("project-organization-race"))
            .filter(character::Column::Name.eq("浮港议会"))
            .count(&db)
            .await
            .expect("count generated characters after conflict");
        assert_eq!(generated_character_count, 0);
        assert_eq!(
            organization_member::Entity::find()
                .count(&db)
                .await
                .expect("count members after conflict"),
            0
        );
        assert_eq!(
            relationship::Entity::find()
                .count(&db)
                .await
                .expect("count relationships after conflict"),
            0
        );

        let run_after = NovelAutopilotRepository::find_owned(&db, &claimed.run.id, "owner-1")
            .await
            .expect("reload run after organization conflict");
        assert_eq!(
            run_after.current_step.as_deref(),
            Some("planning:organization_design")
        );
        let step_after = novel_autopilot_step_run::Entity::find_by_id(&claimed.step.id)
            .one(&db)
            .await
            .expect("reload organization step")
            .expect("organization step exists");
        assert_eq!(
            step_after.status,
            NovelAutopilotStepStatus::Running.as_str()
        );
    }

    #[tokio::test]
    async fn foundation_commit_completes_partial_project_fields_and_terminals_atomically() {
        let db = setup_repository_db().await;
        insert_project(&db, "project-foundation", "owner-1").await;
        let claimed =
            claim_foundation_step(&db, "project-foundation", "owner-1", "foundation-task").await;
        let project_before = project::Entity::find_by_id("project-foundation")
            .one(&db)
            .await
            .expect("load partial foundation project")
            .expect("partial foundation project exists");
        let expected = NovelAutopilotFoundationSnapshot::from_project(&project_before);
        assert!(!expected.is_complete());

        let committed = NovelAutopilotRepository::commit_foundation_step(
            &db,
            &claimed.step.id,
            "owner-1",
            claimed.run.version,
            claimed.run.epoch,
            "planning:foundation",
            Some("foundation-task"),
            &expected,
            generated_foundation_commit(),
        )
        .await
        .expect("commit generated foundation");

        let project_after = project::Entity::find_by_id("project-foundation")
            .one(&db)
            .await
            .expect("load committed foundation project")
            .expect("committed foundation project exists");
        assert_eq!(project_after.title, "苍穹残卷");
        assert_eq!(
            project_after.description.as_deref(),
            Some("群岛文明在失落天幕下争夺旧时代遗产。")
        );
        assert_eq!(project_after.theme.as_deref(), Some("秩序与自由的代价"));
        assert_eq!(project_after.genre.as_deref(), Some("奇幻冒险"));
        assert_eq!(
            project_after.narrative_perspective.as_deref(),
            Some("第三人称限知")
        );
        assert_eq!(committed.run.version, claimed.run.version + 1);
        assert!(committed.run.current_step.is_none());
        assert!(committed.run.active_background_task_id.is_none());
        assert_eq!(
            committed.step.status,
            NovelAutopilotStepStatus::Completed.as_str()
        );
        assert_eq!(
            committed.step.result_digest.as_deref(),
            Some("foundation-result-digest")
        );
    }

    #[tokio::test]
    async fn foundation_commit_rejects_human_change_without_overwriting_project_fields() {
        let db = setup_repository_db().await;
        insert_project(&db, "project-foundation-race", "owner-1").await;
        let claimed = claim_foundation_step(
            &db,
            "project-foundation-race",
            "owner-1",
            "foundation-task-race",
        )
        .await;
        let project_before = project::Entity::find_by_id("project-foundation-race")
            .one(&db)
            .await
            .expect("load initial foundation project")
            .expect("initial foundation project exists");
        let expected = NovelAutopilotFoundationSnapshot::from_project(&project_before);

        let mut human_project = project_before.into_active_model();
        human_project.theme = Set(Some("人工主题".to_string()));
        human_project
            .update(&db)
            .await
            .expect("apply concurrent human foundation change");

        assert_eq!(
            NovelAutopilotRepository::commit_foundation_step(
                &db,
                &claimed.step.id,
                "owner-1",
                claimed.run.version,
                claimed.run.epoch,
                "planning:foundation",
                Some("foundation-task-race"),
                &expected,
                generated_foundation_commit(),
            )
            .await
            .unwrap_err(),
            NovelAutopilotRepositoryError::BusinessDataChanged
        );

        let project_after = project::Entity::find_by_id("project-foundation-race")
            .one(&db)
            .await
            .expect("reload raced foundation project")
            .expect("raced foundation project exists");
        assert_eq!(project_after.theme.as_deref(), Some("人工主题"));
        assert_eq!(project_after.title, "Autopilot project-foundation-race");
        assert!(project_after.description.is_none());
        assert!(project_after.genre.is_none());
        assert!(project_after.narrative_perspective.is_none());

        let run_after = NovelAutopilotRepository::find_owned(&db, &claimed.run.id, "owner-1")
            .await
            .expect("reload foundation run after business race");
        assert_eq!(run_after.version, claimed.run.version);
        assert_eq!(
            run_after.current_step.as_deref(),
            Some("planning:foundation")
        );
        assert_eq!(
            run_after.active_background_task_id.as_deref(),
            Some("foundation-task-race")
        );
        let step_after = novel_autopilot_step_run::Entity::find_by_id(&claimed.step.id)
            .one(&db)
            .await
            .expect("reload foundation step after business race")
            .expect("foundation step exists");
        assert_eq!(
            step_after.status,
            NovelAutopilotStepStatus::Running.as_str()
        );
    }

    #[tokio::test]
    async fn foundation_commit_rejects_stale_version_epoch_and_background_task_fences() {
        let db = setup_repository_db().await;
        insert_project(&db, "project-foundation-fence", "owner-1").await;
        let claimed = claim_foundation_step(
            &db,
            "project-foundation-fence",
            "owner-1",
            "foundation-task-fence",
        )
        .await;
        let project_before = project::Entity::find_by_id("project-foundation-fence")
            .one(&db)
            .await
            .expect("load foundation fence project")
            .expect("foundation fence project exists");
        let expected = NovelAutopilotFoundationSnapshot::from_project(&project_before);

        assert_eq!(
            NovelAutopilotRepository::commit_foundation_step(
                &db,
                &claimed.step.id,
                "owner-1",
                claimed.run.version + 1,
                claimed.run.epoch,
                "planning:foundation",
                Some("foundation-task-fence"),
                &expected,
                generated_foundation_commit(),
            )
            .await
            .unwrap_err(),
            NovelAutopilotRepositoryError::StaleVersion
        );
        assert_eq!(
            NovelAutopilotRepository::commit_foundation_step(
                &db,
                &claimed.step.id,
                "owner-1",
                claimed.run.version,
                claimed.run.epoch + 1,
                "planning:foundation",
                Some("foundation-task-fence"),
                &expected,
                generated_foundation_commit(),
            )
            .await
            .unwrap_err(),
            NovelAutopilotRepositoryError::StaleEpoch
        );
        assert_eq!(
            NovelAutopilotRepository::commit_foundation_step(
                &db,
                &claimed.step.id,
                "owner-1",
                claimed.run.version,
                claimed.run.epoch,
                "planning:foundation",
                Some("late-foundation-task"),
                &expected,
                generated_foundation_commit(),
            )
            .await
            .unwrap_err(),
            NovelAutopilotRepositoryError::InvalidTransition
        );

        let project_after = project::Entity::find_by_id("project-foundation-fence")
            .one(&db)
            .await
            .expect("reload foundation fence project")
            .expect("foundation fence project exists");
        assert_eq!(project_after, project_before);
        let run_after = NovelAutopilotRepository::find_owned(&db, &claimed.run.id, "owner-1")
            .await
            .expect("reload foundation run after fence rejections");
        assert_eq!(run_after.version, claimed.run.version);
        assert_eq!(run_after.epoch, claimed.run.epoch);
        assert_eq!(
            run_after.current_step.as_deref(),
            Some("planning:foundation")
        );
        assert_eq!(
            run_after.active_background_task_id.as_deref(),
            Some("foundation-task-fence")
        );
        let step_after = novel_autopilot_step_run::Entity::find_by_id(&claimed.step.id)
            .one(&db)
            .await
            .expect("reload foundation step after fence rejections")
            .expect("foundation step exists");
        assert_eq!(
            step_after.status,
            NovelAutopilotStepStatus::Running.as_str()
        );
    }
    #[tokio::test]
    async fn budget_wait_is_task_epoch_and_version_fenced() {
        let db = setup_repository_db().await;
        insert_project(&db, "project-budget-wait", "owner-1").await;
        let created = NovelAutopilotRepository::create_or_get_active(
            &db,
            create_input("project-budget-wait", "owner-1"),
        )
        .await
        .expect("create budget run");
        let running = NovelAutopilotRepository::transition_owned(
            &db,
            &created.run.id,
            "owner-1",
            created.run.version,
            NovelAutopilotRunStatus::Running,
        )
        .await
        .expect("start budget run");
        let bound = NovelAutopilotRepository::set_active_background_task_owned(
            &db,
            &running.id,
            "owner-1",
            running.version,
            running.epoch,
            Some("budget-task"),
        )
        .await
        .expect("bind budget task");

        let waiting = NovelAutopilotRepository::wait_for_budget_owned(
            &db,
            &bound.id,
            "owner-1",
            bound.version,
            bound.epoch,
            Some("budget-task"),
            "novel_autopilot_budget_tokens_exhausted",
        )
        .await
        .expect("enter budget human gate");
        assert_eq!(
            waiting.status,
            NovelAutopilotRunStatus::WaitingHuman.as_str()
        );
        assert_eq!(
            waiting.last_error_code.as_deref(),
            Some("novel_autopilot_budget_tokens_exhausted")
        );
        assert!(waiting.active_background_task_id.is_none());
        assert_eq!(waiting.version, bound.version + 1);
        assert_eq!(
            NovelAutopilotRepository::wait_for_budget_owned(
                &db,
                &bound.id,
                "owner-1",
                bound.version,
                bound.epoch,
                Some("budget-task"),
                "novel_autopilot_budget_tokens_exhausted",
            )
            .await
            .unwrap_err(),
            NovelAutopilotRepositoryError::StaleVersion
        );
    }

    #[tokio::test]
    async fn budget_wait_supports_null_active_task_fence() {
        let db = setup_repository_db().await;
        insert_project(&db, "project-budget-wait-null", "owner-1").await;
        let created = NovelAutopilotRepository::create_or_get_active(
            &db,
            create_input("project-budget-wait-null", "owner-1"),
        )
        .await
        .expect("create null-fence budget run");
        let running = NovelAutopilotRepository::transition_owned(
            &db,
            &created.run.id,
            "owner-1",
            created.run.version,
            NovelAutopilotRunStatus::Running,
        )
        .await
        .expect("start null-fence budget run");

        let waiting = NovelAutopilotRepository::wait_for_budget_owned(
            &db,
            &running.id,
            "owner-1",
            running.version,
            running.epoch,
            None,
            "novel_autopilot_budget_tokens_exhausted",
        )
        .await
        .expect("enter budget human gate without an active task");

        assert_eq!(
            waiting.status,
            NovelAutopilotRunStatus::WaitingHuman.as_str()
        );
        assert!(waiting.active_background_task_id.is_none());
        assert_eq!(waiting.version, running.version + 1);
    }

    #[tokio::test]
    async fn estimated_usage_supports_bound_active_task_fence() {
        let db = setup_repository_db().await;
        insert_project(&db, "project-budget-usage-bound", "owner-1").await;
        let created = NovelAutopilotRepository::create_or_get_active(
            &db,
            create_input("project-budget-usage-bound", "owner-1"),
        )
        .await
        .expect("create bound-fence usage run");
        let running = NovelAutopilotRepository::transition_owned(
            &db,
            &created.run.id,
            "owner-1",
            created.run.version,
            NovelAutopilotRunStatus::Running,
        )
        .await
        .expect("start bound-fence usage run");
        let bound = NovelAutopilotRepository::set_active_background_task_owned(
            &db,
            &running.id,
            "owner-1",
            running.version,
            running.epoch,
            Some("usage-task"),
        )
        .await
        .expect("bind usage task");

        let updated = NovelAutopilotRepository::increment_estimated_usage_owned(
            &db,
            &bound.id,
            "owner-1",
            bound.version,
            bound.epoch,
            Some("usage-task"),
            123,
        )
        .await
        .expect("persist usage with a bound task fence");

        assert_eq!(updated.used_tokens, 123);
        assert_eq!(
            updated.active_background_task_id.as_deref(),
            Some("usage-task")
        );
        assert_eq!(updated.version, bound.version + 1);
    }

    #[tokio::test]
    async fn estimated_usage_accumulates_with_epoch_fence() {
        let db = setup_repository_db().await;
        insert_project(&db, "project-budget-usage", "owner-1").await;
        let created = NovelAutopilotRepository::create_or_get_active(
            &db,
            create_input("project-budget-usage", "owner-1"),
        )
        .await
        .expect("create usage run");
        let running = NovelAutopilotRepository::transition_owned(
            &db,
            &created.run.id,
            "owner-1",
            created.run.version,
            NovelAutopilotRunStatus::Running,
        )
        .await
        .expect("start usage run");
        let updated = NovelAutopilotRepository::increment_estimated_usage_owned(
            &db,
            &running.id,
            "owner-1",
            running.version,
            running.epoch,
            None,
            321,
        )
        .await
        .expect("persist estimated usage");
        assert_eq!(updated.used_tokens, 321);
        assert_eq!(updated.version, running.version + 1);

        let paused = NovelAutopilotRepository::transition_owned(
            &db,
            &updated.id,
            "owner-1",
            updated.version,
            NovelAutopilotRunStatus::Paused,
        )
        .await
        .expect("pause usage run");
        assert_eq!(
            NovelAutopilotRepository::increment_estimated_usage_owned(
                &db,
                &paused.id,
                "owner-1",
                paused.version,
                running.epoch,
                None,
                1,
            )
            .await
            .unwrap_err(),
            NovelAutopilotRepositoryError::StaleEpoch
        );
        let reloaded = NovelAutopilotRepository::find_owned(&db, &paused.id, "owner-1")
            .await
            .expect("reload usage run");
        assert_eq!(reloaded.used_tokens, 321);
    }

    #[tokio::test]
    async fn latest_step_attempt_supports_claim_time_budget_preflight() {
        let db = setup_repository_db().await;
        insert_project(&db, "project-budget-attempt", "owner-1").await;
        let created = NovelAutopilotRepository::create_or_get_active(
            &db,
            create_input("project-budget-attempt", "owner-1"),
        )
        .await
        .expect("create attempt run");
        let running = NovelAutopilotRepository::transition_owned(
            &db,
            &created.run.id,
            "owner-1",
            created.run.version,
            NovelAutopilotRunStatus::Running,
        )
        .await
        .expect("start attempt run");
        NovelAutopilotRepository::prepare_and_claim_step(
            &db,
            PrepareAndClaimNovelAutopilotStep {
                attempt: CreateNovelAutopilotStepAttempt {
                    run_id: running.id.clone(),
                    user_id: "owner-1".to_string(),
                    step_key: "planning:foundation".to_string(),
                    step_type: NovelAutopilotStepType::Foundation,
                    phase: NovelAutopilotPhase::Foundation,
                    chapter_id: None,
                    chapter_number: None,
                    run_epoch: running.epoch,
                    input_digest: "budget-attempt-input".to_string(),
                },
                expected_run_version: running.version,
                background_task_id: Some("budget-attempt-task".to_string()),
            },
        )
        .await
        .expect("claim first attempt");

        assert_eq!(
            NovelAutopilotRepository::latest_step_attempt_owned(
                &db,
                &running.id,
                "owner-1",
                "planning:foundation",
            )
            .await
            .expect("load latest attempt"),
            Some(1)
        );
    }
}
