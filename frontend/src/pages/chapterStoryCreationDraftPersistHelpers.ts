import type { CreativeMode, PlotStage, StoryFocus } from '../types';
import type {
  PersistedStoryCreationDraft,
  StoryBeatPlannerDraft,
  StorySceneOutlineDraft,
} from '../utils/storyCreationDraft';

export type LoadStoryCreationPersistenceForPersist = () => Promise<{
  persistStoryCreationDraft: (storageKey: string, draft: PersistedStoryCreationDraft) => void;
}>;

export function persistSingleChapterStoryCreationDraft({
  currentChapterId,
  storageKey,
  loadStoryCreationPersistence,
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
  currentChapterId?: string | null;
  storageKey: string | null;
  loadStoryCreationPersistence: LoadStoryCreationPersistenceForPersist;
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
}): void {
  if (!storageKey || !currentChapterId) {
    return;
  }

  void loadStoryCreationPersistence().then(({ persistStoryCreationDraft }) => {
    persistStoryCreationDraft(storageKey, {
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
      updatedAt: new Date().toISOString(),
    });
  });
}

export function persistBatchStoryCreationDraft({
  storageKey,
  loadStoryCreationPersistence,
  creativeMode,
  storyFocus,
  plotStage,
  storyCreationBriefDraft,
  beatPlannerDraft,
  sceneOutlineDraft,
  isBriefCustomized,
  isBeatPlannerCustomized,
  isSceneOutlineCustomized,
}: {
  storageKey: string | null;
  loadStoryCreationPersistence: LoadStoryCreationPersistenceForPersist;
  creativeMode?: CreativeMode;
  storyFocus?: StoryFocus;
  plotStage?: PlotStage;
  storyCreationBriefDraft: string;
  beatPlannerDraft: StoryBeatPlannerDraft;
  sceneOutlineDraft: StorySceneOutlineDraft;
  isBriefCustomized: boolean;
  isBeatPlannerCustomized: boolean;
  isSceneOutlineCustomized: boolean;
}): void {
  if (!storageKey) {
    return;
  }

  void loadStoryCreationPersistence().then(({ persistStoryCreationDraft }) => {
    persistStoryCreationDraft(storageKey, {
      creativeMode,
      storyFocus,
      plotStage,
      storyCreationBriefDraft,
      beatPlannerDraft,
      sceneOutlineDraft,
      isBriefCustomized,
      isBeatPlannerCustomized,
      isSceneOutlineCustomized,
      updatedAt: new Date().toISOString(),
    });
  });
}
