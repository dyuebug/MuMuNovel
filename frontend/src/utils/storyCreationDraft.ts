import type { CreativeMode, PlotStage, StoryFocus } from '../types';

export interface StoryBeatPlannerDraft {
  openingHook: string;
  chapterGoal: string;
  conflictPressure: string;
  turningPoint: string;
  endingHook: string;
}

export interface StorySceneOutlineDraft {
  setupScene: string;
  confrontationScene: string;
  reversalScene: string;
  payoffScene: string;
}

export type StoryDraftField<T extends object> = {
  key: keyof T;
  label: string;
  placeholder: string;
};

export const EMPTY_STORY_BEAT_PLANNER_DRAFT: StoryBeatPlannerDraft = {
  openingHook: '',
  chapterGoal: '',
  conflictPressure: '',
  turningPoint: '',
  endingHook: '',
};

export const EMPTY_STORY_SCENE_OUTLINE_DRAFT: StorySceneOutlineDraft = {
  setupScene: '',
  confrontationScene: '',
  reversalScene: '',
  payoffScene: '',
};

export const STORY_BEAT_PLANNER_FIELDS: Array<StoryDraftField<StoryBeatPlannerDraft>> = [
  {
    key: 'openingHook',
    label: '????',
    placeholder: '????????????????????',
  },
  {
    key: 'chapterGoal',
    label: '????',
    placeholder: '???????????????',
  },
  {
    key: 'conflictPressure',
    label: '????',
    placeholder: '????????????',
  },
  {
    key: 'turningPoint',
    label: '???',
    placeholder: '????????????',
  },
  {
    key: 'endingHook',
    label: '????',
    placeholder: '????????????',
  },
];

export const STORY_SCENE_OUTLINE_FIELDS: Array<StoryDraftField<StorySceneOutlineDraft>> = [
  {
    key: 'setupScene',
    label: '??????',
    placeholder: '????????????',
  },
  {
    key: 'confrontationScene',
    label: '??????',
    placeholder: '???????????',
  },
  {
    key: 'reversalScene',
    label: '??????',
    placeholder: '?????????',
  },
  {
    key: 'payoffScene',
    label: '??????',
    placeholder: '??????????????',
  },
];

export type PersistedStoryCreationDraft = {
  creativeMode?: CreativeMode;
  storyFocus?: StoryFocus;
  plotStage?: PlotStage;
  narrativePerspective?: string;
  storyCreationBriefDraft?: string;
  beatPlannerDraft?: StoryBeatPlannerDraft;
  sceneOutlineDraft?: StorySceneOutlineDraft;
  isBriefCustomized?: boolean;
  isBeatPlannerCustomized?: boolean;
  isSceneOutlineCustomized?: boolean;
  updatedAt?: string;
};

export type StoryCreationSnapshotReason = 'manual' | 'generate';

export type StoryCreationSnapshotScope = 'single' | 'batch';

export type StoryCreationSnapshot = PersistedStoryCreationDraft & {
  id: string;
  scope: StoryCreationSnapshotScope;
  createdAt: string;
  reason: StoryCreationSnapshotReason;
  label: string;
  prompt?: string;
  promptLayerLabels?: string[];
  promptCharCount?: number;
};

export const normalizeStoryBeatPlannerDraft = (
  draft?: Partial<StoryBeatPlannerDraft> | null,
): StoryBeatPlannerDraft => ({
  openingHook: draft?.openingHook?.trim() ?? '',
  chapterGoal: draft?.chapterGoal?.trim() ?? '',
  conflictPressure: draft?.conflictPressure?.trim() ?? '',
  turningPoint: draft?.turningPoint?.trim() ?? '',
  endingHook: draft?.endingHook?.trim() ?? '',
});

export const normalizeStorySceneOutlineDraft = (
  draft?: Partial<StorySceneOutlineDraft> | null,
): StorySceneOutlineDraft => ({
  setupScene: draft?.setupScene?.trim() ?? '',
  confrontationScene: draft?.confrontationScene?.trim() ?? '',
  reversalScene: draft?.reversalScene?.trim() ?? '',
  payoffScene: draft?.payoffScene?.trim() ?? '',
});

export const isStoryBeatPlannerDraftEmpty = (
  draft?: Partial<StoryBeatPlannerDraft> | null,
): boolean => {
  const normalizedDraft = normalizeStoryBeatPlannerDraft(draft);
  return Object.values(normalizedDraft).every((value) => !value);
};

