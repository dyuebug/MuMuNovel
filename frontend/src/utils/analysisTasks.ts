import type { AnalysisTask } from '../types';

type AnalysisTaskLike = Pick<AnalysisTask, 'status' | 'error_code'> | null | undefined;

export const isAnalysisTaskRetrying = (task: AnalysisTaskLike): boolean => (
  task?.status === 'failed' && task?.error_code === 'retrying'
);
