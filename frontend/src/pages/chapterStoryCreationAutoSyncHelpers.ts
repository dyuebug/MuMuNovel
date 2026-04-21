import {
  EMPTY_STORY_BEAT_PLANNER_DRAFT,
  EMPTY_STORY_SCENE_OUTLINE_DRAFT,
  areStoryBeatPlannerDraftsEqual,
  areStorySceneOutlineDraftsEqual,
  isStoryBeatPlannerDraftEmpty,
  isStorySceneOutlineDraftEmpty,
  type StoryBeatPlannerDraft,
  type StorySceneOutlineDraft,
} from '../utils/storyCreationDraft';

export function syncStoryCreationBriefDraft({
  defaultBrief,
  previousAutoBrief,
  setAutoBriefRef,
  setBriefDraft,
}: {
  defaultBrief: string;
  previousAutoBrief: string;
  setAutoBriefRef: (value: string) => void;
  setBriefDraft: (value: string | ((prev: string) => string)) => void;
}): void {
  if (!defaultBrief) {
    setAutoBriefRef('');
    setBriefDraft((prev) => (prev ? '' : prev));
    return;
  }

  setBriefDraft((prev) => {
    if (!prev.trim() || prev === previousAutoBrief) {
      return defaultBrief;
    }
    return prev;
  });

  setAutoBriefRef(defaultBrief);
}

export function syncStoryBeatPlannerDraft({
  systemPlanner,
  previousAutoPlanner,
  setAutoPlannerRef,
  setPlannerDraft,
}: {
  systemPlanner: StoryBeatPlannerDraft;
  previousAutoPlanner: StoryBeatPlannerDraft;
  setAutoPlannerRef: (value: StoryBeatPlannerDraft) => void;
  setPlannerDraft: (value: StoryBeatPlannerDraft | ((prev: StoryBeatPlannerDraft) => StoryBeatPlannerDraft)) => void;
}): void {
  if (isStoryBeatPlannerDraftEmpty(systemPlanner)) {
    setAutoPlannerRef(EMPTY_STORY_BEAT_PLANNER_DRAFT);
    setPlannerDraft((prev) => (isStoryBeatPlannerDraftEmpty(prev) ? prev : { ...EMPTY_STORY_BEAT_PLANNER_DRAFT }));
    return;
  }

  setPlannerDraft((prev) => {
    if (isStoryBeatPlannerDraftEmpty(prev) || areStoryBeatPlannerDraftsEqual(prev, previousAutoPlanner)) {
      return systemPlanner;
    }
    return prev;
  });

  setAutoPlannerRef(systemPlanner);
}

export function syncStorySceneOutlineDraft({
  suggestedOutline,
  previousSuggestedOutline,
  setAutoSceneOutlineRef,
  setSceneOutlineDraft,
}: {
  suggestedOutline: StorySceneOutlineDraft;
  previousSuggestedOutline: StorySceneOutlineDraft;
  setAutoSceneOutlineRef: (value: StorySceneOutlineDraft) => void;
  setSceneOutlineDraft: (value: StorySceneOutlineDraft | ((prev: StorySceneOutlineDraft) => StorySceneOutlineDraft)) => void;
}): void {
  if (isStorySceneOutlineDraftEmpty(suggestedOutline)) {
    setAutoSceneOutlineRef(EMPTY_STORY_SCENE_OUTLINE_DRAFT);
    setSceneOutlineDraft((prev) => (isStorySceneOutlineDraftEmpty(prev) ? prev : { ...EMPTY_STORY_SCENE_OUTLINE_DRAFT }));
    return;
  }

  setSceneOutlineDraft((prev) => {
    if (isStorySceneOutlineDraftEmpty(prev) || areStorySceneOutlineDraftsEqual(prev, previousSuggestedOutline)) {
      return suggestedOutline;
    }
    return prev;
  });

  setAutoSceneOutlineRef(suggestedOutline);
}

export function syncStoryCreationAutoDrafts({
  defaultBrief,
  previousAutoBrief,
  setAutoBriefRef,
  systemPlanner,
  previousAutoPlanner,
  setAutoPlannerRef,
  suggestedOutline,
  previousSuggestedOutline,
  setAutoSceneOutlineRef,
  setBriefDraft,
  setPlannerDraft,
  setSceneOutlineDraft,
}: {
  defaultBrief: string;
  previousAutoBrief: string;
  setAutoBriefRef: (value: string) => void;
  systemPlanner: StoryBeatPlannerDraft;
  previousAutoPlanner: StoryBeatPlannerDraft;
  setAutoPlannerRef: (value: StoryBeatPlannerDraft) => void;
  suggestedOutline: StorySceneOutlineDraft;
  previousSuggestedOutline: StorySceneOutlineDraft;
  setAutoSceneOutlineRef: (value: StorySceneOutlineDraft) => void;
  setBriefDraft: (value: string | ((prev: string) => string)) => void;
  setPlannerDraft: (value: StoryBeatPlannerDraft | ((prev: StoryBeatPlannerDraft) => StoryBeatPlannerDraft)) => void;
  setSceneOutlineDraft: (value: StorySceneOutlineDraft | ((prev: StorySceneOutlineDraft) => StorySceneOutlineDraft)) => void;
}): void {
  syncStoryCreationBriefDraft({
    defaultBrief,
    previousAutoBrief,
    setAutoBriefRef,
    setBriefDraft,
  });
  syncStoryBeatPlannerDraft({
    systemPlanner,
    previousAutoPlanner,
    setAutoPlannerRef,
    setPlannerDraft,
  });
  syncStorySceneOutlineDraft({
    suggestedOutline,
    previousSuggestedOutline,
    setAutoSceneOutlineRef,
    setSceneOutlineDraft,
  });
}