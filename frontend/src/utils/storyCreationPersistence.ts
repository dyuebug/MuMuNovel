import type { CreativeMode, PlotStage, StoryFocus } from '../types';
import {
  normalizeStoryBeatPlannerDraft,
  normalizeStorySceneOutlineDraft,
  type PersistedStoryCreationDraft,
  type StoryBeatPlannerDraft,
  type StoryCreationSnapshot,
  type StoryCreationSnapshotReason,
  type StoryCreationSnapshotScope,
  type StorySceneOutlineDraft,
} from './storyCreationDraft';
import {
  buildStoryBeatPlannerPrompt,
  buildStoryCreationPromptLayerLabels,
  buildStorySceneOutlinePrompt,
} from './storyCreationPrompt';

const STORY_CREATION_DRAFT_STORAGE_KEY = 'chapter_story_creation_draft_v1';
const STORY_CREATION_SNAPSHOT_STORAGE_KEY = 'chapter_story_creation_snapshot_v1';
const STORY_CREATION_SNAPSHOT_LIMIT = 12;

const normalizePersistedStoryCreationDraft = (value: unknown): PersistedStoryCreationDraft | null => {
  if (!value || typeof value !== 'object') {
    return null;
  }

  const draft = value as Record<string, unknown>;

  return {
    creativeMode: typeof draft.creativeMode === 'string' ? draft.creativeMode as CreativeMode : undefined,
    storyFocus: typeof draft.storyFocus === 'string' ? draft.storyFocus as StoryFocus : undefined,
    plotStage: typeof draft.plotStage === 'string' ? draft.plotStage as PlotStage : undefined,
    narrativePerspective: typeof draft.narrativePerspective === 'string' ? draft.narrativePerspective : undefined,
    storyCreationBriefDraft: typeof draft.storyCreationBriefDraft === 'string' ? draft.storyCreationBriefDraft : undefined,
    beatPlannerDraft: normalizeStoryBeatPlannerDraft(draft.beatPlannerDraft as Partial<StoryBeatPlannerDraft> | null),
    sceneOutlineDraft: normalizeStorySceneOutlineDraft(draft.sceneOutlineDraft as Partial<StorySceneOutlineDraft> | null),
    isBriefCustomized: draft.isBriefCustomized === true,
    isBeatPlannerCustomized: draft.isBeatPlannerCustomized === true,
    isSceneOutlineCustomized: draft.isSceneOutlineCustomized === true,
    updatedAt: typeof draft.updatedAt === 'string' ? draft.updatedAt : undefined,
  };
};

const readPersistedStoryCreationDraftMap = (): Record<string, PersistedStoryCreationDraft> => {
  try {
    const raw = localStorage.getItem(STORY_CREATION_DRAFT_STORAGE_KEY);

    if (!raw) {
      return {};
    }

    const parsed = JSON.parse(raw) as Record<string, unknown>;

    if (!parsed || typeof parsed !== 'object') {
      return {};
    }

    const normalized: Record<string, PersistedStoryCreationDraft> = {};

    Object.entries(parsed).forEach(([storageKey, value]) => {
      const normalizedDraft = normalizePersistedStoryCreationDraft(value);

      if (normalizedDraft) {
        normalized[storageKey] = normalizedDraft;
      }
    });

    return normalized;
  } catch (error) {
    console.warn('Failed to read persisted story creation drafts.', error);
    return {};
  }
};

const writePersistedStoryCreationDraftMap = (map: Record<string, PersistedStoryCreationDraft>): void => {
  try {
    localStorage.setItem(STORY_CREATION_DRAFT_STORAGE_KEY, JSON.stringify(map));
  } catch (error) {
    console.warn('Failed to persist story creation drafts.', error);
  }
};

export const persistStoryCreationDraft = (storageKey: string, draft: PersistedStoryCreationDraft): void => {
  const map = readPersistedStoryCreationDraftMap();
  map[storageKey] = draft;
  writePersistedStoryCreationDraftMap(map);
};

export const getPersistedStoryCreationDraft = (storageKey: string): PersistedStoryCreationDraft | undefined => {
  const map = readPersistedStoryCreationDraftMap();
  return map[storageKey];
};

