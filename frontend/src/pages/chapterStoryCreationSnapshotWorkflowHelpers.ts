import type {
  PersistedStoryCreationDraft,
  StoryBeatPlannerDraft,
  StoryCreationSnapshot,
  StoryCreationSnapshotReason,
  StorySceneOutlineDraft,
} from '../utils/storyCreationDraft';
import type { ResolveStoryCreationPromptState } from './chapterStoryCreationPromptHelpers';
import type { LoadStoryCreationPersistence } from './chapterStoryCreationSnapshotHelpers';
import { saveSingleStoryCreationSnapshotCallback, saveBatchStoryCreationSnapshotCallback } from './chapterStoryCreationSnapshotSaveHelpers';
import { applySingleStoryCreationSnapshotCallback, applyBatchStoryCreationSnapshotCallback } from './chapterStoryCreationSnapshotApplyHelpers';
import { deleteSingleStoryCreationSnapshotCallback, deleteBatchStoryCreationSnapshotCallback } from './chapterStoryCreationSnapshotDeleteHelpers';

export function saveSingleStoryCreationSnapshotWorkflow({
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
  return saveSingleStoryCreationSnapshotCallback({
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
  });
}

export function saveBatchStoryCreationSnapshotWorkflow({
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
  return saveBatchStoryCreationSnapshotCallback({
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
  });
}

export function applySingleStoryCreationSnapshotWorkflow({
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
}: {
  snapshot: StoryCreationSnapshot;
  manualBriefSentinel: string;
  setAutoBriefRef: (value: string) => void;
  setBeatPlannerAutoRef: (value: StoryBeatPlannerDraft) => void;
  setSceneOutlineAutoRef: (value: StorySceneOutlineDraft) => void;
  setTemporaryNarrativePerspective: (value: string | undefined) => void;
  setSelectedCreativeMode: (value: StoryCreationSnapshot['creativeMode']) => void;
  setSelectedStoryFocus: (value: StoryCreationSnapshot['storyFocus']) => void;
  setSelectedPlotStage: (value: StoryCreationSnapshot['plotStage']) => void;
  setStoryCreationBriefDraft: (value: string) => void;
  setBeatPlannerDraft: (value: StoryBeatPlannerDraft) => void;
  setSceneOutlineDraft: (value: StorySceneOutlineDraft) => void;
  inferPlotStage: (args: { chapterNumber?: number | null; totalChapters: number }) => Promise<StoryCreationSnapshot['plotStage']>;
  chapterNumber?: number | null;
  totalChapters: number;
}): void {
  applySingleStoryCreationSnapshotCallback({
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
  });
}

export function applyBatchStoryCreationSnapshotWorkflow({
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
}: {
  snapshot: StoryCreationSnapshot;
  manualBriefSentinel: string;
  setAutoBriefRef: (value: string) => void;
  setBeatPlannerAutoRef: (value: StoryBeatPlannerDraft) => void;
  setSceneOutlineAutoRef: (value: StorySceneOutlineDraft) => void;
  setSelectedCreativeMode: (value: StoryCreationSnapshot['creativeMode']) => void;
  setSelectedStoryFocus: (value: StoryCreationSnapshot['storyFocus']) => void;
  setSelectedPlotStage: (value: StoryCreationSnapshot['plotStage']) => void;
  setStoryCreationBriefDraft: (value: string) => void;
  setBeatPlannerDraft: (value: StoryBeatPlannerDraft) => void;
  setSceneOutlineDraft: (value: StorySceneOutlineDraft) => void;
}): void {
  applyBatchStoryCreationSnapshotCallback({
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
  });
}

export function deleteSingleStoryCreationSnapshotWorkflow({
  storageKey,
  snapshotId,
  loadStoryCreationPersistence,
  setSnapshots,
}: {
  storageKey: string | null;
  snapshotId: string;
  loadStoryCreationPersistence: LoadStoryCreationPersistence;
  setSnapshots: (snapshots: StoryCreationSnapshot[]) => void;
}): Promise<void> {
  return deleteSingleStoryCreationSnapshotCallback({
    storageKey,
    snapshotId,
    loadStoryCreationPersistence,
    setSnapshots,
  });
}

export function deleteBatchStoryCreationSnapshotWorkflow({
  storageKey,
  snapshotId,
  loadStoryCreationPersistence,
  setSnapshots,
}: {
  storageKey: string | null;
  snapshotId: string;
  loadStoryCreationPersistence: LoadStoryCreationPersistence;
  setSnapshots: (snapshots: StoryCreationSnapshot[]) => void;
}): Promise<void> {
  return deleteBatchStoryCreationSnapshotCallback({
    storageKey,
    snapshotId,
    loadStoryCreationPersistence,
    setSnapshots,
  });
}
