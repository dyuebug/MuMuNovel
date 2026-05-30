use sea_orm::DatabaseConnection;
use serde::Serialize;
use serde_json::{json, Value};

use crate::models::chapter;
use crate::services::chapter_service::ChapterService;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CreateChapterRequest {
    project_id: String,
    title: String,
    chapter_number: i32,
    content: Option<String>,
    summary: Option<String>,
    outline_id: Option<String>,
    sub_index: Option<i32>,
}

impl CreateChapterRequest {
    pub(crate) fn new(
        project_id: &str,
        title: &str,
        chapter_number: i32,
        content: Option<&str>,
        summary: Option<&str>,
        outline_id: Option<&str>,
        sub_index: Option<i32>,
    ) -> Self {
        Self {
            project_id: project_id.to_owned(),
            title: title.to_owned(),
            chapter_number,
            content: content.map(str::to_owned),
            summary: summary.map(str::to_owned),
            outline_id: outline_id.map(str::to_owned),
            sub_index,
        }
    }

    pub(crate) fn from_route_payload(
        project_id: String,
        title: String,
        chapter_number: i32,
        content: Option<String>,
        summary: Option<String>,
        outline_id: Option<String>,
        sub_index: Option<i32>,
    ) -> Self {
        Self::new(
            &project_id,
            &title,
            chapter_number,
            content.as_deref(),
            summary.as_deref(),
            outline_id.as_deref(),
            sub_index,
        )
    }

    pub(crate) fn project_id(&self) -> &str {
        &self.project_id
    }

    pub(crate) fn title(&self) -> &str {
        &self.title
    }

    pub(crate) fn chapter_number(&self) -> i32 {
        self.chapter_number
    }

    pub(crate) fn content(&self) -> Option<&str> {
        self.content.as_deref()
    }

    pub(crate) fn summary(&self) -> Option<&str> {
        self.summary.as_deref()
    }

    pub(crate) fn outline_id(&self) -> Option<&str> {
        self.outline_id.as_deref()
    }

