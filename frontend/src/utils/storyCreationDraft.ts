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
    label: '开篇钩子',
    placeholder: '用一句话写清本章开头最抓人的钩子',
  },
  {
    key: 'chapterGoal',
    label: '章节目标',
    placeholder: '写清本章主角最想达成的目标',
  },
  {
    key: 'conflictPressure',
    label: '冲突压力',
    placeholder: '写清本章最核心的阻碍、压力或代价',
  },
  {
    key: 'turningPoint',
    label: '转折点',
    placeholder: '写清中段或后段出现的关键变化',
  },
  {
    key: 'endingHook',
    label: '结尾钩子',
    placeholder: '写清章尾留下的悬念、回报或牵引',
  },
];

export const STORY_SCENE_OUTLINE_FIELDS: Array<StoryDraftField<StorySceneOutlineDraft>> = [
  {
    key: 'setupScene',
    label: '铺垫场景',
    placeholder: '描述开场场景如何铺垫目标与局势',
  },
  {
    key: 'confrontationScene',
    label: '对抗场景',
    placeholder: '描述主要冲突如何升级并压迫角色',
  },
  {
    key: 'reversalScene',
    label: '反转场景',
    placeholder: '描述关键反转如何改变局面',
  },
  {
    key: 'payoffScene',
    label: '回收场景',
    placeholder: '描述结尾如何回收前文并抛出下一章牵引',
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
    labels.push('简介');
  }

  if (!areStoryBeatPlannerDraftsEqual(snapshot.beatPlannerDraft, currentDraft.beatPlannerDraft)) {
    labels.push('节拍');
  }

  if (!areStorySceneOutlineDraftsEqual(snapshot.sceneOutlineDraft, currentDraft.sceneOutlineDraft)) {
    labels.push('场景');
  }

  if (!areStoryCreationDraftMetaFieldsEqual(snapshot, currentDraft, { includeNarrativePerspective })) {
    labels.push('参数');
  }

  return labels;
};
