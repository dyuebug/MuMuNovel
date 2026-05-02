import { useCallback } from 'react';
import { useStore } from '../../../store';
import { startChapterGenerationWorkflow } from '../../../store/chapterGenerationWorkflow';
import type {
  CreativeMode,
  PlotStage,
  QualityPreset,
  StoryFocus,
} from '../../../types';

export function useChapterGenerationWorkflow({
  refreshChapters,
}: {
  refreshChapters: () => Promise<unknown>;
}) {
  const currentProjectId = useStore((state) => state.currentProject?.id);

  const generateChapterContentStream = useCallback(async (
    chapterId: string,
    onProgress?: (content: string) => void,
    styleId?: number,
    targetWordCount?: number,
    onProgressUpdate?: (message: string, progress: number) => void,
    model?: string,
    narrativePerspective?: string,
    creativeMode?: CreativeMode,
    storyFocus?: StoryFocus,
    plotStage?: PlotStage,
    storyCreationBrief?: string,
    qualityPreset?: QualityPreset,
    qualityNotes?: string,
    storyRepairSummary?: string,
    storyRepairTargets?: string[],
    storyPreserveStrengths?: string[],
  ) => {
    return startChapterGenerationWorkflow({
      chapterId,
      projectId: currentProjectId,
      refreshChapters,
      onProgress,
      styleId,
      targetWordCount,
      onProgressUpdate,
      model,
      narrativePerspective,
      creativeMode,
      storyFocus,
      plotStage,
      storyCreationBrief,
      qualityPreset,
      qualityNotes,
      storyRepairSummary,
      storyRepairTargets,
      storyPreserveStrengths,
    });
  }, [refreshChapters, currentProjectId]);

  return {
    generateChapterContentStream,
  };
}
