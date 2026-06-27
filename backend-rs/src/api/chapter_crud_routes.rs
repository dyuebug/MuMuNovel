use axum::{
    extract::{Extension, Path, Query},
    http::StatusCode,
    response::Json,
    routing::get,
    Router,
};
use sea_orm::DatabaseConnection;
#[cfg(test)]
use serde_json::json;
use serde_json::Value;

use self::crud_workflow::{
    build_create_chapter_request_from_route_payload, build_list_chapters_request_from_route_query,
    build_update_chapter_request_from_route_payload,
    build_update_expansion_plan_request_from_route_payload, create_chapter_payload,
    delete_chapter_payload, get_chapter_payload, list_chapters_by_project_path_payload,
    list_chapters_payload, update_chapter_payload, update_expansion_plan_payload,
    CreateChapterRouteRequest, ListChaptersRouteQuery, UpdateChapterRouteRequest,
    UpdateExpansionPlanRouteRequest,
};
use self::error_mapper::{
    map_chapter_crud_success_message_error, map_list_chapters_by_project_path_payload_error,
    map_project_crud_success_message_error,
};
use crate::api::chapters_error_mapper::{
    map_load_annotations_payload_error, map_load_can_generate_payload_error,
    map_load_navigation_payload_error, map_load_quality_trend_payload_error,
    map_quality_trend_query_request_error,
};
use crate::services::auth::Claims;
use crate::services::chapter_query_service::{
    build_quality_trend_query_request_from_route_query, load_annotations_payload,
    load_can_generate_payload, load_navigation_payload, load_quality_trend_payload,
    QualityTrendRouteQuery,
};

const CHAPTERS_PROJECT_LIST_ROUTE: &str = "/chapters/project/{project_id}";
const CHAPTERS_QUALITY_TREND_ROUTE: &str = "/chapters/project/{project_id}/quality-trend";
const CHAPTERS_NAVIGATION_ROUTE: &str = "/chapters/{chapter_id}/navigation";
const CHAPTERS_EXPANSION_PLAN_ROUTE: &str = "/chapters/{chapter_id}/expansion-plan";
const CHAPTERS_ANNOTATIONS_ROUTE: &str = "/chapters/{chapter_id}/annotations";
const CHAPTERS_CAN_GENERATE_ROUTE: &str = "/chapters/{chapter_id}/can-generate";
const CHAPTERS_LIST_CREATE_ROUTE: &str = "/chapters";
const CHAPTERS_DETAIL_ROUTE: &str = "/chapters/{chapter_id}";