export const isStorySceneOutlineDraftEmpty = (
  draft?: Partial<StorySceneOutlineDraft> | null,
): boolean => {
  const normalizedDraft = normalizeStorySceneOutlineDraft(draft);
  return Object.values(normalizedDraft).every((value) => !value);
};

export const areStoryBeatPlannerDraftsEqual = (
  left?: Partial<StoryBeatPlannerDraft> | null,
  right?: Partial<StoryBeatPlannerDraft> | null,
): boolean => {
  const leftDraft = normalizeStoryBeatPlannerDraft(left);
  const rightDraft = normalizeStoryBeatPlannerDraft(right);

  return STORY_BEAT_PLANNER_FIELDS.every((field) => leftDraft[field.key] === rightDraft[field.key]);
};

export const areStorySceneOutlineDraftsEqual = (
  left?: Partial<StorySceneOutlineDraft> | null,
  right?: Partial<StorySceneOutlineDraft> | null,
): boolean => {
  const leftDraft = normalizeStorySceneOutlineDraft(left);
  const rightDraft = normalizeStorySceneOutlineDraft(right);

  return STORY_SCENE_OUTLINE_FIELDS.every((field) => leftDraft[field.key] === rightDraft[field.key]);
};

export const normalizeOptionalText = (value?: string | null): string => value?.trim() ?? '';

export const areStoryCreationDraftMetaFieldsEqual = (
  left?: Partial<PersistedStoryCreationDraft> | null,
  right?: Partial<PersistedStoryCreationDraft> | null,
  options?: { includeNarrativePerspective?: boolean },
): boolean => {
  const includeNarrativePerspective = options?.includeNarrativePerspective === true;

  return (left?.creativeMode ?? undefined) === (right?.creativeMode ?? undefined)
    && (left?.storyFocus ?? undefined) === (right?.storyFocus ?? undefined)
    && (left?.plotStage ?? undefined) === (right?.plotStage ?? undefined)
    && (!includeNarrativePerspective
      || normalizeOptionalText(left?.narrativePerspective) === normalizeOptionalText(right?.narrativePerspective));
};

export const areStoryCreationDraftContentsEqual = (
  left?: Partial<PersistedStoryCreationDraft> | null,
  right?: Partial<PersistedStoryCreationDraft> | null,
  options?: { includeNarrativePerspective?: boolean },
): boolean => (
  areStoryCreationDraftMetaFieldsEqual(left, right, options)
  && normalizeOptionalText(left?.storyCreationBriefDraft) === normalizeOptionalText(right?.storyCreationBriefDraft)
  && areStoryBeatPlannerDraftsEqual(left?.beatPlannerDraft, right?.beatPlannerDraft)
  && areStorySceneOutlineDraftsEqual(left?.sceneOutlineDraft, right?.sceneOutlineDraft)
);

export const hasMeaningfulStoryCreationDraft = (
  draft?: Partial<PersistedStoryCreationDraft> | null,
): boolean => Boolean(
  draft
  && (
    draft.creativeMode
    || draft.storyFocus
    || draft.plotStage
    || normalizeOptionalText(draft.narrativePerspective)
    || normalizeOptionalText(draft.storyCreationBriefDraft)
    || !isStoryBeatPlannerDraftEmpty(draft.beatPlannerDraft)
    || !isStorySceneOutlineDraftEmpty(draft.sceneOutlineDraft)
  )
);

export const buildStoryCreationSnapshotDiffLabels = (
  snapshot?: Partial<PersistedStoryCreationDraft> | null,
  currentDraft?: Partial<PersistedStoryCreationDraft> | null,
  includeNarrativePerspective = false,
): string[] => {
  if (!snapshot || !currentDraft) {
    return [];
  }

  const labels: string[] = [];

  if (normalizeOptionalText(snapshot.storyCreationBriefDraft) !== normalizeOptionalText(currentDraft.storyCreationBriefDraft)) {
    labels.push('??');
  }

  if (!areStoryBeatPlannerDraftsEqual(snapshot.beatPlannerDraft, currentDraft.beatPlannerDraft)) {
    labels.push('????');
  }

  if (!areStorySceneOutlineDraftsEqual(snapshot.sceneOutlineDraft, currentDraft.sceneOutlineDraft)) {
    labels.push('????');
  }

  if (!areStoryCreationDraftMetaFieldsEqual(snapshot, currentDraft, { includeNarrativePerspective })) {
    labels.push('??');
  }

  return labels;
};
