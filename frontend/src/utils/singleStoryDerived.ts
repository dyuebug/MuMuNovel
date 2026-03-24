import type { ChapterQualityMetrics, CreativeMode, PlotStage, StoryFocus } from '../types';
import type {
  CreationPresetId,
  StoryAcceptanceCard,
  StoryCharacterArcCard,
  StoryCreationControlCard,
  StoryExecutionChecklist,
  StoryObjectiveCard,
  StoryRepairPromptPayload,
  StoryRepairTargetCard,
  StoryRepetitionRiskCard,
  StoryResultCard,
} from './creationPresetsCore';
import {
  buildStoryAcceptanceCard,
  buildStoryCharacterArcCard,
  buildStoryExecutionChecklist,
  buildStoryObjectiveCard,
  buildStoryRepetitionRiskCard,
  buildStoryResultCard,
} from './creationPresetsStory';
import {
  buildStoryCreationControlCard,
  buildStoryRepairPromptPayload,
  buildStoryRepairTargetCard,
} from './creationPresetsQuality';

export interface SingleStoryPresetState {
  singleStoryAcceptanceCard?: StoryAcceptanceCard;
  singleStoryCharacterArcCard?: StoryCharacterArcCard;
  singleStoryCreationControlCard?: StoryCreationControlCard;
  singleStoryExecutionChecklist?: StoryExecutionChecklist;
  singleStoryObjectiveCard?: StoryObjectiveCard;
  singleStoryRepairPayload?: StoryRepairPromptPayload;
  singleStoryRepairTargetCard?: StoryRepairTargetCard;
  singleStoryRepetitionRiskCard?: StoryRepetitionRiskCard;
  singleStoryResultCard?: StoryResultCard;
}

interface BuildSingleStoryPresetStateOptions {
  activePresetId?: CreationPresetId | null;
  chapterNumber?: number | null;
  chapterQualityMetrics: ChapterQualityMetrics | null;
  knownStructureChapterCount: number;
  selectedCreativeMode?: CreativeMode;
  selectedPlotStage?: PlotStage | null;
  selectedStoryFocus?: StoryFocus;
}

export const buildSingleStoryPresetState = (
  options: BuildSingleStoryPresetStateOptions,
): SingleStoryPresetState => {
  const singleStoryObjectiveCard = buildStoryObjectiveCard(options.selectedCreativeMode, options.selectedStoryFocus, {
    scene: 'chapter',
    plotStage: options.selectedPlotStage,
  });

  const singleStoryResultCard = buildStoryResultCard(options.selectedCreativeMode, options.selectedStoryFocus, {
    scene: 'chapter',
    plotStage: options.selectedPlotStage,
  });

  const singleStoryExecutionChecklist = buildStoryExecutionChecklist(options.selectedCreativeMode, options.selectedStoryFocus, {
    scene: 'chapter',
    plotStage: options.selectedPlotStage,
  });

  const singleStoryRepetitionRiskCard = buildStoryRepetitionRiskCard(options.selectedCreativeMode, options.selectedStoryFocus, {
    scene: 'chapter',
    plotStage: options.selectedPlotStage,
  });

  const singleStoryAcceptanceCard = buildStoryAcceptanceCard(options.selectedCreativeMode, options.selectedStoryFocus, {
    scene: 'chapter',
    plotStage: options.selectedPlotStage,
  });

  const singleStoryCharacterArcCard = buildStoryCharacterArcCard(options.selectedCreativeMode, options.selectedStoryFocus, {
    scene: 'chapter',
    plotStage: options.selectedPlotStage,
  });

  const singleStoryRepairTargetCard = buildStoryRepairTargetCard(
    options.chapterQualityMetrics,
    options.selectedCreativeMode,
    options.selectedStoryFocus,
    {
      plotStage: options.selectedPlotStage,
      chapterNumber: options.chapterNumber,
      totalChapters: options.knownStructureChapterCount,
      activePresetId: options.activePresetId,
    },
  );

  const singleStoryCreationControlCard = buildStoryCreationControlCard(
    options.chapterQualityMetrics,
    options.selectedCreativeMode,
    options.selectedStoryFocus,
    {
      plotStage: options.selectedPlotStage,
      chapterNumber: options.chapterNumber,
      totalChapters: options.knownStructureChapterCount,
      activePresetId: options.activePresetId,
    },
  );

  const singleStoryRepairPayload = buildStoryRepairPromptPayload(singleStoryRepairTargetCard);

  return {
    singleStoryAcceptanceCard,
    singleStoryCharacterArcCard,
    singleStoryCreationControlCard,
    singleStoryExecutionChecklist,
    singleStoryObjectiveCard,
    singleStoryRepairPayload,
    singleStoryRepairTargetCard,
    singleStoryRepetitionRiskCard,
    singleStoryResultCard,
  };
};
