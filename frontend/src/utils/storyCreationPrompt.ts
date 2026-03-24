import {
  normalizeStoryBeatPlannerDraft,
  normalizeStorySceneOutlineDraft,
  STORY_BEAT_PLANNER_FIELDS,
  STORY_SCENE_OUTLINE_FIELDS,
  type StoryBeatPlannerDraft,
  type StorySceneOutlineDraft,
} from './storyCreationDraft';

const buildJoinedInstruction = (...parts: Array<string | undefined>): string => {
  const normalizedParts = parts.map((item) => item?.trim()).filter((item): item is string => Boolean(item));
  return normalizedParts.join('; ');
};

export const buildStoryBeatPlannerPrompt = (
  draft?: Partial<StoryBeatPlannerDraft> | null,
  scope: 'single' | 'batch' = 'single',
): string | undefined => {
  const normalizedDraft = normalizeStoryBeatPlannerDraft(draft);
  const entries = STORY_BEAT_PLANNER_FIELDS
    .map((field) => ({ label: field.label, value: normalizedDraft[field.key] }))
    .filter((item) => item.value);

  if (entries.length === 0) {
    return undefined;
  }

  const title = scope === 'batch'
    ? '??????????????'
    : '????????????';

  return [title, ...entries.map((item) => `- ${item.label}: ${item.value}`)].join('\n');
};

export const buildStorySceneOutlineSuggestion = (options: {
  beatPlanner?: Partial<StoryBeatPlannerDraft> | null;
  objective?: {
    obstacle?: string;
    turn?: string;
  } | null;
  result?: {
    reveal?: string;
    fallout?: string;
    relationship?: string;
  } | null;
  acceptance?: {
    missionCheck?: string;
  } | null;
}): StorySceneOutlineDraft => {
  const beatPlanner = normalizeStoryBeatPlannerDraft(options.beatPlanner);

  return {
    setupScene: buildJoinedInstruction(beatPlanner.openingHook, beatPlanner.chapterGoal),
    confrontationScene: buildJoinedInstruction(beatPlanner.conflictPressure, options.objective?.obstacle),
    reversalScene: buildJoinedInstruction(beatPlanner.turningPoint, options.result?.reveal, options.result?.relationship),
    payoffScene: buildJoinedInstruction(beatPlanner.endingHook, options.result?.fallout, options.acceptance?.missionCheck),
  };
};

export const buildStorySceneOutlinePrompt = (
  draft?: Partial<StorySceneOutlineDraft> | null,
  scope: 'single' | 'batch' = 'single',
): string | undefined => {
  const normalizedDraft = normalizeStorySceneOutlineDraft(draft);
  const entries = STORY_SCENE_OUTLINE_FIELDS
    .map((field, index) => ({ index: index + 1, label: field.label, value: normalizedDraft[field.key] }))
    .filter((item) => item.value);

  if (entries.length === 0) {
    return undefined;
  }

  const title = scope === 'batch'
    ? '??????????????'
    : '????????????';

  return [title, ...entries.map((item) => `${item.index}. ${item.label}: ${item.value}`)].join('\n');
};

export const mergeStoryCreationInstructions = (...parts: Array<string | undefined>): string | undefined => {
  const normalizedParts = parts
    .map((item) => item?.trim())
    .filter((item): item is string => Boolean(item));

  return normalizedParts.length > 0 ? normalizedParts.join('\n\n') : undefined;
};

export const STORY_CREATION_PROMPT_WARN_THRESHOLD = 1000;

export const buildStoryCreationPromptLayerLabels = (parts: {
  summary?: string;
  beat?: string;
  scene?: string;
}): string[] => [
  parts.summary?.trim() ? '??' : '',
  parts.beat?.trim() ? '????' : '',
  parts.scene?.trim() ? '????' : '',
].filter((item): item is string => Boolean(item));
