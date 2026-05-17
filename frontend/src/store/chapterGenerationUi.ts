import { create } from 'zustand';
import type {
  ActiveStoryRepairPayload,
  ChapterLatestQualityMetrics,
  ChapterQualityMetricsSummary,
  ChapterQualityProfileSummary,
} from '../types';

type BatchGenerationCheckpointCompactionDetail = {
  before?: number | null;
  after?: number | null;
};

export type BatchGenerationCheckpointUiState = {
  current_chapter_number?: number | null;
  candidate_index?: number | null;
  candidate_count?: number | null;
  word_count?: number | null;
  generation_path?: string | null;
  attempt_kind?: string | null;
  rerank_used?: boolean | null;
  word_budget_repair_used?: boolean | null;
  winner_candidate_index?: number | null;
  pre_compaction_total_length?: number | null;
  context_budget_limit?: number | null;
  compaction_applied?: boolean | null;
  compaction_details?: Record<string, BatchGenerationCheckpointCompactionDetail> | null;
};

export type BatchGenerationProgressUiState = {
  status: string;
  total: number;
  completed: number;
  current_chapter_number: number | null;
  progress_percent?: number;
  checkpoint?: BatchGenerationCheckpointUiState | null;
  estimated_time_minutes?: number;
  latest_quality_metrics?: ChapterLatestQualityMetrics | null;
  quality_metrics_summary?: ChapterQualityMetricsSummary | null;
  quality_profile_summary?: ChapterQualityProfileSummary | null;
  failed_chapters?: Array<Record<string, unknown>>;
  active_story_repair_payload?: ActiveStoryRepairPayload | null;
};

type SingleChapterGenerationOverlayState = {
  loading: boolean;
  progress: number;
  message: string;
};

type SingleChapterGenerationOverlayUpdate =
  Partial<SingleChapterGenerationOverlayState>;

interface ChapterGenerationUiState {
  singleOverlay: SingleChapterGenerationOverlayState;
  batchProgress: BatchGenerationProgressUiState | null;
  setSingleOverlay: (update: SingleChapterGenerationOverlayUpdate) => void;
  resetSingleOverlay: () => void;
  setBatchProgress: (nextState: BatchGenerationProgressUiState | null) => void;
}

const INITIAL_SINGLE_OVERLAY_STATE: SingleChapterGenerationOverlayState = {
  loading: false,
  progress: 0,
  message: '',
};

const areBatchProgressStatesEqual = (
  left: BatchGenerationProgressUiState | null,
  right: BatchGenerationProgressUiState | null,
): boolean => {
  if (left === right) {
    return true;
  }
  if (!left || !right) {
    return false;
  }

  return (
    left.status === right.status
    && left.total === right.total
    && left.completed === right.completed
    && left.current_chapter_number === right.current_chapter_number
    && left.progress_percent === right.progress_percent
    && left.estimated_time_minutes === right.estimated_time_minutes
    && JSON.stringify(left.checkpoint ?? null) === JSON.stringify(right.checkpoint ?? null)
    && JSON.stringify(left.latest_quality_metrics ?? null) === JSON.stringify(right.latest_quality_metrics ?? null)
    && JSON.stringify(left.quality_metrics_summary ?? null) === JSON.stringify(right.quality_metrics_summary ?? null)
    && JSON.stringify(left.quality_profile_summary ?? null) === JSON.stringify(right.quality_profile_summary ?? null)
    && JSON.stringify(left.failed_chapters ?? []) === JSON.stringify(right.failed_chapters ?? [])
    && JSON.stringify(left.active_story_repair_payload ?? null) === JSON.stringify(right.active_story_repair_payload ?? null)
  );
};

export const useChapterGenerationUiStore = create<ChapterGenerationUiState>()((set) => ({
  singleOverlay: INITIAL_SINGLE_OVERLAY_STATE,
  batchProgress: null,
  setSingleOverlay: (update) => {
    set((state) => {
      const nextState = {
        ...state.singleOverlay,
        ...update,
      };

      if (
        nextState.loading === state.singleOverlay.loading
        && nextState.progress === state.singleOverlay.progress
        && nextState.message === state.singleOverlay.message
      ) {
        return state;
      }

      return {
        singleOverlay: nextState,
      };
    });
  },
  resetSingleOverlay: () => {
    set((state) => {
      if (
        state.singleOverlay.loading === INITIAL_SINGLE_OVERLAY_STATE.loading
        && state.singleOverlay.progress === INITIAL_SINGLE_OVERLAY_STATE.progress
        && state.singleOverlay.message === INITIAL_SINGLE_OVERLAY_STATE.message
      ) {
        return state;
      }

      return {
        singleOverlay: INITIAL_SINGLE_OVERLAY_STATE,
      };
    });
  },
  setBatchProgress: (nextState) => {
    set((state) => {
      if (areBatchProgressStatesEqual(state.batchProgress, nextState)) {
        return state;
      }

      return {
        batchProgress: nextState,
      };
    });
  },
}));
