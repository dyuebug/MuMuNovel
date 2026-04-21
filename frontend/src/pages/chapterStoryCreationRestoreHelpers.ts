import {
  EMPTY_STORY_BEAT_PLANNER_DRAFT,
  EMPTY_STORY_SCENE_OUTLINE_DRAFT,
  normalizeStoryBeatPlannerDraft,
  normalizeStorySceneOutlineDraft,
  type PersistedStoryCreationDraft,
  type StoryBeatPlannerDraft,
  type StoryCreationSnapshot,
  type StorySceneOutlineDraft,
} from '../utils/storyCreationDraft';
import type { CreativeMode, PlotStage, QualityPreset, StoryFocus } from '../types';

export type LoadStoryCreationPersistenceForRestore = () => Promise<{
  getPersistedStoryCreationDraft: (storageKey: string) => PersistedStoryCreationDraft | undefined;
  getPersistedStoryCreationSnapshots: (storageKey: string) => StoryCreationSnapshot[];
}>;

const applyPersistedDraftRefs = ({
  persistedDraft,
  manualBriefSentinel,
  fallbackBrief,
  setAutoBriefRef,
  setBeatPlannerAutoRef,
  setSceneOutlineAutoRef,
}: {
  persistedDraft: PersistedStoryCreationDraft;
  manualBriefSentinel: string;
  fallbackBrief: string;
  setAutoBriefRef: (value: string) => void;
  setBeatPlannerAutoRef: (value: StoryBeatPlannerDraft) => void;
  setSceneOutlineAutoRef: (value: StorySceneOutlineDraft) => void;
}): void => {
  setAutoBriefRef(
    persistedDraft.isBriefCustomized
      ? manualBriefSentinel
      : (persistedDraft.storyCreationBriefDraft ?? fallbackBrief),
  );
  setBeatPlannerAutoRef(
    persistedDraft.isBeatPlannerCustomized
      ? { ...EMPTY_STORY_BEAT_PLANNER_DRAFT }
      : normalizeStoryBeatPlannerDraft(persistedDraft.beatPlannerDraft),
  );
  setSceneOutlineAutoRef(
    persistedDraft.isSceneOutlineCustomized
      ? { ...EMPTY_STORY_SCENE_OUTLINE_DRAFT }
      : normalizeStorySceneOutlineDraft(persistedDraft.sceneOutlineDraft),
  );
};

export async function restoreSingleChapterStoryCreationDraft({
  currentChapterId,
  currentChapterNumber,
  storageKey,
  loadStoryCreationPersistence,
  resetCockpit,
  manualBriefSentinel,
  singleDefaultBrief,
  projectDefaultBrief,
  projectDefaultCreativeMode,
  projectDefaultStoryFocus,
  projectDefaultPlotStage,
  projectDefaultQualityPreset,
  projectDefaultQualityNotes,
  totalChapters,
  inferPlotStage,
  isCancelled,
  setAutoBriefRef,
  setBeatPlannerAutoRef,
  setSceneOutlineAutoRef,
  setTemporaryNarrativePerspective,
  setSelectedCreativeMode,
  setSelectedStoryFocus,
  setSelectedPlotStage,
  setSelectedQualityPreset,
  setSelectedQualityNotes,
  setStoryCreationBriefDraft,
  setBeatPlannerDraft,
  setSceneOutlineDraft,
}: {
  currentChapterId?: string | null;
  currentChapterNumber?: number | null;
  storageKey: string | null;
  loadStoryCreationPersistence: LoadStoryCreationPersistenceForRestore;
  resetCockpit: (chapterNumber?: number | null) => void;
  manualBriefSentinel: string;
  singleDefaultBrief: string;
  projectDefaultBrief: string;
  projectDefaultCreativeMode?: CreativeMode;
  projectDefaultStoryFocus?: StoryFocus;
  projectDefaultPlotStage?: PlotStage;
  projectDefaultQualityPreset?: QualityPreset;
  projectDefaultQualityNotes: string;
  totalChapters: number;
  inferPlotStage: (args: { chapterNumber?: number | null; totalChapters: number }) => Promise<PlotStage | undefined>;
  isCancelled: () => boolean;
  setAutoBriefRef: (value: string) => void;
  setBeatPlannerAutoRef: (value: StoryBeatPlannerDraft) => void;
  setSceneOutlineAutoRef: (value: StorySceneOutlineDraft) => void;
  setTemporaryNarrativePerspective: (value: string | undefined) => void;
  setSelectedCreativeMode: (value: CreativeMode | undefined) => void;
  setSelectedStoryFocus: (value: StoryFocus | undefined) => void;
  setSelectedPlotStage: (value: PlotStage | undefined) => void;
  setSelectedQualityPreset: (value: QualityPreset | undefined) => void;
  setSelectedQualityNotes: (value: string) => void;
  setStoryCreationBriefDraft: (value: string) => void;
  setBeatPlannerDraft: (value: StoryBeatPlannerDraft) => void;
  setSceneOutlineDraft: (value: StorySceneOutlineDraft) => void;
}): Promise<void> {
  if (!currentChapterId || currentChapterNumber == null) {
    return;
  }

  if (!storageKey) {
    resetCockpit(currentChapterNumber);
    return;
  }

  const { getPersistedStoryCreationDraft } = await loadStoryCreationPersistence();
  const persistedDraft = getPersistedStoryCreationDraft(storageKey);
  if (isCancelled()) {
    return;
  }

  if (!persistedDraft) {
    resetCockpit(currentChapterNumber);
    return;
  }

  applyPersistedDraftRefs({
    persistedDraft,
    manualBriefSentinel,
    fallbackBrief: singleDefaultBrief,
    setAutoBriefRef,
    setBeatPlannerAutoRef,
    setSceneOutlineAutoRef,
  });

  setTemporaryNarrativePerspective(persistedDraft.narrativePerspective);
  setSelectedCreativeMode(persistedDraft.creativeMode ?? projectDefaultCreativeMode);
  setSelectedStoryFocus(persistedDraft.storyFocus ?? projectDefaultStoryFocus);
  setSelectedPlotStage(persistedDraft.plotStage ?? projectDefaultPlotStage);
  setSelectedQualityPreset(projectDefaultQualityPreset);
  setSelectedQualityNotes(projectDefaultQualityNotes);

  if (!persistedDraft.plotStage && !projectDefaultPlotStage) {
    void inferPlotStage({ chapterNumber: currentChapterNumber, totalChapters }).then((stage) => {
      if (!isCancelled()) {
        setSelectedPlotStage(stage);
      }
    });
  }

  setStoryCreationBriefDraft(persistedDraft.storyCreationBriefDraft ?? projectDefaultBrief);
  setBeatPlannerDraft(normalizeStoryBeatPlannerDraft(persistedDraft.beatPlannerDraft));
  setSceneOutlineDraft(normalizeStorySceneOutlineDraft(persistedDraft.sceneOutlineDraft));
}