const normalizeStoryCreationSnapshot = (value: unknown): StoryCreationSnapshot | null => {
  const normalizedDraft = normalizePersistedStoryCreationDraft(value);

  if (!normalizedDraft || !value || typeof value !== 'object') {
    return null;
  }

  const snapshot = value as Record<string, unknown>;
  const scope = snapshot.scope === 'single' || snapshot.scope === 'batch'
    ? snapshot.scope as StoryCreationSnapshotScope
    : null;
  const reason = snapshot.reason === 'manual' || snapshot.reason === 'generate'
    ? snapshot.reason as StoryCreationSnapshotReason
    : null;

  if (!scope || !reason || typeof snapshot.id !== 'string' || !snapshot.id.trim()) {
    return null;
  }

  const normalizedPrompt = typeof snapshot.prompt === 'string' ? snapshot.prompt.trim() : '';
  const normalizedBeatPrompt = buildStoryBeatPlannerPrompt(normalizedDraft.beatPlannerDraft, scope);
  const normalizedScenePrompt = buildStorySceneOutlinePrompt(normalizedDraft.sceneOutlineDraft, scope);
  const normalizedLayerLabels = Array.isArray(snapshot.promptLayerLabels)
    ? snapshot.promptLayerLabels
      .filter((item): item is string => typeof item === 'string' && Boolean(item.trim()))
      .map((item) => item.trim())
    : buildStoryCreationPromptLayerLabels({
      summary: normalizedDraft.storyCreationBriefDraft,
      beat: normalizedBeatPrompt,
      scene: normalizedScenePrompt,
    });
  const createdAt = typeof snapshot.createdAt === 'string' && snapshot.createdAt.trim()
    ? snapshot.createdAt
    : normalizedDraft.updatedAt ?? new Date(0).toISOString();
  const normalizedPromptCharCount = typeof snapshot.promptCharCount === 'number' && Number.isFinite(snapshot.promptCharCount)
    ? Math.max(0, Math.round(snapshot.promptCharCount))
    : normalizedPrompt.length;

  return {
    ...normalizedDraft,
    id: snapshot.id.trim(),
    scope,
    createdAt,
    reason,
    label: typeof snapshot.label === 'string' && snapshot.label.trim()
      ? snapshot.label.trim()
      : reason === 'generate'
        ? '????'
        : '????',
    prompt: normalizedPrompt || undefined,
    promptLayerLabels: normalizedLayerLabels,
    promptCharCount: normalizedPromptCharCount,
  };
};

const resolveStoryCreationSnapshotTimestamp = (value?: string): number => {
  const timestamp = value ? new Date(value).getTime() : Number.NaN;
  return Number.isFinite(timestamp) ? timestamp : 0;
};

const readPersistedStoryCreationSnapshotMap = (): Record<string, StoryCreationSnapshot[]> => {
  try {
    const raw = localStorage.getItem(STORY_CREATION_SNAPSHOT_STORAGE_KEY);

    if (!raw) {
      return {};
    }

    const parsed = JSON.parse(raw) as Record<string, unknown>;

    if (!parsed || typeof parsed !== 'object') {
      return {};
    }

    const normalized: Record<string, StoryCreationSnapshot[]> = {};

    Object.entries(parsed).forEach(([storageKey, value]) => {
      if (!Array.isArray(value)) {
        return;
      }

      const snapshots = value
        .map((item) => normalizeStoryCreationSnapshot(item))
        .filter((item): item is StoryCreationSnapshot => Boolean(item))
        .sort((left, right) => (
          resolveStoryCreationSnapshotTimestamp(right.createdAt)
          - resolveStoryCreationSnapshotTimestamp(left.createdAt)
        ))
        .slice(0, STORY_CREATION_SNAPSHOT_LIMIT);

      if (snapshots.length > 0) {
        normalized[storageKey] = snapshots;
      }
    });

    return normalized;
  } catch (error) {
    console.warn('Failed to read story creation snapshots.', error);
    return {};
  }
};

const writePersistedStoryCreationSnapshotMap = (map: Record<string, StoryCreationSnapshot[]>): void => {
  try {
    localStorage.setItem(STORY_CREATION_SNAPSHOT_STORAGE_KEY, JSON.stringify(map));
  } catch (error) {
    console.warn('Failed to persist story creation snapshots.', error);
  }
};

export const getPersistedStoryCreationSnapshots = (storageKey: string): StoryCreationSnapshot[] => {
  const map = readPersistedStoryCreationSnapshotMap();
  return map[storageKey] ?? [];
};

export const persistStoryCreationSnapshot = (storageKey: string, snapshot: StoryCreationSnapshot): StoryCreationSnapshot[] => {
  const map = readPersistedStoryCreationSnapshotMap();
  const nextSnapshots = [
    snapshot,
    ...(map[storageKey] ?? []).filter((item) => item.id !== snapshot.id),
  ]
    .sort((left, right) => (
      resolveStoryCreationSnapshotTimestamp(right.createdAt)
      - resolveStoryCreationSnapshotTimestamp(left.createdAt)
    ))
    .slice(0, STORY_CREATION_SNAPSHOT_LIMIT);

  map[storageKey] = nextSnapshots;
  writePersistedStoryCreationSnapshotMap(map);
  return nextSnapshots;
};

export const removePersistedStoryCreationSnapshot = (storageKey: string, snapshotId: string): StoryCreationSnapshot[] => {
  const map = readPersistedStoryCreationSnapshotMap();
  const nextSnapshots = (map[storageKey] ?? []).filter((item) => item.id !== snapshotId);

  if (nextSnapshots.length > 0) {
    map[storageKey] = nextSnapshots;
  } else {
    delete map[storageKey];
  }

  writePersistedStoryCreationSnapshotMap(map);
  return nextSnapshots;
};

export const buildStoryCreationSnapshotId = (): string => {
  if (typeof globalThis.crypto?.randomUUID === 'function') {
    return globalThis.crypto.randomUUID();
  }

  return `snapshot_${Date.now()}_${Math.random().toString(36).slice(2, 8)}`;
};
