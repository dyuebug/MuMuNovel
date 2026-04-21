import type {
  PersistedStoryCreationDraft,
  StoryBeatPlannerDraft,
  StoryCreationSnapshot,
  StoryCreationSnapshotReason,
  StorySceneOutlineDraft,
} from '../utils/storyCreationDraft';
import type { ResolveStoryCreationPromptState } from './chapterStoryCreationPromptHelpers';
import {
  saveChapterStoryCreationSnapshot,
  type LoadStoryCreationPersistence,
} from './chapterStoryCreationSnapshotHelpers';

export function saveSingleStoryCreationSnapshotCallback({
  reason,
  options,
  storageKey,
  currentDraft,
  currentSnapshots,
  briefDraft,
  defaultBrief,
  beatPlannerDraft,
  sceneOutlineDraft,
  resolveStoryCreationPromptState,
  loadStoryCreationPersistence,
  setSnapshots,
  chapterNumber,
}: {
  reason: StoryCreationSnapshotReason;
  options?: { silent?: boolean; label?: string };
  storageKey: string | null;
  currentDraft: PersistedStoryCreationDraft;
  currentSnapshots: StoryCreationSnapshot[];
  briefDraft: string;
  defaultBrief: string;
  beatPlannerDraft: StoryBeatPlannerDraft;
  sceneOutlineDraft: StorySceneOutlineDraft;
  resolveStoryCreationPromptState: ResolveStoryCreationPromptState;
  loadStoryCreationPersistence: LoadStoryCreationPersistence;
  setSnapshots: (snapshots: StoryCreationSnapshot[]) => void;
  chapterNumber?: number | null;
}): Promise<StoryCreationSnapshot | null> {
  return saveChapterStoryCreationSnapshot({
    scope: 'single',
    reason,
    storageKey,
    currentDraft,
    currentSnapshots,
    briefDraft,
    defaultBrief,
    beatPlannerDraft,
    sceneOutlineDraft,
    resolveStoryCreationPromptState,
    loadStoryCreationPersistence,
    setSnapshots,
    buildFallbackLabel: (nextReason) => {
      const chapterLabel = chapterNumber ? `\u7B2C${chapterNumber}\u7AE0` : '\u672A\u547D\u540D\u7AE0\u8282';
      return `${chapterLabel} / ${nextReason === 'generate' ? '\u751F\u6210\u524D\u5FEB\u7167' : '\u624B\u52A8\u4FDD\u5B58'}`;
    },
    emptyDraftMessage: '\u5F53\u524D\u6CA1\u6709\u53EF\u4FDD\u5B58\u7684\u521B\u4F5C\u8349\u7A3F',
    duplicateManualMessage: '\u5F53\u524D\u8349\u7A3F\u4E0E\u6700\u8FD1\u4E00\u6B21\u5FEB\u7167\u4E00\u81F4',
    savedGenerateMessage: '\u5DF2\u4FDD\u5B58\u751F\u6210\u524D\u5FEB\u7167',
    savedManualMessage: '\u5DF2\u4FDD\u5B58\u521B\u4F5C\u5FEB\u7167',
    includeNarrativePerspective: true,
    options,
  });
}

export function saveBatchStoryCreationSnapshotCallback({
  reason,
  options,
  storageKey,
  currentDraft,
  currentSnapshots,
  briefDraft,
  defaultBrief,
  beatPlannerDraft,
  sceneOutlineDraft,
  resolveStoryCreationPromptState,
  loadStoryCreationPersistence,
  setSnapshots,
}: {
  reason: StoryCreationSnapshotReason;
  options?: { silent?: boolean; label?: string };
  storageKey: string | null;
  currentDraft: PersistedStoryCreationDraft;
  currentSnapshots: StoryCreationSnapshot[];
  briefDraft: string;
  defaultBrief: string;
  beatPlannerDraft: StoryBeatPlannerDraft;
  sceneOutlineDraft: StorySceneOutlineDraft;
  resolveStoryCreationPromptState: ResolveStoryCreationPromptState;
  loadStoryCreationPersistence: LoadStoryCreationPersistence;
  setSnapshots: (snapshots: StoryCreationSnapshot[]) => void;
}): Promise<StoryCreationSnapshot | null> {
  return saveChapterStoryCreationSnapshot({
    scope: 'batch',
    reason,
    storageKey,
    currentDraft,
    currentSnapshots,
    briefDraft,
    defaultBrief,
    beatPlannerDraft,
    sceneOutlineDraft,
    resolveStoryCreationPromptState,
    loadStoryCreationPersistence,
    setSnapshots,
    buildFallbackLabel: (nextReason) => `\u6279\u91CF\u7AE0\u8282 / ${nextReason === 'generate' ? '\u751F\u6210\u524D\u5FEB\u7167' : '\u624B\u52A8\u4FDD\u5B58'}`,
    emptyDraftMessage: '\u5F53\u524D\u6CA1\u6709\u53EF\u4FDD\u5B58\u7684\u6279\u91CF\u521B\u4F5C\u8349\u7A3F',
    duplicateManualMessage: '\u5F53\u524D\u6279\u91CF\u8349\u7A3F\u4E0E\u6700\u8FD1\u4E00\u6B21\u5FEB\u7167\u4E00\u81F4',
    savedGenerateMessage: '\u5DF2\u4FDD\u5B58\u6279\u91CF\u751F\u6210\u524D\u5FEB\u7167',
    savedManualMessage: '\u5DF2\u4FDD\u5B58\u6279\u91CF\u521B\u4F5C\u5FEB\u7167',
    options,
  });
}