use std::{fmt, str::FromStr};

use chrono::Utc;
use sea_orm::{ColumnTrait, ConnectionTrait, DatabaseConnection, EntityTrait, QueryFilter, Set};
use serde::{de::Error as _, Deserialize, Deserializer, Serialize};

use crate::models::project;

pub const NOVEL_WORKFLOW_SCHEMA_VERSION: u32 = 1;
pub const NOVEL_WORKFLOW_SOURCE: &str = "projects.status";
pub const NOVEL_WORKFLOW_REASON_MAX_CHARS: usize = 500;
pub const NOVEL_WORKFLOW_RELATED_TASK_ID_MAX_CHARS: usize = 128;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum NovelWorkflowPhase {
    Inspiration,
    Foundation,
    WorldBuilding,
    CharacterDesign,
    Outline,
    Writing,
    Reviewing,
    Polishing,
    Completed,
}

impl NovelWorkflowPhase {
    #[cfg(test)]
    pub const ALL: [Self; 9] = [
        Self::Inspiration,
        Self::Foundation,
        Self::WorldBuilding,
        Self::CharacterDesign,
        Self::Outline,
        Self::Writing,
        Self::Reviewing,
        Self::Polishing,
        Self::Completed,
    ];

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Inspiration => "inspiration",
            Self::Foundation => "foundation",
            Self::WorldBuilding => "world_building",
            Self::CharacterDesign => "character_design",
            Self::Outline => "outline",
            Self::Writing => "writing",
            Self::Reviewing => "reviewing",
            Self::Polishing => "polishing",
            Self::Completed => "completed",
        }
    }

    pub const fn default_new_project_phase() -> Self {
        Self::Foundation
    }

    pub const fn allowed_transitions(self) -> &'static [Self] {
        match self {
            Self::Inspiration => &[Self::Foundation],
            Self::Foundation => &[Self::Inspiration, Self::WorldBuilding, Self::Writing],
            Self::WorldBuilding => &[Self::Foundation, Self::CharacterDesign],
            Self::CharacterDesign => &[Self::WorldBuilding, Self::Outline],
            Self::Outline => &[Self::CharacterDesign, Self::Writing],
            Self::Writing => &[Self::Outline, Self::Reviewing, Self::Completed],
            Self::Reviewing => &[Self::Writing, Self::Polishing, Self::Completed],
            Self::Polishing => &[Self::Reviewing, Self::Completed],
            Self::Completed => &[Self::Reviewing, Self::Polishing],
        }
    }

    pub fn can_transition_to(self, target: Self) -> bool {
        self == target || self.allowed_transitions().contains(&target)
    }

    pub const fn suggested_next_phase(self) -> Option<Self> {
        match self {
            Self::Inspiration => Some(Self::Foundation),
            Self::Foundation => Some(Self::WorldBuilding),
            Self::WorldBuilding => Some(Self::CharacterDesign),
            Self::CharacterDesign => Some(Self::Outline),
            Self::Outline => Some(Self::Writing),
            Self::Writing => Some(Self::Reviewing),
            Self::Reviewing => Some(Self::Polishing),
            Self::Polishing => Some(Self::Completed),
            Self::Completed => None,
        }
    }

    pub fn can_rollback(self) -> bool {
        self.allowed_transitions()
            .iter()
            .any(|target| target.rank() < self.rank())
    }

    pub fn requires_reason_for(self, target: Self) -> bool {
        target.rank() < self.rank()
    }

    pub(crate) const fn persisted_values(self) -> &'static [&'static str] {
        match self {
            Self::Inspiration => &["inspiration"],
            Self::Foundation => &["foundation", "planning", "draft"],
            Self::WorldBuilding => &["world_building"],
            Self::CharacterDesign => &["character_design"],
            Self::Outline => &["outline"],
            Self::Writing => &["writing", "active"],
            Self::Reviewing => &["reviewing", "revising"],
            Self::Polishing => &["polishing"],
            Self::Completed => &["completed"],
        }
    }

    const fn rank(self) -> u8 {
        match self {
            Self::Inspiration => 0,
            Self::Foundation => 1,
            Self::WorldBuilding => 2,
            Self::CharacterDesign => 3,
            Self::Outline => 4,
            Self::Writing => 5,
            Self::Reviewing => 6,
            Self::Polishing => 7,
            Self::Completed => 8,
        }
    }
}

