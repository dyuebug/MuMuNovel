import { message } from 'antd';
import type {
  StoryBeatPlannerDraft,
  StoryCreationSnapshotScope,
  StorySceneOutlineDraft,
} from '../utils/storyCreationDraft';
import {
  buildStoryBeatPlannerPrompt,
  buildStoryCreationPromptLayerLabels,
  buildStorySceneOutlinePrompt,
  mergeStoryCreationInstructions,
  STORY_CREATION_PROMPT_WARN_THRESHOLD,
} from '../utils/storyCreationPrompt';

export type ResolveStoryCreationPromptState = (args: {
  scope: StoryCreationSnapshotScope;
  briefDraft?: string | null;
  defaultBrief?: string | null;
  beatPlannerDraft?: Partial<StoryBeatPlannerDraft> | null;
  sceneOutlineDraft?: Partial<StorySceneOutlineDraft> | null;
}) => {
  baseBrief?: string;
  beatBrief?: string;
  sceneBrief?: string;
  prompt?: string;
  promptLayerLabels: string[];
  promptCharCount: number;
  isVerbose: boolean;
};

export const resolveStoryCreationPromptState: ResolveStoryCreationPromptState = (options) => {
  const baseBrief = options.briefDraft?.trim() || options.defaultBrief?.trim() || undefined;
  const beatBrief = buildStoryBeatPlannerPrompt(options.beatPlannerDraft, options.scope);
  const sceneBrief = buildStorySceneOutlinePrompt(options.sceneOutlineDraft, options.scope);
  const prompt = mergeStoryCreationInstructions(baseBrief, beatBrief, sceneBrief);
  const promptLayerLabels = buildStoryCreationPromptLayerLabels({
    summary: baseBrief,
    beat: beatBrief,
    scene: sceneBrief,
  });
  const promptCharCount = prompt?.length ?? 0;

  return {
    baseBrief,
    beatBrief,
    sceneBrief,
    prompt,
    promptLayerLabels,
    promptCharCount,
    isVerbose: promptCharCount >= STORY_CREATION_PROMPT_WARN_THRESHOLD,
  };
};

export async function copyStoryCreationPrompt(
  content: string | undefined,
  scopeLabel: 'single' | 'batch',
): Promise<void> {
  const normalizedContent = content?.trim();
  if (!normalizedContent) {
    message.warning(`${scopeLabel === 'single' ? '\u5355\u7AE0' : '\u6279\u91CF'}\u63D0\u793A\u8BCD\u6682\u65E0\u53EF\u590D\u5236\u5185\u5BB9\u3002`);
    return;
  }

  try {
    if (navigator.clipboard?.writeText) {
      await navigator.clipboard.writeText(normalizedContent);
    } else {
      const tempTextArea = document.createElement('textarea');
      tempTextArea.value = normalizedContent;
      tempTextArea.setAttribute('readonly', 'true');
      tempTextArea.style.position = 'fixed';
      tempTextArea.style.opacity = '0';
      document.body.appendChild(tempTextArea);
      tempTextArea.select();
      document.execCommand('copy');
      document.body.removeChild(tempTextArea);
    }

    message.success(`\u5DF2\u5C06${scopeLabel === 'single' ? '\u5355\u7AE0' : '\u6279\u91CF'}\u63D0\u793A\u8BCD\u590D\u5236\u5230\u526A\u8D34\u677F\u3002`);
  } catch (error) {
    console.error('Failed to copy prompt.', error);
    message.error('\u590D\u5236\u5931\u8D25\uFF0C\u8BF7\u91CD\u8BD5\u3002');
  }
}
