use std::collections::HashMap;
use std::sync::Arc;

use chrono::Utc;
use sea_orm::{ActiveModelTrait, Set};
use serde_json::{json, Value};
use tokio::sync::Mutex;
use uuid::Uuid;

use super::book_import_ai_generation_service::{
    execute_book_import_ai_step, BookImportAiExecutionContext, BookImportAiStepExecutionError,
    BookImportAiStepKind,
};
use super::book_import_apply_execution_service::{
    create_book_import_project, import_book_import_chapters, import_book_import_outlines,
    read_book_import_project_suggestion,
};
use super::txt_parser_service::TxtParserService;

#[derive(Debug, Clone)]
#[allow(dead_code)]
struct StepFailure {
    step_name: String,
    step_label: String,
    error_message: String,
    retry_count: u32,
}

#[derive(Debug, Clone)]
struct BookImportTask {
    task_id: String,
    user_id: String,
    filename: String,
    #[allow(dead_code)]
    import_mode: String,
    status: String,
    progress: i32,
    message: Option<String>,
    error: Option<String>,
    created_at: chrono::DateTime<Utc>,
    updated_at: chrono::DateTime<Utc>,
    preview: Option<Value>,
    cancelled: bool,
    #[allow(dead_code)]
    failed_steps: Vec<StepFailure>,
    imported_project_id: Option<String>,
}

pub struct BookImportService {
    tasks: Arc<Mutex<HashMap<String, BookImportTask>>>,
}

