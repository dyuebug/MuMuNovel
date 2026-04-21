import { message } from 'antd';
import {
  EMPTY_STORY_BEAT_PLANNER_DRAFT,
  EMPTY_STORY_SCENE_OUTLINE_DRAFT,
  areStoryCreationDraftContentsEqual,
  hasMeaningfulStoryCreationDraft,
  normalizeOptionalText,
  normalizeStoryBeatPlannerDraft,
  normalizeStorySceneOutlineDraft,
  type PersistedStoryCreationDraft,
  type StoryBeatPlannerDraft,
  type StoryCreationSnapshot,
  type StoryCreationSnapshotReason,
  type StoryCreationSnapshotScope,
  type StorySceneOutlineDraft,
} from '../utils/storyCreationDraft';
import type { ResolveStoryCreationPromptState } from './chapterStoryCreationPromptHelpers';


export type LoadStoryCreationPersistence = () => Promise<{
  buildStoryCreationSnapshotId: () => string;
  persistStoryCreationSnapshot: (storageKey: string, snapshot: StoryCreationSnapshot) => StoryCreationSnapshot[];
  removePersistedStoryCreationSnapshot: (storageKey: string, snapshotId: string) => StoryCreationSnapshot[];
}>;

type ApplySnapshotDraftRefsArgs = {
  snapshot: StoryCreationSnapshot;
  manualBriefSentinel: string;
  setAutoBriefRef: (value: string) => void;
  setBeatPlannerAutoRef: (value: StoryBeatPlannerDraft) => void;
  setSceneOutlineAutoRef: (value: StorySceneOutlineDraft) => void;
};

const applySnapshotDraftRefs = ({
  snapshot,
  manualBriefSentinel,
  setAutoBriefRef,
  setBeatPlannerAutoRef,
  setSceneOutlineAutoRef,
}: ApplySnapshotDraftRefsArgs): void => {
  setAutoBriefRef(
    snapshot.isBriefCustomized ? manualBriefSentinel : (snapshot.storyCreationBriefDraft ?? ''),
  );
  setBeatPlannerAutoRef(
    snapshot.isBeatPlannerCustomized
      ? { ...EMPTY_STORY_BEAT_PLANNER_DRAFT }
      : normalizeStoryBeatPlannerDraft(snapshot.beatPlannerDraft),
  );
  setSceneOutlineAutoRef(
    snapshot.isSceneOutlineCustomized
      ? { ...EMPTY_STORY_SCENE_OUTLINE_DRAFT }
      : normalizeStorySceneOutlineDraft(snapshot.sceneOutlineDraft),
  );
};

export async function saveChapterStoryCreationSnapshot({
  scope,
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
  buildFallbackLabel,
  emptyDraftMessage,
  duplicateManualMessage,
  savedGenerateMessage,
  savedManualMessage,
  includeNarrativePerspective = false,
  options,
}: {
  scope: StoryCreationSnapshotScope;
  reason: StoryCreationSnapshotReason;
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
  buildFallbackLabel: (reason: StoryCreationSnapshotReason) => string;
  emptyDraftMessage: string;
  duplicateManualMessage: string;
  savedGenerateMessage: string;
  savedManualMessage: string;
  includeNarrativePerspective?: boolean;
  options?: { silent?: boolean; label?: string };
}): Promise<StoryCreationSnapshot | null> {
  if (!storageKey) {
    return null;
  }

  if (!hasMeaningfulStoryCreationDraft(currentDraft)) {
    if (!options?.silent) {
      message.warning(emptyDraftMessage);
    }
    return null;
  }

  const { prompt, promptLayerLabels } = resolveStoryCreationPromptState({
    scope,
    briefDraft,
    defaultBrief,
    beatPlannerDraft,
    sceneOutlineDraft,
  });
  const normalizedPrompt = prompt?.trim();
  const latestSnapshot = currentSnapshots[0];

  if (
    latestSnapshot
    && latestSnapshot.reason === reason
    && areStoryCreationDraftContentsEqual(latestSnapshot, currentDraft, { includeNarrativePerspective })
    && normalizeOptionalText(latestSnapshot.prompt) === normalizeOptionalText(normalizedPrompt)
  ) {
    if (!options?.silent && reason === 'manual') {
      message.info(duplicateManualMessage);
    }
    return latestSnapshot;
  }

  const createdAt = new Date().toISOString();
  const { buildStoryCreationSnapshotId, persistStoryCreationSnapshot } = await loadStoryCreationPersistence();
  const snapshot: StoryCreationSnapshot = {
    ...currentDraft,
    id: buildStoryCreationSnapshotId(),
    scope,
    createdAt,
    updatedAt: createdAt,
    reason,
    label: options?.label?.trim() || buildFallbackLabel(reason),
    prompt: normalizedPrompt || undefined,
    promptLayerLabels: [...promptLayerLabels],
    promptCharCount: normalizedPrompt?.length ?? 0,
  };

  const nextSnapshots = persistStoryCreationSnapshot(storageKey, snapshot);
  setSnapshots(nextSnapshots);

  if (!options?.silent) {
    message.success(reason === 'generate' ? savedGenerateMessage : savedManualMessage);
  }

  return nextSnapshots[0] ?? snapshot;
}

