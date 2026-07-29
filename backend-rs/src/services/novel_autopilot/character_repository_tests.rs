use chrono::NaiveDate;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, Database, DatabaseConnection, DbBackend,
    EntityTrait, PaginatorTrait, QueryFilter, Schema, Set, Statement,
};
use serde_json::{json, Value};

use crate::models::{
    career, character, novel_autopilot_run, novel_autopilot_step_run, organization,
    organization_member, project, relationship,
};

use super::{
    character_repository::{
        NovelAutopilotCharacterCareerAssignmentCommit, NovelAutopilotCharacterCommit,
        NovelAutopilotCharacterItemCommit, NovelAutopilotCharacterOrganizationCommit,
        NovelAutopilotCharacterOrganizationMembershipCommit,
        NovelAutopilotCharacterRelationshipCommit, NovelAutopilotCharacterSnapshot,
        NovelAutopilotCharacterSubCareerCommit,
    },
    repository::{
        ClaimedNovelAutopilotStep, CreateNovelAutopilotRun, CreateNovelAutopilotStepAttempt,
        NovelAutopilotRepository, NovelAutopilotRepositoryError, PrepareAndClaimNovelAutopilotStep,
    },
    types::{
        NovelAutopilotPhase, NovelAutopilotRunConfig, NovelAutopilotRunStatus,
        NovelAutopilotStepStatus, NovelAutopilotStepType,
    },
};

const PROJECT_ID: &str = "project-character-design";
const USER_ID: &str = "owner-character-design";
const TASK_ID: &str = "task-character-design";
const STEP_KEY: &str = "planning:character_design";

