import type { BatchTaskMeta } from './chapterBatchGenerationPollingHelpers';

const BATCH_TASK_META_STORAGE_KEY = 'chapter_batch_task_meta_map_v1';

const isValidBatchTaskMeta = (value: unknown): value is BatchTaskMeta => {
  if (!value || typeof value !== 'object') {
    return false;
  }

  const meta = value as Record<string, unknown>;
  return (
    typeof meta.startChapterNumber === 'number'
    && typeof meta.count === 'number'
    && typeof meta.autoAnalyze === 'boolean'
  );
};

const readPersistedBatchTaskMetaMap = (): Record<string, BatchTaskMeta> => {
  try {
    const raw = localStorage.getItem(BATCH_TASK_META_STORAGE_KEY);
    if (!raw) {
      return {};
    }

    const parsed = JSON.parse(raw) as Record<string, unknown>;
    if (!parsed || typeof parsed !== 'object') {
      return {};
    }

    const normalized: Record<string, BatchTaskMeta> = {};
    Object.entries(parsed).forEach(([taskId, value]) => {
      if (isValidBatchTaskMeta(value)) {
        normalized[taskId] = value;
      }
    });
    return normalized;
  } catch (error) {
    console.warn('Failed to read persisted batch task metadata.', error);
    return {};
  }
};

const writePersistedBatchTaskMetaMap = (map: Record<string, BatchTaskMeta>): void => {
  try {
    localStorage.setItem(BATCH_TASK_META_STORAGE_KEY, JSON.stringify(map));
  } catch (error) {
    console.warn('Failed to persist batch task metadata.', error);
  }
};

export const persistChapterBatchTaskMeta = (taskId: string, meta: BatchTaskMeta): void => {
  const map = readPersistedBatchTaskMetaMap();
  map[taskId] = meta;
  writePersistedBatchTaskMetaMap(map);
};

export const getPersistedChapterBatchTaskMeta = (
  taskId: string,
  projectId?: string,
): BatchTaskMeta | undefined => {
  const map = readPersistedBatchTaskMetaMap();
  const meta = map[taskId];
  if (!meta) {
    return undefined;
  }

  if (projectId && meta.projectId && meta.projectId !== projectId) {
    return undefined;
  }

  return meta;
};

export const removePersistedChapterBatchTaskMeta = (taskId: string): void => {
  const map = readPersistedBatchTaskMetaMap();
  if (!(taskId in map)) {
    return;
  }

  delete map[taskId];
  writePersistedBatchTaskMetaMap(map);
};
