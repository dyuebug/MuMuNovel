import type { StoryCreationSnapshot } from '../utils/storyCreationDraft';
import type { StoryBeatPlannerDraft, StorySceneOutlineDraft } from '../utils/storyCreationDraft';
import {
  applyBatchChapterStoryCreationSnapshot,
  applySingleChapterStoryCreationSnapshot,
} from './chapterStoryCreationSnapshotHelpers';

export function applySingleStoryCreationSnapshotCallback({
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
  applySingleChapterStoryCreationSnapshot({
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
    successMessage: (label) => `\u5DF2\u5E94\u7528\u5FEB\u7167\uFF1A${label}`,
  });
}

export function applyBatchStoryCreationSnapshotCallback({
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
  applyBatchChapterStoryCreationSnapshot({
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
    successMessage: (label) => `\u5DF2\u52A0\u8F7D\u5FEB\u7167\uFF1A${label}`,
  });
}