export async function restoreBatchStoryCreationDraft({
  storageKey,
  loadStoryCreationPersistence,
  resetCockpit,
  manualBriefSentinel,
  batchDefaultBrief,
  projectDefaultBrief,
  projectDefaultCreativeMode,
  projectDefaultStoryFocus,
  projectDefaultPlotStage,
  projectDefaultQualityPreset,
  projectDefaultQualityNotes,
  isCancelled,
  setAutoBriefRef,
  setBeatPlannerAutoRef,
  setSceneOutlineAutoRef,
  setSelectedCreativeMode,
  setSelectedStoryFocus,
  setSelectedPlotStage,
  setSelectedQualityPreset,
  setSelectedQualityNotes,
  setStoryCreationBriefDraft,
  setBeatPlannerDraft,
  setSceneOutlineDraft,
}: {
  storageKey: string | null;
  loadStoryCreationPersistence: LoadStoryCreationPersistenceForRestore;
  resetCockpit: () => void;
  manualBriefSentinel: string;
  batchDefaultBrief: string;
  projectDefaultBrief: string;
  projectDefaultCreativeMode?: CreativeMode;
  projectDefaultStoryFocus?: StoryFocus;
  projectDefaultPlotStage?: PlotStage;
  projectDefaultQualityPreset?: QualityPreset;
  projectDefaultQualityNotes: string;
  isCancelled: () => boolean;
  setAutoBriefRef: (value: string) => void;
  setBeatPlannerAutoRef: (value: StoryBeatPlannerDraft) => void;
  setSceneOutlineAutoRef: (value: StorySceneOutlineDraft) => void;
  setSelectedCreativeMode: (value: CreativeMode | undefined) => void;
  setSelectedStoryFocus: (value: StoryFocus | undefined) => void;
  setSelectedPlotStage: (value: PlotStage | undefined) => void;
  setSelectedQualityPreset: (value: QualityPreset | undefined) => void;
  setSelectedQualityNotes: (value: string) => void;
  setStoryCreationBriefDraft: (value: string) => void;
  setBeatPlannerDraft: (value: StoryBeatPlannerDraft) => void;
  setSceneOutlineDraft: (value: StorySceneOutlineDraft) => void;
}): Promise<void> {
  if (!storageKey) {
    resetCockpit();
    return;
  }

  const { getPersistedStoryCreationDraft } = await loadStoryCreationPersistence();
  const persistedDraft = getPersistedStoryCreationDraft(storageKey);
  if (isCancelled()) {
    return;
  }

  if (!persistedDraft) {
    resetCockpit();
    return;
  }

  applyPersistedDraftRefs({
    persistedDraft,
    manualBriefSentinel,
    fallbackBrief: batchDefaultBrief,
    setAutoBriefRef,
    setBeatPlannerAutoRef,
    setSceneOutlineAutoRef,
  });

  setSelectedCreativeMode(persistedDraft.creativeMode ?? projectDefaultCreativeMode);
  setSelectedStoryFocus(persistedDraft.storyFocus ?? projectDefaultStoryFocus);
  setSelectedPlotStage(persistedDraft.plotStage ?? projectDefaultPlotStage);
  setSelectedQualityPreset(projectDefaultQualityPreset);
  setSelectedQualityNotes(projectDefaultQualityNotes);
  setStoryCreationBriefDraft(persistedDraft.storyCreationBriefDraft ?? projectDefaultBrief);
  setBeatPlannerDraft(normalizeStoryBeatPlannerDraft(persistedDraft.beatPlannerDraft));
  setSceneOutlineDraft(normalizeStorySceneOutlineDraft(persistedDraft.sceneOutlineDraft));
}

export async function loadChapterStoryCreationSnapshots({
  storageKey,
  loadStoryCreationPersistence,
  setSnapshots,
  isCancelled,
}: {
  storageKey: string | null;
  loadStoryCreationPersistence: LoadStoryCreationPersistenceForRestore;
  setSnapshots: (snapshots: StoryCreationSnapshot[]) => void;
  isCancelled: () => boolean;
}): Promise<void> {
  if (!storageKey) {
    setSnapshots([]);
    return;
  }

  const { getPersistedStoryCreationSnapshots } = await loadStoryCreationPersistence();
  const snapshots = getPersistedStoryCreationSnapshots(storageKey);
  if (!isCancelled()) {
    setSnapshots(snapshots);
  }
}
