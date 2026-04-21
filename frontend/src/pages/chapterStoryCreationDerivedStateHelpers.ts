import type { CreativeMode, PlotStage, StoryFocus } from '../types';
import {
  areStoryBeatPlannerDraftsEqual,
  areStorySceneOutlineDraftsEqual,
  isStoryBeatPlannerDraftEmpty,
  isStorySceneOutlineDraftEmpty,
  type PersistedStoryCreationDraft,
  type StoryBeatPlannerDraft,
  type StoryCreationSnapshotScope,
  type StorySceneOutlineDraft,
} from '../utils/storyCreationDraft';
import type { ResolveStoryCreationPromptState } from './chapterStoryCreationPromptHelpers';
import {
  buildStoryCreationCurrentDraft,
  canSaveStoryCreationSnapshot,
} from './chapterStoryCreationCurrentDraftHelpers';

export type StoryCreationDerivedState = {
  defaultBrief: string;
  resolvedBrief?: string;
  promptLayerLabels: string[];
  promptCharCount: number;
  isPromptVerbose: boolean;
  isBriefCustomized: boolean;
  isBeatPlannerCustomized: boolean;
  isSceneOutlineCustomized: boolean;
  isControlCustomized: boolean;
  currentDraft: PersistedStoryCreationDraft;
  canSaveSnapshot: boolean;
};

export function buildStoryCreationDerivedState({
  scope,
  creativeMode,
  storyFocus,
  plotStage,
  narrativePerspective,
  storyCreationBriefDraft,
  systemStoryCreationBrief,
  projectDefaultStoryCreationBrief,
  beatPlannerDraft,
  systemBeatPlannerDraft,
  sceneOutlineDraft,
  suggestedSceneOutlineDraft,
  storageKey,
  hasChapterContext = true,
  resolveStoryCreationPromptState,
}: {
  scope: StoryCreationSnapshotScope;
  creativeMode?: CreativeMode;
  storyFocus?: StoryFocus;
  plotStage?: PlotStage;
  narrativePerspective?: string;
  storyCreationBriefDraft: string;
  systemStoryCreationBrief?: string | null;
  projectDefaultStoryCreationBrief?: string | null;
  beatPlannerDraft: StoryBeatPlannerDraft;
  systemBeatPlannerDraft: StoryBeatPlannerDraft;
  sceneOutlineDraft: StorySceneOutlineDraft;
  suggestedSceneOutlineDraft: StorySceneOutlineDraft;
  storageKey: string | null;
  hasChapterContext?: boolean;
  resolveStoryCreationPromptState: ResolveStoryCreationPromptState;
}): StoryCreationDerivedState {
  const defaultBrief = systemStoryCreationBrief || projectDefaultStoryCreationBrief || '';
  const normalizedStoryCreationBriefDraft = storyCreationBriefDraft.trim();
  const promptState = resolveStoryCreationPromptState({
    scope,
    briefDraft: storyCreationBriefDraft,
    defaultBrief,
    beatPlannerDraft,
    sceneOutlineDraft,
  });

  const isBriefCustomized = Boolean(
    normalizedStoryCreationBriefDraft
    && normalizedStoryCreationBriefDraft !== defaultBrief.trim(),
  );

  const isBeatPlannerCustomized = Boolean(
    !isStoryBeatPlannerDraftEmpty(beatPlannerDraft)
    && !areStoryBeatPlannerDraftsEqual(beatPlannerDraft, systemBeatPlannerDraft),
  );

  const isSceneOutlineCustomized = Boolean(
    !isStorySceneOutlineDraftEmpty(sceneOutlineDraft)
    && !areStorySceneOutlineDraftsEqual(sceneOutlineDraft, suggestedSceneOutlineDraft),
  );

  const isControlCustomized = isBriefCustomized
    || isBeatPlannerCustomized
    || isSceneOutlineCustomized;

  const currentDraft = buildStoryCreationCurrentDraft({
    creativeMode,
    storyFocus,
    plotStage,
    narrativePerspective,
    storyCreationBriefDraft,
    beatPlannerDraft,
    sceneOutlineDraft,
    isBriefCustomized,
    isBeatPlannerCustomized,
    isSceneOutlineCustomized,
  });

  return {
    defaultBrief,
    resolvedBrief: promptState.prompt,
    promptLayerLabels: promptState.promptLayerLabels,
    promptCharCount: promptState.promptCharCount,
    isPromptVerbose: promptState.isVerbose,
    isBriefCustomized,
    isBeatPlannerCustomized,
    isSceneOutlineCustomized,
    isControlCustomized,
    currentDraft,
    canSaveSnapshot: canSaveStoryCreationSnapshot({
      storageKey,
      currentDraft,
      hasChapterContext,
    }),
  };
}