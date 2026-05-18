use crate::models::batch_generation_task;

pub fn task_type(task: &batch_generation_task::Model) -> &'static str {
    if task.chapter_count == 1
        && task
            .chapter_ids
            .as_array()
            .is_some_and(|items| items.len() == 1)
    {
        "chapter_single_generate"
    } else {
        "chapters_batch_generate"
    }
}

pub fn task_stage_code(task: &batch_generation_task::Model) -> &'static str {
    match task.status.as_str() {
        "completed" => "6.writing.completed",
        "failed" => "6.writing.failed",
        "cancelled" => "6.writing.cancelled",
        "running" => "6.writing.generating",
        _ => "6.writing.pending",
    }
}

pub fn task_execution_mode(task: &batch_generation_task::Model) -> &'static str {
    match task_type(task) {
        "chapter_single_generate" => "interactive",
        _ => "interactive",
    }
}
