use serde::{Deserialize, Deserializer};
use serde_json::Value;

const FORESHADOW_LIST_PAGE_DEFAULT: u64 = 1;
const FORESHADOW_LIST_PAGE_MIN: i64 = 1;
const FORESHADOW_LIST_LIMIT_DEFAULT: u64 = 50;
const FORESHADOW_LIST_LIMIT_MIN: i64 = 1;
const FORESHADOW_LIST_LIMIT_MAX: u64 = 100;
const FORESHADOW_LOOKAHEAD_DEFAULT: i32 = 5;
const FORESHADOW_LOOKAHEAD_MIN: i32 = 1;
const FORESHADOW_LOOKAHEAD_MAX: i32 = 20;
const FORESHADOW_CURRENT_CHAPTER_MIN: i32 = 1;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ForeshadowQueryRequestError {
    PageTooSmall,
    LimitTooSmall,
    LimitTooLarge,
    CurrentChapterMissing,
    CurrentChapterTooSmall,
    LookaheadTooSmall,
    LookaheadTooLarge,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub(crate) struct ListForeshadowsRouteQuery {
    status: Option<String>,
    category: Option<String>,
    source_type: Option<String>,
    is_long_term: Option<bool>,
    page: Option<i64>,
    limit: Option<i64>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub(crate) struct ForeshadowStatsRouteQuery {
    current_chapter: Option<i32>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub(crate) struct ForeshadowContextRouteQuery {
    include_pending: Option<bool>,
    include_overdue: Option<bool>,
    lookahead: Option<i32>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub(crate) struct PendingResolveForeshadowsRouteQuery {
    current_chapter: Option<i32>,
    lookahead: Option<i32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ListForeshadowsQueryRequest {
    status: Option<String>,
    category: Option<String>,
    source_type: Option<String>,
    is_long_term: Option<bool>,
    page: u64,
    limit: u64,
}

impl ListForeshadowsQueryRequest {
    pub(crate) fn from_route_query(
        query: ListForeshadowsRouteQuery,
    ) -> Result<Self, ForeshadowQueryRequestError> {
        Ok(Self {
            status: query.status,
            category: query.category,
            source_type: query.source_type,
            is_long_term: query.is_long_term,
            page: validate_optional_min(
                query.page,
                FORESHADOW_LIST_PAGE_DEFAULT,
                FORESHADOW_LIST_PAGE_MIN,
                ForeshadowQueryRequestError::PageTooSmall,
            )?,
            limit: validate_optional_range(
                query.limit,
                FORESHADOW_LIST_LIMIT_DEFAULT,
                FORESHADOW_LIST_LIMIT_MIN,
                FORESHADOW_LIST_LIMIT_MAX,
                ForeshadowQueryRequestError::LimitTooSmall,
                ForeshadowQueryRequestError::LimitTooLarge,
            )?,
        })
    }

    pub(crate) fn status(&self) -> Option<&str> {
        self.status.as_deref()
    }

    pub(crate) fn category(&self) -> Option<&str> {
        self.category.as_deref()
    }

    pub(crate) fn source_type(&self) -> Option<&str> {
        self.source_type.as_deref()
    }

    pub(crate) fn is_long_term(&self) -> Option<bool> {
        self.is_long_term
    }

    pub(crate) fn page(&self) -> u64 {
        self.page
    }

    pub(crate) fn limit(&self) -> u64 {
        self.limit
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ForeshadowStatsQueryRequest {
    current_chapter: Option<i32>,
}

impl ForeshadowStatsQueryRequest {
    pub(crate) fn from_route_query(
        query: ForeshadowStatsRouteQuery,
    ) -> Result<Self, ForeshadowQueryRequestError> {
        if let Some(current_chapter) = query.current_chapter {
            if current_chapter < FORESHADOW_CURRENT_CHAPTER_MIN {
                return Err(ForeshadowQueryRequestError::CurrentChapterTooSmall);
            }
        }

        Ok(Self {
            current_chapter: query.current_chapter,
        })
    }

    pub(crate) fn current_chapter(&self) -> Option<i32> {
        self.current_chapter
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ForeshadowContextQueryRequest {
    include_pending: Option<bool>,
    include_overdue: Option<bool>,
    lookahead: i32,
}

impl ForeshadowContextQueryRequest {
    pub(crate) fn from_route_query(
        query: ForeshadowContextRouteQuery,
    ) -> Result<Self, ForeshadowQueryRequestError> {
        Ok(Self {
            include_pending: query.include_pending,
            include_overdue: query.include_overdue,
            lookahead: validate_optional_i32_range(
                query.lookahead,
                FORESHADOW_LOOKAHEAD_DEFAULT,
                FORESHADOW_LOOKAHEAD_MIN,
                FORESHADOW_LOOKAHEAD_MAX,
                ForeshadowQueryRequestError::LookaheadTooSmall,
                ForeshadowQueryRequestError::LookaheadTooLarge,
            )?,
        })
    }

    pub(crate) fn include_pending(&self) -> Option<bool> {
        self.include_pending
    }

    pub(crate) fn include_overdue(&self) -> Option<bool> {
        self.include_overdue
    }

    pub(crate) fn lookahead(&self) -> i32 {
        self.lookahead
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PendingResolveForeshadowsQueryRequest {
    current_chapter: i32,
    lookahead: i32,
}

impl PendingResolveForeshadowsQueryRequest {
    pub(crate) fn from_route_query(
        query: PendingResolveForeshadowsRouteQuery,
    ) -> Result<Self, ForeshadowQueryRequestError> {
        let Some(current_chapter) = query.current_chapter else {
            return Err(ForeshadowQueryRequestError::CurrentChapterMissing);
        };
        if current_chapter < FORESHADOW_CURRENT_CHAPTER_MIN {
            return Err(ForeshadowQueryRequestError::CurrentChapterTooSmall);
        }

        Ok(Self {
            current_chapter,
            lookahead: validate_optional_i32_range(
                query.lookahead,
                FORESHADOW_LOOKAHEAD_DEFAULT,
                FORESHADOW_LOOKAHEAD_MIN,
                FORESHADOW_LOOKAHEAD_MAX,
                ForeshadowQueryRequestError::LookaheadTooSmall,
                ForeshadowQueryRequestError::LookaheadTooLarge,
            )?,
        })
    }

    pub(crate) fn current_chapter(&self) -> i32 {
        self.current_chapter
    }

    pub(crate) fn lookahead(&self) -> i32 {
        self.lookahead
    }
}

fn validate_optional_min(
    value: Option<i64>,
    default: u64,
    min: i64,
    too_small: ForeshadowQueryRequestError,
) -> Result<u64, ForeshadowQueryRequestError> {
    let Some(value) = value else {
        return Ok(default);
    };
    if value < min {
        return Err(too_small);
    }
    Ok(value as u64)
}

fn validate_optional_range(
    value: Option<i64>,
    default: u64,
    min: i64,
    max: u64,
    too_small: ForeshadowQueryRequestError,
    too_large: ForeshadowQueryRequestError,
) -> Result<u64, ForeshadowQueryRequestError> {
    let Some(value) = value else {
        return Ok(default);
    };
    if value < min {
        return Err(too_small);
    }
    if value > max as i64 {
        return Err(too_large);
    }
    Ok(value as u64)
}

fn validate_optional_i32_range(
    value: Option<i32>,
    default: i32,
    min: i32,
    max: i32,
    too_small: ForeshadowQueryRequestError,
    too_large: ForeshadowQueryRequestError,
) -> Result<i32, ForeshadowQueryRequestError> {
    let Some(value) = value else {
        return Ok(default);
    };
    if value < min {
        return Err(too_small);
    }
    if value > max {
        return Err(too_large);
    }
    Ok(value)
}

#[derive(Debug, Clone, PartialEq)]
enum RouteValueField {
    Missing,
    Present(Value),
}

impl Default for RouteValueField {
    fn default() -> Self {
        Self::Missing
    }
}

impl<'de> Deserialize<'de> for RouteValueField {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Value::deserialize(deserializer).map(Self::Present)
    }
}

impl RouteValueField {
    fn present(&self) -> bool {
        matches!(self, Self::Present(_))
    }

    fn into_option(self) -> Option<Value> {
        match self {
            Self::Missing => None,
            Self::Present(value) => Some(value),
        }
    }
}

fn value_as_string(value: Option<&Value>) -> Option<String> {
    value.and_then(Value::as_str).map(ToString::to_string)
}

fn value_as_i32(value: Option<&Value>) -> Option<i32> {
    value.and_then(Value::as_i64).map(|value| value as i32)
}

fn value_as_bool(value: Option<&Value>) -> Option<bool> {
    value.and_then(Value::as_bool)
}

fn value_as_f64(value: Option<&Value>) -> Option<f64> {
    value.and_then(Value::as_f64)
}

#[derive(Debug, Default, Deserialize)]
pub struct CreateForeshadowRouteRequest {
    project_id: Option<Value>,
    title: Option<Value>,
    content: Option<Value>,
    hint_text: Option<Value>,
    resolution_text: Option<Value>,
    plant_chapter_number: Option<Value>,
    target_resolve_chapter_number: Option<Value>,
    is_long_term: Option<Value>,
    importance: Option<Value>,
    strength: Option<Value>,
    subtlety: Option<Value>,
    #[serde(default)]
    related_characters: RouteValueField,
    #[serde(default)]
    tags: RouteValueField,
    category: Option<Value>,
    notes: Option<Value>,
    resolution_notes: Option<Value>,
    auto_remind: Option<Value>,
    remind_before_chapters: Option<Value>,
    include_in_context: Option<Value>,
}

#[derive(Debug, Default, Deserialize)]
pub struct UpdateForeshadowRouteRequest {
    title: Option<Value>,
    content: Option<Value>,
    hint_text: Option<Value>,
    resolution_text: Option<Value>,
    plant_chapter_number: Option<Value>,
    target_resolve_chapter_number: Option<Value>,
    status: Option<Value>,
    is_long_term: Option<Value>,
    importance: Option<Value>,
    strength: Option<Value>,
    subtlety: Option<Value>,
    urgency: Option<Value>,
    #[serde(default)]
    related_characters: RouteValueField,
    #[serde(default)]
    related_foreshadow_ids: RouteValueField,
    #[serde(default)]
    tags: RouteValueField,
    category: Option<Value>,
    notes: Option<Value>,
    resolution_notes: Option<Value>,
    auto_remind: Option<Value>,
    remind_before_chapters: Option<Value>,
    include_in_context: Option<Value>,
}

#[derive(Debug, Default, Deserialize)]
pub struct PlantForeshadowRouteRequest {
    chapter_id: Option<Value>,
    chapter_number: Option<Value>,
    hint_text: Option<Value>,
}

#[derive(Debug, Default, Deserialize)]
pub struct ResolveForeshadowRouteRequest {
    is_partial: Option<Value>,
    chapter_id: Option<Value>,
    chapter_number: Option<Value>,
    resolution_text: Option<Value>,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct SyncForeshadowFromAnalysisRouteRequest {
    body: Value,
}

impl SyncForeshadowFromAnalysisRouteRequest {
    pub fn new(body: Value) -> Self {
        Self { body }
    }

    pub fn body(&self) -> &Value {
        &self.body
    }
}

impl<'de> Deserialize<'de> for SyncForeshadowFromAnalysisRouteRequest {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Value::deserialize(deserializer).map(|body| Self { body })
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct CreateForeshadowRequest {
    project_id: String,
    title: String,
    content: String,
    hint_text: Option<String>,
    resolution_text: Option<String>,
    plant_chapter_number: Option<i32>,
    target_resolve_chapter_number: Option<i32>,
    is_long_term: bool,
    importance: f64,
    strength: i32,
    subtlety: i32,
    related_characters: Option<Value>,
    tags: Option<Value>,
    category: Option<String>,
    notes: Option<String>,
    resolution_notes: Option<String>,
    auto_remind: bool,
    remind_before_chapters: i32,
    include_in_context: bool,
}

impl CreateForeshadowRequest {
    pub fn project_id(&self) -> &str {
        self.project_id.as_str()
    }

    pub fn title(&self) -> &str {
        self.title.as_str()
    }

    pub fn content(&self) -> &str {
        self.content.as_str()
    }

    pub fn hint_text(&self) -> Option<&str> {
        self.hint_text.as_deref()
    }

    pub fn resolution_text(&self) -> Option<&str> {
        self.resolution_text.as_deref()
    }

    pub fn plant_chapter_number(&self) -> Option<i32> {
        self.plant_chapter_number
    }

    pub fn target_resolve_chapter_number(&self) -> Option<i32> {
        self.target_resolve_chapter_number
    }

    pub fn is_long_term(&self) -> bool {
        self.is_long_term
    }

    pub fn importance(&self) -> f64 {
        self.importance
    }

    pub fn strength(&self) -> i32 {
        self.strength
    }

    pub fn subtlety(&self) -> i32 {
        self.subtlety
    }

    pub fn related_characters(&self) -> Option<&Value> {
        self.related_characters.as_ref()
    }

    pub fn tags(&self) -> Option<&Value> {
        self.tags.as_ref()
    }

    pub fn category(&self) -> Option<&str> {
        self.category.as_deref()
    }

    pub fn notes(&self) -> Option<&str> {
        self.notes.as_deref()
    }

    pub fn resolution_notes(&self) -> Option<&str> {
        self.resolution_notes.as_deref()
    }

    pub fn auto_remind(&self) -> bool {
        self.auto_remind
    }

    pub fn remind_before_chapters(&self) -> i32 {
        self.remind_before_chapters
    }

    pub fn include_in_context(&self) -> bool {
        self.include_in_context
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct UpdateForeshadowRequest {
    title: Option<String>,
    content: Option<String>,
    hint_text: Option<String>,
    resolution_text: Option<String>,
    plant_chapter_number: Option<i32>,
    target_resolve_chapter_number: Option<i32>,
    status: Option<String>,
    is_long_term: Option<bool>,
    importance: Option<f64>,
    strength: Option<i32>,
    subtlety: Option<i32>,
    urgency: Option<i32>,
    related_characters_present: bool,
    related_characters: Option<Value>,
    related_foreshadow_ids_present: bool,
    related_foreshadow_ids: Option<Value>,
    tags_present: bool,
    tags: Option<Value>,
    category: Option<String>,
    notes: Option<String>,
    resolution_notes: Option<String>,
    auto_remind: Option<bool>,
    remind_before_chapters: Option<i32>,
    include_in_context: Option<bool>,
}

impl UpdateForeshadowRequest {
    pub fn title(&self) -> Option<&str> {
        self.title.as_deref()
    }

    pub fn content(&self) -> Option<&str> {
        self.content.as_deref()
    }

    pub fn hint_text(&self) -> Option<&str> {
        self.hint_text.as_deref()
    }

    pub fn resolution_text(&self) -> Option<&str> {
        self.resolution_text.as_deref()
    }

    pub fn plant_chapter_number(&self) -> Option<i32> {
        self.plant_chapter_number
    }

    pub fn target_resolve_chapter_number(&self) -> Option<i32> {
        self.target_resolve_chapter_number
    }

    pub fn status(&self) -> Option<&str> {
        self.status.as_deref()
    }

    pub fn is_long_term(&self) -> Option<bool> {
        self.is_long_term
    }

    pub fn importance(&self) -> Option<f64> {
        self.importance
    }

    pub fn strength(&self) -> Option<i32> {
        self.strength
    }

    pub fn subtlety(&self) -> Option<i32> {
        self.subtlety
    }

    pub fn urgency(&self) -> Option<i32> {
        self.urgency
    }

    pub fn related_characters_present(&self) -> bool {
        self.related_characters_present
    }

    pub fn related_characters(&self) -> Option<&Value> {
        self.related_characters.as_ref()
    }

    pub fn related_foreshadow_ids_present(&self) -> bool {
        self.related_foreshadow_ids_present
    }

    pub fn related_foreshadow_ids(&self) -> Option<&Value> {
        self.related_foreshadow_ids.as_ref()
    }

    pub fn tags_present(&self) -> bool {
        self.tags_present
    }

    pub fn tags(&self) -> Option<&Value> {
        self.tags.as_ref()
    }

    pub fn category(&self) -> Option<&str> {
        self.category.as_deref()
    }

    pub fn notes(&self) -> Option<&str> {
        self.notes.as_deref()
    }

    pub fn resolution_notes(&self) -> Option<&str> {
        self.resolution_notes.as_deref()
    }

    pub fn auto_remind(&self) -> Option<bool> {
        self.auto_remind
    }

    pub fn remind_before_chapters(&self) -> Option<i32> {
        self.remind_before_chapters
    }

    pub fn include_in_context(&self) -> Option<bool> {
        self.include_in_context
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlantForeshadowRequest {
    chapter_id: Option<String>,
    chapter_number: Option<i32>,
    hint_text: Option<String>,
}

impl PlantForeshadowRequest {
    pub fn chapter_id(&self) -> Option<&str> {
        self.chapter_id.as_deref()
    }

    pub fn chapter_number(&self) -> Option<i32> {
        self.chapter_number
    }

    pub fn hint_text(&self) -> Option<&str> {
        self.hint_text.as_deref()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolveForeshadowRequest {
    is_partial: bool,
    chapter_id: Option<String>,
    chapter_number: Option<i32>,
    resolution_text: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyncForeshadowFromAnalysisRequest {
    body: Value,
}

impl ResolveForeshadowRequest {
    pub fn is_partial(&self) -> bool {
        self.is_partial
    }

    pub fn chapter_id(&self) -> Option<&str> {
        self.chapter_id.as_deref()
    }

    pub fn chapter_number(&self) -> Option<i32> {
        self.chapter_number
    }

    pub fn resolution_text(&self) -> Option<&str> {
        self.resolution_text.as_deref()
    }
}

impl SyncForeshadowFromAnalysisRequest {
    pub fn body(&self) -> &Value {
        &self.body
    }
}

pub fn build_create_foreshadow_request_from_route_payload(
    body: CreateForeshadowRouteRequest,
) -> CreateForeshadowRequest {
    CreateForeshadowRequest {
        project_id: value_as_string(body.project_id.as_ref()).unwrap_or_default(),
        title: value_as_string(body.title.as_ref()).unwrap_or_default(),
        content: value_as_string(body.content.as_ref()).unwrap_or_default(),
        hint_text: value_as_string(body.hint_text.as_ref()),
        resolution_text: value_as_string(body.resolution_text.as_ref()),
        plant_chapter_number: value_as_i32(body.plant_chapter_number.as_ref()),
        target_resolve_chapter_number: value_as_i32(body.target_resolve_chapter_number.as_ref()),
        is_long_term: value_as_bool(body.is_long_term.as_ref()).unwrap_or(false),
        importance: value_as_f64(body.importance.as_ref()).unwrap_or(0.5),
        strength: value_as_i32(body.strength.as_ref()).unwrap_or(5),
        subtlety: value_as_i32(body.subtlety.as_ref()).unwrap_or(5),
        related_characters: body.related_characters.into_option(),
        tags: body.tags.into_option(),
        category: value_as_string(body.category.as_ref()),
        notes: value_as_string(body.notes.as_ref()),
        resolution_notes: value_as_string(body.resolution_notes.as_ref()),
        auto_remind: value_as_bool(body.auto_remind.as_ref()).unwrap_or(true),
        remind_before_chapters: value_as_i32(body.remind_before_chapters.as_ref()).unwrap_or(5),
        include_in_context: value_as_bool(body.include_in_context.as_ref()).unwrap_or(true),
    }
}

pub fn build_update_foreshadow_request_from_route_payload(
    body: UpdateForeshadowRouteRequest,
) -> UpdateForeshadowRequest {
    UpdateForeshadowRequest {
        title: value_as_string(body.title.as_ref()),
        content: value_as_string(body.content.as_ref()),
        hint_text: value_as_string(body.hint_text.as_ref()),
        resolution_text: value_as_string(body.resolution_text.as_ref()),
        plant_chapter_number: value_as_i32(body.plant_chapter_number.as_ref()),
        target_resolve_chapter_number: value_as_i32(body.target_resolve_chapter_number.as_ref()),
        status: value_as_string(body.status.as_ref()),
        is_long_term: value_as_bool(body.is_long_term.as_ref()),
        importance: value_as_f64(body.importance.as_ref()),
        strength: value_as_i32(body.strength.as_ref()),
        subtlety: value_as_i32(body.subtlety.as_ref()),
        urgency: value_as_i32(body.urgency.as_ref()),
        related_characters_present: body.related_characters.present(),
        related_characters: body.related_characters.into_option(),
        related_foreshadow_ids_present: body.related_foreshadow_ids.present(),
        related_foreshadow_ids: body.related_foreshadow_ids.into_option(),
        tags_present: body.tags.present(),
        tags: body.tags.into_option(),
        category: value_as_string(body.category.as_ref()),
        notes: value_as_string(body.notes.as_ref()),
        resolution_notes: value_as_string(body.resolution_notes.as_ref()),
        auto_remind: value_as_bool(body.auto_remind.as_ref()),
        remind_before_chapters: value_as_i32(body.remind_before_chapters.as_ref()),
        include_in_context: value_as_bool(body.include_in_context.as_ref()),
    }
}

pub fn build_plant_foreshadow_request_from_route_payload(
    body: PlantForeshadowRouteRequest,
) -> PlantForeshadowRequest {
    PlantForeshadowRequest {
        chapter_id: value_as_string(body.chapter_id.as_ref()),
        chapter_number: value_as_i32(body.chapter_number.as_ref()),
        hint_text: value_as_string(body.hint_text.as_ref()),
    }
}

pub fn build_resolve_foreshadow_request_from_route_payload(
    body: ResolveForeshadowRouteRequest,
) -> ResolveForeshadowRequest {
    ResolveForeshadowRequest {
        is_partial: value_as_bool(body.is_partial.as_ref()).unwrap_or(false),
        chapter_id: value_as_string(body.chapter_id.as_ref()),
        chapter_number: value_as_i32(body.chapter_number.as_ref()),
        resolution_text: value_as_string(body.resolution_text.as_ref()),
    }
}

pub fn build_sync_foreshadow_from_analysis_request_from_route_payload(
    body: SyncForeshadowFromAnalysisRouteRequest,
) -> SyncForeshadowFromAnalysisRequest {
    SyncForeshadowFromAnalysisRequest { body: body.body }
}

#[cfg(test)]
mod tests {
    use serde_json::{json, Value};

    use super::{
        build_create_foreshadow_request_from_route_payload,
        build_plant_foreshadow_request_from_route_payload,
        build_resolve_foreshadow_request_from_route_payload,
        build_sync_foreshadow_from_analysis_request_from_route_payload,
        build_update_foreshadow_request_from_route_payload, CreateForeshadowRouteRequest,
        ForeshadowContextQueryRequest, ForeshadowContextRouteQuery, ForeshadowQueryRequestError,
        ForeshadowStatsQueryRequest, ForeshadowStatsRouteQuery, ListForeshadowsQueryRequest,
        ListForeshadowsRouteQuery, PendingResolveForeshadowsQueryRequest,
        PendingResolveForeshadowsRouteQuery, PlantForeshadowRouteRequest,
        ResolveForeshadowRouteRequest, RouteValueField, SyncForeshadowFromAnalysisRouteRequest,
        UpdateForeshadowRouteRequest,
    };

    #[test]
    fn list_foreshadows_query_request_validates_python_query_bounds() {
        let default_request =
            ListForeshadowsQueryRequest::from_route_query(ListForeshadowsRouteQuery::default())
                .expect("python defaults should be valid");

        assert_eq!(default_request.page(), 1);
        assert_eq!(default_request.limit(), 50);

        let explicit_request =
            ListForeshadowsQueryRequest::from_route_query(ListForeshadowsRouteQuery {
                status: Some("pending".to_string()),
                category: Some("main".to_string()),
                source_type: Some("analysis".to_string()),
                is_long_term: Some(true),
                page: Some(2),
                limit: Some(100),
            })
            .expect("upper python bound should be valid");

        assert_eq!(explicit_request.status(), Some("pending"));
        assert_eq!(explicit_request.category(), Some("main"));
        assert_eq!(explicit_request.source_type(), Some("analysis"));
        assert_eq!(explicit_request.is_long_term(), Some(true));
        assert_eq!(explicit_request.page(), 2);
        assert_eq!(explicit_request.limit(), 100);

        assert_eq!(
            ListForeshadowsQueryRequest::from_route_query(ListForeshadowsRouteQuery {
                page: Some(0),
                ..ListForeshadowsRouteQuery::default()
            }),
            Err(ForeshadowQueryRequestError::PageTooSmall)
        );
        assert_eq!(
            ListForeshadowsQueryRequest::from_route_query(ListForeshadowsRouteQuery {
                limit: Some(0),
                ..ListForeshadowsRouteQuery::default()
            }),
            Err(ForeshadowQueryRequestError::LimitTooSmall)
        );
        assert_eq!(
            ListForeshadowsQueryRequest::from_route_query(ListForeshadowsRouteQuery {
                limit: Some(101),
                ..ListForeshadowsRouteQuery::default()
            }),
            Err(ForeshadowQueryRequestError::LimitTooLarge)
        );
    }

    #[test]
    fn foreshadow_stats_query_request_validates_current_chapter_like_python_query() {
        assert_eq!(
            ForeshadowStatsQueryRequest::from_route_query(ForeshadowStatsRouteQuery {
                current_chapter: None,
            })
            .expect("optional current_chapter may be absent")
            .current_chapter(),
            None
        );
        assert_eq!(
            ForeshadowStatsQueryRequest::from_route_query(ForeshadowStatsRouteQuery {
                current_chapter: Some(1),
            })
            .expect("minimum current_chapter should be valid")
            .current_chapter(),
            Some(1)
        );
        assert_eq!(
            ForeshadowStatsQueryRequest::from_route_query(ForeshadowStatsRouteQuery {
                current_chapter: Some(0),
            }),
            Err(ForeshadowQueryRequestError::CurrentChapterTooSmall)
        );
    }

    #[test]
    fn foreshadow_context_query_request_validates_lookahead_like_python_query() {
        let default_request =
            ForeshadowContextQueryRequest::from_route_query(ForeshadowContextRouteQuery::default())
                .expect("python defaults should be valid");

        assert_eq!(default_request.include_pending(), None);
        assert_eq!(default_request.include_overdue(), None);
        assert_eq!(default_request.lookahead(), 5);

        let explicit_request =
            ForeshadowContextQueryRequest::from_route_query(ForeshadowContextRouteQuery {
                include_pending: Some(false),
                include_overdue: Some(true),
                lookahead: Some(20),
            })
            .expect("upper python bound should be valid");

        assert_eq!(explicit_request.include_pending(), Some(false));
        assert_eq!(explicit_request.include_overdue(), Some(true));
        assert_eq!(explicit_request.lookahead(), 20);

        assert_eq!(
            ForeshadowContextQueryRequest::from_route_query(ForeshadowContextRouteQuery {
                lookahead: Some(0),
                ..ForeshadowContextRouteQuery::default()
            }),
            Err(ForeshadowQueryRequestError::LookaheadTooSmall)
        );
        assert_eq!(
            ForeshadowContextQueryRequest::from_route_query(ForeshadowContextRouteQuery {
                lookahead: Some(21),
                ..ForeshadowContextRouteQuery::default()
            }),
            Err(ForeshadowQueryRequestError::LookaheadTooLarge)
        );
    }

    #[test]
    fn pending_resolve_query_request_requires_current_chapter_and_validates_lookahead() {
        assert_eq!(
            PendingResolveForeshadowsQueryRequest::from_route_query(
                PendingResolveForeshadowsRouteQuery::default()
            ),
            Err(ForeshadowQueryRequestError::CurrentChapterMissing)
        );
        assert_eq!(
            PendingResolveForeshadowsQueryRequest::from_route_query(
                PendingResolveForeshadowsRouteQuery {
                    current_chapter: Some(0),
                    lookahead: None,
                }
            ),
            Err(ForeshadowQueryRequestError::CurrentChapterTooSmall)
        );

        let request = PendingResolveForeshadowsQueryRequest::from_route_query(
            PendingResolveForeshadowsRouteQuery {
                current_chapter: Some(3),
                lookahead: None,
            },
        )
        .expect("required current_chapter with default lookahead should be valid");

        assert_eq!(request.current_chapter(), 3);
        assert_eq!(request.lookahead(), 5);

        assert_eq!(
            PendingResolveForeshadowsQueryRequest::from_route_query(
                PendingResolveForeshadowsRouteQuery {
                    current_chapter: Some(3),
                    lookahead: Some(21),
                }
            ),
            Err(ForeshadowQueryRequestError::LookaheadTooLarge)
        );
    }

    #[test]
    fn build_create_foreshadow_request_from_route_payload_keeps_existing_defaults() {
        let request = build_create_foreshadow_request_from_route_payload(
            CreateForeshadowRouteRequest::default(),
        );

        assert_eq!(request.project_id(), "");
        assert_eq!(request.title(), "");
        assert_eq!(request.content(), "");
        assert_eq!(request.hint_text(), None);
        assert_eq!(request.resolution_text(), None);
        assert_eq!(request.plant_chapter_number(), None);
        assert_eq!(request.target_resolve_chapter_number(), None);
        assert!(!request.is_long_term());
        assert_eq!(request.importance(), 0.5);
        assert_eq!(request.strength(), 5);
        assert_eq!(request.subtlety(), 5);
        assert!(request.related_characters().is_none());
        assert!(request.tags().is_none());
        assert_eq!(request.category(), None);
        assert_eq!(request.notes(), None);
        assert_eq!(request.resolution_notes(), None);
        assert!(request.auto_remind());
        assert_eq!(request.remind_before_chapters(), 5);
        assert!(request.include_in_context());
    }

    #[test]
    fn build_create_foreshadow_request_from_route_payload_keeps_route_values() {
        let request =
            build_create_foreshadow_request_from_route_payload(CreateForeshadowRouteRequest {
                project_id: Some(json!("project-1")),
                title: Some(json!("伏笔标题")),
                content: Some(json!("伏笔内容")),
                hint_text: Some(json!("提示")),
                resolution_text: Some(json!("回收")),
                plant_chapter_number: Some(json!(7)),
                target_resolve_chapter_number: Some(json!(20)),
                is_long_term: Some(json!(true)),
                importance: Some(json!(0.9)),
                strength: Some(json!(8)),
                subtlety: Some(json!(3)),
                related_characters: RouteValueField::Present(json!(["角色A"])),
                tags: RouteValueField::Present(json!(["主线"])),
                category: Some(json!("mystery")),
                notes: Some(json!("备注")),
                resolution_notes: Some(json!("回收备注")),
                auto_remind: Some(json!(false)),
                remind_before_chapters: Some(json!(2)),
                include_in_context: Some(json!(false)),
            });

        assert_eq!(request.project_id(), "project-1");
        assert_eq!(request.title(), "伏笔标题");
        assert_eq!(request.content(), "伏笔内容");
        assert_eq!(request.hint_text(), Some("提示"));
        assert_eq!(request.resolution_text(), Some("回收"));
        assert_eq!(request.plant_chapter_number(), Some(7));
        assert_eq!(request.target_resolve_chapter_number(), Some(20));
        assert!(request.is_long_term());
        assert_eq!(request.importance(), 0.9);
        assert_eq!(request.strength(), 8);
        assert_eq!(request.subtlety(), 3);
        assert_eq!(request.related_characters(), Some(&json!(["角色A"])));
        assert_eq!(request.tags(), Some(&json!(["主线"])));
        assert_eq!(request.category(), Some("mystery"));
        assert_eq!(request.notes(), Some("备注"));
        assert_eq!(request.resolution_notes(), Some("回收备注"));
        assert!(!request.auto_remind());
        assert_eq!(request.remind_before_chapters(), 2);
        assert!(!request.include_in_context());
    }

    #[test]
    fn build_update_foreshadow_request_from_route_payload_keeps_presence_flags() {
        let request =
            build_update_foreshadow_request_from_route_payload(UpdateForeshadowRouteRequest {
                title: Some(json!("新标题")),
                related_characters: RouteValueField::Present(Value::Null),
                related_foreshadow_ids: RouteValueField::Present(json!(["f-1"])),
                tags: RouteValueField::Present(json!(["支线"])),
                include_in_context: Some(json!(false)),
                ..UpdateForeshadowRouteRequest::default()
            });

        assert_eq!(request.title(), Some("新标题"));
        assert!(request.related_characters_present());
        assert_eq!(request.related_characters(), Some(&Value::Null));
        assert!(request.related_foreshadow_ids_present());
        assert_eq!(request.related_foreshadow_ids(), Some(&json!(["f-1"])));
        assert!(request.tags_present());
        assert_eq!(request.tags(), Some(&json!(["支线"])));
        assert_eq!(request.include_in_context(), Some(false));
    }

    #[test]
    fn build_plant_foreshadow_request_from_route_payload_keeps_optional_fields() {
        let request =
            build_plant_foreshadow_request_from_route_payload(PlantForeshadowRouteRequest {
                chapter_id: Some(json!("chapter-1")),
                chapter_number: Some(json!(9)),
                hint_text: Some(json!("埋设提示")),
            });

        assert_eq!(request.chapter_id(), Some("chapter-1"));
        assert_eq!(request.chapter_number(), Some(9));
        assert_eq!(request.hint_text(), Some("埋设提示"));
    }

    #[test]
    fn build_resolve_foreshadow_request_from_route_payload_defaults_partial_to_false() {
        let request =
            build_resolve_foreshadow_request_from_route_payload(ResolveForeshadowRouteRequest {
                chapter_id: Some(json!("chapter-2")),
                chapter_number: Some(json!(18)),
                resolution_text: Some(json!("完成回收")),
                ..ResolveForeshadowRouteRequest::default()
            });

        assert!(!request.is_partial());
        assert_eq!(request.chapter_id(), Some("chapter-2"));
        assert_eq!(request.chapter_number(), Some(18));
        assert_eq!(request.resolution_text(), Some("完成回收"));
    }

    #[test]
    fn build_sync_foreshadow_from_analysis_request_from_route_payload_preserves_any_json_shape() {
        let object_request = build_sync_foreshadow_from_analysis_request_from_route_payload(
            SyncForeshadowFromAnalysisRouteRequest {
                body: json!({
                    "analysis_id": "analysis-1",
                    "items": [{"title": "伏笔A"}]
                }),
            },
        );
        assert_eq!(
            object_request.body(),
            &json!({
                "analysis_id": "analysis-1",
                "items": [{"title": "伏笔A"}]
            })
        );

        let scalar_request = build_sync_foreshadow_from_analysis_request_from_route_payload(
            SyncForeshadowFromAnalysisRouteRequest { body: Value::Null },
        );
        assert_eq!(scalar_request.body(), &Value::Null);
    }
}