async fn setup_repository_db() -> DatabaseConnection {
    let db = Database::connect("sqlite::memory:")
        .await
        .expect("connect character repository SQLite memory database");
    let builder = DbBackend::Sqlite;
    let schema = Schema::new(builder);

    for statement in [
        builder.build(&schema.create_table_from_entity(project::Entity)),
        builder.build(&schema.create_table_from_entity(career::Entity)),
        builder.build(&schema.create_table_from_entity(character::Entity)),
        builder.build(&schema.create_table_from_entity(organization::Entity)),
        builder.build(&schema.create_table_from_entity(organization_member::Entity)),
        builder.build(&schema.create_table_from_entity(relationship::Entity)),
        builder.build(&schema.create_table_from_entity(novel_autopilot_run::Entity)),
        builder.build(&schema.create_table_from_entity(novel_autopilot_step_run::Entity)),
    ] {
        db.execute(statement)
            .await
            .expect("create character repository test table");
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
        .expect("valid test date")
        .and_hms_opt(8, 0, 0)
        .expect("valid test time")
}

async fn insert_project(db: &DatabaseConnection) {
    let now = test_time();
    project::ActiveModel {
        id: Set(PROJECT_ID.to_string()),
        user_id: Set(USER_ID.to_string()),
        title: Set("Character design test project".to_string()),
        target_words: Set(100_000),
        current_words: Set(0),
        status: Set("foundation".to_string()),
        wizard_status: Set("completed".to_string()),
        wizard_step: Set(0),
        outline_mode: Set("linear".to_string()),
        character_count: Set(0),
        created_at: Set(now),
        updated_at: Set(Some(now)),
        ..Default::default()
    }
    .insert(db)
    .await
    .expect("insert test project");
}

async fn insert_career(
    db: &DatabaseConnection,
    id: &str,
    name: &str,
    career_type: &str,
    max_stage: i32,
) {
    let now = test_time();
    career::ActiveModel {
        id: Set(id.to_string()),
        project_id: Set(PROJECT_ID.to_string()),
        name: Set(name.to_string()),
        career_type: Set(career_type.to_string()),
        description: Set(None),
        category: Set(None),
        stages: Set("[]".to_string()),
        max_stage: Set(max_stage),
        requirements: Set(None),
        special_abilities: Set(None),
        worldview_rules: Set(None),
        attribute_bonuses: Set(None),
        source: Set("test".to_string()),
        created_at: Set(now),
        updated_at: Set(Some(now)),
    }
    .insert(db)
    .await
    .expect("insert career catalog row");
}

async fn insert_manual_character(db: &DatabaseConnection, name: &str) {
    let now = test_time();
    character::ActiveModel {
        id: Set(format!("manual-{name}")),
        project_id: Set(PROJECT_ID.to_string()),
        name: Set(name.to_string()),
        age: Set(None),
        gender: Set(None),
        is_organization: Set(false),
        role_type: Set(Some("supporting".to_string())),
        personality: Set(None),
        background: Set(None),
        appearance: Set(None),
        relationships: Set(None),
        organization_type: Set(None),
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
    .expect("insert manual character change");
}

fn create_input() -> CreateNovelAutopilotRun {
    CreateNovelAutopilotRun {
        project_id: PROJECT_ID.to_string(),
        user_id: USER_ID.to_string(),
        total_chapters: 10,
        config: NovelAutopilotRunConfig::default(),
    }
}

async fn claim_character_step(db: &DatabaseConnection) -> ClaimedNovelAutopilotStep {
    let created = NovelAutopilotRepository::create_or_get_active(db, create_input())
        .await
        .expect("create character-design run");
    let running = NovelAutopilotRepository::transition_owned(
        db,
        &created.run.id,
        USER_ID,
        created.run.version,
        NovelAutopilotRunStatus::Running,
    )
    .await
    .expect("start character-design run");

    NovelAutopilotRepository::prepare_and_claim_step(
        db,
        PrepareAndClaimNovelAutopilotStep {
            attempt: CreateNovelAutopilotStepAttempt {
                run_id: running.id.clone(),
                user_id: USER_ID.to_string(),
                step_key: STEP_KEY.to_string(),
                step_type: NovelAutopilotStepType::CharacterDesign,
                phase: NovelAutopilotPhase::CharacterDesign,
                chapter_id: None,
                chapter_number: None,
                run_epoch: running.epoch,
                input_digest: "character-design-input".to_string(),
            },
            expected_run_version: running.version,
            background_task_id: Some(TASK_ID.to_string()),
        },
    )
    .await
    .expect("claim character-design step")
}

fn character_commit() -> NovelAutopilotCharacterCommit {
    NovelAutopilotCharacterCommit {
        characters: vec![
            NovelAutopilotCharacterItemCommit {
                name: "沈砚".to_string(),
                age: "28".to_string(),
                gender: "男".to_string(),
                role_type: "protagonist".to_string(),
                personality: "冷静而坚韧".to_string(),
                background: "流亡的书院学者".to_string(),
                appearance: "青衣长剑".to_string(),
                traits: r#"["冷静","坚韧"]"#.to_string(),
            },
            NovelAutopilotCharacterItemCommit {
                name: "陆昭".to_string(),
                age: "26".to_string(),
                gender: "女".to_string(),
                role_type: "supporting".to_string(),
                personality: "敏锐果断".to_string(),
                background: "城防司密探".to_string(),
                appearance: "黑衣短刃".to_string(),
                traits: r#"["敏锐","果断"]"#.to_string(),
            },
        ],
        organizations: vec![NovelAutopilotCharacterOrganizationCommit {
            name: "玄灯会".to_string(),
            role_type: "supporting".to_string(),
            personality: "纪律严整".to_string(),
            background: "守护旧城秘闻的隐秘组织".to_string(),
            appearance: "暗金灯徽".to_string(),
            organization_type: "秘密组织".to_string(),
            organization_purpose: "守护秘闻并平衡各方势力".to_string(),
            member_names: vec!["沈砚".to_string(), "陆昭".to_string()],
            power_level: 88,
            location: "旧城地宫".to_string(),
            motto: "灯火不熄".to_string(),
            color: "暗金".to_string(),
            traits: r#"["隐秘","纪律"]"#.to_string(),
        }],
        career_assignments: vec![NovelAutopilotCharacterCareerAssignmentCommit {
            character_name: "沈砚".to_string(),
            main_career: "剑修".to_string(),
            main_stage: 3,
            sub_careers: vec![NovelAutopilotCharacterSubCareerCommit {
                career: "阵师".to_string(),
                stage: 2,
            }],
        }],
        relationships: vec![NovelAutopilotCharacterRelationshipCommit {
            source_character_name: "沈砚".to_string(),
            target_character_name: "陆昭".to_string(),
            relationship_type: "盟友".to_string(),
            intimacy_level: 42,
            description: "共同逃离书院后建立信任".to_string(),
            started_at: Some("第一章前".to_string()),
        }],
        organization_memberships: vec![NovelAutopilotCharacterOrganizationMembershipCommit {
            character_name: "陆昭".to_string(),
            organization_name: "玄灯会".to_string(),
            position: "巡查使".to_string(),
            rank: 4,
            loyalty: 83,
        }],
        result_digest: "character-design-digest-v1".to_string(),
    }
}

async fn seed_careers(db: &DatabaseConnection) {
    insert_career(db, "career-main-sword", "剑修", "main", 5).await;
    insert_career(db, "career-sub-array", "阵师", "sub", 4).await;
}

async fn assert_terminal_state(db: &DatabaseConnection, claimed: &ClaimedNovelAutopilotStep) {
    let run = novel_autopilot_run::Entity::find_by_id(&claimed.run.id)
        .one(db)
        .await
        .expect("load terminal run")
        .expect("run persists");
    let step = novel_autopilot_step_run::Entity::find_by_id(&claimed.step.id)
        .one(db)
        .await
        .expect("load terminal step")
        .expect("step persists");

    assert_eq!(run.version, claimed.run.version + 1);
    assert_eq!(run.current_step, None);
    assert_eq!(run.active_background_task_id, None);
    assert_eq!(step.status, NovelAutopilotStepStatus::Completed.as_str());
    assert_eq!(
        step.result_digest.as_deref(),
        Some("character-design-digest-v1")
    );
}

async fn assert_still_claimed(db: &DatabaseConnection, claimed: &ClaimedNovelAutopilotStep) {
    let run = novel_autopilot_run::Entity::find_by_id(&claimed.run.id)
        .one(db)
        .await
        .expect("load claimed run")
        .expect("run persists");
    let step = novel_autopilot_step_run::Entity::find_by_id(&claimed.step.id)
        .one(db)
        .await
        .expect("load claimed step")
        .expect("step persists");

    assert_eq!(run.version, claimed.run.version);
    assert_eq!(run.current_step.as_deref(), Some(STEP_KEY));
    assert_eq!(run.active_background_task_id.as_deref(), Some(TASK_ID));
    assert_eq!(step.status, NovelAutopilotStepStatus::Running.as_str());
    assert_eq!(step.background_task_id.as_deref(), Some(TASK_ID));
}

async fn assert_no_generated_dependents(db: &DatabaseConnection) {
    assert_eq!(
        organization::Entity::find()
            .filter(organization::Column::ProjectId.eq(PROJECT_ID))
            .count(db)
            .await
            .expect("count organizations"),
        0
    );
    assert_eq!(
        relationship::Entity::find()
            .filter(relationship::Column::ProjectId.eq(PROJECT_ID))
            .count(db)
            .await
            .expect("count relationships"),
        0
    );
    assert_eq!(
        organization_member::Entity::find()
            .count(db)
            .await
            .expect("count organization members"),
        0
    );
}

#[tokio::test]
async fn character_design_commit_persists_all_tables_atomically() {
    let db = setup_repository_db().await;
    insert_project(&db).await;
    seed_careers(&db).await;
    let claimed = claim_character_step(&db).await;
    let snapshot = NovelAutopilotCharacterSnapshot::load(&db, PROJECT_ID)
        .await
        .expect("load blank character snapshot");
    assert!(snapshot.is_blank());

    let terminal = NovelAutopilotRepository::commit_character_design_step(
        &db,
        &claimed.step.id,
        USER_ID,
        claimed.run.version,
        claimed.run.epoch,
        STEP_KEY,
        Some(TASK_ID),
        &snapshot,
        character_commit(),
    )
    .await
    .expect("commit character design");

    let protagonist = character::Entity::find()
        .filter(character::Column::ProjectId.eq(PROJECT_ID))
        .filter(character::Column::Name.eq("沈砚"))
        .one(&db)
        .await
        .expect("load protagonist")
        .expect("protagonist is persisted");
    let supporting = character::Entity::find()
        .filter(character::Column::ProjectId.eq(PROJECT_ID))
        .filter(character::Column::Name.eq("陆昭"))
        .one(&db)
        .await
        .expect("load supporting character")
        .expect("supporting character is persisted");
    let organization_character = character::Entity::find()
        .filter(character::Column::ProjectId.eq(PROJECT_ID))
        .filter(character::Column::Name.eq("玄灯会"))
        .one(&db)
        .await
        .expect("load organization character")
        .expect("organization character is persisted");

    assert_eq!(
        protagonist.main_career_id.as_deref(),
        Some("career-main-sword")
    );
    assert_eq!(protagonist.main_career_stage, Some(3));
    assert_eq!(
        serde_json::from_str::<Value>(
            protagonist
                .sub_careers
                .as_deref()
                .expect("sub careers are persisted"),
        )
        .expect("parse stored sub careers"),
        json!([{"career_id": "career-sub-array", "stage": 2}])
    );
    assert!(organization_character.is_organization);

    let persisted_organization = organization::Entity::find()
        .filter(organization::Column::ProjectId.eq(PROJECT_ID))
        .one(&db)
        .await
        .expect("load persisted organization")
        .expect("organization is persisted");
    let persisted_relationship = relationship::Entity::find()
        .filter(relationship::Column::ProjectId.eq(PROJECT_ID))
        .one(&db)
        .await
        .expect("load persisted relationship")
        .expect("relationship is persisted");
    let persisted_membership = organization_member::Entity::find()
        .one(&db)
        .await
        .expect("load persisted organization membership")
        .expect("membership is persisted");

    assert_eq!(
        persisted_organization.character_id,
        organization_character.id
    );
    assert_eq!(persisted_relationship.character_from_id, protagonist.id);
    assert_eq!(persisted_relationship.character_to_id, supporting.id);
    assert_eq!(
        persisted_membership.organization_id,
        persisted_organization.id
    );
    assert_eq!(persisted_membership.character_id, supporting.id);
    assert_eq!(terminal.run.version, claimed.run.version + 1);
    assert_eq!(
        terminal.step.status,
        NovelAutopilotStepStatus::Completed.as_str()
    );
    assert_terminal_state(&db, &claimed).await;
}

#[tokio::test]
async fn character_design_commit_rejects_business_data_changes_without_terminalizing() {
    let db = setup_repository_db().await;
    insert_project(&db).await;
    seed_careers(&db).await;
    let claimed = claim_character_step(&db).await;
    let snapshot = NovelAutopilotCharacterSnapshot::load(&db, PROJECT_ID)
        .await
        .expect("load expected character snapshot");
    insert_manual_character(&db, "人工角色").await;

    let error = NovelAutopilotRepository::commit_character_design_step(
        &db,
        &claimed.step.id,
        USER_ID,
        claimed.run.version,
        claimed.run.epoch,
        STEP_KEY,
        Some(TASK_ID),
        &snapshot,
        character_commit(),
    )
    .await
    .expect_err("human character mutation must reject stale generation");

    assert_eq!(error, NovelAutopilotRepositoryError::BusinessDataChanged);
    assert_eq!(
        character::Entity::find()
            .filter(character::Column::ProjectId.eq(PROJECT_ID))
            .count(&db)
            .await
            .expect("count characters after rejected commit"),
        1
    );
    assert_no_generated_dependents(&db).await;
    assert_still_claimed(&db, &claimed).await;
}

#[tokio::test]
async fn character_design_commit_rejects_stale_fencing_without_writing_business_data() {
    let db = setup_repository_db().await;
    insert_project(&db).await;
    seed_careers(&db).await;
    let claimed = claim_character_step(&db).await;
    let snapshot = NovelAutopilotCharacterSnapshot::load(&db, PROJECT_ID)
        .await
        .expect("load expected character snapshot");

    let stale_version = NovelAutopilotRepository::commit_character_design_step(
        &db,
        &claimed.step.id,
        USER_ID,
        claimed.run.version + 1,
        claimed.run.epoch,
        STEP_KEY,
        Some(TASK_ID),
        &snapshot,
        character_commit(),
    )
    .await
    .expect_err("stale run version must be rejected");
    assert_eq!(stale_version, NovelAutopilotRepositoryError::StaleVersion);

    let stale_epoch = NovelAutopilotRepository::commit_character_design_step(
        &db,
        &claimed.step.id,
        USER_ID,
        claimed.run.version,
        claimed.run.epoch + 1,
        STEP_KEY,
        Some(TASK_ID),
        &snapshot,
        character_commit(),
    )
    .await
    .expect_err("stale run epoch must be rejected");
    assert_eq!(stale_epoch, NovelAutopilotRepositoryError::StaleEpoch);

    let invalid_task = NovelAutopilotRepository::commit_character_design_step(
        &db,
        &claimed.step.id,
        USER_ID,
        claimed.run.version,
        claimed.run.epoch,
        STEP_KEY,
        Some("different-task"),
        &snapshot,
        character_commit(),
    )
    .await
    .expect_err("different background task must be rejected");
    assert_eq!(
        invalid_task,
        NovelAutopilotRepositoryError::InvalidTransition
    );

    assert_eq!(
        character::Entity::find()
            .filter(character::Column::ProjectId.eq(PROJECT_ID))
            .count(&db)
            .await
            .expect("count characters after fencing failures"),
        0
    );
    assert_no_generated_dependents(&db).await;
    assert_still_claimed(&db, &claimed).await;
}

#[tokio::test]
async fn character_design_commit_rolls_back_when_career_assignment_is_invalid() {
    let db = setup_repository_db().await;
    insert_project(&db).await;
    seed_careers(&db).await;
    let claimed = claim_character_step(&db).await;
    let snapshot = NovelAutopilotCharacterSnapshot::load(&db, PROJECT_ID)
        .await
        .expect("load expected character snapshot");
    let mut invalid_commit = character_commit();
    invalid_commit.career_assignments[0].main_stage = 6;

    let error = NovelAutopilotRepository::commit_character_design_step(
        &db,
        &claimed.step.id,
        USER_ID,
        claimed.run.version,
        claimed.run.epoch,
        STEP_KEY,
        Some(TASK_ID),
        &snapshot,
        invalid_commit,
    )
    .await
    .expect_err("career stage above catalog maximum must fail");

    assert_eq!(
        error,
        NovelAutopilotRepositoryError::InvalidConfig {
            field: "career_assignments",
            code: "invalid_main_career",
        }
    );
    assert_eq!(
        character::Entity::find()
            .filter(character::Column::ProjectId.eq(PROJECT_ID))
            .count(&db)
            .await
            .expect("count characters after rolled back commit"),
        0
    );
    assert_no_generated_dependents(&db).await;
    assert_still_claimed(&db, &claimed).await;
}