    pub(crate) fn sub_index(&self) -> Option<i32> {
        self.sub_index
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct UpdateChapterRequest {
    title: Option<String>,
    content: Option<String>,
    summary: Option<String>,
    status: Option<String>,
    chapter_number: Option<i32>,
    expansion_plan: Option<String>,
}

impl UpdateChapterRequest {
    pub(crate) fn new(
        title: Option<&str>,
        content: Option<&str>,
        summary: Option<&str>,
        status: Option<&str>,
        chapter_number: Option<i32>,
        expansion_plan: Option<&str>,
    ) -> Self {
        Self {
            title: title.map(str::to_owned),
            content: content.map(str::to_owned),
            summary: summary.map(str::to_owned),
            status: status.map(str::to_owned),
            chapter_number,
            expansion_plan: expansion_plan.map(str::to_owned),
        }
    }

    pub(crate) fn from_route_payload(
        title: Option<String>,
        content: Option<String>,
        summary: Option<String>,
        status: Option<String>,
        chapter_number: Option<i32>,
        expansion_plan: Option<String>,
    ) -> Self {
        Self::new(
            title.as_deref(),
            content.as_deref(),
            summary.as_deref(),
            status.as_deref(),
            chapter_number,
            expansion_plan.as_deref(),
        )
    }

    pub(crate) fn title(&self) -> Option<&str> {
        self.title.as_deref()
    }

    pub(crate) fn content(&self) -> Option<&str> {
        self.content.as_deref()
    }

    pub(crate) fn summary(&self) -> Option<&str> {
        self.summary.as_deref()
    }

    pub(crate) fn status(&self) -> Option<&str> {
        self.status.as_deref()
    }

    pub(crate) fn chapter_number(&self) -> Option<i32> {
        self.chapter_number
    }

    pub(crate) fn expansion_plan(&self) -> Option<&str> {
        self.expansion_plan.as_deref()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct UpdateExpansionPlanRequest {
    plan: String,
}

impl UpdateExpansionPlanRequest {
    pub(crate) fn new(plan: &str) -> Self {
        Self {
            plan: plan.to_owned(),
        }
    }

    pub(crate) fn from_route_payload(plan: String) -> Self {
        Self::new(&plan)
    }

    pub(crate) fn plan(&self) -> &str {
        &self.plan
    }
}

#[derive(Debug)]
pub enum CrudPayloadError<TNotFound> {
    NotFound(TNotFound),
    Internal(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProjectCrudNotFound {
    ProjectNotFound,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChapterCrudNotFound {
    ChapterNotFound,
}

pub type ProjectCrudPayloadError = CrudPayloadError<ProjectCrudNotFound>;
pub type ChapterCrudPayloadError = CrudPayloadError<ChapterCrudNotFound>;

pub type CreateChapterPayloadError = ProjectCrudPayloadError;
pub type ListChaptersPayloadError = ProjectCrudPayloadError;
pub type ListChaptersByProjectPathPayloadError = ProjectCrudPayloadError;
pub type GetChapterPayloadError = ChapterCrudPayloadError;
pub type UpdateChapterPayloadError = ChapterCrudPayloadError;
pub type DeleteChapterPayloadError = ChapterCrudPayloadError;
pub type UpdateExpansionPlanPayloadError = ChapterCrudPayloadError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ListChaptersRequest {
    project_id: String,
}

impl ListChaptersRequest {
    pub(crate) fn new(project_id: &str) -> Self {
        Self {
            project_id: project_id.to_owned(),
        }
    }

    pub(crate) fn from_route_payload(project_id: String) -> Self {
        Self::new(&project_id)
    }

    pub(crate) fn project_id(&self) -> &str {
        &self.project_id
    }
}

fn serialize_value<T: Serialize + ?Sized>(value: &T, fallback: Value) -> Value {
    serde_json::to_value(value).unwrap_or(fallback)
}

fn compatible_chapter_list_payload(chapters: &[chapter::Model]) -> Value {
    let items = serialize_value(chapters, json!([]));
    json!({
        "success": true,
        "data": items.clone(),
        "items": items,
        "total": chapters.len()
    })
}

fn project_path_chapter_list_payload(chapters: &[chapter::Model]) -> Value {
    let items = serialize_value(chapters, json!([]));
    json!({
        "items": items,
        "total": chapters.len()
    })
}

fn compatible_chapter_payload(chapter: chapter::Model) -> Value {
    let chapter_value = serialize_value(&chapter, json!({}));
    match chapter_value {
        Value::Object(mut map) => {
            let data = Value::Object(map.clone());
            map.insert("success".to_string(), json!(true));
            map.insert("data".to_string(), data);
            Value::Object(map)
        }
        _ => json!({
            "success": true,
            "data": chapter
        }),
    }
}

pub async fn create_chapter_payload(
    db: &DatabaseConnection,
    user_id: &str,
    request: &CreateChapterRequest,
) -> Result<Value, CreateChapterPayloadError> {
    match ChapterService::create(
        db,
        request.project_id(),
        user_id,
        request.title(),
        request.chapter_number(),
        request.content(),
        request.summary(),
        request.outline_id(),
        request.sub_index(),
    )
    .await
    {
        Ok(Some(chapter)) => Ok(compatible_chapter_payload(chapter)),
        Ok(None) => Err(CrudPayloadError::NotFound(
            ProjectCrudNotFound::ProjectNotFound,
        )),
        Err(error) => Err(ProjectCrudPayloadError::Internal(error)),
    }
}

pub async fn list_chapters_payload(
    db: &DatabaseConnection,
    request: &ListChaptersRequest,
    user_id: &str,
) -> Result<Value, ListChaptersPayloadError> {
    match ChapterService::list_by_project(db, request.project_id(), user_id).await {
        Ok(Some(chapters)) => Ok(compatible_chapter_list_payload(&chapters)),
        Ok(None) => Err(CrudPayloadError::NotFound(
            ProjectCrudNotFound::ProjectNotFound,
        )),
        Err(error) => Err(ProjectCrudPayloadError::Internal(error)),
    }
}

pub async fn list_chapters_by_project_path_payload(
    db: &DatabaseConnection,
    project_id: &str,
    user_id: &str,
) -> Result<Value, ListChaptersByProjectPathPayloadError> {
    match ChapterService::list_by_project(db, project_id, user_id).await {
        Ok(Some(chapters)) => Ok(project_path_chapter_list_payload(&chapters)),
        Ok(None) => Err(CrudPayloadError::NotFound(
            ProjectCrudNotFound::ProjectNotFound,
        )),
        Err(error) => Err(ProjectCrudPayloadError::Internal(error)),
    }
}

pub async fn get_chapter_payload(
    db: &DatabaseConnection,
    chapter_id: &str,
    user_id: &str,
) -> Result<Value, GetChapterPayloadError> {
    match ChapterService::get(db, chapter_id, user_id).await {
        Ok(Some(chapter)) => Ok(compatible_chapter_payload(chapter)),
        Ok(None) => Err(CrudPayloadError::NotFound(
            ChapterCrudNotFound::ChapterNotFound,
        )),
        Err(error) => Err(ChapterCrudPayloadError::Internal(error)),
    }
}

pub async fn update_chapter_payload(
    db: &DatabaseConnection,
    chapter_id: &str,
    user_id: &str,
    request: &UpdateChapterRequest,
) -> Result<Value, UpdateChapterPayloadError> {
    match ChapterService::update(
        db,
        chapter_id,
        user_id,
        request.title(),
        request.content(),
        request.summary(),
        request.status(),
        request.chapter_number(),
        request.expansion_plan(),
    )
    .await
    {
        Ok(Some(chapter)) => Ok(compatible_chapter_payload(chapter)),
        Ok(None) => Err(CrudPayloadError::NotFound(
            ChapterCrudNotFound::ChapterNotFound,
        )),
        Err(error) => Err(ChapterCrudPayloadError::Internal(error)),
    }
}

pub async fn delete_chapter_payload(
    db: &DatabaseConnection,
    chapter_id: &str,
    user_id: &str,
) -> Result<Value, DeleteChapterPayloadError> {
    match ChapterService::delete(db, chapter_id, user_id).await {
        Ok(Some(())) => Ok(json!({
            "success": true,
            "message": "Chapter deleted successfully"
        })),
        Ok(None) => Err(CrudPayloadError::NotFound(
            ChapterCrudNotFound::ChapterNotFound,
        )),
        Err(error) => Err(ChapterCrudPayloadError::Internal(error)),
    }
}

pub async fn update_expansion_plan_payload(
    db: &DatabaseConnection,
    chapter_id: &str,
    user_id: &str,
    request: &UpdateExpansionPlanRequest,
) -> Result<Value, UpdateExpansionPlanPayloadError> {
    match ChapterService::update_expansion_plan(db, chapter_id, user_id, request.plan()).await {
        Ok(Some(chapter)) => Ok(compatible_chapter_payload(chapter)),
        Ok(None) => Err(CrudPayloadError::NotFound(
            ChapterCrudNotFound::ChapterNotFound,
        )),
        Err(error) => Err(ChapterCrudPayloadError::Internal(error)),
    }
}

#[cfg(test)]
mod tests {
    use chrono::NaiveDateTime;

    use crate::models::chapter;

    use super::{
        compatible_chapter_list_payload, compatible_chapter_payload,
        project_path_chapter_list_payload, ChapterCrudNotFound, ChapterCrudPayloadError,
        CreateChapterRequest, CrudPayloadError, ListChaptersRequest, ProjectCrudNotFound,
        ProjectCrudPayloadError, UpdateChapterRequest, UpdateExpansionPlanRequest,
    };

    fn chapter_model(id: &str, number: i32) -> chapter::Model {
        chapter::Model {
            id: id.to_string(),
            project_id: "project-1".to_string(),
            chapter_number: number,
            title: format!("第{}章", number),
            content: Some("正文".to_string()),
            summary: None,
            word_count: 2,
            status: "draft".to_string(),
            outline_id: None,
            sub_index: 0,
            expansion_plan: None,
            created_at: NaiveDateTime::default(),
            updated_at: Some(NaiveDateTime::default()),
        }
    }

    #[test]
    fn project_crud_error_alias_keeps_shared_outer_owner() {
        let error: ProjectCrudPayloadError =
            CrudPayloadError::NotFound(ProjectCrudNotFound::ProjectNotFound);

        assert!(matches!(
            error,
            CrudPayloadError::NotFound(ProjectCrudNotFound::ProjectNotFound)
        ));
    }

    #[test]
    fn chapter_crud_error_alias_keeps_shared_outer_owner() {
        let error: ChapterCrudPayloadError =
            CrudPayloadError::NotFound(ChapterCrudNotFound::ChapterNotFound);

        assert!(matches!(
            error,
            CrudPayloadError::NotFound(ChapterCrudNotFound::ChapterNotFound)
        ));
    }

    #[test]
    fn crud_error_internal_branch_keeps_detail() {
        let error: ChapterCrudPayloadError = CrudPayloadError::Internal("boom".to_string());

        assert!(matches!(
            error,
            CrudPayloadError::Internal(detail) if detail == "boom"
        ));
    }

    #[test]
    fn should_build_compatible_chapter_list_payload() {
        let chapters = vec![chapter_model("chapter-1", 1), chapter_model("chapter-2", 2)];

        let payload = compatible_chapter_list_payload(&chapters);

        assert_eq!(payload["success"], true);
        assert_eq!(payload["total"], 2);
        assert_eq!(payload["data"][0]["id"], "chapter-1");
        assert_eq!(payload["items"][1]["id"], "chapter-2");
        assert_eq!(payload["data"], payload["items"]);
    }

    #[test]
    fn should_build_project_path_chapter_list_payload() {
        let chapters = vec![chapter_model("chapter-1", 1)];

        let payload = project_path_chapter_list_payload(&chapters);

        assert_eq!(payload["total"], 1);
        assert_eq!(payload["items"][0]["id"], "chapter-1");
        assert!(payload.get("success").is_none());
        assert!(payload.get("data").is_none());
    }

    #[test]
    fn should_build_compatible_chapter_payload() {
        let payload = compatible_chapter_payload(chapter_model("chapter-1", 1));

        assert_eq!(payload["success"], true);
        assert_eq!(payload["id"], "chapter-1");
        assert_eq!(payload["title"], "第1章");
        assert_eq!(payload["data"]["id"], "chapter-1");
        assert_eq!(payload["data"]["title"], "第1章");
        assert!(payload["data"].get("success").is_none());
    }

    #[test]
    fn should_build_create_chapter_request() {
        let request = CreateChapterRequest::new(
            "project-1",
            "第一章",
            1,
            Some("正文"),
            Some("摘要"),
            Some("outline-1"),
            Some(2),
        );

        assert_eq!(request.project_id(), "project-1");
        assert_eq!(request.title(), "第一章");
        assert_eq!(request.chapter_number(), 1);
        assert_eq!(request.content(), Some("正文"));
        assert_eq!(request.summary(), Some("摘要"));
        assert_eq!(request.outline_id(), Some("outline-1"));
        assert_eq!(request.sub_index(), Some(2));
    }

    #[test]
    fn should_build_create_chapter_request_from_route_payload() {
        let request = CreateChapterRequest::from_route_payload(
            "project-1".to_string(),
            "第一章".to_string(),
            1,
            Some("正文".to_string()),
            Some("摘要".to_string()),
            Some("outline-1".to_string()),
            Some(2),
        );

        assert_eq!(request.project_id(), "project-1");
        assert_eq!(request.title(), "第一章");
        assert_eq!(request.chapter_number(), 1);
        assert_eq!(request.content(), Some("正文"));
        assert_eq!(request.summary(), Some("摘要"));
        assert_eq!(request.outline_id(), Some("outline-1"));
        assert_eq!(request.sub_index(), Some(2));
    }

    #[test]
    fn should_build_update_chapter_request() {
        let request = UpdateChapterRequest::new(
            Some("新标题"),
            None,
            Some("新摘要"),
            Some("draft"),
            Some(3),
            Some("扩写计划"),
        );

        assert_eq!(request.title(), Some("新标题"));
        assert_eq!(request.content(), None);
        assert_eq!(request.summary(), Some("新摘要"));
        assert_eq!(request.status(), Some("draft"));
        assert_eq!(request.chapter_number(), Some(3));
        assert_eq!(request.expansion_plan(), Some("扩写计划"));
    }

    #[test]
    fn should_build_update_chapter_request_from_route_payload() {
        let request = UpdateChapterRequest::from_route_payload(
            Some("新标题".to_string()),
            None,
            Some("新摘要".to_string()),
            Some("draft".to_string()),
            Some(3),
            Some("扩写计划".to_string()),
        );

        assert_eq!(request.title(), Some("新标题"));
        assert_eq!(request.content(), None);
        assert_eq!(request.summary(), Some("新摘要"));
        assert_eq!(request.status(), Some("draft"));
        assert_eq!(request.chapter_number(), Some(3));
        assert_eq!(request.expansion_plan(), Some("扩写计划"));
    }

    #[test]
    fn should_build_update_expansion_plan_request() {
        let request = UpdateExpansionPlanRequest::new("保持节奏，补足冲突");

        assert_eq!(request.plan(), "保持节奏，补足冲突");
    }

    #[test]
    fn should_build_update_expansion_plan_request_from_route_payload() {
        let request =
            UpdateExpansionPlanRequest::from_route_payload("保持节奏，补足冲突".to_string());

        assert_eq!(request.plan(), "保持节奏，补足冲突");
    }

    #[test]
    fn should_build_list_chapters_request_from_route_payload() {
        let request = ListChaptersRequest::from_route_payload("project-1".to_string());

        assert_eq!(request.project_id(), "project-1");
    }
}
