export type NovelAutopilotRunStatus =
  | 'queued'
  | 'running'
  | 'waiting_human'
  | 'paused'
  | 'completed'
  | 'failed'
  | 'cancelled';

export type NovelAutopilotPhase =
  | 'validate'
  | 'foundation'
  | 'world_building'
  | 'career_design'
  | 'character_design'
  | 'organization_design'
  | 'outline'
  | 'chapter_loop'
  | 'book_review'
  | 'book_polish'
  | 'export'
  | 'completed';

export type NovelAutopilotExecutionScope =
  | 'planning_only'
  | 'next_n_chapters'
  | 'continue_from_current'
  | 'complete_book';

export type NovelAutopilotHumanGateMode =
  | 'fully_automatic'
  | 'high_risk_only'
  | 'every_n_chapters'
  | 'every_volume'
  | 'every_chapter';

export type NovelAutopilotCreateHumanGateMode = Exclude<
  NovelAutopilotHumanGateMode,
  'every_volume'
>;

export type NovelAutopilotStepStatus =
  | 'queued'
  | 'running'
  | 'completed'
  | 'skipped'
  | 'failed'
  | 'cancelled'
  | 'stale';

export type NovelAutopilotStepType =
  | 'validate'
  | 'foundation'
  | 'world_building'
  | 'career_design'
  | 'character_design'
  | 'organization_design'
  | 'outline'
  | 'outline_expand'
  | 'chapter_generate'
  | 'chapter_analyze'
  | 'chapter_repair'
  | 'book_review'
  | 'book_polish'
  | 'export';

export type NovelAutopilotQualityDecision =
  | 'accept'
  | 'auto_repair'
  | 'retry'
  | 'manual_review'
  | 'reject';

export type NovelAutopilotHumanDecision = 'accept' | 'retry' | 'repair' | 'stop';

export interface NovelAutopilotRunConfig {
  execution_scope: NovelAutopilotExecutionScope;
  human_gate_mode: NovelAutopilotHumanGateMode;
  gate_interval: number;
  next_chapter_count?: number | null;
  max_chapters: number;
  max_tokens: number;
  max_estimated_cost?: number | null;
  max_runtime_seconds: number;
  max_step_attempts: number;
  max_consecutive_provider_failures: number;
  max_consecutive_quality_failures: number;
  regenerate_existing: boolean;
  run_book_review: boolean;
  run_book_polish: boolean;
  export_format: 'txt' | 'markdown' | 'docx';
}

export interface NovelAutopilotRun {
  id: string;
  project_id: string;
  schema_version: string;
  status: NovelAutopilotRunStatus;
  current_phase: NovelAutopilotPhase;
  current_step: NovelAutopilotStepType | null;
  current_chapter_id: string | null;
  current_chapter_number: number | null;
  total_chapters: number;
  completed_chapters: number;
  failed_chapter_count: number;
  pending_rewrite_count: number;
  total_word_count: number;
  execution_scope: NovelAutopilotExecutionScope;
  human_gate_mode: NovelAutopilotHumanGateMode;
  gate_interval: number | null;
  max_chapters: number | null;
  max_tokens: number | null;
  max_estimated_cost: number | null;
  max_runtime_seconds: number | null;
  next_chapter_count: number | null;
  max_step_attempts: number | null;
  max_consecutive_provider_failures: number | null;
  max_consecutive_quality_failures: number | null;
  regenerate_existing: boolean | null;
  run_book_review: boolean | null;
  run_book_polish: boolean | null;
  export_format: 'txt' | 'markdown' | 'docx' | null;
  used_tokens: number;
  estimated_cost: number;
  epoch: number;
  version: number;
  consecutive_provider_failures: number;
  consecutive_quality_failures: number;
  last_error_code: string | null;
  has_guidance: boolean;
  active_background_task_id: string | null;
  final_export_ref: string | null;
  created_at: string;
  updated_at: string;
  started_at: string | null;
  paused_at: string | null;
  completed_at: string | null;
}

export interface NovelAutopilotStepRun {
  id: string;
  run_id: string;
  step_key: string;
  step_type: NovelAutopilotStepType;
  phase: NovelAutopilotPhase;
  chapter_id: string | null;
  chapter_number: number | null;
  attempt: number;
  run_epoch: number;
  status: NovelAutopilotStepStatus;
  background_task_id: string | null;
  quality_decision: NovelAutopilotQualityDecision | null;
  error_code: string | null;
  started_at: string | null;
  completed_at: string | null;
  created_at: string;
  updated_at: string;
}

export interface NovelAutopilotBackgroundTaskSummary {
  task_id: string | null;
  task_type: string | null;
  status: string | null;
  progress: number | null;
  message: string | null;
}

export type NovelAutopilotCreateRunConfig = Omit<
  NovelAutopilotRunConfig,
  'human_gate_mode' | 'regenerate_existing' | 'export_format'
> & {
  human_gate_mode: NovelAutopilotCreateHumanGateMode;
  regenerate_existing: false;
  export_format: 'txt';
};

export interface CreateNovelAutopilotRunRequest {
  config: NovelAutopilotCreateRunConfig;
  total_chapters?: number;
}

export interface NovelAutopilotRunResponse {
  run: NovelAutopilotRun;
}

export interface CreateNovelAutopilotRunResponse extends NovelAutopilotRunResponse {
  created: boolean;
  background_task: NovelAutopilotBackgroundTaskSummary | null;
}

export interface NovelAutopilotRunMutationResponse extends NovelAutopilotRunResponse {
  background_task?: NovelAutopilotBackgroundTaskSummary | null;
}

export interface NovelAutopilotRunListResponse {
  items: NovelAutopilotRun[];
}

export interface NovelAutopilotStepListResponse {
  items: NovelAutopilotStepRun[];
}

export interface NovelAutopilotVersionedRequest {
  expected_version: number;
}

export interface NovelAutopilotGuidanceRequest extends NovelAutopilotVersionedRequest {
  guidance: string;
}

export interface NovelAutopilotDecisionRequest extends NovelAutopilotVersionedRequest {
  decision: NovelAutopilotHumanDecision;
  guidance?: string;
}

export interface ProjectExportArtifactDescriptorV1 {
  schema_version: 'project-export-artifact/v1';
  project_id: string;
  format: 'txt';
  filename: string;
  content_type: string;
  content_digest: string;
  chapter_count: number;
  total_word_count: number;
}

export const isNovelAutopilotRunTerminal = (status: NovelAutopilotRunStatus) => (
  status === 'completed' || status === 'failed' || status === 'cancelled'
);