#[cfg(test)]
fn build_chapter_crud_route_owner_contract() -> Value {
    json!({
        "owner": "chapter_crud_routes",
        "rust_owner": "backend-rs/src/api/chapter_crud_routes.rs",
        "route_prefix": "/api",
        "routes": {
            "project_list": CHAPTERS_PROJECT_LIST_ROUTE,
            "quality_trend": CHAPTERS_QUALITY_TREND_ROUTE,
            "navigation": CHAPTERS_NAVIGATION_ROUTE,
            "expansion_plan": CHAPTERS_EXPANSION_PLAN_ROUTE,
            "annotations": CHAPTERS_ANNOTATIONS_ROUTE,
            "can_generate": CHAPTERS_CAN_GENERATE_ROUTE,
            "list": CHAPTERS_LIST_CREATE_ROUTE,
            "create": CHAPTERS_LIST_CREATE_ROUTE,
            "detail": CHAPTERS_DETAIL_ROUTE,
            "update": CHAPTERS_DETAIL_ROUTE,
            "delete": CHAPTERS_DETAIL_ROUTE
        },
        "methods": {
            "project_list": ["GET"],
            "quality_trend": ["GET"],
            "navigation": ["GET"],
            "expansion_plan": ["PUT"],
            "annotations": ["GET"],
            "can_generate": ["GET"],
            "list_create": ["GET", "POST"],
            "detail": ["GET", "PUT", "DELETE"]
        },
        "service_handoffs": {
            "crud_workflow_owner": "private crud_workflow module in backend-rs/src/api/chapter_crud_routes.rs",
            "query_owner": "backend-rs/src/services/chapter_query_service.rs",
            "error_mapping": "private error_mapper module in backend-rs/src/api/chapter_crud_routes.rs",
            "shared_query_error_mapping": "backend-rs/src/api/chapters_error_mapper.rs"
        },
        "request_contract": {
            "create": "project_id/title/chapter_number are route payload fields; optional content/summary/status/outline/sub_index/expansion_plan stay compatible",
            "list": "project_id remains required query input",
            "update": "title/content/summary/status remain optional partial update fields",
            "expansion_plan": "plan remains required route body field",
            "quality_trend": "limit defaults to 12 and must stay within 1..=50"
        },
        "readiness_evidence": [
            "chapters-list-auth-guard-rust",
            "chapters-project-list-auth-guard-rust",
            "chapter-crud-list-logged-in-project-not-found-rust",
            "chapter-crud-project-list-logged-in-project-not-found-rust",
            "chapter-crud-detail-logged-in-not-found-rust",
            "chapter-crud-navigation-logged-in-not-found-rust",
            "chapter-crud-annotations-logged-in-not-found-rust",
            "chapter-crud-can-generate-logged-in-not-found-rust",
            "chapter-crud-quality-trend-logged-in-project-not-found-rust",
            "chapter-crud-fixture-import-project-business-rust",
            "chapter-crud-fixture-list-chapters-business-rust",
            "chapter-crud-project-list-business-rust",
            "chapter-crud-detail-business-rust",
            "chapter-crud-navigation-business-rust",
            "chapter-crud-annotations-business-rust",
            "chapter-crud-can-generate-business-rust",
            "chapter-crud-quality-trend-business-rust"
        ],
        "owner_profile": {
            "name": "phase5-chapter-crud-owner",
            "business_probes": [
                "chapter-crud-list-logged-in-project-not-found-rust",
                "chapter-crud-project-list-logged-in-project-not-found-rust",
                "chapter-crud-detail-logged-in-not-found-rust",
                "chapter-crud-navigation-logged-in-not-found-rust",
                "chapter-crud-annotations-logged-in-not-found-rust",
                "chapter-crud-can-generate-logged-in-not-found-rust",
                "chapter-crud-quality-trend-logged-in-project-not-found-rust",
                "chapter-crud-project-list-business-rust",
                "chapter-crud-detail-business-rust",
                "chapter-crud-navigation-business-rust",
                "chapter-crud-annotations-business-rust",
                "chapter-crud-can-generate-business-rust",
                "chapter-crud-quality-trend-business-rust"
            ],
            "fixture_probes": [
                "chapter-crud-fixture-import-project-business-rust",
                "chapter-crud-fixture-list-chapters-business-rust"
            ],
            "route_readiness_probes": [
                "chapters-list-auth-guard-rust",
                "chapters-project-list-auth-guard-rust"
            ],
            "python_fallback_probe_count": 0,
            "manifest_profile": "phase5-chapter-crud-owner",
            "profile_kind": "successful_result_business_readiness"
        },
        "source_map_files": [],
        "rollback_boundary": {
            "source_map_policy": "chapter_crud_route_service_and_schema_source_maps_deleted_after_explicit_closeout_round",
            "python_route_files_status": "chapter_crud_route_service_and_schema_source_maps_deleted",
            "source_map_freeze_candidate_ready": true,
            "full_module_freeze_ready": true,
            "python_bootstrap_status": "chapter_crud_route_runtime_registration_deleted_no_python_route_shell_remains",
            "python_fallback_removal_ready": true,
            "remaining_blockers": [],
            "freeze_reason": "Rust chapter_crud route owner covers route handlers, private CRUD workflow, private error mapper, auth-guard manifest probes for list/project-list, logged-in not-found probes, and successful fixture-backed business probes for project-list/detail/navigation/annotations/can-generate/quality-trend. The Python chapter CRUD route shells, query source-map file, schema source-map file, and bootstrap rollback registration have all been deleted; rollback now stays at Rust route ownership and deployment policy.",
            "rollback_files": []
        },
        "business_smoke_status": {
            "owner_profile": "phase5-chapter-crud-owner",
            "owner_profile_probe_count": 15,
            "business_probe_count": 13,
            "fixture_probe_count": 2,
            "python_fallback_probe_count": 0,
            "status": "covered_by_dedicated_rust_owner_profile"
        },
        "next_cutover_gate": "chapter CRUD route, query, and schema source maps are physically deleted; remaining work is only broader Python exit outside the chapter CRUD package",
        "migration_policy": "Chapter CRUD route business smoke is covered by phase5-chapter-crud-owner; the Python chapter CRUD route shells, query source-map file, schema source-map file, and explicit bootstrap rollback registration have been physically deleted."
    })
}

mod crud_workflow {
    use std::collections::HashMap;

