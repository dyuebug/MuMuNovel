import type { CreativeMode, PlotStage, StoryFocus } from '../types';
import {
  hasMeaningfulStoryCreationDraft,
  type PersistedStoryCreationDraft,
  type StoryBeatPlannerDraft,
  type StorySceneOutlineDraft,
} from '../utils/storyCreationDraft';

export function buildStoryCreationCurrentDraft({
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
}: {
  creativeMode?: CreativeMode;
  storyFocus?: StoryFocus;
  plotStage?: PlotStage;
  narrativePerspective?: string;
  storyCreationBriefDraft: string;
  beatPlannerDraft: StoryBeatPlannerDraft;
  sceneOutlineDraft: StorySceneOutlineDraft;
  isBriefCustomized: boolean;
  isBeatPlannerCustomized: boolean;
  isSceneOutlineCustomized: boolean;
}): PersistedStoryCreationDraft {
  return {
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
  };
}

export function canSaveStoryCreationSnapshot({
  storageKey,
  currentDraft,
  hasChapterContext = true,
}: {
  storageKey: string | null;
  currentDraft: PersistedStoryCreationDraft;
  hasChapterContext?: boolean;
}): boolean {
  return Boolean(storageKey && hasChapterContext && hasMeaningfulStoryCreationDraft(currentDraft));
}