impl BookImportService {
    pub fn new() -> Self {
        Self {
            tasks: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub async fn create_task(
        &self,
        user_id: &str,
        filename: &str,
        file_content: Vec<u8>,
        import_mode: &str,
    ) -> Value {
        let task_id = Uuid::new_v4().to_string();
        let now = Utc::now();
        let task = BookImportTask {
            task_id: task_id.clone(),
            user_id: user_id.to_string(),
            filename: filename.to_string(),
            import_mode: import_mode.to_string(),
            status: "pending".to_string(),
            progress: 0,
            message: Some("任务已创建".to_string()),
            error: None,
            created_at: now,
            updated_at: now,
            preview: None,
            cancelled: false,
            failed_steps: vec![],
            imported_project_id: None,
        };

        {
            let mut tasks = self.tasks.lock().await;
            tasks.insert(task_id.clone(), task);
        }

        let tasks = self.tasks.clone();
        let parser = TxtParserService;
        let tid = task_id.clone();
        tokio::spawn(async move {
            Self::run_pipeline(tasks, parser, tid, file_content).await;
        });

        json!({"task_id": task_id, "status": "pending"})
    }

    async fn run_pipeline(
        tasks: Arc<Mutex<HashMap<String, BookImportTask>>>,
        parser: TxtParserService,
        task_id: String,
        file_content: Vec<u8>,
    ) {
        let task_opt = {
            let tasks_guard = tasks.lock().await;
            tasks_guard.get(&task_id).cloned()
        };

        let Some(mut task) = task_opt else {
            return;
        };

        let update = |task: &mut BookImportTask,
                      status: &str,
                      progress: i32,
                      message: &str,
                      error: Option<&str>| {
            task.status = status.to_string();
            task.progress = progress.clamp(0, 100);
            task.message = Some(message.to_string());
            task.error = error.map(|s| s.to_string());
            task.updated_at = Utc::now();
        };

        // Helper to check cancellation
        let is_cancelled =
            |task: &BookImportTask| -> bool { task.cancelled || task.status == "cancelled" };

        // Progress: 5% — decode
        update(&mut task, "running", 5, "正在识别编码并读取文本...", None);
        if is_cancelled(&task) {
            let progress = task.progress;
            update(&mut task, "cancelled", progress, "任务已取消", None);
            Self::save_task(&tasks, &task).await;
            return;
        }

        let (text, encoding) = parser.decode_bytes(&file_content);
        let cleaned = parser.clean_text(&text);

        // 10% — clean done
        update(
            &mut task,
            "running",
            10,
            &format!("文本清洗完成（编码：{}）", encoding),
            None,
        );
        if is_cancelled(&task) {
            let progress = task.progress;
            update(&mut task, "cancelled", progress, "任务已取消", None);
            Self::save_task(&tasks, &task).await;
            return;
        }

        // 15% — split chapters
        let chapters_data = parser.split_chapters(&cleaned);
        if chapters_data.is_empty() {
            let progress = task.progress;
            update(
                &mut task,
                "failed",
                progress,
                "解析失败",
                Some("未能识别到有效章节，请检查TXT内容"),
            );
            Self::save_task(&tasks, &task).await;
            return;
        }

        update(
            &mut task,
            "running",
            15,
            &format!("已识别 {} 个章节，正在构建预览结构...", chapters_data.len()),
            None,
        );
        if is_cancelled(&task) {
            let progress = task.progress;
            update(&mut task, "cancelled", progress, "任务已取消", None);
            Self::save_task(&tasks, &task).await;
            return;
        }

        // 18% — keep last 10 chapters
        update(
            &mut task,
            "running",
            18,
            "仅保留末10章并重建预览结构...",
            None,
        );
        let preview = Self::build_preview(&task, &chapters_data);
        if is_cancelled(&task) {
            let progress = task.progress;
            update(&mut task, "cancelled", progress, "任务已取消", None);
            Self::save_task(&tasks, &task).await;
            return;
        }

        task.preview = Some(preview);
        update(
            &mut task,
            "completed",
            100,
            "解析完成，可预览并确认导入",
            None,
        );
        Self::save_task(&tasks, &task).await;
    }

    async fn save_task(tasks: &Arc<Mutex<HashMap<String, BookImportTask>>>, task: &BookImportTask) {
        let mut tasks_guard = tasks.lock().await;
        tasks_guard.insert(task.task_id.clone(), task.clone());
    }

    fn build_preview(task: &BookImportTask, chapters_data: &[Value]) -> Value {
        let filename_stem = std::path::Path::new(&task.filename)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("拆书导入项目");

        // Keep last 10 chapters
        let selected: Vec<&Value> = if chapters_data.len() > 10 {
            chapters_data[chapters_data.len() - 10..].iter().collect()
        } else {
            chapters_data.iter().collect()
        };
        let selected_total = selected.len();

        let mut chapters: Vec<Value> = vec![];
        let mut warnings: Vec<Value> = vec![];
        let mut title_counter: HashMap<String, u32> = HashMap::new();

        for (i, chapter) in selected.iter().enumerate() {
            let idx = i + 1;
            let fallback_title = format!("第{}章", idx);
            let raw_title = chapter["title"].as_str().unwrap_or(&fallback_title);
            let raw_title = if raw_title.len() > 200 {
                &raw_title[..200]
            } else {
                raw_title
            };
            let title = strip_chapter_prefix(raw_title);
            let content = chapter["content"].as_str().unwrap_or("").to_string();
            let summary = build_summary(&content, 120);

            chapters.push(json!({
                "title": title,
                "content": content,
                "summary": summary,
                "chapter_number": idx,
                "outline_title": title,
            }));

            *title_counter.entry(title.to_string()).or_insert(0) += 1;

            if content.chars().count() < 300 {
                warnings.push(json!({
                    "code": "chapter_too_short",
                    "message": format!("章节「{}」内容较短，建议检查切分结果", title),
                    "level": "warning",
                }));
            }
            if content.chars().count() > 12000 {
                warnings.push(json!({
                    "code": "chapter_too_long",
                    "message": format!("章节「{}」内容较长，建议确认是否应继续拆分", title),
                    "level": "info",
                }));
            }
        }

        for (title, count) in title_counter.iter() {
            if *count > 1 {
                warnings.push(json!({
                    "code": "duplicate_chapter_title",
                    "message": format!("检测到重复章节标题「{}」共 {} 次", title, count),
                    "level": "warning",
                }));
            }
        }

        if chapters_data.len() as usize > selected_total {
            warnings.push(json!({
                "code": "trimmed_to_last_ten_chapters",
                "message": format!(
                    "已按规则仅保留最后 {} 章用于导入（原始识别 {} 章）",
                    selected_total,
                    chapters_data.len()
                ),
                "level": "info",
            }));
        }

        // Build fallback project suggestion (rule-based, no AI)
        let sampled_chapters: Vec<&Value> = chapters.iter().take(3).collect();
        let sampled_text: String = sampled_chapters
            .iter()
            .filter_map(|c| {
                let content = c["content"].as_str().unwrap_or("");
                let snippet: String = content.chars().take(2000).collect();
                Some(snippet)
            })
            .collect::<Vec<_>>()
            .join("\n\n");

        let fallback_desc_source: String = sampled_chapters
            .iter()
            .filter_map(|c| {
                let summary = c["summary"].as_str().unwrap_or("");
                if !summary.is_empty() {
                    Some(summary.to_string())
                } else {
                    let content = c["content"].as_str().unwrap_or("");
                    let snippet: String = content.chars().take(600).collect();
                    if snippet.is_empty() {
                        None
                    } else {
                        Some(snippet)
                    }
                }
            })
            .collect::<Vec<_>>()
            .join("\n");

        let fallback_description = build_summary(&fallback_desc_source, 500)
            .unwrap_or_else(|| "由拆书功能基于前3章自动提炼：该故事围绕核心人物与主要冲突展开，可在导入前继续修改。".to_string());

        let suggestion = json!({
            "title": if filename_stem.len() > 200 { &filename_stem[..200] } else { filename_stem },
            "description": fallback_description,
            "theme": detect_theme_from_text(&sampled_text),
            "genre": detect_genre_from_text(&sampled_text),
            "narrative_perspective": detect_narrative_perspective(&sampled_text),
            "target_words": 100000,
        });

        // Generate outlines from chapter titles (1:1 mapping, rule-based)
        let outlines: Vec<Value> = chapters
            .iter()
            .enumerate()
            .map(|(i, ch)| {
                json!({
                    "title": ch["title"],
                    "content": null,
                    "order_index": i + 1,
                    "structure": null,
                })
            })
            .collect();

        json!({
            "task_id": task.task_id,
            "project_suggestion": suggestion,
            "chapters": chapters,
            "outlines": outlines,
            "warnings": warnings,
        })
    }

    pub async fn get_task_status(&self, task_id: &str, user_id: &str) -> Result<Value, String> {
        let task = self.get_task(task_id, user_id).await?;
        Ok(json!({
            "task_id": task.task_id,
            "status": task.status,
            "progress": task.progress,
            "message": task.message,
            "error": task.error,
            "created_at": task.created_at.to_rfc3339(),
            "updated_at": task.updated_at.to_rfc3339(),
        }))
    }

    pub async fn get_preview(&self, task_id: &str, user_id: &str) -> Result<Value, String> {
        let task = self.get_task(task_id, user_id).await?;
        if task.status != "completed" {
            return Err("任务尚未完成，无法获取预览".to_string());
        }
        task.preview
            .clone()
            .ok_or_else(|| "预览数据不存在".to_string())
    }

    pub async fn cancel_task(&self, task_id: &str, user_id: &str) -> Result<Value, String> {
        let task = self.get_task(task_id, user_id).await?;
        if ["completed", "failed", "cancelled"].contains(&task.status.as_str()) {
            return Ok(
                json!({"success": true, "message": format!("任务已是终态：{}", task.status)}),
            );
        }

        let mut tasks = self.tasks.lock().await;
        if let Some(t) = tasks.get_mut(task_id) {
            t.cancelled = true;
            t.status = "cancelled".to_string();
            t.message = Some("任务已取消".to_string());
            t.updated_at = Utc::now();
        }

        Ok(json!({"success": true, "message": "取消成功"}))
    }

    async fn get_task(&self, task_id: &str, user_id: &str) -> Result<BookImportTask, String> {
        let tasks = self.tasks.lock().await;
        let task = tasks.get(task_id).ok_or("任务不存在".to_string())?;
        if task.user_id != user_id {
            return Err("无权访问该任务".to_string());
        }
        Ok(task.clone())
    }

    async fn set_imported_project_id(&self, task_id: &str, project_id: &str) {
        let mut tasks = self.tasks.lock().await;
        if let Some(task) = tasks.get_mut(task_id) {
            task.imported_project_id = Some(project_id.to_string());
        }
    }

    async fn execute_book_import_ai_generation_step(
        db: &sea_orm::DatabaseConnection,
        project: &crate::models::project::Model,
        ai_service: &crate::ai::service::AIService,
        step: BookImportAiStepKind,
        description: &str,
        theme: Option<&str>,
        genre: Option<&str>,
    ) -> Result<Option<Value>, String> {
        execute_book_import_ai_step(
            db,
            project,
            ai_service,
            step,
            BookImportAiExecutionContext {
                description,
                theme,
                genre,
            },
        )
        .await
        .map_err(|error| match error {
            BookImportAiStepExecutionError::PromptFormat(detail)
            | BookImportAiStepExecutionError::Ai(detail) => detail,
        })
    }

    pub async fn apply_import(
        &self,
        db: &sea_orm::DatabaseConnection,
        task_id: &str,
        user_id: &str,
        project_suggestion: &Value,
        chapters: &[Value],
        outlines: &[Value],
        import_mode: &str,
    ) -> Result<Value, String> {
        let task = self.get_task(task_id, user_id).await?;
        if task.status != "completed" {
            return Err("任务尚未完成解析，无法导入".to_string());
        }

        let suggestion = read_book_import_project_suggestion(project_suggestion);
        let project = create_book_import_project(db, user_id, import_mode, &suggestion).await?;
        self.set_imported_project_id(task_id, &project.id).await;
        import_book_import_outlines(db, &project.id, user_id, outlines).await;
        let _chapter_summary =
            import_book_import_chapters(db, &project.id, user_id, chapters).await;

        // --- Step 4-6: AI Wizard generation ---
        let ai_result = Self::run_wizard_generation(
            db,
            user_id,
            &project,
            &suggestion.description,
            suggestion.theme.as_deref(),
            suggestion.genre.as_deref(),
            None,
            None,
        )
        .await;

        // --- Update task ---
        {
            let mut tasks = self.tasks.lock().await;
            if let Some(t) = tasks.get_mut(task_id) {
                t.status = "imported".to_string();
                t.progress = 100;
                t.message = Some("导入完成".to_string());
                t.updated_at = Utc::now();
            }
        }

        let mut result = json!({
            "project_id": project.id,
            "project_title": project.title,
        });
        if let Some(ref ai) = ai_result {
            result["ai_generation"] = ai.clone();
        }

        Ok(result)
    }

    /// SSE streaming variant of apply_import with per-step progress events
    pub async fn apply_import_stream(
        &self,
        db: &sea_orm::DatabaseConnection,
        task_id: &str,
        user_id: &str,
        project_suggestion: &Value,
        chapters: &[Value],
        outlines: &[Value],
        import_mode: &str,
        channel: &crate::utils::sse::SseChannel,
    ) {
        use crate::ai::service::AIService;
        use crate::services::prompt_template_service::PromptTemplateService;
        use crate::services::settings_service::SettingsService;

        let task = match self.get_task(task_id, user_id).await {
            Ok(t) => t,
            Err(e) => {
                channel.error(&e, 400).await;
                channel.done().await;
                return;
            }
        };
        if task.status != "completed" {
            channel.error("任务尚未完成解析，无法导入", 400).await;
            channel.done().await;
            return;
        }

        if import_mode != "append" && import_mode != "overwrite" {
            channel
                .error("import_mode 仅支持 append 或 overwrite", 400)
                .await;
            channel.done().await;
            return;
        }

        let suggestion = read_book_import_project_suggestion(project_suggestion);
        channel
            .progress("开始导入拆书数据...", 0, "processing")
            .await;

        // --- Step 1: Create project (0-5%) ---
        channel.progress("正在创建项目...", 2, "processing").await;
        let project = match create_book_import_project(db, user_id, import_mode, &suggestion).await
        {
            Ok(p) => p,
            Err(e) => {
                channel.error(&e, 500).await;
                channel.done().await;
                return;
            }
        };
        self.set_imported_project_id(task_id, &project.id).await;
        channel.progress("项目创建完成", 5, "processing").await;

        // --- Step 2: Import outlines (5-10%) ---
        let outline_count = outlines.len();
        channel
            .progress(
                &format!("正在导入 {} 个大纲...", outline_count),
                6,
                "processing",
            )
            .await;
        import_book_import_outlines(db, &project.id, user_id, outlines).await;
        channel
            .progress(
                &format!("已导入 {} 个大纲", outline_count),
                10,
                "processing",
            )
            .await;

        // --- Step 3: Import chapters (10-20%) ---
        let chapter_count = chapters.len();
        channel
            .progress(
                &format!("正在导入 {} 个章节...", chapter_count),
                12,
                "processing",
            )
            .await;
        let chapter_summary = import_book_import_chapters(db, &project.id, user_id, chapters).await;
        let total_words = chapter_summary.total_words;
        channel
            .progress(
                &format!("已导入 {} 个章节（{}字）", chapter_count, total_words),
                20,
                "processing",
            )
            .await;

        // --- Build AI config ---
        let ai_config = match SettingsService::build_ai_config(db, user_id, None, None, None).await
        {
            Ok(cfg) => cfg,
            Err(e) => {
                channel.error(&format!("AI配置加载失败: {}", e), 500).await;
                channel.done().await;
                return;
            }
        };
        let ai_service = AIService::new(ai_config);
        let mut failed_steps: Vec<Value> = Vec::new();

        // --- Step 4: World building (20-40%) ---
        channel
            .progress("🌍 正在生成世界观...", 22, "processing")
            .await;
        channel
            .progress("🌍 正在初始化AI服务...", 25, "processing")
            .await;
        if PromptTemplateService::system_template_info("WORLD_BUILDING").is_some() {
            channel
                .progress("🌍 正在准备世界观提示词...", 28, "processing")
                .await;
            channel
                .progress("🌍 AI正在生成世界观...", 32, "processing")
                .await;
            match Self::execute_book_import_ai_generation_step(
                db,
                &project,
                &ai_service,
                BookImportAiStepKind::WorldBuilding,
                &suggestion.description,
                suggestion.theme.as_deref(),
                suggestion.genre.as_deref(),
            )
            .await
            {
                Ok(Some(_)) => {
                    channel
                        .progress("🌍 正在解析世界观数据...", 36, "processing")
                        .await;
                    channel
                        .progress("🌍 世界观写入完成", 38, "processing")
                        .await;
                    channel
                        .progress("🌍 世界观生成完成", 40, "processing")
                        .await;
                }
                Ok(None) => {
                    channel
                        .progress("🌍 世界观生成完成", 40, "processing")
                        .await;
                }
                Err(e) => {
                    failed_steps.push(json!({
                        "step": BookImportAiStepKind::WorldBuilding.step_name(),
                        "label": BookImportAiStepKind::WorldBuilding.step_label(),
                        "error": e,
                        "retry_count": 0,
                    }));
                    channel
                        .progress(
                            &format!("⚠️ 世界观生成失败：{}，将继续后续步骤", e),
                            40,
                            "warning",
                        )
                        .await;
                }
            }
        } else {
            channel
                .progress("🌍 世界观生成完成", 40, "processing")
                .await;
        }

        // --- Step 5: Career system (40-65%) ---
        channel
            .progress("💼 正在生成职业体系...", 42, "processing")
            .await;
        if PromptTemplateService::system_template_info("CAREER_SYSTEM_GENERATION").is_some() {
            channel
                .progress("💼 正在准备职业体系提示词...", 45, "processing")
                .await;
            channel
                .progress("💼 AI正在生成职业体系...", 50, "processing")
                .await;
            match Self::execute_book_import_ai_generation_step(
                db,
                &project,
                &ai_service,
                BookImportAiStepKind::CareerSystem,
                &suggestion.description,
                suggestion.theme.as_deref(),
                suggestion.genre.as_deref(),
            )
            .await
            {
                Ok(Some(data)) => {
                    let ccount = data.get("count").and_then(Value::as_i64).unwrap_or(0);
                    channel
                        .progress("💼 正在解析职业数据...", 58, "processing")
                        .await;
                    channel
                        .progress("💼 正在保存职业数据...", 62, "processing")
                        .await;
                    channel
                        .progress(
                            &format!("💼 职业体系生成完成（{}个）", ccount),
                            65,
                            "processing",
                        )
                        .await;
                }
                Ok(None) => {
                    channel
                        .progress("💼 职业体系生成完成（0个）", 65, "processing")
                        .await;
                }
                Err(e) => {
                    failed_steps.push(json!({
                        "step": BookImportAiStepKind::CareerSystem.step_name(),
                        "label": BookImportAiStepKind::CareerSystem.step_label(),
                        "error": e,
                        "retry_count": 0,
                    }));
                    channel
                        .progress(
                            &format!("⚠️ 职业体系生成失败：{}，将继续后续步骤", e),
                            65,
                            "warning",
                        )
                        .await;
                }
            }
        } else {
            channel
                .progress("💼 职业体系生成完成（0个）", 65, "processing")
                .await;
        }

        // --- Step 6: Characters (65-92%) ---
        channel
            .progress("👥 正在生成角色与组织...", 67, "processing")
            .await;
        if PromptTemplateService::system_template_info("CHARACTERS_BATCH_GENERATION").is_some() {
            channel
                .progress("👥 正在准备角色提示词...", 70, "processing")
                .await;
            channel
                .progress("👥 AI正在生成角色...", 75, "processing")
                .await;
            match Self::execute_book_import_ai_generation_step(
                db,
                &project,
                &ai_service,
                BookImportAiStepKind::Characters,
                &suggestion.description,
                suggestion.theme.as_deref(),
                suggestion.genre.as_deref(),
            )
            .await
            {
                Ok(Some(data)) => {
                    let chcount = data.get("count").and_then(Value::as_i64).unwrap_or(0);
                    channel
                        .progress("👥 正在解析角色数据...", 85, "processing")
                        .await;
                    channel
                        .progress("👥 正在保存角色...", 88, "processing")
                        .await;
                    channel
                        .progress(
                            &format!("👥 角色/组织生成完成（{}个）", chcount),
                            92,
                            "processing",
                        )
                        .await;
                }
                Ok(None) => {
                    channel
                        .progress("👥 角色/组织生成完成（0个）", 92, "processing")
                        .await;
                }
                Err(e) => {
                    failed_steps.push(json!({
                        "step": BookImportAiStepKind::Characters.step_name(),
                        "label": BookImportAiStepKind::Characters.step_label(),
                        "error": e,
                        "retry_count": 0,
                    }));
                    channel
                        .progress(&format!("⚠️ 角色/组织生成失败：{}", e), 92, "warning")
                        .await;
                }
            }
        } else {
            channel
                .progress("👥 角色/组织生成完成（0个）", 92, "processing")
                .await;
        }

        // --- Step 7: Commit (92-100%) ---
        channel
            .progress("正在保存到数据库...", 95, "processing")
            .await;

        // Update project wizard status
        let mut pactive: crate::models::project::ActiveModel = project.clone().into();
        pactive.wizard_step = Set(4);
        pactive.wizard_status = Set("completed".to_string());
        pactive.status = Set("writing".to_string());
        pactive.updated_at = Set(Some(chrono::Utc::now().naive_utc()));
        let _ = pactive.update(db).await;

        // Update task + store failed_steps for retry
        {
            let mut tasks = self.tasks.lock().await;
            if let Some(t) = tasks.get_mut(task_id) {
                t.status = "imported".to_string();
                t.progress = 100;
                t.message = Some("导入完成".to_string());
                t.updated_at = Utc::now();
                t.failed_steps = failed_steps
                    .iter()
                    .map(|fs| StepFailure {
                        step_name: fs["step"].as_str().unwrap_or("").to_string(),
                        step_label: fs["label"].as_str().unwrap_or("").to_string(),
                        error_message: fs["error"].as_str().unwrap_or("").to_string(),
                        retry_count: 0,
                    })
                    .collect();
            }
        }

        channel.progress("数据保存完成", 98, "processing").await;

        if !failed_steps.is_empty() {
            let failed_count = failed_steps.len();
            channel
                .progress(
                    &format!(
                        "⚠️ 导入完成，但有 {} 个生成步骤失败，可点击重试",
                        failed_count
                    ),
                    98,
                    "warning",
                )
                .await;
            channel
                .progress(
                    &serde_json::to_string(&json!(failed_steps)).unwrap_or_default(),
                    98,
                    "step_failures",
                )
                .await;
        }

        channel
            .result(&json!({
                "success": true,
                "project_id": project.id,
                "statistics": {
                    "chapters_imported": chapter_count,
                    "total_words": total_words,
                    "outlines_imported": outline_count,
                },
            }))
            .await;

        channel.progress("导入完成！", 100, "success").await;
        channel.done().await;
    }

    /// SSE streaming retry for failed AI steps
    pub async fn retry_stream(
        &self,
        db: &sea_orm::DatabaseConnection,
        task_id: &str,
        user_id: &str,
        steps_to_retry: &[String],
        channel: &crate::utils::sse::SseChannel,
    ) {
        use crate::ai::service::AIService;
        use crate::services::project_service::ProjectService;
        use crate::services::prompt_template_service::PromptTemplateService;
        use crate::services::settings_service::SettingsService;

        let task = match self.get_task(task_id, user_id).await {
            Ok(t) => t,
            Err(e) => {
                channel.error(&e, 400).await;
                channel.done().await;
                return;
            }
        };

        let project_id = match &task.imported_project_id {
            Some(id) => id.clone(),
            None => {
                channel.error("该任务尚未完成导入，无法重试", 400).await;
                channel.done().await;
                return;
            }
        };

        let failed_names: std::collections::HashSet<String> = task
            .failed_steps
            .iter()
            .map(|f| f.step_name.clone())
            .collect();
        let invalid: Vec<&String> = steps_to_retry
            .iter()
            .filter(|s| !failed_names.contains(*s))
            .collect();
        if !invalid.is_empty() {
            channel
                .error(&format!("以下步骤不在失败列表中: {:?}", invalid), 400)
                .await;
            channel.done().await;
            return;
        }

        let project = match ProjectService::get(db, &project_id, user_id).await {
            Ok(Some(p)) => p,
            Ok(None) => {
                channel.error("项目不存在或无权访问", 404).await;
                channel.done().await;
                return;
            }
            Err(e) => {
                channel.error(&format!("加载项目失败: {}", e), 500).await;
                channel.done().await;
                return;
            }
        };

        let ai_config = match SettingsService::build_ai_config(db, user_id, None, None, None).await
        {
            Ok(cfg) => cfg,
            Err(e) => {
                channel.error(&format!("AI配置加载失败: {}", e), 500).await;
                channel.done().await;
                return;
            }
        };
        let ai_service = AIService::new(ai_config);

        channel
            .progress("开始重试失败的生成步骤...", 0, "processing")
            .await;

        let total = steps_to_retry.len();
        let mut still_failed: Vec<Value> = Vec::new();
        let mut retry_results: HashMap<String, Value> = HashMap::new();

        for (idx, step) in steps_to_retry.iter().enumerate() {
            let start_pct = (5 + idx * 85 / total) as u32;
            let end_pct = (5 + (idx + 1) * 85 / total) as u32;
            let step_kind = BookImportAiStepKind::from_step_name(step);
            let label = step_kind
                .map(|kind| kind.step_label())
                .unwrap_or("未知步骤");

            channel
                .progress(&format!("🔄 正在重试{}...", label), start_pct, "processing")
                .await;

            let result: Result<Value, String> = match step_kind {
                Some(step_kind) => {
                    if PromptTemplateService::system_template_info(step_kind.template_key())
                        .is_none()
                    {
                        Err(step_kind.missing_template_message().into())
                    } else {
                        channel
                            .progress(
                                step_kind.ai_progress_message(),
                                start_pct + (end_pct - start_pct) / 2,
                                "processing",
                            )
                            .await;
                        Self::execute_book_import_ai_generation_step(
                            db,
                            &project,
                            &ai_service,
                            step_kind,
                            project.description.as_deref().unwrap_or_default(),
                            project.theme.as_deref(),
                            project.genre.as_deref(),
                        )
                        .await
                        .map(|value| value.unwrap_or(Value::Null))
                    }
                }
                None => Err(format!("未知步骤: {}", step)),
            };

            match result {
                Ok(data) => {
                    channel
                        .progress(&format!("✅ {}重试成功", label), end_pct, "processing")
                        .await;
                    retry_results.insert(step.clone(), data);
                }
                Err(e) => {
                    let retry_count = task
                        .failed_steps
                        .iter()
                        .find(|f| f.step_name == *step)
                        .map(|f| f.retry_count + 1)
                        .unwrap_or(1);
                    still_failed.push(json!({"step": step, "label": label, "error": e, "retry_count": retry_count}));
                    channel
                        .progress(&format!("⚠️ {}重试失败：{}", label, e), end_pct, "warning")
                        .await;
                }
            }
        }

        channel
            .progress("正在保存到数据库...", 93, "processing")
            .await;
        {
            let mut tasks = self.tasks.lock().await;
            if let Some(t) = tasks.get_mut(task_id) {
                t.failed_steps = still_failed
                    .iter()
                    .map(|fs| StepFailure {
                        step_name: fs["step"].as_str().unwrap_or("").to_string(),
                        step_label: fs["label"].as_str().unwrap_or("").to_string(),
                        error_message: fs["error"].as_str().unwrap_or("").to_string(),
                        retry_count: fs["retry_count"].as_u64().unwrap_or(0) as u32,
                    })
                    .collect();
                t.updated_at = Utc::now();
            }
        }
        channel.progress("数据保存完成", 96, "processing").await;

        if !still_failed.is_empty() {
            channel
                .progress(
                    &serde_json::to_string(&json!({"failed_steps": still_failed}))
                        .unwrap_or_default(),
                    98,
                    "step_failures",
                )
                .await;
        }

        channel
            .result(&json!({
                "success": true,
                "project_id": project_id,
                "retry_results": retry_results,
                "still_failed": still_failed,
            }))
            .await;

        if still_failed.is_empty() {
            channel.progress("所有步骤重试成功！", 100, "success").await;
        } else {
            channel
                .progress(
                    &format!("重试完成，仍有 {} 个步骤失败", still_failed.len()),
                    100,
                    "warning",
                )
                .await;
        }
        channel.done().await;
    }

    async fn run_wizard_generation(
        db: &sea_orm::DatabaseConnection,
        user_id: &str,
        project: &crate::models::project::Model,
        description: &str,
        theme: Option<&str>,
        genre: Option<&str>,
        provider_override: Option<&str>,
        model_override: Option<&str>,
    ) -> Option<Value> {
        use crate::ai::service::AIService;
        use crate::services::settings_service::SettingsService;

        let ai_config = match SettingsService::build_ai_config(
            db,
            user_id,
            provider_override,
            model_override,
            None,
        )
        .await
        {
            Ok(cfg) => cfg,
            Err(_) => return None,
        };
        let ai_service = AIService::new(ai_config);

        let mut results = Vec::new();

        for step in BookImportAiStepKind::ALL {
            match Self::execute_book_import_ai_generation_step(
                db,
                project,
                &ai_service,
                step,
                description,
                theme,
                genre,
            )
            .await
            {
                Ok(Some(data)) => match step {
                    BookImportAiStepKind::WorldBuilding => {
                        results.push(json!({"step": step.step_name(), "status": "ok"}));
                    }
                    BookImportAiStepKind::CareerSystem | BookImportAiStepKind::Characters => {
                        let count = data.get("count").and_then(Value::as_i64).unwrap_or(0);
                        results.push(
                            json!({"step": step.step_name(), "status": "ok", "count": count}),
                        );
                    }
                },
                Ok(None) => {}
                Err(error) => {
                    results.push(
                        json!({"step": step.step_name(), "status": "failed", "error": error}),
                    );
                }
            }
        }

        Some(json!({
            "steps": results,
            "total_steps": 3,
        }))
    }
}

fn build_summary(content: &str, max_len: usize) -> Option<String> {
    if content.is_empty() {
        return None;
    }
    let normalized: String = content.split_whitespace().collect::<Vec<_>>().join(" ");
    if normalized.chars().count() <= max_len {
        Some(normalized)
    } else {
        let truncated: String = normalized.chars().take(max_len).collect();
        Some(format!("{}…", truncated))
    }
}

fn strip_chapter_prefix(title: &str) -> String {
    let normalized = title.trim();
    if normalized.is_empty() {
        return normalized.to_string();
    }

    let chars: Vec<char> = normalized.chars().collect();
    if chars.is_empty() || chars[0] != '第' {
        return normalized.to_string();
    }

    // Find end of digit portion
    let mut i = 1;
    while i < chars.len() {
        let ch = chars[i];
        if ch.is_ascii_digit()
            || is_chinese_digit(ch)
            || ch == '零'
            || ch == '〇'
            || ch == '两'
            || ch == ' '
            || ch == '\u{3000}'
        {
            i += 1;
        } else {
            break;
        }
    }

    // Check for chapter unit character
    if i < chars.len() {
        let unit_chars = ['章', '节', '回', '卷', '集', '部', '篇'];
        if unit_chars.contains(&chars[i]) {
            i += 1;
            // Skip trailing separators: - — : ： 、 . ． ） ) 】 ] ]
            while i < chars.len() {
                let ch = chars[i];
                if matches!(
                    ch,
                    '-' | '—' | ':' | '：' | '、' | '.' | '．' | '）' | ')' | '】' | ']' | ' '
                ) {
                    i += 1;
                } else {
                    break;
                }
            }
            let stripped: String = chars[i..].iter().collect();
            let trimmed = stripped.trim().to_string();
            if !trimmed.is_empty() {
                return trimmed;
            }
        }
    }

    normalized.to_string()
}

fn is_chinese_digit(ch: char) -> bool {
    matches!(
        ch,
        '一' | '二' | '三' | '四' | '五' | '六' | '七' | '八' | '九' | '十' | '百' | '千' | '万'
    )
}

fn detect_theme_from_text(text: &str) -> &'static str {
    let keywords = [
        ("复仇", "复仇与救赎"),
        ("报仇", "复仇与救赎"),
        ("雪恨", "复仇与救赎"),
        ("成长", "成长与逆袭"),
        ("蜕变", "成长与逆袭"),
        ("逆袭", "成长与逆袭"),
        ("真相", "真相与抉择"),
        ("谜团", "真相与抉择"),
        ("秘密", "真相与抉择"),
        ("调查", "真相与抉择"),
        ("权谋", "权力与人性"),
        ("争权", "权力与人性"),
        ("朝堂", "权力与人性"),
        ("家族", "权力与人性"),
        ("爱情", "爱情与选择"),
        ("喜欢", "爱情与选择"),
        ("恋爱", "爱情与选择"),
        ("婚约", "爱情与选择"),
    ];
    for (keyword, theme) in &keywords {
        if text.contains(keyword) {
            return theme;
        }
    }
    "命运与选择"
}

fn detect_genre_from_text(text: &str) -> &'static str {
    let keywords = [
        ("修仙", "仙侠"),
        ("宗门", "仙侠"),
        ("灵气", "仙侠"),
        ("飞升", "仙侠"),
        ("仙门", "仙侠"),
        ("玄幻", "玄幻"),
        ("异界", "玄幻"),
        ("魔法", "玄幻"),
        ("斗气", "玄幻"),
        ("星际", "科幻"),
        ("机甲", "科幻"),
        ("赛博", "科幻"),
        ("人工智能", "科幻"),
        ("宇宙", "科幻"),
        ("悬疑", "悬疑"),
        ("凶案", "悬疑"),
        ("推理", "悬疑"),
        ("谜案", "悬疑"),
        ("诡", "悬疑"),
        ("总裁", "都市"),
        ("职场", "都市"),
        ("都市", "都市"),
        ("豪门", "都市"),
        ("恋爱", "言情"),
        ("言情", "言情"),
        ("心动", "言情"),
        ("告白", "言情"),
    ];
    for (keyword, genre) in &keywords {
        if text.contains(keyword) {
            return genre;
        }
    }
    "通用"
}

fn detect_narrative_perspective(text: &str) -> &'static str {
    let snippet: String = text.chars().take(6000).collect();
    let first_person: u32 = snippet.matches(&['我', '咱', '俺']).count() as u32;
    let third_person: u32 = snippet.matches(&['他', '她', '它']).count() as u32;

    if first_person >= 20 && first_person as f64 > third_person as f64 * 1.2 {
        "第一人称"
    } else {
        "第三人称"
    }
}