impl fmt::Display for NovelWorkflowPhase {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl FromStr for NovelWorkflowPhase {
    type Err = NovelWorkflowError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let normalized = value.trim().to_ascii_lowercase();
        match normalized.as_str() {
            "inspiration" => Ok(Self::Inspiration),
            "foundation" | "planning" | "draft" => Ok(Self::Foundation),
            "world_building" => Ok(Self::WorldBuilding),
            "character_design" => Ok(Self::CharacterDesign),
            "outline" => Ok(Self::Outline),
            "writing" | "active" => Ok(Self::Writing),
            "reviewing" | "revising" => Ok(Self::Reviewing),
            "polishing" => Ok(Self::Polishing),
            "completed" => Ok(Self::Completed),
            _ => Err(NovelWorkflowError::InvalidPhase {
                value: value.to_string(),
            }),
        }
    }
}

impl<'de> Deserialize<'de> for NovelWorkflowPhase {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Self::from_str(&value).map_err(D::Error::custom)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct NovelWorkflowStateView {
    pub schema_version: u32,
    pub project_id: String,
    pub phase: NovelWorkflowPhase,
    pub allowed_transitions: Vec<NovelWorkflowPhase>,
    pub can_rollback: bool,
    pub suggested_next_phase: Option<NovelWorkflowPhase>,
    pub updated_at: chrono::NaiveDateTime,
    pub source: &'static str,
}

impl NovelWorkflowStateView {
    pub fn new(
        project_id: String,
        phase: NovelWorkflowPhase,
        updated_at: chrono::NaiveDateTime,
    ) -> Self {
        Self {
            schema_version: NOVEL_WORKFLOW_SCHEMA_VERSION,
            project_id,
            phase,
            allowed_transitions: phase.allowed_transitions().to_vec(),
            can_rollback: phase.can_rollback(),
            suggested_next_phase: phase.suggested_next_phase(),
            updated_at,
            source: NOVEL_WORKFLOW_SOURCE,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct NovelWorkflowTransitionReceipt {
    pub schema_version: u32,
    pub changed: bool,
    pub previous_phase: NovelWorkflowPhase,
    pub state: NovelWorkflowStateView,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct NovelWorkflowAuditContext {
    pub reason: Option<String>,
    pub related_task_id: Option<String>,
}

impl NovelWorkflowAuditContext {
    pub fn sanitized(self) -> Self {
        Self {
            reason: sanitize_audit_field(self.reason, NOVEL_WORKFLOW_REASON_MAX_CHARS),
            related_task_id: sanitize_audit_field(
                self.related_task_id,
                NOVEL_WORKFLOW_RELATED_TASK_ID_MAX_CHARS,
            ),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NovelWorkflowError {
    InvalidPhase {
        value: String,
    },
    UnknownPersistedPhase {
        value: String,
    },
    IllegalTransition {
        from: NovelWorkflowPhase,
        to: NovelWorkflowPhase,
    },
    ReasonRequired {
        from: NovelWorkflowPhase,
        to: NovelWorkflowPhase,
    },
    StaleExpectedPhase {
        expected: NovelWorkflowPhase,
        actual: NovelWorkflowPhase,
    },
    NotFoundOrAccessDenied,
    Internal(String),
}

impl fmt::Display for NovelWorkflowError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidPhase { value } => write!(formatter, "invalid workflow phase: {value}"),
            Self::UnknownPersistedPhase { value } => {
                write!(formatter, "unknown persisted workflow phase: {value}")
            }
            Self::IllegalTransition { from, to } => {
                write!(formatter, "illegal workflow transition: {from} -> {to}")
            }
            Self::ReasonRequired { from, to } => {
                write!(
                    formatter,
                    "workflow transition reason is required: {from} -> {to}"
                )
            }
            Self::StaleExpectedPhase { expected, actual } => write!(
                formatter,
                "stale expected workflow phase: expected {expected}, actual {actual}"
            ),
            Self::NotFoundOrAccessDenied => {
                formatter.write_str("project not found or access denied")
            }
            Self::Internal(message) => formatter.write_str(message),
        }
    }
}

impl std::error::Error for NovelWorkflowError {}

pub fn parse_persisted_phase(value: &str) -> Result<NovelWorkflowPhase, NovelWorkflowError> {
    NovelWorkflowPhase::from_str(value).map_err(|_| NovelWorkflowError::UnknownPersistedPhase {
        value: value.to_string(),
    })
}

pub fn canonicalize_import_phase(
    value: Option<&str>,
) -> Result<NovelWorkflowPhase, NovelWorkflowError> {
    match value {
        Some(value) if !value.trim().is_empty() => NovelWorkflowPhase::from_str(value),
        _ => Ok(NovelWorkflowPhase::default_new_project_phase()),
    }
}

pub fn validate_public_transition(
    from: NovelWorkflowPhase,
    to: NovelWorkflowPhase,
    audit: &NovelWorkflowAuditContext,
) -> Result<(), NovelWorkflowError> {
    if !from.can_transition_to(to) {
        return Err(NovelWorkflowError::IllegalTransition { from, to });
    }

    if from != to && from.requires_reason_for(to) && audit.reason.is_none() {
        return Err(NovelWorkflowError::ReasonRequired { from, to });
    }

    Ok(())
}

pub fn resolve_public_transition(
    current_persisted: &str,
    requested_phase: &str,
    audit: NovelWorkflowAuditContext,
) -> Result<NovelWorkflowPhase, NovelWorkflowError> {
    let current = parse_persisted_phase(current_persisted)?;
    let target = NovelWorkflowPhase::from_str(requested_phase)?;
    validate_public_transition(current, target, &audit.sanitized())?;
    Ok(target)
}

pub fn resolve_internal_writing_transition(
    current_persisted: &str,
) -> Result<NovelWorkflowPhase, NovelWorkflowError> {
    let current = parse_persisted_phase(current_persisted)?;
    let target = NovelWorkflowPhase::Writing;
    validate_public_transition(current, target, &NovelWorkflowAuditContext::default())?;
    Ok(target)
}

pub fn resolve_internal_foundation_reset(
    current_persisted: &str,
) -> Result<NovelWorkflowPhase, NovelWorkflowError> {
    parse_persisted_phase(current_persisted)?;
    Ok(NovelWorkflowPhase::Foundation)
}

pub async fn get_state(
    db: &DatabaseConnection,
    project_id: &str,
    user_id: &str,
) -> Result<NovelWorkflowStateView, NovelWorkflowError> {
    let model = load_owned_project(db, project_id, user_id).await?;
    state_view_from_model(&model)
}

pub async fn transition(
    db: &DatabaseConnection,
    project_id: &str,
    user_id: &str,
    expected: NovelWorkflowPhase,
    target: NovelWorkflowPhase,
    audit: NovelWorkflowAuditContext,
) -> Result<NovelWorkflowTransitionReceipt, NovelWorkflowError> {
    transition_with_connection(db, project_id, user_id, expected, target, audit).await
}

pub(crate) async fn transition_with_connection<C>(
    db: &C,
    project_id: &str,
    user_id: &str,
    expected: NovelWorkflowPhase,
    target: NovelWorkflowPhase,
    audit: NovelWorkflowAuditContext,
) -> Result<NovelWorkflowTransitionReceipt, NovelWorkflowError>
where
    C: ConnectionTrait,
{
    let audit = audit.sanitized();
    let current_model = load_owned_project(db, project_id, user_id).await?;
    let current = parse_persisted_phase(&current_model.status)?;

    if current != expected {
        return Err(NovelWorkflowError::StaleExpectedPhase {
            expected,
            actual: current,
        });
    }

    validate_public_transition(current, target, &audit)?;

    if current == target {
        emit_transition_audit(project_id, user_id, current, target, &audit, false, "noop");
        return Ok(NovelWorkflowTransitionReceipt {
            schema_version: NOVEL_WORKFLOW_SCHEMA_VERSION,
            changed: false,
            previous_phase: current,
            state: state_view_from_model(&current_model)?,
        });
    }

    let now = Utc::now().naive_utc();
    let update = project::ActiveModel {
        status: Set(target.as_str().to_string()),
        updated_at: Set(Some(now)),
        ..Default::default()
    };

    let mut expected_persisted_values = expected
        .persisted_values()
        .iter()
        .map(|value| (*value).to_string())
        .collect::<Vec<_>>();
    if !expected_persisted_values.contains(&current_model.status) {
        expected_persisted_values.push(current_model.status.clone());
    }

    let result = project::Entity::update_many()
        .set(update)
        .filter(project::Column::Id.eq(project_id))
        .filter(project::Column::UserId.eq(user_id))
        .filter(project::Column::Status.is_in(expected_persisted_values))
        .exec(db)
        .await
        .map_err(|error| NovelWorkflowError::Internal(error.to_string()))?;

    if result.rows_affected == 0 {
        let actual_model = load_owned_project(db, project_id, user_id).await?;
        let actual = parse_persisted_phase(&actual_model.status)?;
        if actual != expected {
            return Err(NovelWorkflowError::StaleExpectedPhase { expected, actual });
        }
        return Err(NovelWorkflowError::Internal(
            "workflow conditional update affected no rows".to_string(),
        ));
    }

    emit_transition_audit(
        project_id, user_id, current, target, &audit, true, "changed",
    );
    Ok(NovelWorkflowTransitionReceipt {
        schema_version: NOVEL_WORKFLOW_SCHEMA_VERSION,
        changed: true,
        previous_phase: current,
        state: NovelWorkflowStateView::new(project_id.to_string(), target, now),
    })
}

async fn load_owned_project<C>(
    db: &C,
    project_id: &str,
    user_id: &str,
) -> Result<project::Model, NovelWorkflowError>
where
    C: ConnectionTrait,
{
    project::Entity::find()
        .filter(project::Column::Id.eq(project_id))
        .filter(project::Column::UserId.eq(user_id))
        .one(db)
        .await
        .map_err(|error| NovelWorkflowError::Internal(error.to_string()))?
        .ok_or(NovelWorkflowError::NotFoundOrAccessDenied)
}

fn state_view_from_model(
    model: &project::Model,
) -> Result<NovelWorkflowStateView, NovelWorkflowError> {
    let phase = parse_persisted_phase(&model.status)?;
    Ok(NovelWorkflowStateView::new(
        model.id.clone(),
        phase,
        model.updated_at.unwrap_or(model.created_at),
    ))
}

fn emit_transition_audit(
    project_id: &str,
    actor_user_id: &str,
    from_phase: NovelWorkflowPhase,
    to_phase: NovelWorkflowPhase,
    audit: &NovelWorkflowAuditContext,
    changed: bool,
    result: &'static str,
) {
    tracing::info!(
        event = "novel_workflow_phase_transition",
        schema_version = NOVEL_WORKFLOW_SCHEMA_VERSION,
        project_id = %project_id,
        actor_user_id = %actor_user_id,
        from_phase = %from_phase,
        to_phase = %to_phase,
        reason = audit.reason.as_deref(),
        related_task_id = audit.related_task_id.as_deref(),
        changed,
        result,
        "novel workflow phase transition"
    );
}
fn sanitize_audit_field(value: Option<String>, max_chars: usize) -> Option<String> {
    let sanitized = value?
        .chars()
        .filter(|character| !character.is_control())
        .take(max_chars)
        .collect::<String>();
    let trimmed = sanitized.trim();
    (!trimmed.is_empty()).then(|| trimmed.to_string())
}

#[cfg(test)]
mod tests {
    use sea_orm::{ActiveModelTrait, ConnectionTrait, Database, DbBackend, Schema};

    use super::*;

    async fn setup_workflow_db() -> DatabaseConnection {
        setup_workflow_db_for("sqlite::memory:", DbBackend::Sqlite).await
    }

    async fn setup_postgres_workflow_db() -> DatabaseConnection {
        let database_url = std::env::var("MUMU_R3_POSTGRES_URL")
            .expect("MUMU_R3_POSTGRES_URL must point to an isolated PostgreSQL database");
        setup_workflow_db_for(&database_url, DbBackend::Postgres).await
    }

    async fn setup_workflow_db_for(database_url: &str, backend: DbBackend) -> DatabaseConnection {
        let db = Database::connect(database_url)
            .await
            .expect("connect workflow test database");
        let schema = Schema::new(backend);
        db.execute(backend.build(&schema.create_table_from_entity(project::Entity)))
            .await
            .expect("create isolated workflow projects table");
        db
    }

    fn workflow_timestamp(hour: u32) -> chrono::NaiveDateTime {
        chrono::NaiveDate::from_ymd_opt(2026, 7, 14)
            .expect("valid workflow test date")
            .and_hms_opt(hour, 0, 0)
            .expect("valid workflow test time")
    }

    async fn insert_project(
        db: &DatabaseConnection,
        id: &str,
        user_id: &str,
        status: &str,
        updated_at: Option<chrono::NaiveDateTime>,
    ) -> project::Model {
        project::ActiveModel {
            id: Set(id.to_string()),
            user_id: Set(user_id.to_string()),
            title: Set(format!("Workflow {id}")),
            target_words: Set(100_000),
            current_words: Set(0),
            status: Set(status.to_string()),
            wizard_status: Set("completed".to_string()),
            wizard_step: Set(0),
            outline_mode: Set("linear".to_string()),
            character_count: Set(0),
            created_at: Set(workflow_timestamp(8)),
            updated_at: Set(updated_at),
            ..Default::default()
        }
        .insert(db)
        .await
        .expect("insert workflow project")
    }

    async fn load_project(db: &DatabaseConnection, id: &str) -> project::Model {
        project::Entity::find_by_id(id)
            .one(db)
            .await
            .expect("load workflow project")
            .expect("workflow project exists")
    }

    #[test]
    fn canonical_phases_round_trip_through_serde() {
        for phase in NovelWorkflowPhase::ALL {
            let serialized = serde_json::to_string(&phase).expect("serialize phase");
            assert_eq!(serialized, format!("\"{}\"", phase.as_str()));
            let deserialized: NovelWorkflowPhase =
                serde_json::from_str(&serialized).expect("deserialize phase");
            assert_eq!(deserialized, phase);
        }
    }

    #[test]
    fn legacy_aliases_normalize_to_canonical_phases() {
        let cases = [
            ("planning", NovelWorkflowPhase::Foundation),
            ("draft", NovelWorkflowPhase::Foundation),
            ("revising", NovelWorkflowPhase::Reviewing),
            ("active", NovelWorkflowPhase::Writing),
            ("  PLANNING  ", NovelWorkflowPhase::Foundation),
        ];

        for (raw, expected) in cases {
            assert_eq!(NovelWorkflowPhase::from_str(raw).unwrap(), expected);
        }
    }

    #[test]
    fn unknown_input_and_persisted_values_fail_explicitly() {
        assert!(matches!(
            NovelWorkflowPhase::from_str("mystery"),
            Err(NovelWorkflowError::InvalidPhase { value }) if value == "mystery"
        ));
        assert!(matches!(
            parse_persisted_phase("mystery"),
            Err(NovelWorkflowError::UnknownPersistedPhase { value }) if value == "mystery"
        ));
    }

    #[test]
    fn transition_matrix_is_exhaustive_and_same_phase_is_idempotent() {
        for from in NovelWorkflowPhase::ALL {
            for to in NovelWorkflowPhase::ALL {
                let expected = from == to || from.allowed_transitions().contains(&to);
                assert_eq!(
                    from.can_transition_to(to),
                    expected,
                    "unexpected transition result for {from} -> {to}"
                );
            }
            assert!(!from.allowed_transitions().contains(&from));
        }
    }

    #[test]
    fn allowed_transition_matrix_matches_the_public_contract() {
        use NovelWorkflowPhase::*;

        let cases: [(NovelWorkflowPhase, &[NovelWorkflowPhase]); 9] = [
            (Inspiration, &[Foundation]),
            (Foundation, &[Inspiration, WorldBuilding, Writing]),
            (WorldBuilding, &[Foundation, CharacterDesign]),
            (CharacterDesign, &[WorldBuilding, Outline]),
            (Outline, &[CharacterDesign, Writing]),
            (Writing, &[Outline, Reviewing, Completed]),
            (Reviewing, &[Writing, Polishing, Completed]),
            (Polishing, &[Reviewing, Completed]),
            (Completed, &[Reviewing, Polishing]),
        ];

        for (phase, expected) in cases {
            assert_eq!(phase.allowed_transitions(), expected);
        }
    }

    #[test]
    fn rollback_and_suggested_next_phase_follow_phase_order() {
        assert!(!NovelWorkflowPhase::Inspiration.can_rollback());
        for phase in NovelWorkflowPhase::ALL.into_iter().skip(1) {
            assert!(phase.can_rollback(), "{phase} should allow rollback");
        }

        assert_eq!(
            NovelWorkflowPhase::Writing.suggested_next_phase(),
            Some(NovelWorkflowPhase::Reviewing)
        );
        assert_eq!(NovelWorkflowPhase::Completed.suggested_next_phase(), None);
    }

    #[test]
    fn rollback_requires_a_non_empty_sanitized_reason() {
        let empty = NovelWorkflowAuditContext {
            reason: Some(" \n\t ".to_string()),
            related_task_id: None,
        }
        .sanitized();
        assert!(matches!(
            validate_public_transition(
                NovelWorkflowPhase::Writing,
                NovelWorkflowPhase::Outline,
                &empty
            ),
            Err(NovelWorkflowError::ReasonRequired { .. })
        ));

        let with_reason = NovelWorkflowAuditContext {
            reason: Some("  回退\n到大纲  ".to_string()),
            related_task_id: Some(" task\0-42 ".to_string()),
        }
        .sanitized();
        assert_eq!(with_reason.reason.as_deref(), Some("回退到大纲"));
        assert_eq!(with_reason.related_task_id.as_deref(), Some("task-42"));
        assert!(validate_public_transition(
            NovelWorkflowPhase::Writing,
            NovelWorkflowPhase::Outline,
            &with_reason
        )
        .is_ok());
    }

    #[test]
    fn public_transition_helper_normalizes_aliases_and_validates_rollbacks() {
        assert_eq!(
            resolve_public_transition(" Planning ", "ACTIVE", NovelWorkflowAuditContext::default())
                .unwrap(),
            NovelWorkflowPhase::Writing
        );

        assert!(matches!(
            resolve_public_transition("writing", "outline", NovelWorkflowAuditContext::default()),
            Err(NovelWorkflowError::ReasonRequired { .. })
        ));
        assert_eq!(
            resolve_public_transition(
                "writing",
                "outline",
                NovelWorkflowAuditContext {
                    reason: Some("调整故事结构".to_string()),
                    related_task_id: None,
                }
            )
            .unwrap(),
            NovelWorkflowPhase::Outline
        );
    }

    #[test]
    fn internal_wizard_transitions_are_bounded() {
        assert_eq!(
            resolve_internal_writing_transition("planning").unwrap(),
            NovelWorkflowPhase::Writing
        );
        assert!(matches!(
            resolve_internal_writing_transition("completed"),
            Err(NovelWorkflowError::IllegalTransition { .. })
        ));
        assert_eq!(
            resolve_internal_foundation_reset("completed").unwrap(),
            NovelWorkflowPhase::Foundation
        );
        assert!(matches!(
            resolve_internal_foundation_reset("unknown"),
            Err(NovelWorkflowError::UnknownPersistedPhase { .. })
        ));
    }

    #[test]
    fn import_phase_defaults_and_aliases_are_canonical() {
        assert_eq!(
            canonicalize_import_phase(None).unwrap(),
            NovelWorkflowPhase::Foundation
        );
        assert_eq!(
            canonicalize_import_phase(Some("draft")).unwrap(),
            NovelWorkflowPhase::Foundation
        );
        assert!(canonicalize_import_phase(Some("unknown")).is_err());
    }

    #[tokio::test]
    async fn get_state_returns_owned_project_and_hides_foreign_project() {
        let db = setup_workflow_db().await;
        let updated_at = workflow_timestamp(9);
        insert_project(
            &db,
            "project-owned",
            "owner-1",
            "foundation",
            Some(updated_at),
        )
        .await;

        let state = get_state(&db, "project-owned", "owner-1")
            .await
            .expect("owner reads workflow state");
        assert_eq!(state.project_id, "project-owned");
        assert_eq!(state.phase, NovelWorkflowPhase::Foundation);
        assert_eq!(state.updated_at, updated_at);

        assert_eq!(
            get_state(&db, "project-owned", "owner-2").await,
            Err(NovelWorkflowError::NotFoundOrAccessDenied)
        );
    }

    #[tokio::test]
    async fn legacy_case_and_whitespace_values_read_and_transition_canonically() {
        let db = setup_workflow_db().await;
        let cases = [
            ("legacy-planning", "planning"),
            ("uppercase-planning", "PLANNING"),
            ("padded-planning", "  Planning  "),
        ];

        for (project_id, persisted_status) in cases {
            insert_project(&db, project_id, "owner-1", persisted_status, None).await;

            let state = get_state(&db, project_id, "owner-1")
                .await
                .expect("legacy phase remains readable");
            assert_eq!(state.phase, NovelWorkflowPhase::Foundation);

            let receipt = transition(
                &db,
                project_id,
                "owner-1",
                NovelWorkflowPhase::Foundation,
                NovelWorkflowPhase::Writing,
                NovelWorkflowAuditContext::default(),
            )
            .await
            .expect("legacy phase participates in conditional transition");
            assert!(receipt.changed);
            assert_eq!(receipt.previous_phase, NovelWorkflowPhase::Foundation);
            assert_eq!(receipt.state.phase, NovelWorkflowPhase::Writing);
            assert_eq!(load_project(&db, project_id).await.status, "writing");
        }
    }

    #[tokio::test]
    async fn unknown_persisted_phase_fails_without_mutating_project() {
        let db = setup_workflow_db().await;
        let updated_at = workflow_timestamp(10);
        insert_project(&db, "unknown-phase", "owner-1", "mystery", Some(updated_at)).await;

        assert!(matches!(
            get_state(&db, "unknown-phase", "owner-1").await,
            Err(NovelWorkflowError::UnknownPersistedPhase { value }) if value == "mystery"
        ));
        assert!(matches!(
            transition(
                &db,
                "unknown-phase",
                "owner-1",
                NovelWorkflowPhase::Foundation,
                NovelWorkflowPhase::Writing,
                NovelWorkflowAuditContext::default(),
            )
            .await,
            Err(NovelWorkflowError::UnknownPersistedPhase { value }) if value == "mystery"
        ));

        let stored = load_project(&db, "unknown-phase").await;
        assert_eq!(stored.status, "mystery");
        assert_eq!(stored.updated_at, Some(updated_at));
    }

    #[tokio::test]
    async fn same_phase_transition_is_a_noop_and_preserves_updated_at() {
        let db = setup_workflow_db().await;
        let updated_at = workflow_timestamp(11);
        insert_project(&db, "same-phase", "owner-1", "writing", Some(updated_at)).await;

        let receipt = transition(
            &db,
            "same-phase",
            "owner-1",
            NovelWorkflowPhase::Writing,
            NovelWorkflowPhase::Writing,
            NovelWorkflowAuditContext::default(),
        )
        .await
        .expect("same phase transition succeeds idempotently");

        assert!(!receipt.changed);
        assert_eq!(receipt.previous_phase, NovelWorkflowPhase::Writing);
        assert_eq!(receipt.state.phase, NovelWorkflowPhase::Writing);
        assert_eq!(receipt.state.updated_at, updated_at);
        let stored = load_project(&db, "same-phase").await;
        assert_eq!(stored.status, "writing");
        assert_eq!(stored.updated_at, Some(updated_at));
    }

    #[tokio::test]
    async fn stale_expected_phase_preserves_current_state() {
        let db = setup_workflow_db().await;
        let updated_at = workflow_timestamp(12);
        insert_project(
            &db,
            "stale-expected",
            "owner-1",
            "writing",
            Some(updated_at),
        )
        .await;

        assert_eq!(
            transition(
                &db,
                "stale-expected",
                "owner-1",
                NovelWorkflowPhase::Foundation,
                NovelWorkflowPhase::WorldBuilding,
                NovelWorkflowAuditContext::default(),
            )
            .await,
            Err(NovelWorkflowError::StaleExpectedPhase {
                expected: NovelWorkflowPhase::Foundation,
                actual: NovelWorkflowPhase::Writing,
            })
        );

        let stored = load_project(&db, "stale-expected").await;
        assert_eq!(stored.status, "writing");
        assert_eq!(stored.updated_at, Some(updated_at));
    }

    async fn assert_concurrent_transitions_are_compare_and_swap(db: &DatabaseConnection) {
        insert_project(db, "concurrent-cas", "owner-1", "foundation", None).await;

        let left_db = db.clone();
        let right_db = db.clone();
        let (left, right) = tokio::join!(
            transition(
                &left_db,
                "concurrent-cas",
                "owner-1",
                NovelWorkflowPhase::Foundation,
                NovelWorkflowPhase::WorldBuilding,
                NovelWorkflowAuditContext::default(),
            ),
            transition(
                &right_db,
                "concurrent-cas",
                "owner-1",
                NovelWorkflowPhase::Foundation,
                NovelWorkflowPhase::Writing,
                NovelWorkflowAuditContext::default(),
            )
        );

        let stored = load_project(db, "concurrent-cas").await;
        let final_phase = parse_persisted_phase(&stored.status).expect("stored canonical phase");
        assert!(matches!(
            final_phase,
            NovelWorkflowPhase::WorldBuilding | NovelWorkflowPhase::Writing
        ));

        let results = [left, right];
        let changed_count = results
            .iter()
            .filter(|result| matches!(result, Ok(receipt) if receipt.changed))
            .count();
        assert_eq!(changed_count, 1, "only one CAS transition may mutate state");

        for result in results {
            match result {
                Ok(receipt) => {
                    assert!(receipt.changed);
                    assert_eq!(receipt.previous_phase, NovelWorkflowPhase::Foundation);
                    assert_eq!(receipt.state.phase, final_phase);
                }
                Err(NovelWorkflowError::StaleExpectedPhase { expected, actual }) => {
                    assert_eq!(expected, NovelWorkflowPhase::Foundation);
                    assert_eq!(actual, final_phase);
                }
                other => panic!("unexpected concurrent transition result: {other:?}"),
            }
        }
    }

    #[tokio::test]
    async fn concurrent_transitions_with_same_expected_phase_change_at_most_once() {
        let db = setup_workflow_db().await;
        assert_concurrent_transitions_are_compare_and_swap(&db).await;
    }

    #[tokio::test]
    #[ignore = "requires MUMU_R3_POSTGRES_URL for a fresh isolated PostgreSQL database"]
    async fn postgres_concurrent_transitions_with_same_expected_phase_change_at_most_once() {
        let db = setup_postgres_workflow_db().await;
        assert_concurrent_transitions_are_compare_and_swap(&db).await;
    }

    #[test]
    fn state_view_exposes_server_owned_transition_metadata() {
        let updated_at = chrono::NaiveDate::from_ymd_opt(2026, 7, 14)
            .unwrap()
            .and_hms_opt(12, 0, 0)
            .unwrap();
        let view = NovelWorkflowStateView::new(
            "project-1".to_string(),
            NovelWorkflowPhase::Writing,
            updated_at,
        );

        assert_eq!(view.schema_version, 1);
        assert_eq!(view.source, "projects.status");
        assert_eq!(
            view.allowed_transitions,
            vec![
                NovelWorkflowPhase::Outline,
                NovelWorkflowPhase::Reviewing,
                NovelWorkflowPhase::Completed
            ]
        );
        assert!(view.can_rollback);
        assert_eq!(
            view.suggested_next_phase,
            Some(NovelWorkflowPhase::Reviewing)
        );
    }
}
