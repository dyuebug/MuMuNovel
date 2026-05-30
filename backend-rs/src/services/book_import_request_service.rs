use serde::Deserialize;
use serde_json::{json, Value};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BuildBookImportCreateTaskRequestError {
    MissingFile,
    EmptyFileContent,
    UnsupportedFileType,
    UnsupportedImportMode,
    ProjectIdNotSupported,
    ExistingProjectImportNotSupported,
    FileTooLarge,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BookImportCreateTaskRequest {
    filename: String,
    file_content: Vec<u8>,
    import_mode: String,
}

impl BookImportCreateTaskRequest {
    pub fn filename(&self) -> &str {
        self.filename.as_str()
    }

    pub fn into_file_content(self) -> Vec<u8> {
        self.file_content
    }

    pub fn import_mode(&self) -> &str {
        self.import_mode.as_str()
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BookImportCreateTaskRouteFields {
    pub file_content: Option<Vec<u8>>,
    pub filename: Option<String>,
    pub project_id: Option<String>,
    pub create_new_project: bool,
    pub import_mode: String,
}

impl BookImportCreateTaskRouteFields {
    pub fn new() -> Self {
        Self {
            file_content: None,
            filename: None,
            project_id: None,
            create_new_project: true,
            import_mode: "append".to_string(),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct BookImportApplyRequest {
    project_suggestion: Value,
    chapters: Vec<Value>,
    outlines: Vec<Value>,
    import_mode: String,
}

impl BookImportApplyRequest {
    pub fn project_suggestion(&self) -> &Value {
        &self.project_suggestion
    }

    pub fn chapters(&self) -> &[Value] {
        self.chapters.as_slice()
    }

    pub fn outlines(&self) -> &[Value] {
        self.outlines.as_slice()
    }

    pub fn import_mode(&self) -> &str {
        self.import_mode.as_str()
    }
}

#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
pub struct BookImportApplyRouteRequest {
    #[serde(default)]
    pub project_suggestion: Option<Value>,
    #[serde(default)]
    pub chapters: Option<Value>,
    #[serde(default)]
    pub outlines: Option<Value>,
    #[serde(default)]
    pub import_mode: Option<Value>,
}

impl BookImportApplyRouteRequest {
    fn into_body(self) -> Value {
        json!({
            "project_suggestion": self.project_suggestion,
            "chapters": self.chapters,
            "outlines": self.outlines,
            "import_mode": self.import_mode,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BookImportRetryRequest {
    steps: Vec<String>,
}

impl BookImportRetryRequest {
    pub fn steps(&self) -> &[String] {
        self.steps.as_slice()
    }
}

pub fn build_book_import_create_task_request_from_route_fields(
    fields: BookImportCreateTaskRouteFields,
    max_txt_size: usize,
) -> Result<BookImportCreateTaskRequest, BuildBookImportCreateTaskRequestError> {
    let filename = fields
        .filename
        .ok_or(BuildBookImportCreateTaskRequestError::MissingFile)?;

    if !filename.to_lowercase().ends_with(".txt") {
        return Err(BuildBookImportCreateTaskRequestError::UnsupportedFileType);
    }

    if fields.import_mode != "append" && fields.import_mode != "overwrite" {
        return Err(BuildBookImportCreateTaskRequestError::UnsupportedImportMode);
    }

    if fields.project_id.is_some() {
        return Err(BuildBookImportCreateTaskRequestError::ProjectIdNotSupported);
    }

    if !fields.create_new_project {
        return Err(BuildBookImportCreateTaskRequestError::ExistingProjectImportNotSupported);
    }

    let file_content = fields
        .file_content
        .ok_or(BuildBookImportCreateTaskRequestError::EmptyFileContent)?;

    if file_content.is_empty() {
        return Err(BuildBookImportCreateTaskRequestError::EmptyFileContent);
    }

    if file_content.len() > max_txt_size {
        return Err(BuildBookImportCreateTaskRequestError::FileTooLarge);
    }

    Ok(BookImportCreateTaskRequest {
        filename,
        file_content,
        import_mode: fields.import_mode,
    })
}

pub fn build_book_import_apply_request_from_route_body(body: &Value) -> BookImportApplyRequest {
    BookImportApplyRequest {
        project_suggestion: body
            .get("project_suggestion")
            .cloned()
            .unwrap_or(Value::Null),
        chapters: body
            .get("chapters")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default(),
        outlines: body
            .get("outlines")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default(),
        import_mode: body
            .get("import_mode")
            .and_then(Value::as_str)
            .unwrap_or("append")
            .to_string(),
    }
}

pub fn build_book_import_apply_request_from_route_payload(
    route_request: BookImportApplyRouteRequest,
) -> BookImportApplyRequest {
    build_book_import_apply_request_from_route_body(&route_request.into_body())
}

pub fn build_book_import_retry_request_from_route_body(body: &Value) -> BookImportRetryRequest {
    BookImportRetryRequest {
        steps: body
            .get("steps")
            .and_then(Value::as_array)
            .map(|items| {
                items
                    .iter()
                    .filter_map(|value| value.as_str().map(ToString::to_string))
                    .collect()
            })
            .unwrap_or_default(),
    }
}

#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
pub struct BookImportRetryRouteRequest {
    #[serde(default)]
    pub steps: Option<Value>,
}

impl BookImportRetryRouteRequest {
    fn into_body(self) -> Value {
        json!({
            "steps": self.steps,
        })
    }
}

pub fn build_book_import_retry_request_from_route_payload(
    route_request: BookImportRetryRouteRequest,
) -> BookImportRetryRequest {
    build_book_import_retry_request_from_route_body(&route_request.into_body())
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{
        build_book_import_apply_request_from_route_body,
        build_book_import_apply_request_from_route_payload,
        build_book_import_create_task_request_from_route_fields,
        build_book_import_retry_request_from_route_body,
        build_book_import_retry_request_from_route_payload, BookImportApplyRouteRequest,
        BookImportCreateTaskRouteFields, BookImportRetryRouteRequest,
        BuildBookImportCreateTaskRequestError,
    };

    const MAX_TXT_SIZE: usize = 50 * 1024 * 1024;

    #[test]
    fn build_book_import_create_task_request_from_route_fields_keeps_valid_fields() {
        let request = build_book_import_create_task_request_from_route_fields(
            BookImportCreateTaskRouteFields {
                file_content: Some(vec![1, 2, 3]),
                filename: Some("novel.txt".to_string()),
                project_id: None,
                create_new_project: true,
                import_mode: "overwrite".to_string(),
            },
            MAX_TXT_SIZE,
        )
        .expect("valid create task request should build");

        assert_eq!(request.filename(), "novel.txt");
        assert_eq!(request.clone().into_file_content(), vec![1, 2, 3]);
        assert_eq!(request.import_mode(), "overwrite");
    }

    #[test]
    fn build_book_import_create_task_request_from_route_fields_rejects_missing_file() {
        let error = build_book_import_create_task_request_from_route_fields(
            BookImportCreateTaskRouteFields::new(),
            MAX_TXT_SIZE,
        )
        .expect_err("missing file should fail");

        assert_eq!(error, BuildBookImportCreateTaskRequestError::MissingFile);
    }

    #[test]
    fn build_book_import_create_task_request_from_route_fields_rejects_empty_content() {
        let error = build_book_import_create_task_request_from_route_fields(
            BookImportCreateTaskRouteFields {
                file_content: Some(vec![]),
                filename: Some("novel.txt".to_string()),
                project_id: None,
                create_new_project: true,
                import_mode: "append".to_string(),
            },
            MAX_TXT_SIZE,
        )
        .expect_err("empty content should fail");

        assert_eq!(
            error,
            BuildBookImportCreateTaskRequestError::EmptyFileContent
        );
    }

    #[test]
    fn build_book_import_create_task_request_from_route_fields_rejects_unsupported_type() {
        let error = build_book_import_create_task_request_from_route_fields(
            BookImportCreateTaskRouteFields {
                file_content: Some(vec![1]),
                filename: Some("novel.md".to_string()),
                project_id: None,
                create_new_project: true,
                import_mode: "append".to_string(),
            },
            MAX_TXT_SIZE,
        )
        .expect_err("unsupported file type should fail");

        assert_eq!(
            error,
            BuildBookImportCreateTaskRequestError::UnsupportedFileType
        );
    }

    #[test]
    fn build_book_import_create_task_request_from_route_fields_rejects_invalid_import_mode() {
        let error = build_book_import_create_task_request_from_route_fields(
            BookImportCreateTaskRouteFields {
                file_content: Some(vec![1]),
                filename: Some("novel.txt".to_string()),
                project_id: None,
                create_new_project: true,
                import_mode: "replace".to_string(),
            },
            MAX_TXT_SIZE,
        )
        .expect_err("unsupported import mode should fail");

        assert_eq!(
            error,
            BuildBookImportCreateTaskRequestError::UnsupportedImportMode
        );
    }

    #[test]
    fn build_book_import_create_task_request_from_route_fields_rejects_project_id() {
        let error = build_book_import_create_task_request_from_route_fields(
            BookImportCreateTaskRouteFields {
                file_content: Some(vec![1]),
                filename: Some("novel.txt".to_string()),
                project_id: Some("project-1".to_string()),
                create_new_project: true,
                import_mode: "append".to_string(),
            },
            MAX_TXT_SIZE,
        )
        .expect_err("project_id should fail");

        assert_eq!(
            error,
            BuildBookImportCreateTaskRequestError::ProjectIdNotSupported
        );
    }

    #[test]
    fn build_book_import_create_task_request_from_route_fields_rejects_non_new_project_mode() {
        let error = build_book_import_create_task_request_from_route_fields(
            BookImportCreateTaskRouteFields {
                file_content: Some(vec![1]),
                filename: Some("novel.txt".to_string()),
                project_id: None,
                create_new_project: false,
                import_mode: "append".to_string(),
            },
            MAX_TXT_SIZE,
        )
        .expect_err("existing project import should fail");

        assert_eq!(
            error,
            BuildBookImportCreateTaskRequestError::ExistingProjectImportNotSupported
        );
    }

    #[test]
    fn build_book_import_create_task_request_from_route_fields_rejects_large_file() {
        let error = build_book_import_create_task_request_from_route_fields(
            BookImportCreateTaskRouteFields {
                file_content: Some(vec![1, 2, 3, 4]),
                filename: Some("novel.txt".to_string()),
                project_id: None,
                create_new_project: true,
                import_mode: "append".to_string(),
            },
            3,
        )
        .expect_err("oversized file should fail");

        assert_eq!(error, BuildBookImportCreateTaskRequestError::FileTooLarge);
    }

    #[test]
    fn build_book_import_apply_request_from_route_body_keeps_payload_fields() {
        let request = build_book_import_apply_request_from_route_body(&json!({
            "project_suggestion": {
                "title": "项目标题"
            },
            "chapters": [{"title": "第一章"}],
            "outlines": [{"title": "第一节"}],
            "import_mode": "overwrite"
        }));

        assert_eq!(request.project_suggestion(), &json!({"title": "项目标题"}));
        assert_eq!(request.chapters(), &[json!({"title": "第一章"})]);
        assert_eq!(request.outlines(), &[json!({"title": "第一节"})]);
        assert_eq!(request.import_mode(), "overwrite");
    }

    #[test]
    fn build_book_import_apply_request_from_route_body_uses_existing_defaults() {
        let request = build_book_import_apply_request_from_route_body(&json!({}));

        assert!(request.project_suggestion().is_null());
        assert!(request.chapters().is_empty());
        assert!(request.outlines().is_empty());
        assert_eq!(request.import_mode(), "append");
    }

    #[test]
    fn build_book_import_apply_request_from_route_payload_keeps_existing_contract() {
        let request =
            build_book_import_apply_request_from_route_payload(BookImportApplyRouteRequest {
                project_suggestion: Some(json!({"title": "项目标题"})),
                chapters: Some(json!([{"title": "第一章"}])),
                outlines: Some(json!([{"title": "第一节"}])),
                import_mode: Some(json!("overwrite")),
            });

        assert_eq!(request.project_suggestion(), &json!({"title": "项目标题"}));
        assert_eq!(request.chapters(), &[json!({"title": "第一章"})]);
        assert_eq!(request.outlines(), &[json!({"title": "第一节"})]);
        assert_eq!(request.import_mode(), "overwrite");
    }

    #[test]
    fn build_book_import_retry_request_from_route_body_filters_non_string_steps() {
        let request = build_book_import_retry_request_from_route_body(&json!({
            "steps": ["parse", 3, "import", null, true]
        }));

        assert_eq!(
            request.steps(),
            &["parse".to_string(), "import".to_string()]
        );
    }

    #[test]
    fn build_book_import_retry_request_from_route_body_defaults_to_empty_steps() {
        let request = build_book_import_retry_request_from_route_body(&json!({
            "steps": "invalid"
        }));

        assert!(request.steps().is_empty());
    }

    #[test]
    fn build_book_import_retry_request_from_route_payload_keeps_compat_parsing() {
        let request =
            build_book_import_retry_request_from_route_payload(BookImportRetryRouteRequest {
                steps: Some(json!(["parse", 3, "import", null, true])),
            });

        assert_eq!(
            request.steps(),
            &["parse".to_string(), "import".to_string()]
        );
    }
}
