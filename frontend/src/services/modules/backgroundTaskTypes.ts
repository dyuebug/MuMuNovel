import type { ActiveStoryRepairPayload, NovelWorkflowPhase } from '../../types';

export type BackgroundTaskType =
  | 'novel_autopilot'
  | 'novel_book_autopilot'
  | 'chapters_batch_generate'
  | 'chapter_single_generate'
  | 'chapter_analysis'
  | 'chapter_regenerate'
  | 'chapter_partial_regenerate'
  | 'book_import_apply'
  | 'book_import_retry_failed_steps'
  | 'polish_text'
  | 'polish_batch'
  | 'inspiration_generate_options'
  | 'inspiration_refine_options'
  | 'inspiration_quick_generate'
  | 'careers_generate_system'
  | 'character_generate'
  | 'organization_generate'
  | 'world_regenerate'
  | 'outline_generate'
  | 'outline_expand'
  | 'outline_batch_expand'
  | 'wizard_world_building'
  | 'wizard_career_system'
  | 'wizard_characters'
  | 'wizard_outline'
  | 'unknown';

export type BackgroundTaskRuntimeStatus = 'pending' | 'running' | 'completed' | 'failed' | 'cancelled';

export interface ConfirmedAutopilotWorkflowTransitionRequest {
  tool_name: 'transition_project_workflow';
  arguments: {
    expected_phase: NovelWorkflowPhase;
    target_phase: NovelWorkflowPhase;
    reason?: string;
    related_task_id?: string;
  };
  confirmed_by_user: true;
}

export interface BackgroundTaskStatus {
  task_id: string;
  task_type: BackgroundTaskType;
  project_id: string;
  status: BackgroundTaskRuntimeStatus;
  progress: number;
  message: string;
  result?: Record<string, unknown> | null;
  error?: string | null;
  stage_code?: string | null;
  execution_mode?: 'interactive' | 'auto' | null;
  workflow_scope?: string | null;
  checkpoint?: Record<string, unknown> | null;
  active_story_repair_payload?: ActiveStoryRepairPayload | null;
  terminal_reason?: string | null;
  terminal_label?: string | null;
  review_required?: boolean | null;
  can_resume?: boolean | null;
  created_at?: string | null;
  updated_at?: string | null;
  started_at?: string | null;
  completed_at?: string | null;
}

export interface BackgroundTaskListResponse {
  total: number;
  items: BackgroundTaskStatus[];
}