export function applySingleChapterStoryCreationSnapshot({
  snapshot,
  manualBriefSentinel,
  setAutoBriefRef,
  setBeatPlannerAutoRef,
  setSceneOutlineAutoRef,
  setTemporaryNarrativePerspective,
  setSelectedCreativeMode,
  setSelectedStoryFocus,
  setSelectedPlotStage,
  setStoryCreationBriefDraft,
  setBeatPlannerDraft,
  setSceneOutlineDraft,
  inferPlotStage,
  chapterNumber,
  totalChapters,
  successMessage,
}: {
  snapshot: StoryCreationSnapshot;
  manualBriefSentinel: string;
  setAutoBriefRef: (value: string) => void;
  setBeatPlannerAutoRef: (value: StoryBeatPlannerDraft) => void;
  setSceneOutlineAutoRef: (value: StorySceneOutlineDraft) => void;
  setTemporaryNarrativePerspective: (value: string | undefined) => void;
  setSelectedCreativeMode: (value: PersistedStoryCreationDraft['creativeMode']) => void;
  setSelectedStoryFocus: (value: PersistedStoryCreationDraft['storyFocus']) => void;
  setSelectedPlotStage: (value: PersistedStoryCreationDraft['plotStage']) => void;
  setStoryCreationBriefDraft: (value: string) => void;
  setBeatPlannerDraft: (value: StoryBeatPlannerDraft) => void;
  setSceneOutlineDraft: (value: StorySceneOutlineDraft) => void;
  inferPlotStage: (args: { chapterNumber?: number | null; totalChapters: number }) => Promise<PersistedStoryCreationDraft['plotStage']>;
  chapterNumber?: number | null;
  totalChapters: number;
  successMessage: (label: string) => string;
}): void {
  applySnapshotDraftRefs({
    snapshot,
    manualBriefSentinel,
    setAutoBriefRef,
    setBeatPlannerAutoRef,
    setSceneOutlineAutoRef,
  });

  setTemporaryNarrativePerspective(snapshot.narrativePerspective);
  setSelectedCreativeMode(snapshot.creativeMode);
  setSelectedStoryFocus(snapshot.storyFocus);
  setSelectedPlotStage(snapshot.plotStage);

  if (!snapshot.plotStage) {
    void inferPlotStage({ chapterNumber, totalChapters }).then((stage) => {
      setSelectedPlotStage(stage);
    });
  }

  setStoryCreationBriefDraft(snapshot.storyCreationBriefDraft ?? '');
  setBeatPlannerDraft(normalizeStoryBeatPlannerDraft(snapshot.beatPlannerDraft));
  setSceneOutlineDraft(normalizeStorySceneOutlineDraft(snapshot.sceneOutlineDraft));
  message.success(successMessage(snapshot.label));
}

export function applyBatchChapterStoryCreationSnapshot({
  snapshot,
  manualBriefSentinel,
  setAutoBriefRef,
  setBeatPlannerAutoRef,
  setSceneOutlineAutoRef,
  setSelectedCreativeMode,
  setSelectedStoryFocus,
  setSelectedPlotStage,
  setStoryCreationBriefDraft,
  setBeatPlannerDraft,
  setSceneOutlineDraft,
  successMessage,
}: {
  snapshot: StoryCreationSnapshot;
  manualBriefSentinel: string;
  setAutoBriefRef: (value: string) => void;
  setBeatPlannerAutoRef: (value: StoryBeatPlannerDraft) => void;
  setSceneOutlineAutoRef: (value: StorySceneOutlineDraft) => void;
  setSelectedCreativeMode: (value: PersistedStoryCreationDraft['creativeMode']) => void;
  setSelectedStoryFocus: (value: PersistedStoryCreationDraft['storyFocus']) => void;
  setSelectedPlotStage: (value: PersistedStoryCreationDraft['plotStage']) => void;
  setStoryCreationBriefDraft: (value: string) => void;
  setBeatPlannerDraft: (value: StoryBeatPlannerDraft) => void;
  setSceneOutlineDraft: (value: StorySceneOutlineDraft) => void;
  successMessage: (label: string) => string;
}): void {
  applySnapshotDraftRefs({
    snapshot,
    manualBriefSentinel,
    setAutoBriefRef,
    setBeatPlannerAutoRef,
    setSceneOutlineAutoRef,
  });

  setSelectedCreativeMode(snapshot.creativeMode);
  setSelectedStoryFocus(snapshot.storyFocus);
  setSelectedPlotStage(snapshot.plotStage);
  setStoryCreationBriefDraft(snapshot.storyCreationBriefDraft ?? '');
  setBeatPlannerDraft(normalizeStoryBeatPlannerDraft(snapshot.beatPlannerDraft));
  setSceneOutlineDraft(normalizeStorySceneOutlineDraft(snapshot.sceneOutlineDraft));
  message.success(successMessage(snapshot.label));
}

export async function deleteChapterStoryCreationSnapshot({
  storageKey,
  snapshotId,
  loadStoryCreationPersistence,
  setSnapshots,
  successMessage,
}: {
  storageKey: string | null;
  snapshotId: string;
  loadStoryCreationPersistence: LoadStoryCreationPersistence;
  setSnapshots: (snapshots: StoryCreationSnapshot[]) => void;
  successMessage: string;
}): Promise<void> {
  if (!storageKey) {
    return;
  }

  const { removePersistedStoryCreationSnapshot } = await loadStoryCreationPersistence();
  const nextSnapshots = removePersistedStoryCreationSnapshot(storageKey, snapshotId);
  setSnapshots(nextSnapshots);
  message.success(successMessage);
}
