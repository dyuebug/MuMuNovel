import { create } from 'zustand';
import type { AnalysisTask } from '../types';

type AnalysisTaskMap = Record<string, AnalysisTask>;

export const areAnalysisTaskSnapshotsEqual = (
  leftTask?: AnalysisTask | null,
  rightTask?: AnalysisTask | null,
) => (
  leftTask?.has_task === rightTask?.has_task
  && leftTask?.task_id === rightTask?.task_id
  && leftTask?.chapter_id === rightTask?.chapter_id
  && leftTask?.status === rightTask?.status
  && leftTask?.progress === rightTask?.progress
  && leftTask?.error_message === rightTask?.error_message
  && leftTask?.error_code === rightTask?.error_code
  && leftTask?.auto_recovered === rightTask?.auto_recovered
  && leftTask?.created_at === rightTask?.created_at
  && leftTask?.started_at === rightTask?.started_at
  && leftTask?.completed_at === rightTask?.completed_at
  && leftTask?.latest_quality_metrics === rightTask?.latest_quality_metrics
  && leftTask?.quality_metrics_summary === rightTask?.quality_metrics_summary
  && leftTask?.quality_profile_summary === rightTask?.quality_profile_summary
);

const areAnalysisTaskMapsEqual = (
  left: AnalysisTaskMap,
  right: AnalysisTaskMap,
) => {
  const leftKeys = Object.keys(left);
  const rightKeys = Object.keys(right);

  if (leftKeys.length !== rightKeys.length) {
    return false;
  }

  return leftKeys.every((key) => areAnalysisTaskSnapshotsEqual(left[key], right[key]));
};

interface ChapterAnalysisUiState {
  tasksMap: AnalysisTaskMap;
  setTasksMap: (next: AnalysisTaskMap | ((prev: AnalysisTaskMap) => AnalysisTaskMap)) => void;
  resetTasksMap: () => void;
}

export const useChapterAnalysisUiStore = create<ChapterAnalysisUiState>()((set) => ({
  tasksMap: {},
  setTasksMap: (next) => {
    set((state) => {
      const resolved = typeof next === 'function'
        ? (next as (prev: AnalysisTaskMap) => AnalysisTaskMap)(state.tasksMap)
        : next;

      if (areAnalysisTaskMapsEqual(state.tasksMap, resolved)) {
        return state;
      }

      return {
        tasksMap: resolved,
      };
    });
  },
  resetTasksMap: () => {
    set((state) => {
      if (Object.keys(state.tasksMap).length === 0) {
        return state;
      }

      return {
        tasksMap: {},
      };
    });
  },
}));
