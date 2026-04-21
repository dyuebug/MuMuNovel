import type {
  ActiveStoryRepairPayload,
  ChapterLatestQualityMetrics,
  ChapterQualityMetricsSummary,
  ChapterQualityProfileSummary,
} from '../../types';
import type {
  ChapterBatchFailedChapter,
  ChapterGenerationTaskType,
} from './chapterTaskState';

export interface ChapterBatchGenerateResponse {
  batch_id: string;
  message: string;
  chapters_to_generate: Array<{ id: string; chapter_number: number; title: string }>;
  estimated_time_minutes: number;
}

export interface ChapterBatchGenerateStatusResponse {
  batch_id: string;
  status: string;
  stage_code?: string | null;
  execution_mode?: 'interactive' | 'auto' | null;
  total: number;
  completed: number;
  current_chapter_id?: string | null;
  current_chapter_number?: number | null;
  current_retry_count?: number | null;
  max_retries?: number | null;
  checkpoint?: Record<string, unknown> | null;
  failed_chapters?: ChapterBatchFailedChapter[];
  created_at?: string | null;
  started_at?: string | null;
  completed_at?: string | null;
  error_message?: string | null;
  latest_quality_metrics?: ChapterLatestQualityMetrics | null;
  quality_metrics_summary?: ChapterQualityMetricsSummary | null;
  quality_profile_summary?: ChapterQualityProfileSummary | null;
  active_story_repair_payload?: ActiveStoryRepairPayload | null;
  terminal_reason?: string | null;
  terminal_label?: string | null;
  review_required?: boolean | null;
  can_resume?: boolean | null;
}

export interface ChapterBatchActiveTask {
  batch_id: string;
  status: string;
  stage_code?: string | null;
  execution_mode?: 'interactive' | 'auto' | null;
  total: number;
  completed: number;
  current_chapter_id?: string | null;
  current_chapter_number?: number | null;
  checkpoint?: Record<string, unknown> | null;
  latest_quality_metrics?: ChapterLatestQualityMetrics | null;
  quality_metrics_summary?: ChapterQualityMetricsSummary | null;
  quality_profile_summary?: ChapterQualityProfileSummary | null;
  active_story_repair_payload?: ActiveStoryRepairPayload | null;
  created_at?: string | null;
  started_at?: string | null;
}

export interface ChapterBatchActiveResponse {
  has_active_task: boolean;
  task: ChapterBatchActiveTask | null;
}

export interface ChapterBatchCancelResponse {
  message: string;
  batch_id: string;
  completed_chapters: number;
  total_chapters: number;
}

export interface ChapterBatchResumeResponse {
  message: string;
  batch_id: string;
  project_id?: string;
  task_type?: ChapterGenerationTaskType;
  status: string;
  stage_code?: string | null;
  execution_mode?: 'interactive' | 'auto' | null;
  checkpoint?: Record<string, unknown> | null;
  resumed_from_batch_id?: string;
  total_chapters: number;
  completed_chapters: number;
  created_at?: string | null;
}

export interface ChapterSingleGenerateResponse {
  task_id: string;
  chapter_id: string;
  status: string;
  message: string;
  estimated_time_minutes?: number;
  active_story_repair_payload?: ActiveStoryRepairPayload | null;
}