    use chrono::NaiveDateTime;
    use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter};
    use serde::{Deserialize, Serialize};
    use serde_json::{json, Value};

    use crate::models::{chapter, outline};
    use crate::services::chapter_service::ChapterService;

    #[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
    pub(crate) struct CreateChapterRouteRequest {
        pub(crate) project_id: String,
        pub(crate) title: String,
        pub(crate) chapter_number: i32,
        pub(crate) content: Option<String>,
        pub(crate) summary: Option<String>,
        pub(crate) status: Option<String>,
        pub(crate) outline_id: Option<String>,
        pub(crate) sub_index: Option<i32>,
        pub(crate) expansion_plan: Option<String>,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub(crate) struct CreateChapterRequest {
        project_id: String,
        title: String,
        chapter_number: i32,
        content: Option<String>,
        summary: Option<String>,
        status: Option<String>,
        outline_id: Option<String>,
        sub_index: Option<i32>,
        expansion_plan: Option<String>,
    }

    impl CreateChapterRequest {
        pub(crate) fn new(
            project_id: &str,
            title: &str,
            chapter_number: i32,
            content: Option<&str>,
            summary: Option<&str>,
            status: Option<&str>,
            outline_id: Option<&str>,
            sub_index: Option<i32>,
            expansion_plan: Option<&str>,
        ) -> Self {
            Self {
                project_id: project_id.to_owned(),
                title: title.to_owned(),
                chapter_number,
                content: content.map(str::to_owned),
                summary: summary.map(str::to_owned),
                status: status.map(str::to_owned),
                outline_id: outline_id.map(str::to_owned),
                sub_index,
                expansion_plan: expansion_plan.map(str::to_owned),
            }
        }

        fn from_route_request(route_request: CreateChapterRouteRequest) -> Self {
            Self::new(
                &route_request.project_id,
                &route_request.title,
                route_request.chapter_number,
                route_request.content.as_deref(),
                route_request.summary.as_deref(),
                route_request.status.as_deref(),
                route_request.outline_id.as_deref(),
                route_request.sub_index,
                route_request.expansion_plan.as_deref(),
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

        pub(crate) fn status(&self) -> Option<&str> {
            self.status.as_deref()
        }

        pub(crate) fn outline_id(&self) -> Option<&str> {
            self.outline_id.as_deref()
        }

        pub(crate) fn sub_index(&self) -> Option<i32> {
            self.sub_index
        }

        pub(crate) fn expansion_plan(&self) -> Option<&str> {
            self.expansion_plan.as_deref()
        }
    }

    pub(crate) fn build_create_chapter_request_from_route_payload(
        route_request: CreateChapterRouteRequest,
    ) -> CreateChapterRequest {
        CreateChapterRequest::from_route_request(route_request)
    }

    #[derive(Debug, Clone, Default, Deserialize, PartialEq, Eq)]
    pub(crate) struct UpdateChapterRouteRequest {
        pub(crate) title: Option<String>,
        pub(crate) content: Option<String>,
        pub(crate) summary: Option<String>,
        pub(crate) status: Option<String>,
    }

    #[derive(Debug, Clone, Default, PartialEq, Eq)]
    pub(crate) struct UpdateChapterRequest {
        title: Option<String>,
        content: Option<String>,
        summary: Option<String>,
        status: Option<String>,
    }

    impl UpdateChapterRequest {
        pub(crate) fn new(
            title: Option<&str>,
            content: Option<&str>,
            summary: Option<&str>,
            status: Option<&str>,
        ) -> Self {
            Self {
                title: title.map(str::to_owned),
                content: content.map(str::to_owned),
                summary: summary.map(str::to_owned),
                status: status.map(str::to_owned),
            }
        }

        fn from_route_request(route_request: UpdateChapterRouteRequest) -> Self {
            Self::new(
                route_request.title.as_deref(),
                route_request.content.as_deref(),
                route_request.summary.as_deref(),
                route_request.status.as_deref(),
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
    }

    pub(crate) fn build_update_chapter_request_from_route_payload(
        route_request: UpdateChapterRouteRequest,
    ) -> UpdateChapterRequest {
        UpdateChapterRequest::from_route_request(route_request)
    }

    #[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
    pub(crate) struct UpdateExpansionPlanRouteRequest {
        pub(crate) plan: String,
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

        fn from_route_request(route_request: UpdateExpansionPlanRouteRequest) -> Self {
            Self::new(&route_request.plan)
        }

        pub(crate) fn plan(&self) -> &str {
            &self.plan
        }
    }

    pub(crate) fn build_update_expansion_plan_request_from_route_payload(
        route_request: UpdateExpansionPlanRouteRequest,
    ) -> UpdateExpansionPlanRequest {
        UpdateExpansionPlanRequest::from_route_request(route_request)
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

        fn from_route_query(route_query: ListChaptersRouteQuery) -> Self {
            Self::new(&route_query.project_id)
        }

        pub(crate) fn project_id(&self) -> &str {
            &self.project_id
        }
    }

    #[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
    pub(crate) struct ListChaptersRouteQuery {
        pub(crate) project_id: String,
    }

    pub(crate) fn build_list_chapters_request_from_route_query(
        route_query: ListChaptersRouteQuery,
    ) -> ListChaptersRequest {
        ListChaptersRequest::from_route_query(route_query)
    }

    #[derive(Debug, Clone, Serialize)]
    struct ChapterResponseItem {
        id: String,
        project_id: String,
        chapter_number: i32,
        title: String,
        content: Option<String>,
        summary: Option<String>,
        word_count: i32,
        status: String,
        outline_id: Option<String>,
        sub_index: i32,
        expansion_plan: Option<String>,
        outline_title: Option<String>,
        outline_order: Option<i32>,
        created_at: NaiveDateTime,
        updated_at: Option<NaiveDateTime>,
    }

    fn chapter_response_item(
        chapter: &chapter::Model,
        outline: Option<&outline::Model>,
    ) -> ChapterResponseItem {
        ChapterResponseItem {
            id: chapter.id.clone(),
            project_id: chapter.project_id.clone(),
            chapter_number: chapter.chapter_number,
            title: chapter.title.clone(),
            content: chapter.content.clone(),
            summary: chapter.summary.clone(),
            word_count: chapter.word_count,
            status: chapter.status.clone(),
            outline_id: chapter.outline_id.clone(),
            sub_index: chapter.sub_index,
            expansion_plan: chapter.expansion_plan.clone(),
            outline_title: outline.map(|item| item.title.clone()),
            outline_order: outline.and_then(|item| item.order_index),
            created_at: chapter.created_at,
            updated_at: chapter.updated_at,
        }
    }

    fn chapter_response_items(
        chapters: &[chapter::Model],
        outlines: &HashMap<String, outline::Model>,
    ) -> Vec<ChapterResponseItem> {
        chapters
            .iter()
            .map(|chapter| {
                let outline = chapter
                    .outline_id
                    .as_deref()
                    .and_then(|outline_id| outlines.get(outline_id));
                chapter_response_item(chapter, outline)
            })
            .collect()
    }

    fn serialize_value<T: Serialize + ?Sized>(value: &T, fallback: Value) -> Value {
        serde_json::to_value(value).unwrap_or(fallback)
    }

    fn compatible_chapter_list_payload(chapters: &[chapter::Model]) -> Value {
        let outlines = HashMap::new();
        let chapter_items = chapter_response_items(chapters, &outlines);
        let items = serialize_value(&chapter_items, json!([]));
        json!({
            "success": true,
            "data": items.clone(),
            "items": items,
            "total": chapters.len()
        })
    }

    #[cfg(test)]
    fn project_path_chapter_list_payload(chapters: &[chapter::Model]) -> Value {
        let items = serialize_value(chapters, json!([]));
        json!({
            "items": items,
            "total": chapters.len()
        })
    }

    fn project_path_chapter_list_payload_with_outlines(
        chapters: &[chapter::Model],
        outlines: &HashMap<String, outline::Model>,
    ) -> Value {
        let items = chapter_response_items(chapters, outlines);
        json!({
            "items": serialize_value(&items, json!([])),
            "total": chapters.len()
        })
    }

    fn compatible_chapter_payload(chapter: chapter::Model) -> Value {
        let chapter_item = chapter_response_item(&chapter, None);
        let chapter_value = serialize_value(&chapter_item, json!({}));
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
            request.status(),
            request.outline_id(),
            request.sub_index(),
            request.expansion_plan(),
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
            Ok(Some(chapters)) => {
                let outline_ids = chapters
                    .iter()
                    .filter_map(|chapter| chapter.outline_id.clone())
                    .collect::<Vec<_>>();
                let outlines = if outline_ids.is_empty() {
                    HashMap::new()
                } else {
                    outline::Entity::find()
                        .filter(outline::Column::Id.is_in(outline_ids))
                        .all(db)
                        .await
                        .map_err(|error| ProjectCrudPayloadError::Internal(error.to_string()))?
                        .into_iter()
                        .map(|outline| (outline.id.clone(), outline))
                        .collect::<HashMap<_, _>>()
                };
                Ok(project_path_chapter_list_payload_with_outlines(
                    &chapters, &outlines,
                ))
            }
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
        use std::collections::HashMap;

        use chrono::NaiveDateTime;
        use serde_json::json;

        use crate::models::{chapter, outline};

        use super::{
            build_create_chapter_request_from_route_payload,
            build_list_chapters_request_from_route_query,
            build_update_chapter_request_from_route_payload,
            build_update_expansion_plan_request_from_route_payload,
            compatible_chapter_list_payload, compatible_chapter_payload,
            project_path_chapter_list_payload, project_path_chapter_list_payload_with_outlines,
            ChapterCrudNotFound, ChapterCrudPayloadError, CreateChapterRequest,
            CreateChapterRouteRequest, CrudPayloadError, ListChaptersRouteQuery,
            ProjectCrudNotFound, ProjectCrudPayloadError, UpdateChapterRequest,
            UpdateChapterRouteRequest, UpdateExpansionPlanRequest, UpdateExpansionPlanRouteRequest,
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

        fn chapter_model_with_outline(
            id: &str,
            number: i32,
            outline_id: Option<&str>,
        ) -> chapter::Model {
            chapter::Model {
                outline_id: outline_id.map(str::to_string),
                ..chapter_model(id, number)
            }
        }

        fn outline_model(id: &str, title: &str, order_index: Option<i32>) -> outline::Model {
            outline::Model {
                id: id.to_string(),
                project_id: "project-1".to_string(),
                title: title.to_string(),
                content: Some("大纲内容".to_string()),
                structure: None,
                order_index,
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
            assert!(payload["data"][0]["outline_title"].is_null());
            assert!(payload["data"][0]["outline_order"].is_null());
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
        fn should_build_project_path_chapter_list_payload_with_outline_metadata_like_python() {
            let chapters = vec![
                chapter_model_with_outline("chapter-1", 1, Some("outline-1")),
                chapter_model_with_outline("chapter-2", 2, None),
            ];
            let outlines = HashMap::from([(
                "outline-1".to_string(),
                outline_model("outline-1", "第一节大纲", Some(7)),
            )]);

            let payload = project_path_chapter_list_payload_with_outlines(&chapters, &outlines);

            assert_eq!(payload["total"], 2);
            assert_eq!(payload["items"][0]["id"], "chapter-1");
            assert_eq!(payload["items"][0]["outline_id"], "outline-1");
            assert_eq!(payload["items"][0]["outline_title"], "第一节大纲");
            assert_eq!(payload["items"][0]["outline_order"], 7);
            assert_eq!(payload["items"][1]["id"], "chapter-2");
            assert!(payload["items"][1]["outline_title"].is_null());
            assert!(payload["items"][1]["outline_order"].is_null());
            assert!(payload.get("success").is_none());
            assert!(payload.get("data").is_none());
        }

        #[test]
        fn should_build_compatible_chapter_payload() {
            let payload = compatible_chapter_payload(chapter_model("chapter-1", 1));

            assert_eq!(payload["success"], true);
            assert_eq!(payload["id"], "chapter-1");
            assert_eq!(payload["title"], "第1章");
            assert!(payload["outline_title"].is_null());
            assert!(payload["outline_order"].is_null());
            assert_eq!(payload["data"]["id"], "chapter-1");
            assert_eq!(payload["data"]["title"], "第1章");
            assert!(payload["data"]["outline_title"].is_null());
            assert!(payload["data"]["outline_order"].is_null());
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
                Some("writing"),
                Some("outline-1"),
                Some(2),
                Some("{\"key_events\":[]}"),
            );

            assert_eq!(request.project_id(), "project-1");
            assert_eq!(request.title(), "第一章");
            assert_eq!(request.chapter_number(), 1);
            assert_eq!(request.content(), Some("正文"));
            assert_eq!(request.summary(), Some("摘要"));
            assert_eq!(request.status(), Some("writing"));
            assert_eq!(request.outline_id(), Some("outline-1"));
            assert_eq!(request.sub_index(), Some(2));
            assert_eq!(request.expansion_plan(), Some("{\"key_events\":[]}"));
        }

        #[test]
        fn should_build_create_chapter_request_from_route_payload() {
            let request =
                build_create_chapter_request_from_route_payload(CreateChapterRouteRequest {
                    project_id: "project-1".to_string(),
                    title: "第一章".to_string(),
                    chapter_number: 1,
                    content: Some("正文".to_string()),
                    summary: Some("摘要".to_string()),
                    status: Some("writing".to_string()),
                    outline_id: Some("outline-1".to_string()),
                    sub_index: Some(2),
                    expansion_plan: Some("{\"key_events\":[]}".to_string()),
                });

            assert_eq!(request.project_id(), "project-1");
            assert_eq!(request.title(), "第一章");
            assert_eq!(request.chapter_number(), 1);
            assert_eq!(request.content(), Some("正文"));
            assert_eq!(request.summary(), Some("摘要"));
            assert_eq!(request.status(), Some("writing"));
            assert_eq!(request.outline_id(), Some("outline-1"));
            assert_eq!(request.sub_index(), Some(2));
            assert_eq!(request.expansion_plan(), Some("{\"key_events\":[]}"));
        }

        #[test]
        fn should_build_update_chapter_request() {
            let request =
                UpdateChapterRequest::new(Some("新标题"), None, Some("新摘要"), Some("draft"));

            assert_eq!(request.title(), Some("新标题"));
            assert_eq!(request.content(), None);
            assert_eq!(request.summary(), Some("新摘要"));
            assert_eq!(request.status(), Some("draft"));
        }

        #[test]
        fn should_build_update_chapter_request_from_route_payload() {
            let request =
                build_update_chapter_request_from_route_payload(UpdateChapterRouteRequest {
                    title: Some("新标题".to_string()),
                    content: None,
                    summary: Some("新摘要".to_string()),
                    status: Some("draft".to_string()),
                });

            assert_eq!(request.title(), Some("新标题"));
            assert_eq!(request.content(), None);
            assert_eq!(request.summary(), Some("新摘要"));
            assert_eq!(request.status(), Some("draft"));
        }

        #[test]
        fn should_ignore_unsupported_update_chapter_fields_like_python_schema() {
            let route_request: UpdateChapterRouteRequest = serde_json::from_value(json!({
                "title": "新标题",
                "chapter_number": 99,
                "expansion_plan": "不应通过通用章节更新入口写入"
            }))
            .expect("unknown update fields should be ignored like Python ChapterUpdate");

            let request = build_update_chapter_request_from_route_payload(route_request);

            assert_eq!(request.title(), Some("新标题"));
            assert_eq!(request.content(), None);
            assert_eq!(request.summary(), None);
            assert_eq!(request.status(), None);
        }

        #[test]
        fn should_build_update_expansion_plan_request() {
            let request = UpdateExpansionPlanRequest::new("保持节奏，补足冲突");

            assert_eq!(request.plan(), "保持节奏，补足冲突");
        }

        #[test]
        fn should_build_update_expansion_plan_request_from_route_payload() {
            let request = build_update_expansion_plan_request_from_route_payload(
                UpdateExpansionPlanRouteRequest {
                    plan: "保持节奏，补足冲突".to_string(),
                },
            );

            assert_eq!(request.plan(), "保持节奏，补足冲突");
        }

        #[test]
        fn should_build_list_chapters_request_from_route_payload() {
            let request = build_list_chapters_request_from_route_query(ListChaptersRouteQuery {
                project_id: "project-1".to_string(),
            });

            assert_eq!(request.project_id(), "project-1");
        }
    }
}

mod error_mapper {
    use axum::{http::StatusCode, Json};
    use serde_json::{json, Value};

    use super::crud_workflow::{
        ChapterCrudPayloadError, CrudPayloadError, ListChaptersByProjectPathPayloadError,
        ProjectCrudNotFound, ProjectCrudPayloadError,
    };

    type ChapterCrudRouteError = (StatusCode, Json<Value>);

    fn success_message_error(
        status: StatusCode,
        message: impl Into<String>,
    ) -> ChapterCrudRouteError {
        (
            status,
            Json(json!({
                "success": false,
                "message": message.into(),
            })),
        )
    }

    fn detail_error(status: StatusCode, detail: impl Into<String>) -> ChapterCrudRouteError {
        (status, Json(json!({ "detail": detail.into() })))
    }

    fn internal_success_message_error(detail: impl Into<String>) -> ChapterCrudRouteError {
        success_message_error(StatusCode::INTERNAL_SERVER_ERROR, detail)
    }

    fn project_not_found_or_access_denied_message_error() -> ChapterCrudRouteError {
        success_message_error(StatusCode::NOT_FOUND, "Project not found or access denied")
    }

    fn chapter_not_found_or_access_denied_message_error() -> ChapterCrudRouteError {
        success_message_error(StatusCode::NOT_FOUND, "Chapter not found or access denied")
    }

    fn map_crud_success_message_error<TNotFound>(
        error: CrudPayloadError<TNotFound>,
        not_found_error: impl FnOnce(TNotFound) -> ChapterCrudRouteError,
    ) -> ChapterCrudRouteError {
        match error {
            CrudPayloadError::NotFound(not_found) => not_found_error(not_found),
            CrudPayloadError::Internal(detail) => internal_success_message_error(detail),
        }
    }

    pub(super) fn map_project_crud_success_message_error(
        error: ProjectCrudPayloadError,
    ) -> ChapterCrudRouteError {
        map_crud_success_message_error(error, |_| {
            project_not_found_or_access_denied_message_error()
        })
    }

    pub(super) fn map_chapter_crud_success_message_error(
        error: ChapterCrudPayloadError,
    ) -> ChapterCrudRouteError {
        map_crud_success_message_error(error, |_| {
            chapter_not_found_or_access_denied_message_error()
        })
    }

    pub(super) fn map_list_chapters_by_project_path_payload_error(
        error: ListChaptersByProjectPathPayloadError,
    ) -> ChapterCrudRouteError {
        match error {
            CrudPayloadError::NotFound(ProjectCrudNotFound::ProjectNotFound) => {
                detail_error(StatusCode::NOT_FOUND, "Project not found")
            }
            CrudPayloadError::Internal(detail) => {
                detail_error(StatusCode::INTERNAL_SERVER_ERROR, detail)
            }
        }
    }

    #[cfg(test)]
    mod tests {
        use super::super::crud_workflow::{
            ChapterCrudNotFound, CrudPayloadError, ProjectCrudNotFound,
        };
        use super::{
            map_chapter_crud_success_message_error,
            map_list_chapters_by_project_path_payload_error,
            map_project_crud_success_message_error,
        };
        use axum::http::StatusCode;
        use serde_json::json;

        #[test]
        fn project_crud_success_message_owner_keeps_not_found_shape() {
            let response = map_project_crud_success_message_error(CrudPayloadError::NotFound(
                ProjectCrudNotFound::ProjectNotFound,
            ));

            assert_eq!(response.0, StatusCode::NOT_FOUND);
            assert_eq!(
                response.1 .0,
                json!({ "success": false, "message": "Project not found or access denied" })
            );
        }

        #[test]
        fn chapter_crud_success_message_owner_keeps_not_found_shape() {
            let response = map_chapter_crud_success_message_error(CrudPayloadError::NotFound(
                ChapterCrudNotFound::ChapterNotFound,
            ));

            assert_eq!(response.0, StatusCode::NOT_FOUND);
            assert_eq!(
                response.1 .0,
                json!({ "success": false, "message": "Chapter not found or access denied" })
            );
        }

        #[test]
        fn create_chapter_project_not_found_keeps_success_message_shape() {
            let response = map_project_crud_success_message_error(
                super::super::crud_workflow::CreateChapterPayloadError::NotFound(
                    ProjectCrudNotFound::ProjectNotFound,
                ),
            );

            assert_eq!(response.0, StatusCode::NOT_FOUND);
            assert_eq!(
                response.1 .0,
                json!({ "success": false, "message": "Project not found or access denied" })
            );
        }

        #[test]
        fn list_chapters_by_project_path_project_not_found_keeps_detail_shape() {
            let response = map_list_chapters_by_project_path_payload_error(
                CrudPayloadError::NotFound(ProjectCrudNotFound::ProjectNotFound),
            );

            assert_eq!(response.0, StatusCode::NOT_FOUND);
            assert_eq!(response.1 .0, json!({ "detail": "Project not found" }));
        }

        #[test]
        fn get_chapter_not_found_keeps_success_message_shape() {
            let response = map_chapter_crud_success_message_error(
                super::super::crud_workflow::GetChapterPayloadError::NotFound(
                    ChapterCrudNotFound::ChapterNotFound,
                ),
            );

            assert_eq!(response.0, StatusCode::NOT_FOUND);
            assert_eq!(
                response.1 .0,
                json!({ "success": false, "message": "Chapter not found or access denied" })
            );
        }
    }
}

async fn create_chapter(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Json(body): Json<CreateChapterRouteRequest>,
) -> Result<(StatusCode, Json<Value>), (StatusCode, Json<Value>)> {
    let request = build_create_chapter_request_from_route_payload(body);
    let payload = create_chapter_payload(&db, &claims.sub, &request)
        .await
        .map_err(map_project_crud_success_message_error)?;
    Ok((StatusCode::CREATED, Json(payload)))
}

async fn list_chapters(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Query(query): Query<ListChaptersRouteQuery>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let request = build_list_chapters_request_from_route_query(query);
    let payload = list_chapters_payload(&db, &request, &claims.sub)
        .await
        .map_err(map_project_crud_success_message_error)?;
    Ok(Json(payload))
}

async fn list_chapters_by_project_path(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Path(project_id): Path<String>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let payload = list_chapters_by_project_path_payload(&db, &project_id, &claims.sub)
        .await
        .map_err(map_list_chapters_by_project_path_payload_error)?;
    Ok(Json(payload))
}

async fn get_chapter(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Path(chapter_id): Path<String>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let payload = get_chapter_payload(&db, &chapter_id, &claims.sub)
        .await
        .map_err(map_chapter_crud_success_message_error)?;
    Ok(Json(payload))
}

async fn update_chapter(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Path(chapter_id): Path<String>,
    Json(body): Json<UpdateChapterRouteRequest>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let request = build_update_chapter_request_from_route_payload(body);
    let payload = update_chapter_payload(&db, &chapter_id, &claims.sub, &request)
        .await
        .map_err(map_chapter_crud_success_message_error)?;
    Ok(Json(payload))
}

async fn delete_chapter(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Path(chapter_id): Path<String>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let payload = delete_chapter_payload(&db, &chapter_id, &claims.sub)
        .await
        .map_err(map_chapter_crud_success_message_error)?;
    Ok(Json(payload))
}

async fn get_navigation(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Path(chapter_id): Path<String>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let payload = load_navigation_payload(&db, &chapter_id, &claims.sub)
        .await
        .map_err(map_load_navigation_payload_error)?;
    Ok(Json(payload))
}

async fn update_expansion_plan(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Path(chapter_id): Path<String>,
    Json(body): Json<UpdateExpansionPlanRouteRequest>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let request = build_update_expansion_plan_request_from_route_payload(body);
    let payload = update_expansion_plan_payload(&db, &chapter_id, &claims.sub, &request)
        .await
        .map_err(map_chapter_crud_success_message_error)?;
    Ok(Json(payload))
}

async fn get_annotations(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Path(chapter_id): Path<String>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let payload = load_annotations_payload(&db, &chapter_id, &claims.sub)
        .await
        .map_err(map_load_annotations_payload_error)?;
    Ok(Json(payload))
}

async fn get_quality_trend(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Path(project_id): Path<String>,
    Query(query): Query<QualityTrendRouteQuery>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let request = build_quality_trend_query_request_from_route_query(query)
        .map_err(map_quality_trend_query_request_error)?;
    let payload = load_quality_trend_payload(&db, &project_id, &claims.sub, request)
        .await
        .map_err(map_load_quality_trend_payload_error)?;
    Ok(Json(payload))
}

async fn get_can_generate(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Path(chapter_id): Path<String>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let payload = load_can_generate_payload(&db, &chapter_id, &claims.sub)
        .await
        .map_err(map_load_can_generate_payload_error)?;
    Ok(Json(payload))
}

pub(crate) fn routes() -> Router {
    Router::new()
        .route(
            CHAPTERS_PROJECT_LIST_ROUTE,
            get(list_chapters_by_project_path),
        )
        .route(CHAPTERS_QUALITY_TREND_ROUTE, get(get_quality_trend))
        .route(CHAPTERS_NAVIGATION_ROUTE, get(get_navigation))
        .route(
            CHAPTERS_EXPANSION_PLAN_ROUTE,
            axum::routing::put(update_expansion_plan),
        )
        .route(CHAPTERS_ANNOTATIONS_ROUTE, get(get_annotations))
        .route(CHAPTERS_CAN_GENERATE_ROUTE, get(get_can_generate))
        .route(
            CHAPTERS_LIST_CREATE_ROUTE,
            axum::routing::get(list_chapters).post(create_chapter),
        )
        .route(
            CHAPTERS_DETAIL_ROUTE,
            get(get_chapter).put(update_chapter).delete(delete_chapter),
        )
}

#[cfg(test)]
mod tests {
    use super::crud_workflow::{
        build_create_chapter_request_from_route_payload,
        build_list_chapters_request_from_route_query,
        build_update_chapter_request_from_route_payload,
        build_update_expansion_plan_request_from_route_payload, CreateChapterRouteRequest,
        ListChaptersRouteQuery, UpdateChapterRouteRequest, UpdateExpansionPlanRouteRequest,
    };
    use super::{
        build_chapter_crud_route_owner_contract, CHAPTERS_ANNOTATIONS_ROUTE,
        CHAPTERS_CAN_GENERATE_ROUTE, CHAPTERS_DETAIL_ROUTE, CHAPTERS_EXPANSION_PLAN_ROUTE,
        CHAPTERS_LIST_CREATE_ROUTE, CHAPTERS_NAVIGATION_ROUTE, CHAPTERS_PROJECT_LIST_ROUTE,
        CHAPTERS_QUALITY_TREND_ROUTE,
    };
    use crate::services::chapter_query_service::{
        build_quality_trend_query_request_from_route_query, QualityTrendQueryRequestError,
        QualityTrendRouteQuery,
    };
    use serde_json::json;

    #[test]
    fn should_publish_chapter_crud_route_owner_contract() {
        let contract = build_chapter_crud_route_owner_contract();

        assert_eq!(contract["owner"], "chapter_crud_routes");
        assert_eq!(
            contract["rust_owner"],
            "backend-rs/src/api/chapter_crud_routes.rs"
        );
        assert_eq!(
            contract["service_handoffs"]["error_mapping"],
            "private error_mapper module in backend-rs/src/api/chapter_crud_routes.rs"
        );
        assert_eq!(
            contract["routes"]["project_list"],
            CHAPTERS_PROJECT_LIST_ROUTE
        );
        assert_eq!(contract["routes"]["list"], CHAPTERS_LIST_CREATE_ROUTE);
        assert_eq!(contract["routes"]["detail"], CHAPTERS_DETAIL_ROUTE);
        assert_eq!(
            contract["request_contract"]["quality_trend"],
            "limit defaults to 12 and must stay within 1..=50"
        );
        assert_eq!(
            contract["readiness_evidence"][1],
            "chapters-project-list-auth-guard-rust"
        );
        assert_eq!(
            contract["owner_profile"]["name"],
            "phase5-chapter-crud-owner"
        );
        assert_eq!(
            contract["readiness_evidence"][2],
            "chapter-crud-list-logged-in-project-not-found-rust"
        );
        assert_eq!(
            contract["owner_profile"]["route_readiness_probes"][1],
            "chapters-project-list-auth-guard-rust"
        );
        assert_eq!(
            contract["owner_profile"]["python_fallback_probe_count"],
            json!(0)
        );
        assert!(contract["owner_profile"]["business_probes"]
            .as_array()
            .expect("business probes should be an array")
            .contains(&json!("chapter-crud-detail-logged-in-not-found-rust")));
        assert!(contract["owner_profile"]["business_probes"]
            .as_array()
            .expect("business probes should be an array")
            .contains(&json!("chapter-crud-detail-business-rust")));
        assert_eq!(
            contract["owner_profile"]["fixture_probes"],
            json!([
                "chapter-crud-fixture-import-project-business-rust",
                "chapter-crud-fixture-list-chapters-business-rust"
            ])
        );
        assert_eq!(
            contract["owner_profile"]["profile_kind"],
            "successful_result_business_readiness"
        );
        assert_eq!(
            contract["owner_profile"]["manifest_profile"],
            "phase5-chapter-crud-owner"
        );
        assert_eq!(
            contract["rollback_boundary"]["source_map_freeze_candidate_ready"],
            true
        );
        assert_eq!(
            contract["rollback_boundary"]["full_module_freeze_ready"],
            true
        );
        assert_eq!(
            contract["rollback_boundary"]["python_bootstrap_status"],
            "chapter_crud_route_runtime_registration_deleted_no_python_route_shell_remains"
        );
        assert_eq!(
            contract["rollback_boundary"]["python_route_files_status"],
            "chapter_crud_route_service_and_schema_source_maps_deleted"
        );
        assert_eq!(
            contract["rollback_boundary"]["python_fallback_removal_ready"],
            true
        );
        assert_eq!(
            contract["rollback_boundary"]["remaining_blockers"],
            json!([])
        );
        assert_eq!(contract["source_map_files"], json!([]));
        assert_eq!(
            contract["business_smoke_status"]["status"],
            "covered_by_dedicated_rust_owner_profile"
        );
        assert_eq!(
            contract["business_smoke_status"]["owner_profile_probe_count"],
            json!(15)
        );
        assert_eq!(
            contract["business_smoke_status"]["business_probe_count"],
            json!(13)
        );
        assert_eq!(
            contract["business_smoke_status"]["fixture_probe_count"],
            json!(2)
        );
        assert_eq!(
            contract["next_cutover_gate"],
            "chapter CRUD route, query, and schema source maps are physically deleted; remaining work is only broader Python exit outside the chapter CRUD package"
        );
        assert_eq!(
            contract["migration_policy"],
            "Chapter CRUD route business smoke is covered by phase5-chapter-crud-owner; the Python chapter CRUD route shells, query source-map file, schema source-map file, and explicit bootstrap rollback registration have been physically deleted."
        );
    }

    #[test]
    fn should_keep_chapter_crud_route_paths_stable() {
        assert_eq!(
            json!({
                "project_list": CHAPTERS_PROJECT_LIST_ROUTE,
                "quality_trend": CHAPTERS_QUALITY_TREND_ROUTE,
                "navigation": CHAPTERS_NAVIGATION_ROUTE,
                "expansion_plan": CHAPTERS_EXPANSION_PLAN_ROUTE,
                "annotations": CHAPTERS_ANNOTATIONS_ROUTE,
                "can_generate": CHAPTERS_CAN_GENERATE_ROUTE,
                "list_create": CHAPTERS_LIST_CREATE_ROUTE,
                "detail": CHAPTERS_DETAIL_ROUTE
            }),
            json!({
                "project_list": "/chapters/project/{project_id}",
                "quality_trend": "/chapters/project/{project_id}/quality-trend",
                "navigation": "/chapters/{chapter_id}/navigation",
                "expansion_plan": "/chapters/{chapter_id}/expansion-plan",
                "annotations": "/chapters/{chapter_id}/annotations",
                "can_generate": "/chapters/{chapter_id}/can-generate",
                "list_create": "/chapters",
                "detail": "/chapters/{chapter_id}"
            })
        );
    }

    #[test]
    fn should_build_create_chapter_request_from_route_payload() {
        let request = build_create_chapter_request_from_route_payload(CreateChapterRouteRequest {
            project_id: "project-1".to_string(),
            title: "第一章".to_string(),
            chapter_number: 1,
            content: Some("正文".to_string()),
            summary: Some("摘要".to_string()),
            status: Some("writing".to_string()),
            outline_id: Some("outline-1".to_string()),
            sub_index: Some(2),
            expansion_plan: Some("{\"key_events\":[]}".to_string()),
        });

        assert_eq!(request.project_id(), "project-1");
        assert_eq!(request.title(), "第一章");
        assert_eq!(request.chapter_number(), 1);
        assert_eq!(request.content(), Some("正文"));
        assert_eq!(request.summary(), Some("摘要"));
        assert_eq!(request.status(), Some("writing"));
        assert_eq!(request.outline_id(), Some("outline-1"));
        assert_eq!(request.sub_index(), Some(2));
        assert_eq!(request.expansion_plan(), Some("{\"key_events\":[]}"));
    }

    #[test]
    fn should_build_update_chapter_request_from_route_payload() {
        let request = build_update_chapter_request_from_route_payload(UpdateChapterRouteRequest {
            title: Some("新标题".to_string()),
            content: None,
            summary: Some("新摘要".to_string()),
            status: Some("draft".to_string()),
        });

        assert_eq!(request.title(), Some("新标题"));
        assert_eq!(request.content(), None);
        assert_eq!(request.summary(), Some("新摘要"));
        assert_eq!(request.status(), Some("draft"));
    }

    #[test]
    fn should_build_update_expansion_plan_request_from_route_payload() {
        let request = build_update_expansion_plan_request_from_route_payload(
            UpdateExpansionPlanRouteRequest {
                plan: "保持节奏，补足冲突".to_string(),
            },
        );

        assert_eq!(request.plan(), "保持节奏，补足冲突");
    }

    #[test]
    fn should_build_list_chapters_request_from_route_query() {
        let request = build_list_chapters_request_from_route_query(ListChaptersRouteQuery {
            project_id: "project-1".to_string(),
        });

        assert_eq!(request.project_id(), "project-1");
    }

    #[test]
    fn should_build_quality_trend_request_from_route_query_like_python_route() {
        let default_request =
            build_quality_trend_query_request_from_route_query(QualityTrendRouteQuery {
                limit: None,
            })
            .expect("default limit should match Python route");
        let explicit_request =
            build_quality_trend_query_request_from_route_query(QualityTrendRouteQuery {
                limit: Some(50),
            })
            .expect("upper bound should be accepted");

        assert_eq!(default_request.limit(), 12);
        assert_eq!(explicit_request.limit(), 50);
        assert_eq!(
            build_quality_trend_query_request_from_route_query(QualityTrendRouteQuery {
                limit: Some(0),
            }),
            Err(QualityTrendQueryRequestError::LimitTooSmall)
        );
        assert_eq!(
            build_quality_trend_query_request_from_route_query(QualityTrendRouteQuery {
                limit: Some(51),
            }),
            Err(QualityTrendQueryRequestError::LimitTooLarge)
        );
    }
}
