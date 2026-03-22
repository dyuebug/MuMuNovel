import { Suspense, lazy, useState, useEffect, useRef, useMemo, useCallback } from 'react';

import { List, Button, Modal, Form, Input, Select, message, Empty, Space, Badge, Tag, Card, InputNumber, Alert, Radio, Descriptions, Collapse, Popconfirm, FloatButton, Tooltip, Progress } from 'antd';

import { EditOutlined, FileTextOutlined, ThunderboltOutlined, LockOutlined, DownloadOutlined, SettingOutlined, FundOutlined, SyncOutlined, CheckCircleOutlined, CloseCircleOutlined, RocketOutlined, StopOutlined, InfoCircleOutlined, CaretRightOutlined, DeleteOutlined, BookOutlined, FormOutlined, PlusOutlined, ReadOutlined } from '@ant-design/icons';

import { useStore } from '../store';
import { useChapterSync } from '../store/hooks';
import { projectApi, writingStyleApi, chapterApi, chapterBatchTaskApi } from '../services/api';
import type { Chapter, ChapterUpdate, ApiError, WritingStyle, AnalysisTask, ExpansionPlanData, ChapterLatestQualityMetrics, ChapterQualityMetrics, ChapterQualityMetricsSummary, ChapterQualityProfileSummary, CreativeMode, PlotStage, StoryFocus } from '../types';
import { hasUsableApiCredentials } from '../utils/apiKey';
import type { TextAreaRef } from 'antd/es/input/TextArea';

import ExpansionPlanEditor from '../components/ExpansionPlanEditor';

import FloatingIndexPanel from '../components/FloatingIndexPanel';
import ChapterReader from '../components/ChapterReader';
import PartialRegenerateToolbar from '../components/PartialRegenerateToolbar';
import PartialRegenerateModal from '../components/PartialRegenerateModal';
import {
  buildCreationBlueprint,
  buildBatchScoreDrivenRecommendationCard,
  buildBatchStoryAfterScorecard,
  buildBatchStoryCreationControlCard,
  buildBatchStoryRepairTargetCard,
  buildCreationPresetRecommendation,
  buildScoreDrivenRecommendationCard,
  buildStoryAfterScorecard,
  buildStoryCreationControlCard,
  buildStoryRepairPromptPayload,
  buildStoryRepairTargetCard,
  buildStoryExecutionChecklist,
  buildStoryObjectiveCard,
  buildStoryRepetitionRiskCard,
  buildStoryResultCard,
  buildStoryAcceptanceCard,
  buildStoryCharacterArcCard,
  buildVolumePacingPlan,
  CREATION_PLOT_STAGE_OPTIONS,
  CREATION_PRESETS,
  getCreationPresetById,
  getCreationPresetByModes,
  inferCreationPlotStage,
  type CreationPresetId,
} from '../utils/creationPresets';


const { TextArea } = Input;

interface StoryBeatPlannerDraft {
  openingHook: string;
  chapterGoal: string;
  conflictPressure: string;
  turningPoint: string;
  endingHook: string;
}

interface StorySceneOutlineDraft {
  setupScene: string;
  confrontationScene: string;
  reversalScene: string;
  payoffScene: string;
}

const EMPTY_STORY_BEAT_PLANNER_DRAFT: StoryBeatPlannerDraft = {
  openingHook: '',
  chapterGoal: '',
  conflictPressure: '',
  turningPoint: '',
  endingHook: '',
};

const EMPTY_STORY_SCENE_OUTLINE_DRAFT: StorySceneOutlineDraft = {
  setupScene: '',
  confrontationScene: '',
  reversalScene: '',
  payoffScene: '',
};

const STORY_BEAT_PLANNER_FIELDS: Array<{
  key: keyof StoryBeatPlannerDraft;
  label: string;
  placeholder: string;
}> = [
  {
    key: 'openingHook',
    label: '开篇钩子',
    placeholder: '一句话抓住读者注意力，说明本章的开篇亮点',
  },
  {
    key: 'chapterGoal',
    label: '章节目标',
    placeholder: '本章需要达成的主要目标或推进点',
  },
  {
    key: 'conflictPressure',
    label: '冲突压力',
    placeholder: '本章的主要冲突与压力来源',
  },
  {
    key: 'turningPoint',
    label: '转折点',
    placeholder: '本章出现的关键转折或意外',
  },
  {
    key: 'endingHook',
    label: '结尾钩子',
    placeholder: '结尾留下悬念或引导下一章',
  },
];

const STORY_SCENE_OUTLINE_FIELDS: Array<{
  key: keyof StorySceneOutlineDraft;
  label: string;
  placeholder: string;
}> = [
  {
    key: 'setupScene',
    label: '场景一：铺垫',
    placeholder: '交代场景、角色状态与背景',
  },
  {
    key: 'confrontationScene',
    label: '场景二：对抗',
    placeholder: '冲突升级，推动情节发展',
  },
  {
    key: 'reversalScene',
    label: '场景三：转折',
    placeholder: '出现反转或新的信息',
  },
  {
    key: 'payoffScene',
    label: '场景四：收束',
    placeholder: '解决本章矛盾并埋下下一章线索',
  },
];

const normalizeStoryBeatPlannerDraft = (
  draft?: Partial<StoryBeatPlannerDraft> | null,
): StoryBeatPlannerDraft => ({
  openingHook: draft?.openingHook?.trim() ?? '',
  chapterGoal: draft?.chapterGoal?.trim() ?? '',
  conflictPressure: draft?.conflictPressure?.trim() ?? '',
  turningPoint: draft?.turningPoint?.trim() ?? '',
  endingHook: draft?.endingHook?.trim() ?? '',
});

const normalizeStorySceneOutlineDraft = (
  draft?: Partial<StorySceneOutlineDraft> | null,
): StorySceneOutlineDraft => ({
  setupScene: draft?.setupScene?.trim() ?? '',
  confrontationScene: draft?.confrontationScene?.trim() ?? '',
  reversalScene: draft?.reversalScene?.trim() ?? '',
  payoffScene: draft?.payoffScene?.trim() ?? '',
});

const isStoryBeatPlannerDraftEmpty = (
  draft?: Partial<StoryBeatPlannerDraft> | null,
): boolean => {
  const normalizedDraft = normalizeStoryBeatPlannerDraft(draft);
  return Object.values(normalizedDraft).every((value) => !value);
};

const isStorySceneOutlineDraftEmpty = (
  draft?: Partial<StorySceneOutlineDraft> | null,
): boolean => {
  const normalizedDraft = normalizeStorySceneOutlineDraft(draft);
  return Object.values(normalizedDraft).every((value) => !value);
};

const areStoryBeatPlannerDraftsEqual = (
  left?: Partial<StoryBeatPlannerDraft> | null,
  right?: Partial<StoryBeatPlannerDraft> | null,
): boolean => {
  const leftDraft = normalizeStoryBeatPlannerDraft(left);
  const rightDraft = normalizeStoryBeatPlannerDraft(right);

  return STORY_BEAT_PLANNER_FIELDS.every((field) => leftDraft[field.key] === rightDraft[field.key]);
};

const areStorySceneOutlineDraftsEqual = (
  left?: Partial<StorySceneOutlineDraft> | null,
  right?: Partial<StorySceneOutlineDraft> | null,
): boolean => {
  const leftDraft = normalizeStorySceneOutlineDraft(left);
  const rightDraft = normalizeStorySceneOutlineDraft(right);

  return STORY_SCENE_OUTLINE_FIELDS.every((field) => leftDraft[field.key] === rightDraft[field.key]);
};

const buildJoinedInstruction = (...parts: Array<string | undefined>): string => {
  const normalizedParts = parts.map((item) => item?.trim()).filter((item): item is string => Boolean(item));
  return normalizedParts.join('; ');
};

const buildStoryBeatPlannerPrompt = (
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
    ? '批量故事节拍规划（逐项填写）'
    : '故事节拍规划（逐项填写）';
  return [title, ...entries.map((item) => `- ${item.label}: ${item.value}`)].join('\n');
};

const buildStorySceneOutlineSuggestion = (options: {
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

const buildStorySceneOutlinePrompt = (
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
    ? '批量故事场景提纲（逐项填写）'
    : '故事场景提纲（逐项填写）';
  return [title, ...entries.map((item) => `${item.index}. ${item.label}: ${item.value}`)].join('\n');
};

const mergeStoryCreationInstructions = (...parts: Array<string | undefined>): string | undefined => {
  const normalizedParts = parts
    .map((item) => item?.trim())
    .filter((item): item is string => Boolean(item));

  return normalizedParts.length > 0 ? normalizedParts.join('\n\n') : undefined;
};

const STORY_CREATION_PROMPT_WARN_THRESHOLD = 1000;

const buildStoryCreationPromptLayerLabels = (parts: {
  summary?: string;
  beat?: string;
  scene?: string;
}): string[] => [
  parts.summary?.trim() ? '梗概' : '',
  parts.beat?.trim() ? '节拍规划' : '',
  parts.scene?.trim() ? '场景提纲' : '',
].filter(Boolean);





const WORD_COUNT_CACHE_KEY = 'chapter_default_word_count';

const BATCH_TASK_META_STORAGE_KEY = 'chapter_batch_task_meta_map_v1';

const STORY_CREATION_DRAFT_STORAGE_KEY = 'chapter_story_creation_draft_v1';

const STORY_CREATION_SNAPSHOT_STORAGE_KEY = 'chapter_story_creation_snapshot_v1';

const STORY_CREATION_SNAPSHOT_LIMIT = 12;

const STORY_CREATION_SNAPSHOT_PREVIEW_LIMIT = 5;

const DEFAULT_WORD_COUNT = 3000;

const writingStylesLoadPromises = new Map<string, Promise<void>>();

const batchTaskRestorePromises = new Map<string, Promise<void>>();



const LazyChapterAnalysis = lazy(() => import('../components/ChapterAnalysis'));



const LazySSELoadingOverlay = lazy(async () => {

  const module = await import('../components/SSELoadingOverlay');

  return { default: module.SSELoadingOverlay };

});



const LazySSEProgressModal = lazy(async () => {

  const module = await import('../components/SSEProgressModal');

  return { default: module.SSEProgressModal };

});

const writingStylesCache = new Map<string, { styles: WritingStyle[]; defaultStyleId?: number }>();

const chapterAnalysisTasksCache = new Map<string, Record<string, AnalysisTask>>();



const getOverallScoreColor = (score?: number): string => {

  if ((score ?? 0) >= 75) return 'green';

  if ((score ?? 0) >= 60) return 'gold';

  return 'red';

};



const getMetricRateColor = (rate?: number): string => {

  if ((rate ?? 0) >= 70) return 'green';

  if ((rate ?? 0) >= 45) return 'gold';

  return 'red';

};



const getMetricStrokeColor = (rate?: number): string => {

  if ((rate ?? 0) >= 70) return '#52c41a';

  if ((rate ?? 0) >= 45) return '#faad14';

  return '#ff4d4f';

};



const QUALITY_METRIC_TIPS: Record<string, string> = {
  conflict: '冲突链覆盖情况。',
  rule: '规则锚定情况。',
  opening: '开篇钩子表现。',
  payoff: '回收链表现。',
  cliffhanger: '悬念收尾强度。',
  dialogue: '对话自然度。',
  outline: '大纲贴合度。',
};


const CREATIVE_MODE_OPTIONS: Array<{ value: CreativeMode; label: string; description: string }> = [
  { value: 'balanced', label: '均衡推进', description: '在多个故事节拍之间保持均衡。' },
  { value: 'hook', label: '强化钩子', description: '突出开篇钩子与吸引力。' },
  { value: 'emotion', label: '情感共鸣', description: '强化情绪张力与共鸣。' },
  { value: 'suspense', label: '悬念拉升', description: '增强悬念感与紧张感。' },
  { value: 'relationship', label: '关系推进', description: '聚焦人物关系变化。' },
  { value: 'payoff', label: '强化回收', description: '加强伏笔回收与结果落地。' },
];


const STORY_FOCUS_OPTIONS: Array<{ value: StoryFocus; label: string; description: string }> = [
  { value: 'advance_plot', label: '推进主线', description: '推动主线情节继续发展。' },
  { value: 'deepen_character', label: '深化角色', description: '增强角色层次与深度。' },
  { value: 'escalate_conflict', label: '升级冲突', description: '提高风险与冲突强度。' },
  { value: 'reveal_mystery', label: '揭示谜团', description: '揭示新的信息或线索。' },
  { value: 'relationship_shift', label: '关系转变', description: '推动关系或阵营发生变化。' },
  { value: 'foreshadow_payoff', label: '铺垫回收', description: '为后续回收埋设伏笔。' },
];



const getWeakestQualityMetric = (metrics: ChapterQualityMetrics): { label: string; value: number } => {
  const items = [
    { label: '冲突链', value: metrics.conflict_chain_hit_rate },
    { label: '规则锚定', value: metrics.rule_grounding_hit_rate },
    { label: '开篇钩子', value: metrics.opening_hook_rate },
    { label: '回收链', value: metrics.payoff_chain_rate },
    { label: '悬念收尾', value: metrics.cliffhanger_rate },
    { label: '对话自然度', value: metrics.dialogue_naturalness_rate },
    { label: '大纲贴合度', value: metrics.outline_alignment_rate },
  ];
  return items.reduce((min, item) => (item.value < min.value ? item : min), items[0]);
};




const getQualityMetricItems = (metrics: ChapterQualityMetrics) => [
  { key: 'conflict', label: '冲突链', value: metrics.conflict_chain_hit_rate, tip: QUALITY_METRIC_TIPS.conflict },
  { key: 'rule', label: '规则锚定', value: metrics.rule_grounding_hit_rate, tip: QUALITY_METRIC_TIPS.rule },
  { key: 'opening', label: '开篇钩子', value: metrics.opening_hook_rate, tip: QUALITY_METRIC_TIPS.opening },
  { key: 'payoff', label: '回收链', value: metrics.payoff_chain_rate, tip: QUALITY_METRIC_TIPS.payoff },
  { key: 'cliffhanger', label: '悬念收尾', value: metrics.cliffhanger_rate, tip: QUALITY_METRIC_TIPS.cliffhanger },
  { key: 'dialogue', label: '对话自然度', value: metrics.dialogue_naturalness_rate, tip: QUALITY_METRIC_TIPS.dialogue },
  { key: 'outline', label: '大纲贴合度', value: metrics.outline_alignment_rate, tip: QUALITY_METRIC_TIPS.outline },
];




const getBatchSummaryMetricItems = (summary?: {
  avg_conflict_chain_hit_rate?: number;
  avg_rule_grounding_hit_rate?: number;
  avg_opening_hook_rate?: number;
  avg_payoff_chain_rate?: number;
  avg_cliffhanger_rate?: number;
}) => [
  { key: 'conflict', label: '冲突链', value: summary?.avg_conflict_chain_hit_rate ?? 0, tip: QUALITY_METRIC_TIPS.conflict },
  { key: 'rule', label: '规则锚定', value: summary?.avg_rule_grounding_hit_rate ?? 0, tip: QUALITY_METRIC_TIPS.rule },
  { key: 'opening', label: '开篇钩子', value: summary?.avg_opening_hook_rate ?? 0, tip: QUALITY_METRIC_TIPS.opening },
  { key: 'payoff', label: '回收链', value: summary?.avg_payoff_chain_rate ?? 0, tip: QUALITY_METRIC_TIPS.payoff },
  { key: 'cliffhanger', label: '悬念收尾', value: summary?.avg_cliffhanger_rate ?? 0, tip: QUALITY_METRIC_TIPS.cliffhanger },
];




const QUALITY_PROFILE_BLOCK_ORDER: Array<keyof Pick<ChapterQualityProfileSummary, 'generation' | 'checker' | 'reviser' | 'mcp_guard' | 'external_assets_block'>> = [

  'generation',

  'checker',

  'reviser',

  'mcp_guard',

  'external_assets_block',

];



const QUALITY_PROFILE_BLOCK_LABELS: Record<typeof QUALITY_PROFILE_BLOCK_ORDER[number], string> = {
  generation: '生成',
  checker: '检查',
  reviser: '修订',
  mcp_guard: 'MCP 守卫',
  external_assets_block: '外部资源',
};




const getQualityProfileDisplayItems = (summary?: ChapterQualityProfileSummary | null) => {
  if (!summary) {
    return [];
  }

  const items: Array<{ key: string; label: string; description: string }> = [];

  if (summary.baseline_id) {
    items.push({ key: 'baseline', label: '基线', description: summary.baseline_id });
  }

  if (summary.version) {
    items.push({ key: 'version', label: '版本', description: summary.version });
  }

  if (summary.style_profile) {
    items.push({ key: 'style', label: '风格', description: summary.style_profile });
  }

  if (summary.genre_profiles?.length) {
    items.push({ key: 'genres', label: '题材', description: summary.genre_profiles.join(' / ') });
  }

  if (summary.quality_dimensions?.length) {
    items.push({ key: 'dimensions', label: '维度', description: summary.quality_dimensions.join(' / ') });
  }

  QUALITY_PROFILE_BLOCK_ORDER.forEach((blockKey) => {
    const block = summary[blockKey];
    const description = block?.summary || block?.title || block?.lines?.[0] || block?.prompt_blocks?.[0];
    if (description) {
      items.push({
        key: blockKey,
        label: QUALITY_PROFILE_BLOCK_LABELS[blockKey],
        description,
      });
    }
  });

  return items;
};

const getCachedWordCount = (): number => {

  try {

    const cached = localStorage.getItem(WORD_COUNT_CACHE_KEY);

    if (cached) {

      const value = parseInt(cached, 10);

      if (!isNaN(value) && value >= 500 && value <= 10000) {

        return value;

      }

    }

  } catch (error) {

    console.warn('闁荤姴娲╅褑銇愰崶鈹惧亾濞戞瑯娈曢柡鍡欏枔缁辨捇骞樺畷鍥ㄦ喖婵犮垺鍎肩划鍓ф喆?', error);

  }

  return DEFAULT_WORD_COUNT;

};




const setCachedWordCount = (value: number): void => {

  try {

    localStorage.setItem(WORD_COUNT_CACHE_KEY, String(value));

  } catch (error) {

    console.warn('婵烇絽娲︾换鍌炴偤閵娧€鍋撳☉娆樻畷闁哄棛鍠撶槐鎾诲箻瀹曞洦鎲兼繝銏″劶缁墽鎲?', error);

  }

};



type PersistedStoryCreationDraft = {
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

type StoryCreationSnapshotReason = 'manual' | 'generate';

type StoryCreationSnapshotScope = 'single' | 'batch';

type StoryCreationSnapshot = PersistedStoryCreationDraft & {
  id: string;
  scope: StoryCreationSnapshotScope;
  createdAt: string;
  reason: StoryCreationSnapshotReason;
  label: string;
  prompt?: string;
  promptLayerLabels?: string[];
  promptCharCount?: number;
};

const MANUAL_STORY_CREATION_BRIEF_SENTINEL = '__manual_story_creation_brief__';

const normalizePersistedStoryCreationDraft = (value: unknown): PersistedStoryCreationDraft | null => {
  if (!value || typeof value !== 'object') {
    return null;
  }

  const draft = value as Record<string, unknown>;

  return {
    creativeMode: typeof draft.creativeMode === 'string' ? draft.creativeMode as CreativeMode : undefined,
    storyFocus: typeof draft.storyFocus === 'string' ? draft.storyFocus as StoryFocus : undefined,
    plotStage: typeof draft.plotStage === 'string' ? draft.plotStage as PlotStage : undefined,
    narrativePerspective: typeof draft.narrativePerspective === 'string' ? draft.narrativePerspective : undefined,
    storyCreationBriefDraft: typeof draft.storyCreationBriefDraft === 'string' ? draft.storyCreationBriefDraft : undefined,
    beatPlannerDraft: normalizeStoryBeatPlannerDraft(draft.beatPlannerDraft as Partial<StoryBeatPlannerDraft> | null),
    sceneOutlineDraft: normalizeStorySceneOutlineDraft(draft.sceneOutlineDraft as Partial<StorySceneOutlineDraft> | null),
    isBriefCustomized: draft.isBriefCustomized === true,
    isBeatPlannerCustomized: draft.isBeatPlannerCustomized === true,
    isSceneOutlineCustomized: draft.isSceneOutlineCustomized === true,
    updatedAt: typeof draft.updatedAt === 'string' ? draft.updatedAt : undefined,
  };
};

const readPersistedStoryCreationDraftMap = (): Record<string, PersistedStoryCreationDraft> => {
  try {
    const raw = localStorage.getItem(STORY_CREATION_DRAFT_STORAGE_KEY);

    if (!raw) {
      return {};
    }

    const parsed = JSON.parse(raw) as Record<string, unknown>;

    if (!parsed || typeof parsed !== 'object') {
      return {};
    }

    const normalized: Record<string, PersistedStoryCreationDraft> = {};

    Object.entries(parsed).forEach(([storageKey, value]) => {
      const normalizedDraft = normalizePersistedStoryCreationDraft(value);

      if (normalizedDraft) {
        normalized[storageKey] = normalizedDraft;
      }
    });

    return normalized;
  } catch (error) {
    console.warn('闁荤姴娲╅褑銇愰崶顒€绀嗘繛鎴烆殘缁嬪﹤顪冭ぐ鎺旂暫闁宠甯￠幊婊勬綇椤愩垻锛樼紓浣割儓濞夋洜妲愰敂閿亾濞戞顏堝Φ閹寸姵瀚?', error);
    return {};
  }
};

const writePersistedStoryCreationDraftMap = (map: Record<string, PersistedStoryCreationDraft>): void => {
  try {
    localStorage.setItem(STORY_CREATION_DRAFT_STORAGE_KEY, JSON.stringify(map));
  } catch (error) {
    console.warn('婵烇絽娲︾换鍌炴偤閵娾晛绀嗘繛鎴烆殘缁嬪﹤顪冭ぐ鎺旂暫闁宠甯￠幊婊勬綇椤愩垻锛樼紓浣割儓濞夋洜妲愰敂閿亾濞戞顏堝Φ閹寸姵瀚?', error);
  }
};

const persistStoryCreationDraft = (storageKey: string, draft: PersistedStoryCreationDraft): void => {
  const map = readPersistedStoryCreationDraftMap();
  map[storageKey] = draft;
  writePersistedStoryCreationDraftMap(map);
};

const getPersistedStoryCreationDraft = (storageKey: string): PersistedStoryCreationDraft | undefined => {
  const map = readPersistedStoryCreationDraftMap();
  return map[storageKey];
};

const normalizeStoryCreationSnapshot = (value: unknown): StoryCreationSnapshot | null => {
  const normalizedDraft = normalizePersistedStoryCreationDraft(value);

  if (!normalizedDraft || !value || typeof value !== 'object') {
    return null;
  }

  const snapshot = value as Record<string, unknown>;
  const scope = snapshot.scope === 'single' || snapshot.scope === 'batch'
    ? snapshot.scope as StoryCreationSnapshotScope
    : null;
  const reason = snapshot.reason === 'manual' || snapshot.reason === 'generate'
    ? snapshot.reason as StoryCreationSnapshotReason
    : null;

  if (!scope || !reason || typeof snapshot.id !== 'string' || !snapshot.id.trim()) {
    return null;
  }

  const normalizedPrompt = typeof snapshot.prompt === 'string' ? snapshot.prompt.trim() : '';
  const normalizedBeatPrompt = buildStoryBeatPlannerPrompt(normalizedDraft.beatPlannerDraft, scope);
  const normalizedScenePrompt = buildStorySceneOutlinePrompt(normalizedDraft.sceneOutlineDraft, scope);
  const normalizedLayerLabels = Array.isArray(snapshot.promptLayerLabels)
    ? snapshot.promptLayerLabels
      .filter((item): item is string => typeof item === 'string' && Boolean(item.trim()))
      .map((item) => item.trim())
    : buildStoryCreationPromptLayerLabels({
      summary: normalizedDraft.storyCreationBriefDraft,
      beat: normalizedBeatPrompt,
      scene: normalizedScenePrompt,
    });
  const createdAt = typeof snapshot.createdAt === 'string' && snapshot.createdAt.trim()
    ? snapshot.createdAt
    : normalizedDraft.updatedAt ?? new Date(0).toISOString();
  const normalizedPromptCharCount = typeof snapshot.promptCharCount === 'number' && Number.isFinite(snapshot.promptCharCount)
    ? Math.max(0, Math.round(snapshot.promptCharCount))
    : normalizedPrompt.length;

  return {
    ...normalizedDraft,
    id: snapshot.id.trim(),
    scope,
    createdAt,
    reason,
    label: typeof snapshot.label === 'string' && snapshot.label.trim()
      ? snapshot.label.trim()
      : reason === 'generate'
        ? '自动生成'
        : '手动保存',
    prompt: normalizedPrompt || undefined,
    promptLayerLabels: normalizedLayerLabels,
    promptCharCount: normalizedPromptCharCount,
  };
};

const resolveStoryCreationSnapshotTimestamp = (value?: string): number => {
  const timestamp = value ? new Date(value).getTime() : Number.NaN;
  return Number.isFinite(timestamp) ? timestamp : 0;
};

const readPersistedStoryCreationSnapshotMap = (): Record<string, StoryCreationSnapshot[]> => {
  try {
    const raw = localStorage.getItem(STORY_CREATION_SNAPSHOT_STORAGE_KEY);

    if (!raw) {
      return {};
    }

    const parsed = JSON.parse(raw) as Record<string, unknown>;

    if (!parsed || typeof parsed !== 'object') {
      return {};
    }

    const normalized: Record<string, StoryCreationSnapshot[]> = {};

    Object.entries(parsed).forEach(([storageKey, value]) => {
      if (!Array.isArray(value)) {
        return;
      }

      const snapshots = value
        .map((item) => normalizeStoryCreationSnapshot(item))
        .filter((item): item is StoryCreationSnapshot => Boolean(item))
        .sort((left, right) => (
          resolveStoryCreationSnapshotTimestamp(right.createdAt)
          - resolveStoryCreationSnapshotTimestamp(left.createdAt)
        ))
        .slice(0, STORY_CREATION_SNAPSHOT_LIMIT);

      if (snapshots.length > 0) {
        normalized[storageKey] = snapshots;
      }
    });

    return normalized;
  } catch (error) {
    console.warn('鐠囪褰囬崚娑楃稊韫囶偆鍙庣紓鎾崇摠婢惰精瑙?', error);
    return {};
  }
};

const writePersistedStoryCreationSnapshotMap = (map: Record<string, StoryCreationSnapshot[]>): void => {
  try {
    localStorage.setItem(STORY_CREATION_SNAPSHOT_STORAGE_KEY, JSON.stringify(map));
  } catch (error) {
    console.warn('娣囨繂鐡ㄩ崚娑楃稊韫囶偆鍙庣紓鎾崇摠婢惰精瑙?', error);
  }
};

const getPersistedStoryCreationSnapshots = (storageKey: string): StoryCreationSnapshot[] => {
  const map = readPersistedStoryCreationSnapshotMap();
  return map[storageKey] ?? [];
};

const persistStoryCreationSnapshot = (storageKey: string, snapshot: StoryCreationSnapshot): StoryCreationSnapshot[] => {
  const map = readPersistedStoryCreationSnapshotMap();
  const nextSnapshots = [
    snapshot,
    ...(map[storageKey] ?? []).filter((item) => item.id !== snapshot.id),
  ]
    .sort((left, right) => (
      resolveStoryCreationSnapshotTimestamp(right.createdAt)
      - resolveStoryCreationSnapshotTimestamp(left.createdAt)
    ))
    .slice(0, STORY_CREATION_SNAPSHOT_LIMIT);

  map[storageKey] = nextSnapshots;
  writePersistedStoryCreationSnapshotMap(map);
  return nextSnapshots;
};

const removePersistedStoryCreationSnapshot = (storageKey: string, snapshotId: string): StoryCreationSnapshot[] => {
  const map = readPersistedStoryCreationSnapshotMap();
  const nextSnapshots = (map[storageKey] ?? []).filter((item) => item.id !== snapshotId);

  if (nextSnapshots.length > 0) {
    map[storageKey] = nextSnapshots;
  } else {
    delete map[storageKey];
  }

  writePersistedStoryCreationSnapshotMap(map);
  return nextSnapshots;
};

const buildStoryCreationSnapshotId = (): string => {
  if (typeof globalThis.crypto?.randomUUID === 'function') {
    return globalThis.crypto.randomUUID();
  }

  return `snapshot_${Date.now()}_${Math.random().toString(36).slice(2, 8)}`;
};

const normalizeOptionalText = (value?: string | null): string => value?.trim() ?? '';

const areStoryCreationDraftMetaFieldsEqual = (
  left?: Partial<PersistedStoryCreationDraft> | null,
  right?: Partial<PersistedStoryCreationDraft> | null,
  options?: { includeNarrativePerspective?: boolean },
): boolean => {
  const includeNarrativePerspective = options?.includeNarrativePerspective === true;

  return (left?.creativeMode ?? undefined) === (right?.creativeMode ?? undefined)
    && (left?.storyFocus ?? undefined) === (right?.storyFocus ?? undefined)
    && (left?.plotStage ?? undefined) === (right?.plotStage ?? undefined)
    && (!includeNarrativePerspective || normalizeOptionalText(left?.narrativePerspective) === normalizeOptionalText(right?.narrativePerspective));
};

const areStoryCreationDraftContentsEqual = (
  left?: Partial<PersistedStoryCreationDraft> | null,
  right?: Partial<PersistedStoryCreationDraft> | null,
  options?: { includeNarrativePerspective?: boolean },
): boolean => (
  areStoryCreationDraftMetaFieldsEqual(left, right, options)
  && normalizeOptionalText(left?.storyCreationBriefDraft) === normalizeOptionalText(right?.storyCreationBriefDraft)
  && areStoryBeatPlannerDraftsEqual(left?.beatPlannerDraft, right?.beatPlannerDraft)
  && areStorySceneOutlineDraftsEqual(left?.sceneOutlineDraft, right?.sceneOutlineDraft)
);

const hasMeaningfulStoryCreationDraft = (
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

const buildStoryCreationSnapshotDiffLabels = (
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
    labels.push('节拍规划');
  }

  if (!areStorySceneOutlineDraftsEqual(snapshot.sceneOutlineDraft, currentDraft.sceneOutlineDraft)) {
    labels.push('场景提纲');
  }

  if (!areStoryCreationDraftMetaFieldsEqual(snapshot, currentDraft, { includeNarrativePerspective })) {
    labels.push('设置');
  }

  return labels;
};

const buildSingleStoryCreationDraftStorageKey = (projectId: string, chapterId: string): string => (
  `${projectId}::single::${chapterId}`
);

const buildBatchStoryCreationDraftStorageKey = (projectId: string): string => (
  `${projectId}::batch`
);

type StoryCreationSnapshotPanelProps = {
  scopeLabel: 'single' | 'batch';
  description: string;
  emptyText: string;
  snapshots: StoryCreationSnapshot[];
  currentDraft: PersistedStoryCreationDraft;
  canSave: boolean;
  onSave: () => void;
  onApply: (snapshot: StoryCreationSnapshot) => void;
  onDelete: (snapshotId: string) => void;
  onCopy: (content: string | undefined, scopeLabel: 'single' | 'batch') => Promise<void>;
  includeNarrativePerspective?: boolean;
};

const StoryCreationSnapshotPanel = ({
  scopeLabel,
  description,
  emptyText,
  snapshots,
  currentDraft,
  canSave,
  onSave,
  onApply,
  onDelete,
  onCopy,
  includeNarrativePerspective = false,
}: StoryCreationSnapshotPanelProps) => {
  const recentSnapshots = snapshots.slice(0, STORY_CREATION_SNAPSHOT_PREVIEW_LIMIT);

  return (
    <div style={{ padding: '10px 12px', border: '1px solid #f0f0f0', borderRadius: 8 }}>
      <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'flex-start', gap: 12, marginBottom: 8 }}>
        <div>
          <div style={{ fontWeight: 600, marginBottom: 4 }}>{'快照'}</div>
          <div style={{ color: 'var(--color-text-secondary)', fontSize: 12 }}>{description}</div>
        </div>
        <Space size={[8, 8]} wrap>
          {snapshots.length > 0 && <Tag color="purple">{`总数：${snapshots.length}`}</Tag>}
          <Button size="small" onClick={onSave} disabled={!canSave}>
            保存快照
          </Button>
        </Space>
      </div>
      {recentSnapshots.length > 0 ? (
        <Space direction="vertical" size={8} style={{ display: 'flex' }}>
          {recentSnapshots.map((snapshot) => {
            const diffLabels = buildStoryCreationSnapshotDiffLabels(snapshot, currentDraft, includeNarrativePerspective);

            return (
              <div
                key={snapshot.id}
                style={{
                  padding: '10px 12px',
                  border: '1px solid #f0f0f0',
                  borderRadius: 8,
                  background: '#fafafa',
                }}
              >
                <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'flex-start', gap: 12, marginBottom: 8 }}>
                  <div>
                    <div style={{ fontWeight: 600 }}>{snapshot.label}</div>
                    <div style={{ color: 'var(--color-text-secondary)', fontSize: 12 }}>
                      {new Date(snapshot.createdAt).toLocaleString()}
                    </div>
                  </div>
                  <Space wrap size={[6, 6]}>
                    <Tag color={snapshot.reason === 'manual' ? 'green' : 'purple'}>
                      {snapshot.reason === 'manual' ? '手动' : '自动'}
                    </Tag>
                    <Tag color={(snapshot.promptCharCount ?? 0) >= STORY_CREATION_PROMPT_WARN_THRESHOLD ? 'gold' : 'blue'}>
                      {`字符：${snapshot.promptCharCount ?? 0}`}
                    </Tag>
                  </Space>
                </div>
                {snapshot.promptLayerLabels?.length ? (
                  <Space wrap size={[6, 6]} style={{ marginBottom: 8 }}>
                    {snapshot.promptLayerLabels.map((item) => (
                      <Tag key={`${snapshot.id}-${item}`} color="processing">{item}</Tag>
                    ))}
                  </Space>
                ) : null}
                {diffLabels.length > 0 && (
                  <Space wrap size={[6, 6]} style={{ marginBottom: 8 }}>
                    {diffLabels.map((item) => (
                      <Tag key={`${snapshot.id}-${item}`} color="orange">{item}</Tag>
                    ))}
                  </Space>
                )}
                <div style={{ color: 'var(--color-text-secondary)', fontSize: 12, marginBottom: 8 }}>
                  {snapshot.prompt
                    ? '已保存提示词文本，可直接复用。'
                    : '该快照未保存提示词文本。'}
                </div>
                <Space wrap size={[8, 8]}>
                  <Button size="small" onClick={() => onApply(snapshot)}>
                    应用
                  </Button>
                  <Button
                    size="small"
                    type="link"
                    disabled={!snapshot.prompt}
                    onClick={() => void onCopy(snapshot.prompt, scopeLabel)}
                  >
                    复制
                  </Button>
                  <Popconfirm
                    title="删除这个快照？"
                    okText="删除"
                    cancelText="取消"
                    onConfirm={() => onDelete(snapshot.id)}
                  >
                    <Button size="small" type="link" danger>
                      删除
                    </Button>
                  </Popconfirm>
                </Space>
              </div>
            );
          })}
        </Space>
      ) : (
        <Empty image={Empty.PRESENTED_IMAGE_SIMPLE} description={emptyText} />
      )}
    </div>
  );
};

type BatchTaskMeta = {

  startChapterNumber: number;

  count: number;

  autoAnalyze: boolean;

  projectId?: string;

};



const isValidBatchTaskMeta = (value: unknown): value is BatchTaskMeta => {

  if (!value || typeof value !== 'object') {

    return false;

  }



  const meta = value as Record<string, unknown>;

  return (

    typeof meta.startChapterNumber === 'number' &&

    typeof meta.count === 'number' &&

    typeof meta.autoAnalyze === 'boolean'

  );

};



const readPersistedBatchTaskMetaMap = (): Record<string, BatchTaskMeta> => {

  try {

    const raw = localStorage.getItem(BATCH_TASK_META_STORAGE_KEY);

    if (!raw) {

      return {};

    }



    const parsed = JSON.parse(raw) as Record<string, unknown>;

    if (!parsed || typeof parsed !== 'object') {

      return {};

    }



    const normalized: Record<string, BatchTaskMeta> = {};

    Object.entries(parsed).forEach(([taskId, value]) => {

      if (isValidBatchTaskMeta(value)) {

        normalized[taskId] = value;

      }

    });

    return normalized;

  } catch (error) {

    console.warn('闁荤姴娲╅褑銇愰崶顒€绠ョ憸鐗堝笒濞呫倕霉閻樹警鍤欏┑顔惧枛瀹曟宕橀埡鍌涱啀闂佺顕栭崰姘辨閿旈敮鍋撳☉娅亪濡甸幋鐘冲?', error);

    return {};

  }

};



const writePersistedBatchTaskMetaMap = (map: Record<string, BatchTaskMeta>): void => {

  try {

    localStorage.setItem(BATCH_TASK_META_STORAGE_KEY, JSON.stringify(map));

  } catch (error) {

    console.warn('婵烇絽娲︾换鍌炴偤閵娾晛绠ョ憸鐗堝笒濞呫倕霉閻樹警鍤欏┑顔惧枛瀹曟宕橀埡鍌涱啀闂佺顕栭崰姘辨閿旈敮鍋撳☉娅亪濡甸幋鐘冲?', error);

  }

};



const persistBatchTaskMeta = (taskId: string, meta: BatchTaskMeta): void => {

  const map = readPersistedBatchTaskMetaMap();

  map[taskId] = meta;

  writePersistedBatchTaskMetaMap(map);

};



const getPersistedBatchTaskMeta = (taskId: string, projectId?: string): BatchTaskMeta | undefined => {

  const map = readPersistedBatchTaskMetaMap();

  const meta = map[taskId];

  if (!meta) {

    return undefined;

  }



  if (projectId && meta.projectId && meta.projectId !== projectId) {

    return undefined;

  }



  return meta;

};



const removePersistedBatchTaskMeta = (taskId: string): void => {

  const map = readPersistedBatchTaskMetaMap();

  if (!(taskId in map)) {

    return;

  }



  delete map[taskId];

  writePersistedBatchTaskMetaMap(map);

};



export default function Chapters() {

  const currentProject = useStore((state) => state.currentProject);
  const projectDefaultCreativeMode = currentProject?.default_creative_mode;
  const projectDefaultStoryFocus = currentProject?.default_story_focus;
  const projectDefaultPlotStage = currentProject?.default_plot_stage;
  const projectDefaultStoryCreationBrief = currentProject?.default_story_creation_brief?.trim() ?? '';

  const chapters = useStore((state) => state.chapters);

  const outlines = useStore((state) => state.outlines);

  const setCurrentChapter = useStore((state) => state.setCurrentChapter);

  const setCurrentProject = useStore((state) => state.setCurrentProject);

  const [modal, contextHolder] = Modal.useModal();

  const [isModalOpen, setIsModalOpen] = useState(false);

  const [isEditorOpen, setIsEditorOpen] = useState(false);

  const [isContinuing, setIsContinuing] = useState(false);

  const [isGenerating, setIsGenerating] = useState(false);

  const [editingId, setEditingId] = useState<string | null>(null);

  const editingChapterIdRef = useRef<string | null>(null);

  const isEditorOpenRef = useRef(false);

  const [runningSingleChapterTasks, setRunningSingleChapterTasks] = useState<Record<string, string>>({});

  const [form] = Form.useForm();

  const [editorForm] = Form.useForm();

  const [isMobile, setIsMobile] = useState(window.innerWidth <= 768);

  const contentTextAreaRef = useRef<TextAreaRef>(null);

  const [writingStyles, setWritingStyles] = useState<WritingStyle[]>([]);

  const [selectedStyleId, setSelectedStyleId] = useState<number | undefined>();

  const [targetWordCount, setTargetWordCount] = useState<number>(getCachedWordCount);

  const [availableModels, setAvailableModels] = useState<Array<{ value: string, label: string }>>([]);
  const [selectedModel, setSelectedModel] = useState<string | undefined>();
  const [batchSelectedModel, setBatchSelectedModel] = useState<string | undefined>(); // 闂佸綊娼х紞濠囧闯濞差亝鍋ㄩ柣鏃傤焾閻忓洭鏌ｉ妸銉ヮ仾閼垛晠鏌涢妸銉剶闁逞屽墮椤︽壆鈧?
  const [temporaryNarrativePerspective, setTemporaryNarrativePerspective] = useState<string | undefined>(); // 婵炴垶鎸搁悺銊ヮ渻閸屾稓顩查柧蹇撳ⅲ閻愮儤鐒诲璺侯儏椤?
  const [selectedCreativeMode, setSelectedCreativeMode] = useState<CreativeMode | undefined>();
  const [batchSelectedCreativeMode, setBatchSelectedCreativeMode] = useState<CreativeMode | undefined>();
  const [selectedStoryFocus, setSelectedStoryFocus] = useState<StoryFocus | undefined>();
  const [batchSelectedStoryFocus, setBatchSelectedStoryFocus] = useState<StoryFocus | undefined>();
  const [selectedPlotStage, setSelectedPlotStage] = useState<PlotStage | undefined>();
  const [batchSelectedPlotStage, setBatchSelectedPlotStage] = useState<PlotStage | undefined>();
  const [singleStoryCreationBriefDraft, setSingleStoryCreationBriefDraft] = useState('');
  const [batchStoryCreationBriefDraft, setBatchStoryCreationBriefDraft] = useState('');
  const [singleStoryBeatPlannerDraft, setSingleStoryBeatPlannerDraft] = useState<StoryBeatPlannerDraft>(EMPTY_STORY_BEAT_PLANNER_DRAFT);
  const [batchStoryBeatPlannerDraft, setBatchStoryBeatPlannerDraft] = useState<StoryBeatPlannerDraft>(EMPTY_STORY_BEAT_PLANNER_DRAFT);
  const [singleStorySceneOutlineDraft, setSingleStorySceneOutlineDraft] = useState<StorySceneOutlineDraft>(EMPTY_STORY_SCENE_OUTLINE_DRAFT);
  const [batchStorySceneOutlineDraft, setBatchStorySceneOutlineDraft] = useState<StorySceneOutlineDraft>(EMPTY_STORY_SCENE_OUTLINE_DRAFT);
  const [singleStoryCreationSnapshots, setSingleStoryCreationSnapshots] = useState<StoryCreationSnapshot[]>([]);
  const [batchStoryCreationSnapshots, setBatchStoryCreationSnapshots] = useState<StoryCreationSnapshot[]>([]);
  const [analysisVisible, setAnalysisVisible] = useState(false);
  const singleStoryCreationAutoBriefRef = useRef('');
  const batchStoryCreationAutoBriefRef = useRef('');
  const singleStoryBeatPlannerAutoRef = useRef<StoryBeatPlannerDraft>(EMPTY_STORY_BEAT_PLANNER_DRAFT);
  const batchStoryBeatPlannerAutoRef = useRef<StoryBeatPlannerDraft>(EMPTY_STORY_BEAT_PLANNER_DRAFT);
  const singleStorySceneOutlineAutoRef = useRef<StorySceneOutlineDraft>(EMPTY_STORY_SCENE_OUTLINE_DRAFT);
  const batchStorySceneOutlineAutoRef = useRef<StorySceneOutlineDraft>(EMPTY_STORY_SCENE_OUTLINE_DRAFT);

  const activeSingleCreationPreset = useMemo(
    () => getCreationPresetByModes(selectedCreativeMode, selectedStoryFocus),
    [selectedCreativeMode, selectedStoryFocus],
  );

  const activeBatchCreationPreset = useMemo(
    () => getCreationPresetByModes(batchSelectedCreativeMode, batchSelectedStoryFocus),
    [batchSelectedCreativeMode, batchSelectedStoryFocus],
  );

  const singleCreationBlueprint = useMemo(
    () => buildCreationBlueprint(selectedCreativeMode, selectedStoryFocus, {
      scene: 'chapter',
      plotStage: selectedPlotStage,
    }),
    [selectedCreativeMode, selectedStoryFocus, selectedPlotStage],
  );

  const batchCreationBlueprint = useMemo(
    () => buildCreationBlueprint(batchSelectedCreativeMode, batchSelectedStoryFocus, {
      scene: 'chapter',
      plotStage: batchSelectedPlotStage,
    }),
    [batchSelectedCreativeMode, batchSelectedStoryFocus, batchSelectedPlotStage],
  );

  const singleStoryObjectiveCard = useMemo(
    () => buildStoryObjectiveCard(selectedCreativeMode, selectedStoryFocus, {
      scene: 'chapter',
      plotStage: selectedPlotStage,
    }),
    [selectedCreativeMode, selectedStoryFocus, selectedPlotStage],
  );

  const batchStoryObjectiveCard = useMemo(
    () => buildStoryObjectiveCard(batchSelectedCreativeMode, batchSelectedStoryFocus, {
      scene: 'chapter',
      plotStage: batchSelectedPlotStage,
    }),
    [batchSelectedCreativeMode, batchSelectedStoryFocus, batchSelectedPlotStage],
  );

  const singleStoryResultCard = useMemo(
    () => buildStoryResultCard(selectedCreativeMode, selectedStoryFocus, {
      scene: 'chapter',
      plotStage: selectedPlotStage,
    }),
    [selectedCreativeMode, selectedStoryFocus, selectedPlotStage],
  );

  const batchStoryResultCard = useMemo(
    () => buildStoryResultCard(batchSelectedCreativeMode, batchSelectedStoryFocus, {
      scene: 'chapter',
      plotStage: batchSelectedPlotStage,
    }),
    [batchSelectedCreativeMode, batchSelectedStoryFocus, batchSelectedPlotStage],
  );

  const singleStoryExecutionChecklist = useMemo(
    () => buildStoryExecutionChecklist(selectedCreativeMode, selectedStoryFocus, {
      scene: 'chapter',
      plotStage: selectedPlotStage,
    }),
    [selectedCreativeMode, selectedStoryFocus, selectedPlotStage],
  );

  const batchStoryExecutionChecklist = useMemo(
    () => buildStoryExecutionChecklist(batchSelectedCreativeMode, batchSelectedStoryFocus, {
      scene: 'chapter',
      plotStage: batchSelectedPlotStage,
    }),
    [batchSelectedCreativeMode, batchSelectedStoryFocus, batchSelectedPlotStage],
  );

  const singleStoryRepetitionRiskCard = useMemo(
    () => buildStoryRepetitionRiskCard(selectedCreativeMode, selectedStoryFocus, {
      scene: 'chapter',
      plotStage: selectedPlotStage,
    }),
    [selectedCreativeMode, selectedStoryFocus, selectedPlotStage],
  );

  const batchStoryRepetitionRiskCard = useMemo(
    () => buildStoryRepetitionRiskCard(batchSelectedCreativeMode, batchSelectedStoryFocus, {
      scene: 'chapter',
      plotStage: batchSelectedPlotStage,
    }),
    [batchSelectedCreativeMode, batchSelectedStoryFocus, batchSelectedPlotStage],
  );

  const singleStoryAcceptanceCard = useMemo(
    () => buildStoryAcceptanceCard(selectedCreativeMode, selectedStoryFocus, {
      scene: 'chapter',
      plotStage: selectedPlotStage,
    }),
    [selectedCreativeMode, selectedStoryFocus, selectedPlotStage],
  );

  const batchStoryAcceptanceCard = useMemo(
    () => buildStoryAcceptanceCard(batchSelectedCreativeMode, batchSelectedStoryFocus, {
      scene: 'chapter',
      plotStage: batchSelectedPlotStage,
    }),
    [batchSelectedCreativeMode, batchSelectedStoryFocus, batchSelectedPlotStage],
  );

  const singleStoryCharacterArcCard = useMemo(
    () => buildStoryCharacterArcCard(selectedCreativeMode, selectedStoryFocus, {
      scene: 'chapter',
      plotStage: selectedPlotStage,
    }),
    [selectedCreativeMode, selectedStoryFocus, selectedPlotStage],
  );

  const batchStoryCharacterArcCard = useMemo(
    () => buildStoryCharacterArcCard(batchSelectedCreativeMode, batchSelectedStoryFocus, {
      scene: 'chapter',
      plotStage: batchSelectedPlotStage,
    }),
    [batchSelectedCreativeMode, batchSelectedStoryFocus, batchSelectedPlotStage],
  );

  const applySingleCreationPreset = useCallback((presetId: CreationPresetId) => {
    const preset = getCreationPresetById(presetId);
    if (!preset) return;
    setSelectedCreativeMode(preset.creativeMode);
    setSelectedStoryFocus(preset.storyFocus);
  }, []);

  const applyBatchCreationPreset = useCallback((presetId: CreationPresetId) => {
    const preset = getCreationPresetById(presetId);
    if (!preset) return;
    setBatchSelectedCreativeMode(preset.creativeMode);
    setBatchSelectedStoryFocus(preset.storyFocus);
  }, []);
  const [analysisChapterId, setAnalysisChapterId] = useState<string | null>(null);


  const [analysisTasksMap, setAnalysisTasksMap] = useState<Record<string, AnalysisTask>>({});

  const pollingIntervalsRef = useRef<Record<string, number>>({});

  const updateAnalysisTasksMap = useCallback((

    updater: Record<string, AnalysisTask> | ((prev: Record<string, AnalysisTask>) => Record<string, AnalysisTask>)

  ) => {

    setAnalysisTasksMap((prev) => {

      const next = typeof updater === 'function'

        ? (updater as (prev: Record<string, AnalysisTask>) => Record<string, AnalysisTask>)(prev)

        : updater;



      if (currentProject?.id) {

        chapterAnalysisTasksCache.set(currentProject.id, next);

      }



      return next;

    });

  }, [currentProject?.id]);

  const [isIndexPanelVisible, setIsIndexPanelVisible] = useState(false);




  const [readerVisible, setReaderVisible] = useState(false);

  const [readingChapter, setReadingChapter] = useState<Chapter | null>(null);




  const [planEditorVisible, setPlanEditorVisible] = useState(false);

  const [editingPlanChapter, setEditingPlanChapter] = useState<Chapter | null>(null);




  const [partialRegenerateToolbarVisible, setPartialRegenerateToolbarVisible] = useState(false);

  const [partialRegenerateToolbarPosition, setPartialRegenerateToolbarPosition] = useState({ top: 0, left: 0 });

  const [selectedTextForRegenerate, setSelectedTextForRegenerate] = useState('');

  const [selectionStartPosition, setSelectionStartPosition] = useState(0);

  const [selectionEndPosition, setSelectionEndPosition] = useState(0);

  const [partialRegenerateModalVisible, setPartialRegenerateModalVisible] = useState(false);




  const [singleChapterProgress, setSingleChapterProgress] = useState(0);
  const [singleChapterProgressMessage, setSingleChapterProgressMessage] = useState('');
  const [chapterQualityMetrics, setChapterQualityMetrics] = useState<ChapterQualityMetrics | null>(null);
  const [chapterQualityProfileSummary, setChapterQualityProfileSummary] = useState<ChapterQualityProfileSummary | null>(null);
  const [chapterQualityGeneratedAt, setChapterQualityGeneratedAt] = useState<string | null>(null);
  const [chapterQualityLoading, setChapterQualityLoading] = useState(false);

  const recommendedCreationPresets = useMemo(
    () => buildCreationPresetRecommendation(chapterQualityMetrics),
    [chapterQualityMetrics],
  );

  const [batchGenerateVisible, setBatchGenerateVisible] = useState(false);
  const [batchGenerating, setBatchGenerating] = useState(false);
  const [batchTaskId, setBatchTaskId] = useState<string | null>(null);
  const [batchForm] = Form.useForm();
  const [manualCreateForm] = Form.useForm();
  const batchStartChapterNumber = Form.useWatch('startChapterNumber', batchForm) as number | undefined;
  const [batchProgress, setBatchProgress] = useState<{
    status: string;

    total: number;

    completed: number;

    current_chapter_number: number | null;

    estimated_time_minutes?: number;

    latest_quality_metrics?: ChapterLatestQualityMetrics | null;
    quality_metrics_summary?: ChapterQualityMetricsSummary | null;
    quality_profile_summary?: ChapterQualityProfileSummary | null;
  } | null>(null);

  const maxKnownChapterNumber = useMemo(
    () => chapters.reduce((maxValue, chapter) => Math.max(maxValue, chapter.chapter_number || 0), 0),
    [chapters],
  );

  const knownStructureChapterCount = useMemo(
    () => Math.max(maxKnownChapterNumber, outlines.length),
    [maxKnownChapterNumber, outlines.length],
  );

  const currentEditingChapter = useMemo(
    () => chapters.find((chapter) => chapter.id === editingId),
    [chapters, editingId],
  );

  const singleStoryCreationDraftStorageKey = useMemo(
    () => (currentProject?.id && currentEditingChapter?.id
      ? buildSingleStoryCreationDraftStorageKey(currentProject.id, currentEditingChapter.id)
      : null),
    [currentProject?.id, currentEditingChapter?.id],
  );

  const batchStoryCreationDraftStorageKey = useMemo(
    () => (currentProject?.id ? buildBatchStoryCreationDraftStorageKey(currentProject.id) : null),
    [currentProject?.id],
  );

  const resetSingleStoryCreationCockpit = useCallback((chapterNumber?: number | null) => {
    singleStoryCreationAutoBriefRef.current = '';
    singleStoryBeatPlannerAutoRef.current = { ...EMPTY_STORY_BEAT_PLANNER_DRAFT };
    singleStorySceneOutlineAutoRef.current = { ...EMPTY_STORY_SCENE_OUTLINE_DRAFT };
    setTemporaryNarrativePerspective(undefined);
    setSelectedCreativeMode(projectDefaultCreativeMode);
    setSelectedStoryFocus(projectDefaultStoryFocus);
    setSelectedPlotStage(projectDefaultPlotStage ?? inferCreationPlotStage({
      chapterNumber: chapterNumber ?? undefined,
      totalChapters: knownStructureChapterCount,
    }));
    setSingleStoryCreationBriefDraft(projectDefaultStoryCreationBrief);
    setSingleStoryBeatPlannerDraft({ ...EMPTY_STORY_BEAT_PLANNER_DRAFT });
    setSingleStorySceneOutlineDraft({ ...EMPTY_STORY_SCENE_OUTLINE_DRAFT });
  }, [
    knownStructureChapterCount,
    projectDefaultCreativeMode,
    projectDefaultPlotStage,
    projectDefaultStoryCreationBrief,
    projectDefaultStoryFocus,
  ]);

  const resetBatchStoryCreationCockpit = useCallback(() => {
    batchStoryCreationAutoBriefRef.current = '';
    batchStoryBeatPlannerAutoRef.current = { ...EMPTY_STORY_BEAT_PLANNER_DRAFT };
    batchStorySceneOutlineAutoRef.current = { ...EMPTY_STORY_SCENE_OUTLINE_DRAFT };
    setBatchSelectedCreativeMode(projectDefaultCreativeMode);
    setBatchSelectedStoryFocus(projectDefaultStoryFocus);
    setBatchSelectedPlotStage(projectDefaultPlotStage);
    setBatchStoryCreationBriefDraft(projectDefaultStoryCreationBrief);
    setBatchStoryBeatPlannerDraft({ ...EMPTY_STORY_BEAT_PLANNER_DRAFT });
    setBatchStorySceneOutlineDraft({ ...EMPTY_STORY_SCENE_OUTLINE_DRAFT });
  }, [
    projectDefaultCreativeMode,
    projectDefaultPlotStage,
    projectDefaultStoryCreationBrief,
    projectDefaultStoryFocus,
  ]);

  const applyInferredSinglePlotStage = useCallback(() => {
    const inferredStage = inferCreationPlotStage({
      chapterNumber: currentEditingChapter?.chapter_number,
      totalChapters: knownStructureChapterCount,
      presetId: activeSingleCreationPreset?.id,
      storyFocus: selectedStoryFocus,
      metrics: chapterQualityMetrics,
    });
    setSelectedPlotStage(inferredStage);
  }, [activeSingleCreationPreset?.id, chapterQualityMetrics, currentEditingChapter?.chapter_number, knownStructureChapterCount, selectedStoryFocus]);

  const applyInferredBatchPlotStage = useCallback(() => {
    const inferredStage = inferCreationPlotStage({
      chapterNumber: batchStartChapterNumber,
      totalChapters: knownStructureChapterCount,
      presetId: activeBatchCreationPreset?.id,
      storyFocus: batchSelectedStoryFocus,
      metrics: chapterQualityMetrics,
    });
    setBatchSelectedPlotStage(inferredStage);
  }, [activeBatchCreationPreset?.id, batchSelectedStoryFocus, batchStartChapterNumber, chapterQualityMetrics, knownStructureChapterCount]);

  const singleVolumePacingPlan = useMemo(
    () => buildVolumePacingPlan(knownStructureChapterCount, {
      preferredStage: selectedPlotStage,
      currentChapterNumber: currentEditingChapter?.chapter_number,
    }),
    [currentEditingChapter?.chapter_number, knownStructureChapterCount, selectedPlotStage],
  );

  const batchVolumePacingPlan = useMemo(
    () => buildVolumePacingPlan(knownStructureChapterCount, {
      preferredStage: batchSelectedPlotStage,
      currentChapterNumber: batchStartChapterNumber,
    }),
    [batchSelectedPlotStage, batchStartChapterNumber, knownStructureChapterCount],
  );

  const singleAfterScorecard = useMemo(
    () => buildStoryAfterScorecard(chapterQualityMetrics, selectedCreativeMode, selectedStoryFocus, {
      plotStage: selectedPlotStage,
    }),
    [chapterQualityMetrics, selectedCreativeMode, selectedStoryFocus, selectedPlotStage],
  );

  const batchAfterScorecard = useMemo(
    () => buildBatchStoryAfterScorecard(batchProgress?.quality_metrics_summary ?? null, batchSelectedCreativeMode, batchSelectedStoryFocus, {
      plotStage: batchSelectedPlotStage,
    }),
    [batchProgress?.quality_metrics_summary, batchSelectedCreativeMode, batchSelectedStoryFocus, batchSelectedPlotStage],
  );

  const singleScoreDrivenRecommendationCard = useMemo(
    () => buildScoreDrivenRecommendationCard(chapterQualityMetrics, selectedCreativeMode, selectedStoryFocus, {
      plotStage: selectedPlotStage,
      chapterNumber: currentEditingChapter?.chapter_number,
      totalChapters: knownStructureChapterCount,
      activePresetId: activeSingleCreationPreset?.id,
    }),
    [
      activeSingleCreationPreset?.id,
      chapterQualityMetrics,
      currentEditingChapter?.chapter_number,
      knownStructureChapterCount,
      selectedCreativeMode,
      selectedPlotStage,
      selectedStoryFocus,
    ],
  );

  const batchScoreDrivenRecommendationCard = useMemo(
    () => buildBatchScoreDrivenRecommendationCard(batchProgress?.quality_metrics_summary ?? null, batchSelectedCreativeMode, batchSelectedStoryFocus, {
      plotStage: batchSelectedPlotStage,
      chapterNumber: batchStartChapterNumber,
      totalChapters: knownStructureChapterCount,
      activePresetId: activeBatchCreationPreset?.id,
    }),
    [
      activeBatchCreationPreset?.id,
      batchProgress?.quality_metrics_summary,
      batchSelectedCreativeMode,
      batchSelectedPlotStage,
      batchSelectedStoryFocus,
      batchStartChapterNumber,
      knownStructureChapterCount,
    ],
  );

  const singleStoryRepairTargetCard = useMemo(
    () => buildStoryRepairTargetCard(chapterQualityMetrics, selectedCreativeMode, selectedStoryFocus, {
      plotStage: selectedPlotStage,
      chapterNumber: currentEditingChapter?.chapter_number,
      totalChapters: knownStructureChapterCount,
      activePresetId: activeSingleCreationPreset?.id,
    }),
    [
      activeSingleCreationPreset?.id,
      chapterQualityMetrics,
      currentEditingChapter?.chapter_number,
      knownStructureChapterCount,
      selectedCreativeMode,
      selectedPlotStage,
      selectedStoryFocus,
    ],
  );

  const batchStoryRepairTargetCard = useMemo(
    () => buildBatchStoryRepairTargetCard(batchProgress?.quality_metrics_summary ?? null, batchSelectedCreativeMode, batchSelectedStoryFocus, {
      plotStage: batchSelectedPlotStage,
      chapterNumber: batchStartChapterNumber,
      totalChapters: knownStructureChapterCount,
      activePresetId: activeBatchCreationPreset?.id,
    }),
    [
      activeBatchCreationPreset?.id,
      batchProgress?.quality_metrics_summary,
      batchSelectedCreativeMode,
      batchSelectedPlotStage,
      batchSelectedStoryFocus,
      batchStartChapterNumber,
      knownStructureChapterCount,
    ],
  );

  const singleStoryCreationControlCard = useMemo(
    () => buildStoryCreationControlCard(chapterQualityMetrics, selectedCreativeMode, selectedStoryFocus, {
      plotStage: selectedPlotStage,
      chapterNumber: currentEditingChapter?.chapter_number,
      totalChapters: knownStructureChapterCount,
      activePresetId: activeSingleCreationPreset?.id,
    }),
    [
      activeSingleCreationPreset?.id,
      chapterQualityMetrics,
      currentEditingChapter?.chapter_number,
      knownStructureChapterCount,
      selectedCreativeMode,
      selectedPlotStage,
      selectedStoryFocus,
    ],
  );

  const batchStoryCreationControlCard = useMemo(
    () => buildBatchStoryCreationControlCard(batchProgress?.quality_metrics_summary ?? null, batchSelectedCreativeMode, batchSelectedStoryFocus, {
      plotStage: batchSelectedPlotStage,
      chapterNumber: batchStartChapterNumber,
      totalChapters: knownStructureChapterCount,
      activePresetId: activeBatchCreationPreset?.id,
    }),
    [
      activeBatchCreationPreset?.id,
      batchProgress?.quality_metrics_summary,
      batchSelectedCreativeMode,
      batchSelectedPlotStage,
      batchSelectedStoryFocus,
      batchStartChapterNumber,
      knownStructureChapterCount,
    ],
  );

  const singleSystemStoryBeatPlanner = useMemo<StoryBeatPlannerDraft>(() => ({
    openingHook: singleStoryObjectiveCard?.hook || singleStoryExecutionChecklist?.opening || '',
    chapterGoal: singleStoryObjectiveCard?.objective || singleStoryResultCard?.progress || '',
    conflictPressure: singleStoryObjectiveCard?.obstacle || singleStoryExecutionChecklist?.pressure || '',
    turningPoint: singleStoryObjectiveCard?.turn || singleStoryExecutionChecklist?.pivot || '',
    endingHook: singleStoryExecutionChecklist?.closing || singleStoryResultCard?.fallout || '',
  }), [singleStoryExecutionChecklist, singleStoryObjectiveCard, singleStoryResultCard]);

  const batchSystemStoryBeatPlanner = useMemo<StoryBeatPlannerDraft>(() => ({
    openingHook: batchStoryObjectiveCard?.hook || batchStoryExecutionChecklist?.opening || '',
    chapterGoal: batchStoryObjectiveCard?.objective || batchStoryResultCard?.progress || '',
    conflictPressure: batchStoryObjectiveCard?.obstacle || batchStoryExecutionChecklist?.pressure || '',
    turningPoint: batchStoryObjectiveCard?.turn || batchStoryExecutionChecklist?.pivot || '',
    endingHook: batchStoryExecutionChecklist?.closing || batchStoryResultCard?.fallout || '',
  }), [batchStoryExecutionChecklist, batchStoryObjectiveCard, batchStoryResultCard]);

  const singleSuggestedStorySceneOutline = useMemo<StorySceneOutlineDraft>(() => buildStorySceneOutlineSuggestion({
    beatPlanner: singleStoryBeatPlannerDraft,
    objective: singleStoryObjectiveCard,
    result: singleStoryResultCard,
    acceptance: singleStoryAcceptanceCard,
  }), [singleStoryAcceptanceCard, singleStoryBeatPlannerDraft, singleStoryObjectiveCard, singleStoryResultCard]);

  const batchSuggestedStorySceneOutline = useMemo<StorySceneOutlineDraft>(() => buildStorySceneOutlineSuggestion({
    beatPlanner: batchStoryBeatPlannerDraft,
    objective: batchStoryObjectiveCard,
    result: batchStoryResultCard,
    acceptance: batchStoryAcceptanceCard,
  }), [batchStoryAcceptanceCard, batchStoryBeatPlannerDraft, batchStoryObjectiveCard, batchStoryResultCard]);

  const singleSystemStoryCreationBrief = singleStoryCreationControlCard?.promptBrief ?? '';

  const batchSystemStoryCreationBrief = batchStoryCreationControlCard?.promptBrief ?? '';

  const singleDefaultStoryCreationBrief = singleSystemStoryCreationBrief || projectDefaultStoryCreationBrief || '';

  const batchDefaultStoryCreationBrief = batchSystemStoryCreationBrief || projectDefaultStoryCreationBrief || '';

  const normalizedSingleStoryCreationBriefDraft = singleStoryCreationBriefDraft.trim();

  const normalizedBatchStoryCreationBriefDraft = batchStoryCreationBriefDraft.trim();

  const singleStoryBeatPlannerBrief = useMemo(
    () => buildStoryBeatPlannerPrompt(singleStoryBeatPlannerDraft, 'single'),
    [singleStoryBeatPlannerDraft],
  );

  const batchStoryBeatPlannerBrief = useMemo(
    () => buildStoryBeatPlannerPrompt(batchStoryBeatPlannerDraft, 'batch'),
    [batchStoryBeatPlannerDraft],
  );

  const singleStorySceneOutlineBrief = useMemo(
    () => buildStorySceneOutlinePrompt(singleStorySceneOutlineDraft, 'single'),
    [singleStorySceneOutlineDraft],
  );

  const batchStorySceneOutlineBrief = useMemo(
    () => buildStorySceneOutlinePrompt(batchStorySceneOutlineDraft, 'batch'),
    [batchStorySceneOutlineDraft],
  );

  const singleStoryCreationBaseBrief = normalizedSingleStoryCreationBriefDraft || singleDefaultStoryCreationBrief || undefined;

  const batchStoryCreationBaseBrief = normalizedBatchStoryCreationBriefDraft || batchDefaultStoryCreationBrief || undefined;

  const resolvedSingleStoryCreationBrief = mergeStoryCreationInstructions(
    singleStoryCreationBaseBrief,
    singleStoryBeatPlannerBrief,
    singleStorySceneOutlineBrief,
  );

  const resolvedBatchStoryCreationBrief = mergeStoryCreationInstructions(
    batchStoryCreationBaseBrief,
    batchStoryBeatPlannerBrief,
    batchStorySceneOutlineBrief,
  );

  const singleStoryCreationPromptLayerLabels = useMemo(
    () => buildStoryCreationPromptLayerLabels({
      summary: singleStoryCreationBaseBrief,
      beat: singleStoryBeatPlannerBrief,
      scene: singleStorySceneOutlineBrief,
    }),
    [singleStoryBeatPlannerBrief, singleStoryCreationBaseBrief, singleStorySceneOutlineBrief],
  );

  const batchStoryCreationPromptLayerLabels = useMemo(
    () => buildStoryCreationPromptLayerLabels({
      summary: batchStoryCreationBaseBrief,
      beat: batchStoryBeatPlannerBrief,
      scene: batchStorySceneOutlineBrief,
    }),
    [batchStoryBeatPlannerBrief, batchStoryCreationBaseBrief, batchStorySceneOutlineBrief],
  );

  const singleStoryCreationPromptCharCount = resolvedSingleStoryCreationBrief?.length ?? 0;

  const batchStoryCreationPromptCharCount = resolvedBatchStoryCreationBrief?.length ?? 0;

  const isSingleStoryCreationPromptVerbose = singleStoryCreationPromptCharCount >= STORY_CREATION_PROMPT_WARN_THRESHOLD;

  const isBatchStoryCreationPromptVerbose = batchStoryCreationPromptCharCount >= STORY_CREATION_PROMPT_WARN_THRESHOLD;

  const isSingleStoryCreationBriefCustomized = Boolean(
    normalizedSingleStoryCreationBriefDraft
    && normalizedSingleStoryCreationBriefDraft !== singleDefaultStoryCreationBrief.trim(),
  );

  const isBatchStoryCreationBriefCustomized = Boolean(
    normalizedBatchStoryCreationBriefDraft
    && normalizedBatchStoryCreationBriefDraft !== batchDefaultStoryCreationBrief.trim(),
  );

  const isSingleStoryBeatPlannerCustomized = Boolean(
    !isStoryBeatPlannerDraftEmpty(singleStoryBeatPlannerDraft)
    && !areStoryBeatPlannerDraftsEqual(singleStoryBeatPlannerDraft, singleSystemStoryBeatPlanner),
  );

  const isBatchStoryBeatPlannerCustomized = Boolean(
    !isStoryBeatPlannerDraftEmpty(batchStoryBeatPlannerDraft)
    && !areStoryBeatPlannerDraftsEqual(batchStoryBeatPlannerDraft, batchSystemStoryBeatPlanner),
  );

  const isSingleStorySceneOutlineCustomized = Boolean(
    !isStorySceneOutlineDraftEmpty(singleStorySceneOutlineDraft)
    && !areStorySceneOutlineDraftsEqual(singleStorySceneOutlineDraft, singleSuggestedStorySceneOutline),
  );

  const isBatchStorySceneOutlineCustomized = Boolean(
    !isStorySceneOutlineDraftEmpty(batchStorySceneOutlineDraft)
    && !areStorySceneOutlineDraftsEqual(batchStorySceneOutlineDraft, batchSuggestedStorySceneOutline),
  );

  const isSingleStoryCreationControlCustomized = isSingleStoryCreationBriefCustomized
    || isSingleStoryBeatPlannerCustomized
    || isSingleStorySceneOutlineCustomized;

  const isBatchStoryCreationControlCustomized = isBatchStoryCreationBriefCustomized
    || isBatchStoryBeatPlannerCustomized
    || isBatchStorySceneOutlineCustomized;


  const singleStoryCreationCurrentDraft = useMemo<PersistedStoryCreationDraft>(() => ({
    creativeMode: selectedCreativeMode,
    storyFocus: selectedStoryFocus,
    plotStage: selectedPlotStage,
    narrativePerspective: temporaryNarrativePerspective,
    storyCreationBriefDraft: singleStoryCreationBriefDraft,
    beatPlannerDraft: singleStoryBeatPlannerDraft,
    sceneOutlineDraft: singleStorySceneOutlineDraft,
    isBriefCustomized: isSingleStoryCreationBriefCustomized,
    isBeatPlannerCustomized: isSingleStoryBeatPlannerCustomized,
    isSceneOutlineCustomized: isSingleStorySceneOutlineCustomized,
  }), [
    isSingleStoryBeatPlannerCustomized,
    isSingleStoryCreationBriefCustomized,
    isSingleStorySceneOutlineCustomized,
    selectedCreativeMode,
    selectedPlotStage,
    selectedStoryFocus,
    singleStoryBeatPlannerDraft,
    singleStoryCreationBriefDraft,
    singleStorySceneOutlineDraft,
    temporaryNarrativePerspective,
  ]);

  const batchStoryCreationCurrentDraft = useMemo<PersistedStoryCreationDraft>(() => ({
    creativeMode: batchSelectedCreativeMode,
    storyFocus: batchSelectedStoryFocus,
    plotStage: batchSelectedPlotStage,
    storyCreationBriefDraft: batchStoryCreationBriefDraft,
    beatPlannerDraft: batchStoryBeatPlannerDraft,
    sceneOutlineDraft: batchStorySceneOutlineDraft,
    isBriefCustomized: isBatchStoryCreationBriefCustomized,
    isBeatPlannerCustomized: isBatchStoryBeatPlannerCustomized,
    isSceneOutlineCustomized: isBatchStorySceneOutlineCustomized,
  }), [
    batchSelectedCreativeMode,
    batchSelectedPlotStage,
    batchSelectedStoryFocus,
    batchStoryBeatPlannerDraft,
    batchStoryCreationBriefDraft,
    batchStorySceneOutlineDraft,
    isBatchStoryBeatPlannerCustomized,
    isBatchStoryCreationBriefCustomized,
    isBatchStorySceneOutlineCustomized,
  ]);

  const canSaveSingleStoryCreationSnapshot = Boolean(
    singleStoryCreationDraftStorageKey
    && currentEditingChapter
    && hasMeaningfulStoryCreationDraft(singleStoryCreationCurrentDraft)
  );

  const canSaveBatchStoryCreationSnapshot = Boolean(
    batchStoryCreationDraftStorageKey
    && hasMeaningfulStoryCreationDraft(batchStoryCreationCurrentDraft)
  );


  useEffect(() => {
    if (!currentEditingChapter) {
      return;
    }

    if (!singleStoryCreationDraftStorageKey) {
      resetSingleStoryCreationCockpit(currentEditingChapter.chapter_number);
      return;
    }

    const persistedDraft = getPersistedStoryCreationDraft(singleStoryCreationDraftStorageKey);

    if (!persistedDraft) {
      resetSingleStoryCreationCockpit(currentEditingChapter.chapter_number);
      return;
    }

    singleStoryCreationAutoBriefRef.current = persistedDraft.isBriefCustomized
      ? MANUAL_STORY_CREATION_BRIEF_SENTINEL
      : persistedDraft.storyCreationBriefDraft ?? singleDefaultStoryCreationBrief;
    singleStoryBeatPlannerAutoRef.current = persistedDraft.isBeatPlannerCustomized
      ? { ...EMPTY_STORY_BEAT_PLANNER_DRAFT }
      : normalizeStoryBeatPlannerDraft(persistedDraft.beatPlannerDraft);
    singleStorySceneOutlineAutoRef.current = persistedDraft.isSceneOutlineCustomized
      ? { ...EMPTY_STORY_SCENE_OUTLINE_DRAFT }
      : normalizeStorySceneOutlineDraft(persistedDraft.sceneOutlineDraft);

    setTemporaryNarrativePerspective(persistedDraft.narrativePerspective);
    setSelectedCreativeMode(persistedDraft.creativeMode ?? projectDefaultCreativeMode);
    setSelectedStoryFocus(persistedDraft.storyFocus ?? projectDefaultStoryFocus);
    setSelectedPlotStage(
      persistedDraft.plotStage
      ?? projectDefaultPlotStage
      ?? inferCreationPlotStage({
        chapterNumber: currentEditingChapter.chapter_number,
        totalChapters: knownStructureChapterCount,
      }),
    );
    setSingleStoryCreationBriefDraft(persistedDraft.storyCreationBriefDraft ?? projectDefaultStoryCreationBrief);
    setSingleStoryBeatPlannerDraft(normalizeStoryBeatPlannerDraft(persistedDraft.beatPlannerDraft));
    setSingleStorySceneOutlineDraft(normalizeStorySceneOutlineDraft(persistedDraft.sceneOutlineDraft));
  }, [
    currentEditingChapter?.chapter_number,
    currentEditingChapter?.id,
    knownStructureChapterCount,
    projectDefaultCreativeMode,
    projectDefaultPlotStage,
    projectDefaultStoryCreationBrief,
    projectDefaultStoryFocus,
    resetSingleStoryCreationCockpit,
    singleDefaultStoryCreationBrief,
    singleStoryCreationDraftStorageKey,
  ]);

  useEffect(() => {
    if (!batchStoryCreationDraftStorageKey) {
      resetBatchStoryCreationCockpit();
      return;
    }

    const persistedDraft = getPersistedStoryCreationDraft(batchStoryCreationDraftStorageKey);

    if (!persistedDraft) {
      resetBatchStoryCreationCockpit();
      return;
    }

    batchStoryCreationAutoBriefRef.current = persistedDraft.isBriefCustomized
      ? MANUAL_STORY_CREATION_BRIEF_SENTINEL
      : persistedDraft.storyCreationBriefDraft ?? batchDefaultStoryCreationBrief;
    batchStoryBeatPlannerAutoRef.current = persistedDraft.isBeatPlannerCustomized
      ? { ...EMPTY_STORY_BEAT_PLANNER_DRAFT }
      : normalizeStoryBeatPlannerDraft(persistedDraft.beatPlannerDraft);
    batchStorySceneOutlineAutoRef.current = persistedDraft.isSceneOutlineCustomized
      ? { ...EMPTY_STORY_SCENE_OUTLINE_DRAFT }
      : normalizeStorySceneOutlineDraft(persistedDraft.sceneOutlineDraft);

    setBatchSelectedCreativeMode(persistedDraft.creativeMode ?? projectDefaultCreativeMode);
    setBatchSelectedStoryFocus(persistedDraft.storyFocus ?? projectDefaultStoryFocus);
    setBatchSelectedPlotStage(persistedDraft.plotStage ?? projectDefaultPlotStage);
    setBatchStoryCreationBriefDraft(persistedDraft.storyCreationBriefDraft ?? projectDefaultStoryCreationBrief);
    setBatchStoryBeatPlannerDraft(normalizeStoryBeatPlannerDraft(persistedDraft.beatPlannerDraft));
    setBatchStorySceneOutlineDraft(normalizeStorySceneOutlineDraft(persistedDraft.sceneOutlineDraft));
  }, [
    batchDefaultStoryCreationBrief,
    batchStoryCreationDraftStorageKey,
    projectDefaultCreativeMode,
    projectDefaultPlotStage,
    projectDefaultStoryCreationBrief,
    projectDefaultStoryFocus,
    resetBatchStoryCreationCockpit,
  ]);

  useEffect(() => {
    if (!singleStoryCreationDraftStorageKey) {
      setSingleStoryCreationSnapshots([]);
      return;
    }

    setSingleStoryCreationSnapshots(getPersistedStoryCreationSnapshots(singleStoryCreationDraftStorageKey));
  }, [singleStoryCreationDraftStorageKey]);

  useEffect(() => {
    if (!batchStoryCreationDraftStorageKey) {
      setBatchStoryCreationSnapshots([]);
      return;
    }

    setBatchStoryCreationSnapshots(getPersistedStoryCreationSnapshots(batchStoryCreationDraftStorageKey));
  }, [batchStoryCreationDraftStorageKey]);

  useEffect(() => {
    if (!singleStoryCreationDraftStorageKey || !currentEditingChapter) {
      return;
    }

    persistStoryCreationDraft(singleStoryCreationDraftStorageKey, {
      creativeMode: selectedCreativeMode,
      storyFocus: selectedStoryFocus,
      plotStage: selectedPlotStage,
      narrativePerspective: temporaryNarrativePerspective,
      storyCreationBriefDraft: singleStoryCreationBriefDraft,
      beatPlannerDraft: singleStoryBeatPlannerDraft,
      sceneOutlineDraft: singleStorySceneOutlineDraft,
      isBriefCustomized: isSingleStoryCreationBriefCustomized,
      isBeatPlannerCustomized: isSingleStoryBeatPlannerCustomized,
      isSceneOutlineCustomized: isSingleStorySceneOutlineCustomized,
      updatedAt: new Date().toISOString(),
    });
  }, [
    currentEditingChapter?.id,
    isSingleStoryBeatPlannerCustomized,
    isSingleStoryCreationBriefCustomized,
    isSingleStorySceneOutlineCustomized,
    selectedCreativeMode,
    selectedPlotStage,
    selectedStoryFocus,
    singleStoryBeatPlannerDraft,
    singleStoryCreationBriefDraft,
    singleStoryCreationDraftStorageKey,
    singleStorySceneOutlineDraft,
    temporaryNarrativePerspective,
  ]);

  useEffect(() => {
    if (!batchStoryCreationDraftStorageKey) {
      return;
    }

    persistStoryCreationDraft(batchStoryCreationDraftStorageKey, {
      creativeMode: batchSelectedCreativeMode,
      storyFocus: batchSelectedStoryFocus,
      plotStage: batchSelectedPlotStage,
      storyCreationBriefDraft: batchStoryCreationBriefDraft,
      beatPlannerDraft: batchStoryBeatPlannerDraft,
      sceneOutlineDraft: batchStorySceneOutlineDraft,
      isBriefCustomized: isBatchStoryCreationBriefCustomized,
      isBeatPlannerCustomized: isBatchStoryBeatPlannerCustomized,
      isSceneOutlineCustomized: isBatchStorySceneOutlineCustomized,
      updatedAt: new Date().toISOString(),
    });
  }, [
    batchSelectedCreativeMode,
    batchSelectedPlotStage,
    batchSelectedStoryFocus,
    batchStoryBeatPlannerDraft,
    batchStoryCreationBriefDraft,
    batchStoryCreationDraftStorageKey,
    batchStorySceneOutlineDraft,
    isBatchStoryBeatPlannerCustomized,
    isBatchStoryCreationBriefCustomized,
    isBatchStorySceneOutlineCustomized,
  ]);

  const saveSingleStoryCreationSnapshot = useCallback((
    reason: StoryCreationSnapshotReason = 'manual',
    options?: { silent?: boolean; label?: string },
  ): StoryCreationSnapshot | null => {
    if (!singleStoryCreationDraftStorageKey || !currentEditingChapter) {
      return null;
    }

    if (!hasMeaningfulStoryCreationDraft(singleStoryCreationCurrentDraft)) {
      if (!options?.silent) {
        message.warning('请先填写创作内容再保存快照');
      }
      return null;
    }

    const prompt = resolvedSingleStoryCreationBrief?.trim();
    const latestSnapshot = singleStoryCreationSnapshots[0];

    if (
      latestSnapshot
      && latestSnapshot.reason === reason
      && areStoryCreationDraftContentsEqual(latestSnapshot, singleStoryCreationCurrentDraft, { includeNarrativePerspective: true })
      && normalizeOptionalText(latestSnapshot.prompt) === normalizeOptionalText(prompt)
    ) {
      if (!options?.silent && reason === 'manual') {
        message.info('当前内容无变化，已保留上次快照');
      }
      return latestSnapshot;
    }

    const createdAt = new Date().toISOString();
    const chapterLabel = currentEditingChapter.chapter_number ? `第${currentEditingChapter.chapter_number}章` : '未编号';
    const snapshot: StoryCreationSnapshot = {
      ...singleStoryCreationCurrentDraft,
      id: buildStoryCreationSnapshotId(),
      scope: 'single',
      createdAt,
      updatedAt: createdAt,
      reason,
      label: options?.label?.trim() || `${chapterLabel} · ${reason === 'generate' ? '自动生成' : '手动保存'}`,
      prompt: prompt || undefined,
      promptLayerLabels: [...singleStoryCreationPromptLayerLabels],
      promptCharCount: prompt?.length ?? 0,
    };

    const nextSnapshots = persistStoryCreationSnapshot(singleStoryCreationDraftStorageKey, snapshot);
    setSingleStoryCreationSnapshots(nextSnapshots);

    if (!options?.silent) {
      message.success(reason === 'generate' ? '已保存生成快照' : '已保存草稿快照');
    }

    return nextSnapshots[0] ?? snapshot;
  }, [
    currentEditingChapter,
    resolvedSingleStoryCreationBrief,
    singleStoryCreationCurrentDraft,
    singleStoryCreationDraftStorageKey,
    singleStoryCreationPromptLayerLabels,
    singleStoryCreationSnapshots,
  ]);

  const saveBatchStoryCreationSnapshot = useCallback((
    reason: StoryCreationSnapshotReason = 'manual',
    options?: { silent?: boolean; label?: string },
  ): StoryCreationSnapshot | null => {
    if (!batchStoryCreationDraftStorageKey) {
      return null;
    }

    if (!hasMeaningfulStoryCreationDraft(batchStoryCreationCurrentDraft)) {
      if (!options?.silent) {
        message.warning('请先填写创作内容再保存快照');
      }
      return null;
    }

    const prompt = resolvedBatchStoryCreationBrief?.trim();
    const latestSnapshot = batchStoryCreationSnapshots[0];

    if (
      latestSnapshot
      && latestSnapshot.reason === reason
      && areStoryCreationDraftContentsEqual(latestSnapshot, batchStoryCreationCurrentDraft)
      && normalizeOptionalText(latestSnapshot.prompt) === normalizeOptionalText(prompt)
    ) {
      if (!options?.silent && reason === 'manual') {
        message.info('当前内容无变化，已保留上次快照');
      }
      return latestSnapshot;
    }

    const createdAt = new Date().toISOString();
    const snapshot: StoryCreationSnapshot = {
      ...batchStoryCreationCurrentDraft,
      id: buildStoryCreationSnapshotId(),
      scope: 'batch',
      createdAt,
      updatedAt: createdAt,
      reason,
      label: options?.label?.trim() || `批量 · ${reason === 'generate' ? '自动生成' : '手动保存'}`,
      prompt: prompt || undefined,
      promptLayerLabels: [...batchStoryCreationPromptLayerLabels],
      promptCharCount: prompt?.length ?? 0,
    };

    const nextSnapshots = persistStoryCreationSnapshot(batchStoryCreationDraftStorageKey, snapshot);
    setBatchStoryCreationSnapshots(nextSnapshots);

    if (!options?.silent) {
      message.success(reason === 'generate' ? '已保存生成快照' : '已保存草稿快照');
    }

    return nextSnapshots[0] ?? snapshot;
  }, [
    batchStoryCreationCurrentDraft,
    batchStoryCreationDraftStorageKey,
    batchStoryCreationPromptLayerLabels,
    batchStoryCreationSnapshots,
    resolvedBatchStoryCreationBrief,
  ]);

  const applySingleStoryCreationSnapshot = useCallback((snapshot: StoryCreationSnapshot) => {
    singleStoryCreationAutoBriefRef.current = snapshot.isBriefCustomized
      ? MANUAL_STORY_CREATION_BRIEF_SENTINEL
      : snapshot.storyCreationBriefDraft ?? '';
    singleStoryBeatPlannerAutoRef.current = snapshot.isBeatPlannerCustomized
      ? { ...EMPTY_STORY_BEAT_PLANNER_DRAFT }
      : normalizeStoryBeatPlannerDraft(snapshot.beatPlannerDraft);
    singleStorySceneOutlineAutoRef.current = snapshot.isSceneOutlineCustomized
      ? { ...EMPTY_STORY_SCENE_OUTLINE_DRAFT }
      : normalizeStorySceneOutlineDraft(snapshot.sceneOutlineDraft);

    setTemporaryNarrativePerspective(snapshot.narrativePerspective);
    setSelectedCreativeMode(snapshot.creativeMode);
    setSelectedStoryFocus(snapshot.storyFocus);
    setSelectedPlotStage(
      snapshot.plotStage
      ?? inferCreationPlotStage({
        chapterNumber: currentEditingChapter?.chapter_number,
        totalChapters: knownStructureChapterCount,
      }),
    );
    setSingleStoryCreationBriefDraft(snapshot.storyCreationBriefDraft ?? '');
    setSingleStoryBeatPlannerDraft(normalizeStoryBeatPlannerDraft(snapshot.beatPlannerDraft));
    setSingleStorySceneOutlineDraft(normalizeStorySceneOutlineDraft(snapshot.sceneOutlineDraft));
    message.success(`已应用快照：${snapshot.label}`);
  }, [currentEditingChapter?.chapter_number, knownStructureChapterCount]);

  const applyBatchStoryCreationSnapshot = useCallback((snapshot: StoryCreationSnapshot) => {
    batchStoryCreationAutoBriefRef.current = snapshot.isBriefCustomized
      ? MANUAL_STORY_CREATION_BRIEF_SENTINEL
      : snapshot.storyCreationBriefDraft ?? '';
    batchStoryBeatPlannerAutoRef.current = snapshot.isBeatPlannerCustomized
      ? { ...EMPTY_STORY_BEAT_PLANNER_DRAFT }
      : normalizeStoryBeatPlannerDraft(snapshot.beatPlannerDraft);
    batchStorySceneOutlineAutoRef.current = snapshot.isSceneOutlineCustomized
      ? { ...EMPTY_STORY_SCENE_OUTLINE_DRAFT }
      : normalizeStorySceneOutlineDraft(snapshot.sceneOutlineDraft);

    setBatchSelectedCreativeMode(snapshot.creativeMode);
    setBatchSelectedStoryFocus(snapshot.storyFocus);
    setBatchSelectedPlotStage(snapshot.plotStage);
    setBatchStoryCreationBriefDraft(snapshot.storyCreationBriefDraft ?? '');
    setBatchStoryBeatPlannerDraft(normalizeStoryBeatPlannerDraft(snapshot.beatPlannerDraft));
    setBatchStorySceneOutlineDraft(normalizeStorySceneOutlineDraft(snapshot.sceneOutlineDraft));
    message.success(`已加载快照：${snapshot.label}`);
  }, []);

  const deleteSingleStoryCreationSnapshot = useCallback((snapshotId: string) => {
    if (!singleStoryCreationDraftStorageKey) {
      return;
    }

    const nextSnapshots = removePersistedStoryCreationSnapshot(singleStoryCreationDraftStorageKey, snapshotId);
    setSingleStoryCreationSnapshots(nextSnapshots);
    message.success('快照已删除。');
  }, [singleStoryCreationDraftStorageKey]);

  const deleteBatchStoryCreationSnapshot = useCallback((snapshotId: string) => {
    if (!batchStoryCreationDraftStorageKey) {
      return;
    }

    const nextSnapshots = removePersistedStoryCreationSnapshot(batchStoryCreationDraftStorageKey, snapshotId);
    setBatchStoryCreationSnapshots(nextSnapshots);
    message.success('快照已删除。');
  }, [batchStoryCreationDraftStorageKey]);

  const copyStoryCreationPrompt = useCallback(async (
    content: string | undefined,
    scopeLabel: 'single' | 'batch',
  ) => {
    const normalizedContent = content?.trim();
    if (!normalizedContent) {
      message.warning(`${scopeLabel === 'single' ? '单章' : '批量'}提示词暂无可复制内容。`);

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

    message.success(`已将${scopeLabel === 'single' ? '单章' : '批量'}提示词复制到剪贴板。`);
    } catch (error) {
      console.error('Failed to copy prompt.', error);
      message.error('复制失败，请重试。');
    }
  }, []);
  useEffect(() => {
    const previousAutoBrief = singleStoryCreationAutoBriefRef.current;

    if (!singleDefaultStoryCreationBrief) {
      singleStoryCreationAutoBriefRef.current = '';
      setSingleStoryCreationBriefDraft(prev => (prev ? '' : prev));
      return;
    }

    setSingleStoryCreationBriefDraft(prev => {
      if (!prev.trim() || prev === previousAutoBrief) {
        return singleDefaultStoryCreationBrief;
      }

      return prev;
    });

    singleStoryCreationAutoBriefRef.current = singleDefaultStoryCreationBrief;
  }, [singleDefaultStoryCreationBrief]);

  useEffect(() => {
    const previousAutoBrief = batchStoryCreationAutoBriefRef.current;

    if (!batchDefaultStoryCreationBrief) {
      batchStoryCreationAutoBriefRef.current = '';
      setBatchStoryCreationBriefDraft(prev => (prev ? '' : prev));
      return;
    }

    setBatchStoryCreationBriefDraft(prev => {
      if (!prev.trim() || prev === previousAutoBrief) {
        return batchDefaultStoryCreationBrief;
      }

      return prev;
    });

    batchStoryCreationAutoBriefRef.current = batchDefaultStoryCreationBrief;
  }, [batchDefaultStoryCreationBrief]);

  useEffect(() => {
    const previousAutoPlanner = singleStoryBeatPlannerAutoRef.current;

    if (isStoryBeatPlannerDraftEmpty(singleSystemStoryBeatPlanner)) {
      singleStoryBeatPlannerAutoRef.current = EMPTY_STORY_BEAT_PLANNER_DRAFT;
      setSingleStoryBeatPlannerDraft((prev) => (isStoryBeatPlannerDraftEmpty(prev) ? prev : { ...EMPTY_STORY_BEAT_PLANNER_DRAFT }));
      return;
    }

    setSingleStoryBeatPlannerDraft((prev) => {
      if (isStoryBeatPlannerDraftEmpty(prev) || areStoryBeatPlannerDraftsEqual(prev, previousAutoPlanner)) {
        return singleSystemStoryBeatPlanner;
      }

      return prev;
    });

    singleStoryBeatPlannerAutoRef.current = singleSystemStoryBeatPlanner;
  }, [singleSystemStoryBeatPlanner]);

  useEffect(() => {
    const previousAutoPlanner = batchStoryBeatPlannerAutoRef.current;

    if (isStoryBeatPlannerDraftEmpty(batchSystemStoryBeatPlanner)) {
      batchStoryBeatPlannerAutoRef.current = EMPTY_STORY_BEAT_PLANNER_DRAFT;
      setBatchStoryBeatPlannerDraft((prev) => (isStoryBeatPlannerDraftEmpty(prev) ? prev : { ...EMPTY_STORY_BEAT_PLANNER_DRAFT }));
      return;
    }

    setBatchStoryBeatPlannerDraft((prev) => {
      if (isStoryBeatPlannerDraftEmpty(prev) || areStoryBeatPlannerDraftsEqual(prev, previousAutoPlanner)) {
        return batchSystemStoryBeatPlanner;
      }

      return prev;
    });

    batchStoryBeatPlannerAutoRef.current = batchSystemStoryBeatPlanner;
  }, [batchSystemStoryBeatPlanner]);

  useEffect(() => {
    const previousSuggestedOutline = singleStorySceneOutlineAutoRef.current;

    if (isStorySceneOutlineDraftEmpty(singleSuggestedStorySceneOutline)) {
      singleStorySceneOutlineAutoRef.current = EMPTY_STORY_SCENE_OUTLINE_DRAFT;
      setSingleStorySceneOutlineDraft((prev) => (isStorySceneOutlineDraftEmpty(prev) ? prev : { ...EMPTY_STORY_SCENE_OUTLINE_DRAFT }));
      return;
    }

    setSingleStorySceneOutlineDraft((prev) => {
      if (isStorySceneOutlineDraftEmpty(prev) || areStorySceneOutlineDraftsEqual(prev, previousSuggestedOutline)) {
        return singleSuggestedStorySceneOutline;
      }

      return prev;
    });

    singleStorySceneOutlineAutoRef.current = singleSuggestedStorySceneOutline;
  }, [singleSuggestedStorySceneOutline]);

  useEffect(() => {
    const previousSuggestedOutline = batchStorySceneOutlineAutoRef.current;

    if (isStorySceneOutlineDraftEmpty(batchSuggestedStorySceneOutline)) {
      batchStorySceneOutlineAutoRef.current = EMPTY_STORY_SCENE_OUTLINE_DRAFT;
      setBatchStorySceneOutlineDraft((prev) => (isStorySceneOutlineDraftEmpty(prev) ? prev : { ...EMPTY_STORY_SCENE_OUTLINE_DRAFT }));
      return;
    }

    setBatchStorySceneOutlineDraft((prev) => {
      if (isStorySceneOutlineDraftEmpty(prev) || areStorySceneOutlineDraftsEqual(prev, previousSuggestedOutline)) {
        return batchSuggestedStorySceneOutline;
      }

      return prev;
    });

    batchStorySceneOutlineAutoRef.current = batchSuggestedStorySceneOutline;
  }, [batchSuggestedStorySceneOutline]);

  const singleStoryRepairPayload = useMemo(
    () => buildStoryRepairPromptPayload(singleStoryRepairTargetCard),
    [singleStoryRepairTargetCard],
  );

  const batchStoryRepairPayload = useMemo(
    () => buildStoryRepairPromptPayload(batchStoryRepairTargetCard),
    [batchStoryRepairTargetCard],
  );

  const batchPollingIntervalRef = useRef<number | null>(null);

  const batchTaskMetaRef = useRef<Record<string, BatchTaskMeta>>({});



  useEffect(() => {

    const handleResize = () => {

      setIsMobile(window.innerWidth <= 768);

    };



    window.addEventListener('resize', handleResize);

    return () => window.removeEventListener('resize', handleResize);

  }, []);



  useEffect(() => {

    editingChapterIdRef.current = editingId;

  }, [editingId]);



  useEffect(() => {

    isEditorOpenRef.current = isEditorOpen;

  }, [isEditorOpen]);




  const handleTextSelection = useCallback(() => {


    if (!isEditorOpen) {

      setPartialRegenerateToolbarVisible(false);

      return;

    }



    const selection = window.getSelection();

    if (!selection || selection.rangeCount === 0) {

      setPartialRegenerateToolbarVisible(false);

      return;

    }



    const selectedText = selection.toString().trim();

    


    if (selectedText.length < 10) {

      setPartialRegenerateToolbarVisible(false);

      return;

    }




    const textArea = contentTextAreaRef.current?.resizableTextArea?.textArea;

    if (!textArea) {

      setPartialRegenerateToolbarVisible(false);

      return;

    }

    


    if (document.activeElement !== textArea) {

      setPartialRegenerateToolbarVisible(false);

      return;

    }




    const start = textArea.selectionStart;

    const end = textArea.selectionEnd;

    const textContent = textArea.value;

    const selectedInTextArea = textContent.substring(start, end);



    if (selectedInTextArea.trim().length < 10) {

      setPartialRegenerateToolbarVisible(false);

      return;

    }




    const rect = textArea.getBoundingClientRect();

    const computedStyle = window.getComputedStyle(textArea);

    const lineHeight = parseFloat(computedStyle.lineHeight) || 24;

    const paddingTop = parseFloat(computedStyle.paddingTop) || 0;

    


    const textBeforeSelection = textContent.substring(0, start);

    const startLine = textBeforeSelection.split('\n').length - 1;

    



    const scrollTop = textArea.scrollTop;

    const visualTop = (startLine * lineHeight) + paddingTop - scrollTop;

    


    const toolbarTop = rect.top + visualTop - 45;

    


    const toolbarLeft = rect.right - 180;



    setSelectedTextForRegenerate(selectedInTextArea);

    setSelectionStartPosition(start);

    setSelectionEndPosition(end);

    


    let finalTop = toolbarTop;

    if (visualTop < 0) {

      finalTop = rect.top + 10;

    } else if (visualTop > textArea.clientHeight) {

      finalTop = rect.bottom - 50;

    }

    

    setPartialRegenerateToolbarPosition({

      top: Math.max(rect.top + 10, Math.min(finalTop, rect.bottom - 50)),

      left: Math.min(Math.max(rect.left + 20, toolbarLeft), window.innerWidth - 200),

    });

    setPartialRegenerateToolbarVisible(true);

  }, [isEditorOpen]);




  const updateToolbarPosition = useCallback(() => {

    if (!partialRegenerateToolbarVisible || !selectedTextForRegenerate) return;

    

    const textArea = contentTextAreaRef.current?.resizableTextArea?.textArea;

    if (!textArea) return;

    

    const rect = textArea.getBoundingClientRect();

    const computedStyle = window.getComputedStyle(textArea);

    const lineHeight = parseFloat(computedStyle.lineHeight) || 24;

    const paddingTop = parseFloat(computedStyle.paddingTop) || 0;

    

    const textContent = textArea.value;

    const textBeforeSelection = textContent.substring(0, selectionStartPosition);

    const startLine = textBeforeSelection.split('\n').length - 1;

    

    const scrollTop = textArea.scrollTop;

    const visualTop = (startLine * lineHeight) + paddingTop - scrollTop;

    

    const toolbarTop = rect.top + visualTop - 45;


    const toolbarLeft = rect.right - 180;

    




    let finalTop = toolbarTop;

    if (visualTop < 0) {


      finalTop = rect.top + 10;

    } else if (visualTop > textArea.clientHeight) {


      finalTop = rect.bottom - 50;

    }

    

    setPartialRegenerateToolbarPosition({

      top: Math.max(rect.top + 10, Math.min(finalTop, rect.bottom - 50)),

      left: Math.min(Math.max(rect.left + 20, toolbarLeft), window.innerWidth - 200),

    });

  }, [partialRegenerateToolbarVisible, selectedTextForRegenerate, selectionStartPosition]);




  useEffect(() => {

    if (!isEditorOpen) return;



    const textArea = contentTextAreaRef.current?.resizableTextArea?.textArea;

    if (!textArea) return;



    const handleMouseUp = () => {


      setTimeout(handleTextSelection, 50);

    };



    const handleKeyUp = (e: KeyboardEvent) => {


      if (e.shiftKey && ['ArrowLeft', 'ArrowRight', 'ArrowUp', 'ArrowDown'].includes(e.key)) {

        setTimeout(handleTextSelection, 50);

      }

    };



    const handleScroll = () => {


      requestAnimationFrame(updateToolbarPosition);

    };




    textArea.addEventListener('mouseup', handleMouseUp);

    textArea.addEventListener('keyup', handleKeyUp);

    textArea.addEventListener('scroll', handleScroll);




    const modalBody = textArea.closest('.ant-modal-body');

    if (modalBody) {

      modalBody.addEventListener('scroll', handleScroll);

    }




    window.addEventListener('resize', handleScroll);



    return () => {

      textArea.removeEventListener('mouseup', handleMouseUp);

      textArea.removeEventListener('keyup', handleKeyUp);

      textArea.removeEventListener('scroll', handleScroll);

      if (modalBody) {

        modalBody.removeEventListener('scroll', handleScroll);

      }

      window.removeEventListener('resize', handleScroll);

    };

  }, [isEditorOpen, handleTextSelection, updateToolbarPosition]);



  // 闂佺粯鍔楅幊鎾诲吹椤曗偓瀹曟寮甸悽鐢告惃闂佸憡鐗曢幖顐︽偂濞嗘挸绫嶉柛顐ｆ礃椤撴椽鏌﹀Ο铏圭濞村吋鍔欏畷妤呮煥鐎ｎ剙鐒?

  useEffect(() => {

    const handleClickOutside = (e: MouseEvent) => {

      const target = e.target as HTMLElement;

      


      if (target.closest('[data-partial-regenerate-toolbar]')) {

        return;

      }

      


      if (target.tagName === 'TEXTAREA') {

        return;

      }

      


      if (target.closest('.ant-modal-content')) {

        return;

      }

      


      setPartialRegenerateToolbarVisible(false);

    };



    if (partialRegenerateToolbarVisible) {

      document.addEventListener('click', handleClickOutside);

      return () => document.removeEventListener('click', handleClickOutside);

    }

  }, [partialRegenerateToolbarVisible]);



  const {

    refreshChapters,

    updateChapter,

    deleteChapter,

    generateChapterContentStream

  } = useChapterSync();



  useEffect(() => {

    if (currentProject?.id) {

      if (chapters.length === 0) {

        refreshChapters();

      }

      loadWritingStyles();

      loadAnalysisTasks();

      checkAndRestoreBatchTask();

    }

    // eslint-disable-next-line react-hooks/exhaustive-deps

  }, [currentProject?.id]);




  useEffect(() => {

    const pollingIntervals = pollingIntervalsRef.current;

    const batchPollingInterval = batchPollingIntervalRef.current;

    return () => {

      Object.values(pollingIntervals).forEach(interval => {

        clearInterval(interval);

      });

      if (batchPollingInterval) {

        clearInterval(batchPollingInterval);

      }

    };

  }, []);





  const loadAnalysisTasks = async (chaptersToLoad?: typeof chapters) => {

    const targetChapters = chaptersToLoad || chapters;

    if (!targetChapters || targetChapters.length === 0) return;





    if (currentProject?.id && !chaptersToLoad) {

      const cachedTasks = chapterAnalysisTasksCache.get(currentProject.id);

      if (cachedTasks) {

        updateAnalysisTasksMap(cachedTasks);

        return;

      }

    }

    const taskEntries = await Promise.all(

      targetChapters

        .filter((chapter) => chapter.content && chapter.content.trim() !== '')

        .map(async (chapter) => {

          try {

            const task = await chapterApi.getChapterAnalysisStatus(chapter.id, currentProject?.id);

            return [chapter.id, task] as const;

          } catch {

            console.debug(`No analysis task for chapter ${chapter.id}`);

            return null;

          }

        })

    );



    const tasksMap: Record<string, AnalysisTask> = {};



    taskEntries.forEach((entry) => {

      if (!entry) {

        return;

      }



      const [chapterId, task] = entry;

      tasksMap[chapterId] = task;



      if (task.status === 'pending' || task.status === 'running') {

        startPollingTask(chapterId);

      }

    });



    updateAnalysisTasksMap(tasksMap);

  };




  const startPollingTask = (chapterId: string) => {


    if (pollingIntervalsRef.current[chapterId]) {

      clearInterval(pollingIntervalsRef.current[chapterId]);

    }



    const interval = window.setInterval(async () => {

      try {

        const task = await chapterApi.getChapterAnalysisStatus(chapterId, currentProject?.id);



        updateAnalysisTasksMap(prev => ({

          ...prev,

          [chapterId]: task

        }));



        // Stop polling once the task finishes.

        if (task.status === 'completed' || task.status === 'failed') {

          clearInterval(pollingIntervalsRef.current[chapterId]);

          delete pollingIntervalsRef.current[chapterId];



          if (task.status === 'completed') {

            message.success('章节分析已完成。');

          } else if (task.status === 'failed') {

            message.error(`章节分析失败：${task.error_message || '未知错误'}`);

          }

        }

      } catch (error) {

        console.error('Failed to poll analysis task.', error);

      }

    }, 2000);



    pollingIntervalsRef.current[chapterId] = interval;



    // Stop polling after 5 minutes to avoid runaway timers.

    setTimeout(() => {

      if (pollingIntervalsRef.current[chapterId]) {

        clearInterval(pollingIntervalsRef.current[chapterId]);

        delete pollingIntervalsRef.current[chapterId];

      }

    }, 300000);

  };



  const refreshChapterAnalysisTask = async (chapterId: string) => {

    const task = await chapterApi.getChapterAnalysisStatus(chapterId, currentProject?.id);

    updateAnalysisTasksMap(prev => ({

      ...prev,

      [chapterId]: task

    }));



    if (task.status === 'pending' || task.status === 'running') {

      startPollingTask(chapterId);

    }

  };



  const triggerDeferredBatchAnalysis = async (

    startChapterNumber: number,

    count: number,

    latestChapters: Chapter[]

  ) => {

    if (!currentProject?.id || count <= 0) return;



    const targetChapterNumbers = new Set(

      Array.from({ length: count }, (_, index) => startChapterNumber + index)

    );



    const candidateChapters = latestChapters.filter(ch =>

      targetChapterNumbers.has(ch.chapter_number) &&

      Boolean(ch.content && ch.content.trim() !== '')

    );



    if (candidateChapters.length === 0) return;



    let queuedCount = 0;

    let skippedCount = 0;

    let failedCount = 0;



    const ensureAnalysisTask = async (chapter: Chapter) => {

      const localTask = analysisTasksMap[chapter.id];

      if (localTask?.has_task && ['pending', 'running', 'completed'].includes(localTask.status)) {

        skippedCount += 1;

        if (localTask.status === 'pending' || localTask.status === 'running') {

          startPollingTask(chapter.id);

        }

        return;

      }



      try {

        const remoteTask = await chapterApi.getChapterAnalysisStatus(chapter.id, currentProject.id);

        if (remoteTask.has_task && ['pending', 'running', 'completed'].includes(remoteTask.status)) {

          skippedCount += 1;

          if (remoteTask.status === 'pending' || remoteTask.status === 'running') {

            startPollingTask(chapter.id);

          }

          return;

        }

      } catch {


      }



      try {

        await chapterApi.triggerChapterAnalysis(chapter.id, currentProject.id);

        queuedCount += 1;

        startPollingTask(chapter.id);

      } catch (error) {

        failedCount += 1;

        console.error(`闁荤喐鐟辩粻鎴ｃ亹閸屾粎绠?{chapter.chapter_number}缂備焦姊绘慨鎾垂鎼淬劌鍑犻柟閭﹀厵娴滃ジ鎮?`, error);

      }

    };



    const chunkSize = 3;

    for (let index = 0; index < candidateChapters.length; index += chunkSize) {

      const chunk = candidateChapters.slice(index, index + chunkSize);

      await Promise.all(chunk.map(ensureAnalysisTask));

    }



    if (queuedCount > 0) {

      message.info(`已将 ${queuedCount} 章加入分析队列。`);

    } else if (skippedCount > 0 && failedCount === 0) {

      message.info('已跳过完成分析的章节。');

    }



    if (failedCount > 0) {

      message.warning(`有 ${failedCount} 个章节分析失败。`);

    }



    await loadAnalysisTasks(latestChapters);

  };





  const loadWritingStyles = async () => {

    if (!currentProject?.id) return;



    const projectId = currentProject.id;

    const cachedStyles = writingStylesCache.get(projectId);

    if (cachedStyles) {

      setWritingStyles(cachedStyles.styles);

      setSelectedStyleId(cachedStyles.defaultStyleId);

      return;

    }



    const existingPromise = writingStylesLoadPromises.get(projectId);

    if (existingPromise) {

      await existingPromise;

      return;

    }



    const loadPromise = (async () => {

      try {

        const response = await writingStyleApi.getProjectStyles(projectId);

        setWritingStyles(response.styles);



        const defaultStyle = response.styles.find(s => s.is_default);

        setSelectedStyleId(defaultStyle?.id);

        writingStylesCache.set(projectId, {

          styles: response.styles,

          defaultStyleId: defaultStyle?.id,

        });

      } catch (error) {

        console.error('Failed to load writing styles.', error);

        message.error('加载写作风格失败。');

      }

    })();



    writingStylesLoadPromises.set(projectId, loadPromise);

    try {

      await loadPromise;

    } finally {

      writingStylesLoadPromises.delete(projectId);

    }

  };



  const loadAvailableModels = async () => {

    try {


      const settingsResponse = await fetch('/api/settings');

      if (settingsResponse.ok) {

        const settings = await settingsResponse.json();

        const { api_key, api_base_url, api_provider } = settings;



        if (hasUsableApiCredentials(api_key, api_base_url)) {

          try {

            const modelsResponse = await fetch(

              `/api/settings/models?api_key=${encodeURIComponent(api_key)}&api_base_url=${encodeURIComponent(api_base_url)}&provider=${api_provider}`

            );

            if (modelsResponse.ok) {

              const data = await modelsResponse.json();

              if (data.models && data.models.length > 0) {

                setAvailableModels(data.models);

                // Keep selected model if available.

                setSelectedModel(settings.llm_model);

                return settings.llm_model; // Preserve preferred model.

              }

            }

          } catch {

            console.log('Failed to load models list.');

          }

        }

      }

    } catch (error) {

      console.error('Failed to load model settings.', error);

    }

    return null;

  };



  // Check and restore batch task state.



  const checkAndRestoreBatchTask = async () => {

    if (!currentProject?.id) return;



    const projectId = currentProject.id;

    const existingPromise = batchTaskRestorePromises.get(projectId);

    if (existingPromise) {

      await existingPromise;

      return;

    }



    const restorePromise = (async () => {

      try {

        const data = await chapterBatchTaskApi.getActiveBatchGenerateTask(projectId);



        if (data.has_active_task && data.task) {

          const task = data.task;

          const persistedTaskMeta = getPersistedBatchTaskMeta(task.batch_id, projectId);

          if (persistedTaskMeta) {

            batchTaskMetaRef.current[task.batch_id] = persistedTaskMeta;

          }



          setBatchTaskId(task.batch_id);

          setBatchProgress({

            status: task.status,

            total: task.total,

            completed: task.completed,

            current_chapter_number: task.current_chapter_number ?? null,

            latest_quality_metrics: (task.latest_quality_metrics as {

              overall_score?: number;

              conflict_chain_hit_rate?: number;

              rule_grounding_hit_rate?: number;

              opening_hook_rate?: number;

              payoff_chain_rate?: number;

              cliffhanger_rate?: number;

            } | null | undefined) ?? undefined,

            quality_metrics_summary: (task.quality_metrics_summary as {

              avg_overall_score?: number;

              avg_conflict_chain_hit_rate?: number;

              avg_rule_grounding_hit_rate?: number;

              avg_opening_hook_rate?: number;

              avg_payoff_chain_rate?: number;

              avg_cliffhanger_rate?: number;

              chapter_count?: number;

            } | null | undefined) ?? undefined,

            quality_profile_summary: task.quality_profile_summary ?? null,

          });

          setBatchGenerating(true);

          setBatchGenerateVisible(false);

          startBatchPolling(task.batch_id);

          message.info('已恢复批量生成任务并继续运行。');

        }

      } catch (error) {

        console.error('Failed to restore batch task.', error);

      }

    })();



    batchTaskRestorePromises.set(projectId, restorePromise);

    try {

      await restorePromise;

    } finally {

      batchTaskRestorePromises.delete(projectId);

    }

  };



  const showBrowserNotification = (title: string, body: string, type: 'success' | 'error' | 'info' = 'info') => {

    // Notifications are optional; fall back to console when unsupported.

    if (!('Notification' in window)) {

      console.log('Notifications are not supported in this browser.');

      return;

    }



    // Show a notification if permission is granted.

    if (Notification.permission === 'granted') {

      // Use a small icon; success/error share the app icon.

      const icon = type === 'success' ? '/logo.svg' : type === 'error' ? '/favicon.ico' : '/logo.svg';

      

      const notification = new Notification(title, {

        body,

        icon,

        badge: '/favicon.ico',

        tag: 'batch-generation', // de-dupe notifications

        requireInteraction: false, // allow auto-dismiss

        silent: false, // 闂佸湱铏庨崢浠嬪棘娓氣偓楠炴捇骞囬杞扮驳闂?

      });




      notification.onclick = () => {

        window.focus();

        notification.close();

      };




      setTimeout(() => {

        notification.close();

      }, 5000);

    } else if (Notification.permission !== 'denied') {


      Notification.requestPermission().then(permission => {

        if (permission === 'granted') {

          showBrowserNotification(title, body, type);

        }

      });

    }

  };

  // Precompute chapter ordering, grouping and generation availability before early return.

  const {

    sortedChapters,

    groupedChapters,

    chapterGenerationStateById,

    batchStartChapterOptions,

    firstIncompleteChapter,

  } = useMemo(() => {

    const sorted = [...chapters].sort((a, b) => a.chapter_number - b.chapter_number);

    const groups: Record<string, {

      outlineId: string | null;

      outlineTitle: string;

      outlineOrder: number;

      chapters: Chapter[];

    }> = {};

    const generationStateById: Record<string, { canGenerate: boolean; disabledReason: string }> = {};

    const batchStartOptions: Chapter[] = [];

    const incompletePreviousChapterNumbers: number[] = [];

    let currentChapterNumber: number | null = null;

    let currentChapterGroup: Chapter[] = [];



    const flushChapterGroup = () => {

      currentChapterGroup.forEach(groupChapter => {

        if (!groupChapter.content || groupChapter.content.trim() === '') {

          incompletePreviousChapterNumbers.push(groupChapter.chapter_number);

        }

      });

      currentChapterGroup = [];

    };



    sorted.forEach(chapter => {

      if (currentChapterNumber !== null && chapter.chapter_number !== currentChapterNumber) {

        flushChapterGroup();

      }

      currentChapterNumber = chapter.chapter_number;



      const key = chapter.outline_id || 'uncategorized';

      if (!groups[key]) {

        groups[key] = {

          outlineId: chapter.outline_id || null,

          outlineTitle: chapter.outline_title || '未分类',

          outlineOrder: chapter.outline_order ?? 999,

          chapters: []

        };

      }

      groups[key].chapters.push(chapter);



      const disabledReason = incompletePreviousChapterNumbers.length > 0

        ? `Complete previous chapters first: ${incompletePreviousChapterNumbers.join(', ')}`

        : '';



      generationStateById[chapter.id] = {

        canGenerate: disabledReason === '',

        disabledReason,

      };



      if ((!chapter.content || chapter.content.trim() === '') && disabledReason === '') {

        batchStartOptions.push(chapter);

      }



      currentChapterGroup.push(chapter);

    });



    const grouped = Object.values(groups).sort((a, b) => a.outlineOrder - b.outlineOrder);



    return {

      sortedChapters: sorted,

      groupedChapters: grouped,

      chapterGenerationStateById: generationStateById,

      batchStartChapterOptions: batchStartOptions,

      firstIncompleteChapter: sorted.find(ch => !ch.content || ch.content.trim() === ''),

    };

  }, [chapters]);



  const canGenerateChapter = (chapter: Chapter): boolean => {

    return chapterGenerationStateById[chapter.id]?.canGenerate ?? false;

  };



  const getGenerateDisabledReason = (chapter: Chapter): string => {

    return chapterGenerationStateById[chapter.id]?.disabledReason || '';

  };



  if (!currentProject) return null;



  const getNarrativePerspectiveText = (perspective?: string): string => {

    const texts: Record<string, string> = {

      first_person: '第一人称',
      third_person: '第三人称',
      omniscient: '全知视角',
    };

    return texts[perspective || ''] || '第三人称';

  };

  const loadChapterQualityMetrics = async (chapterId: string) => {

    setChapterQualityLoading(true);

    try {

      const result = await chapterApi.getChapterQualityMetrics(chapterId);

      if (result.has_metrics && result.latest_metrics) {

        setChapterQualityMetrics(result.latest_metrics);

        setChapterQualityProfileSummary(result.quality_profile_summary ?? null);

        setChapterQualityGeneratedAt(result.generated_at);

      } else {

        setChapterQualityMetrics(null);

        setChapterQualityProfileSummary(result.quality_profile_summary ?? null);

        setChapterQualityGeneratedAt(null);

      }

    } catch (error) {

      console.error('闂佸憡姊绘慨鎯归崶鈺冨崥闁绘ê鎼灐闁荤姴娲ょ€氼剟宕规惔銏犵窞閺夊牜鍋夎:', error);

      setChapterQualityMetrics(null);

      setChapterQualityProfileSummary(null);

      setChapterQualityGeneratedAt(null);

    } finally {

      setChapterQualityLoading(false);

    }

  };



  const handleOpenModal = (id: string) => {

    const chapter = chapters.find(c => c.id === id);

    if (chapter) {

      form.setFieldsValue(chapter);

      setEditingId(id);

      setIsModalOpen(true);

    }

  };



  const handleSubmit = async (values: ChapterUpdate) => {

    if (!editingId) return;



    try {

      await updateChapter(editingId, values);




      await refreshChapters();



      message.success('章节已更新。');

      setIsModalOpen(false);

      form.resetFields();

    } catch {

      message.error('更新章节失败。');

    }

  };



  const handleOpenEditor = (id: string) => {

    const chapter = chapters.find(c => c.id === id);

    if (chapter) {

      setCurrentChapter(chapter);

      editorForm.setFieldsValue({

        title: chapter.title,

        content: chapter.content,

      });
      resetSingleStoryCreationCockpit(chapter.chapter_number);
      setEditingId(id);
      setIsEditorOpen(true);
      setChapterQualityMetrics(null);

      setChapterQualityProfileSummary(null);

      setChapterQualityGeneratedAt(null);

      // 闂佺懓鐏氶幐鍝ユ閹寸姷纾介柡宥庡墰鐢棛绱掗幇顓ф當鐟滅増鐩顔炬崉閸濆嫷娼遍柡澶屽仩婵倛鍟梺鎼炲妼椤戝懘宕归鍡樺仒?

      loadAvailableModels();


      void loadChapterQualityMetrics(chapter.id);

    }

  };



  const handleEditorSubmit = async (values: ChapterUpdate) => {

    if (!editingId || !currentProject) return;



    try {

      await updateChapter(editingId, values);




      const updatedProject = await projectApi.getProject(currentProject.id);

      setCurrentProject(updatedProject);



      message.success('项目已更新。');

      setIsEditorOpen(false);

    } catch {

      message.error('更新项目失败。');

    }

  };



  const handleGenerate = async () => {

    if (!editingId) return;

    const chapterId = editingId;

    if (runningSingleChapterTasks[chapterId]) {

      message.info('该章节正在生成中。');

      return;

    }

    const progressMessageKey = `chapter-generate-progress-${chapterId}`;



    try {

      saveSingleStoryCreationSnapshot('generate', { silent: true });

      setIsContinuing(true);

      setIsGenerating(true);

      setSingleChapterProgress(0);

      setSingleChapterProgressMessage('Generating chapter...');



      const result = await generateChapterContentStream(
        chapterId,
        undefined,
        selectedStyleId,
        targetWordCount,

        (progressMsg, progressValue) => {


          setSingleChapterProgress(progressValue);

          setSingleChapterProgressMessage(progressMsg);

        },
        selectedModel,
        temporaryNarrativePerspective,
        selectedCreativeMode,
        selectedStoryFocus,
        selectedPlotStage,
        resolvedSingleStoryCreationBrief,
        singleStoryRepairPayload?.storyRepairSummary,
        singleStoryRepairPayload?.storyRepairTargets,
        singleStoryRepairPayload?.storyPreserveStrengths,
      );


      if (result.generation_task_id) {

        setRunningSingleChapterTasks(prev => ({

          ...prev,

          [chapterId]: result.generation_task_id

        }));

      }



      message.open({

        key: progressMessageKey,

        type: 'loading',

        content: '正在继续生成章节内容，请稍候...',

        duration: 0,

      });




      result.completion

        .then(async (finalResult) => {

          if (isEditorOpenRef.current && editingChapterIdRef.current === chapterId) {

            const hasContentTouched = editorForm.isFieldsTouched(['content']);

            if (!hasContentTouched && finalResult?.content) {

              editorForm.setFieldsValue({ content: finalResult.content });

            } else if (hasContentTouched) {

              message.info('内容已被编辑，已保留你的修改。');

            }

          }



          message.open({

            key: progressMessageKey,

            type: 'success',

            content: '生成完成。',

            duration: 2,

          });



          if (finalResult?.analysis_task_id) {

            const taskId = finalResult.analysis_task_id;

            const pendingTask: AnalysisTask = {

              has_task: true,

              task_id: taskId,

              chapter_id: chapterId,

              status: 'pending',

              progress: 0

            };

            updateAnalysisTasksMap(prev => ({

              ...prev,

              [chapterId]: pendingTask

            }));

            chapterApi.upsertChapterAnalysisTaskToStore(pendingTask, currentProject?.id, 'chapter-analysis-task');

            startPollingTask(chapterId);

          }

          await loadChapterQualityMetrics(chapterId);

        })

        .catch((error) => {

          const completionError = error as ApiError;

          message.open({

            key: progressMessageKey,

            type: 'error',

            content: '章节分析失败：' + (completionError.response?.data?.detail || completionError.message || '未知错误'),

            duration: 4,

          });

        })

        .finally(() => {

          setRunningSingleChapterTasks(prev => {

            if (!(chapterId in prev)) return prev;

            const next = { ...prev };

            delete next[chapterId];

            return next;

          });

        });



      message.success('已创建章节分析任务。');

    } catch (error) {

      const apiError = error as ApiError;

      message.error('续写失败：' + (apiError.response?.data?.detail || apiError.message || '未知错误'));

    } finally {

      setIsContinuing(false);

      setIsGenerating(false);

    }

  };



  const showGenerateModal = (chapter: Chapter) => {

    const previousChapters = chapters.filter(

      c => c.chapter_number < chapter.chapter_number

    ).sort((a, b) => a.chapter_number - b.chapter_number);



    const selectedStyle = writingStyles.find(s => s.id === selectedStyleId);

    const creativeModeLabel = selectedCreativeMode
      ? (CREATIVE_MODE_OPTIONS.find((item) => item.value === selectedCreativeMode)?.label || selectedCreativeMode)
      : '未选择';
    const storyFocusLabel = selectedStoryFocus
      ? (STORY_FOCUS_OPTIONS.find((item) => item.value === selectedStoryFocus)?.label || selectedStoryFocus)
      : '未选择';
    const plotStageLabel = selectedPlotStage
      ? (CREATION_PLOT_STAGE_OPTIONS.find((item) => item.value === selectedPlotStage)?.label || selectedPlotStage)
      : '未选择';

    const instance = modal.confirm({
      title: '确认继续生成',
      width: 700,
      centered: true,
      content: (
        <div style={{ marginTop: 16 }}>
          <p>将按当前设置继续生成本章内容。</p>
          <ul>
            <li>写作风格：{selectedStyle?.name ?? '未选择'}</li>
            <li>创作模式：{creativeModeLabel}</li>
            <li>故事聚焦：{storyFocusLabel}</li>
            <li>剧情阶段：{plotStageLabel}</li>
            <li>目标字数：{targetWordCount}</li>
          </ul>
          {previousChapters.length > 0 && (
            <div
              style={{
                marginTop: 16,
                padding: 12,
                background: 'var(--color-info-bg)',
                borderRadius: 4,
                border: '1px solid var(--color-info-border)',
              }}
            >
              <div style={{ marginBottom: 8, fontWeight: 500, color: 'var(--color-primary)' }}>
                已生成的{previousChapters.length}章将作为参考：
              </div>
              <div style={{ maxHeight: 150, overflowY: 'auto' }}>
                {previousChapters.map((ch) => (
                  <div key={ch.id} style={{ padding: '4px 0', fontSize: 13 }}>
                    第{ch.chapter_number}章：{ch.title}（{ch.word_count || 0}字）
                  </div>
                ))}
              </div>
              <div style={{ marginTop: 8, fontSize: 12, color: '#666' }}>
                继续操作将覆盖当前章节内容。
              </div>
            </div>
          )}
          <p style={{ color: '#ff4d4f', marginTop: 16, marginBottom: 0 }}>
            请先确认重要内容已经保存，再继续操作。
          </p>
        </div>
      ),
      okText: '继续生成',
      okButtonProps: { danger: true },
      cancelText: '取消',
      onOk: async () => {

        instance.update({

          okButtonProps: { danger: true, loading: true },

          cancelButtonProps: { disabled: true },

          closable: false,

          maskClosable: false,

          keyboard: false,

        });



        try {

          if (!selectedStyleId) {

      message.error('请先选择写作风格。');

            instance.update({

              okButtonProps: { danger: true, loading: false },

              cancelButtonProps: { disabled: false },

              closable: true,

              maskClosable: true,

              keyboard: true,

            });

            return;

          }

          await handleGenerate();

          instance.destroy();

        } catch {

          instance.update({

            okButtonProps: { danger: true, loading: false },

            cancelButtonProps: { disabled: false },

            closable: true,

            maskClosable: true,

            keyboard: true,

          });

        }

      },

    });

  };



  const getStatusColor = (status: string) => {

    const colors: Record<string, string> = {

      'draft': 'default',

      'writing': 'processing',

      'completed': 'success',

    };

    return colors[status] || 'default';

  };



  const getStatusText = (status: string) => {

    const texts: Record<string, string> = {

      'draft': '草稿',

      'writing': '创作中',

      'completed': '已完成',

    };

    return texts[status] || status;

  };



  const handleExport = () => {

    if (chapters.length === 0) {

      message.warning('暂无可导出的章节。');

      return;

    }



    modal.confirm({

      title: '导出项目',

      content: `确认导出项目“${currentProject.title}”吗？`,

      centered: true,

      okText: '导出',

      cancelText: '取消',

      onOk: () => {

        try {

          projectApi.exportProject(currentProject.id);

          message.success('已开始导出。');

        } catch {

          message.error('导出失败。');

        }

      },

    });

  };



  const handleShowAnalysis = (chapterId: string) => {

    setAnalysisChapterId(chapterId);

    setAnalysisVisible(true);

  };




  const handleBatchGenerate = async (values: {
    startChapterNumber: number;
    count: number;
    enableAnalysis: boolean;
    styleId?: number;
    targetWordCount?: number;
    model?: string;
    creativeMode?: CreativeMode;
    storyFocus?: StoryFocus;
    plotStage?: PlotStage;
  }) => {
    if (!currentProject?.id) return;




    console.log('[闂佸綊娼х紞濠囧闯濞差亝鍋ㄩ柣鏃傤焾閻忓槼 闁荤偞绋忛崝灞界暦閻掋倹lues:', values);

    console.log('[闂佸綊娼х紞濠囧闯濞差亝鍋ㄩ柣鏃傤焾閻忓槼 batchSelectedModel闂佺粯顭堥崺鏍焵?', batchSelectedModel);




    const styleId = values.styleId || selectedStyleId;

    const wordCount = values.targetWordCount || targetWordCount;



    const model = batchSelectedModel;
    const creativeMode = batchSelectedCreativeMode;
    const storyFocus = batchSelectedStoryFocus;
    const plotStage = batchSelectedPlotStage;


    console.log('[闂佸綊娼х紞濠囧闯濞差亝鍋ㄩ柣鏃傤焾閻忓槼 闂佸搫鐗冮崑鎾剁磽娴ｅ摜澧斿┑鐐叉喘閹粙濡歌閻ｇ湌odel:', model);



    if (!styleId) {

      message.error('请先选择写作风格。');

      return;

    }



    try {

      saveBatchStoryCreationSnapshot('generate', { silent: true });

      setBatchGenerating(true);

      setBatchGenerateVisible(false); // 闂佺绻戞繛濠偽涢幘顔界厐鐎广儱娲ㄩ弸鍌炴倵閻㈡鏀伴柣锕佹椤╁ジ宕遍鐘殿槷闂備緡鍓欓悘婵嬪储閵堝鐒兼い鏃囨閻繝寮堕埡鍌溾槈閻庣懓鍟块锝夊捶椤撶姴鐐?



      const requestBody: {
        start_chapter_number: number;
        count: number;
        enable_analysis: boolean;
        style_id: number;
        target_word_count: number;
        model?: string;
        creative_mode?: CreativeMode;
        story_focus?: StoryFocus;
        plot_stage?: PlotStage;
        story_creation_brief?: string;
        story_repair_summary?: string;
        story_repair_targets?: string[];
        story_preserve_strengths?: string[];
      } = {
        start_chapter_number: values.startChapterNumber,
        count: values.count,
        enable_analysis: false,
        style_id: styleId,
        target_word_count: wordCount,
      };



      if (model) {

        requestBody.model = model;

        console.log('[batch] model selected:', model);

      } else {
        console.log('[batch] no model selected, using default');
      }

      if (creativeMode) {
        requestBody.creative_mode = creativeMode;
      }

      if (storyFocus) {
        requestBody.story_focus = storyFocus;
      }

      if (plotStage) {
        requestBody.plot_stage = plotStage;
      }

      if (resolvedBatchStoryCreationBrief) {
        requestBody.story_creation_brief = resolvedBatchStoryCreationBrief;
      }

      if (batchStoryRepairPayload?.storyRepairSummary) {
        requestBody.story_repair_summary = batchStoryRepairPayload.storyRepairSummary;
      }

      if (batchStoryRepairPayload?.storyRepairTargets?.length) {
        requestBody.story_repair_targets = batchStoryRepairPayload.storyRepairTargets;
      }

      if (batchStoryRepairPayload?.storyPreserveStrengths?.length) {
        requestBody.story_preserve_strengths = batchStoryRepairPayload.storyPreserveStrengths;
      }


      console.log('[闂佸綊娼х紞濠囧闯濞差亝鍋ㄩ柣鏃傤焾閻忓槼 闁诲海鎳撻張顒勫汲閿濆洦瀚氶梺鍨儑濠€鏉懨?', JSON.stringify(requestBody, null, 2));



      const result = await chapterBatchTaskApi.createBatchGenerateTask(currentProject.id, requestBody);

      setBatchTaskId(result.batch_id);

      batchTaskMetaRef.current[result.batch_id] = {

        startChapterNumber: values.startChapterNumber,

        count: values.count,

        autoAnalyze: values.enableAnalysis,

        projectId: currentProject.id,

      };

      persistBatchTaskMeta(result.batch_id, batchTaskMetaRef.current[result.batch_id]);

      setBatchProgress({

        status: 'running',

        total: result.chapters_to_generate.length,

        completed: 0,

        current_chapter_number: values.startChapterNumber,

        estimated_time_minutes: result.estimated_time_minutes,

        latest_quality_metrics: undefined,

        quality_metrics_summary: undefined,

        quality_profile_summary: null,

      });



      message.success(`已开始批量生成，预计耗时 ${result.estimated_time_minutes} 分钟。`);




      showBrowserNotification(

        '批量生成已开始',

        `计划生成 ${result.chapters_to_generate.length} 章，预计耗时 ${result.estimated_time_minutes} 分钟。`,

        'info'

      );



      // 閻庢鍠掗崑鎾斥攽椤旂⒈鍎撻柣銈呮閹风娀锝為鐔峰簥闂佸憡妫戠槐鏇熸叏閹间礁绠?

      startBatchPolling(result.batch_id);



    } catch (error: unknown) {

      const err = error as Error;

      message.error('批量生成失败：' + (err.message || '未知错误'));

      setBatchGenerating(false);

      setBatchGenerateVisible(false);

    }

  };




  const startBatchPolling = (taskId: string) => {

    if (batchPollingIntervalRef.current) {

      clearInterval(batchPollingIntervalRef.current);

    }



    const poll = async () => {

      try {

        const status = await chapterBatchTaskApi.getBatchGenerateStatus(taskId, currentProject?.id);

        setBatchProgress({

          status: status.status,

          total: status.total,

          completed: status.completed,

          current_chapter_number: status.current_chapter_number ?? null,

          latest_quality_metrics: (status.latest_quality_metrics as {

            overall_score?: number;

            conflict_chain_hit_rate?: number;

            rule_grounding_hit_rate?: number;

            opening_hook_rate?: number;

            payoff_chain_rate?: number;

            cliffhanger_rate?: number;

          } | null | undefined) ?? undefined,

          quality_metrics_summary: (status.quality_metrics_summary as {

            avg_overall_score?: number;

            avg_conflict_chain_hit_rate?: number;

            avg_rule_grounding_hit_rate?: number;

            avg_opening_hook_rate?: number;

            avg_payoff_chain_rate?: number;

            avg_cliffhanger_rate?: number;

            chapter_count?: number;

          } | null | undefined) ?? undefined,

          quality_profile_summary: status.quality_profile_summary ?? null,

        });





        if (status.completed > 0) {

          const latestChapters = await refreshChapters();

          await loadAnalysisTasks(latestChapters);




          if (currentProject?.id) {

            const updatedProject = await projectApi.getProject(currentProject.id);

            setCurrentProject(updatedProject);

          }

        }




        if (status.status === 'completed' || status.status === 'failed' || status.status === 'cancelled') {

          if (batchPollingIntervalRef.current) {

            clearInterval(batchPollingIntervalRef.current);

            batchPollingIntervalRef.current = null;

          }



          setBatchGenerating(false);

          const taskMeta = batchTaskMetaRef.current[taskId] ?? getPersistedBatchTaskMeta(taskId, currentProject?.id);





          const finalChapters = await refreshChapters();

          await loadAnalysisTasks(finalChapters);




          if (currentProject?.id) {

            const updatedProject = await projectApi.getProject(currentProject.id);

            setCurrentProject(updatedProject);

          }



          if (status.status === 'completed') {

            message.success(`批量生成完成，共生成 ${status.completed} 章。`);


            showBrowserNotification(

              '批量生成已完成',

              `项目“${currentProject?.title || '未命名项目'}”已完成 ${status.completed} 章生成。`,

              'success'

            );



            if (taskMeta?.autoAnalyze) {

              void triggerDeferredBatchAnalysis(taskMeta.startChapterNumber, taskMeta.count, finalChapters);

            }

          } else if (status.status === 'failed') {

            message.error(`批量生成失败：${status.error_message || '未知错误'}`);


            showBrowserNotification(

              '批量生成失败',

              status.error_message || '未知错误',

              'error'

            );

          } else if (status.status === 'cancelled') {

            message.warning('批量生成已取消。');

          }



          delete batchTaskMetaRef.current[taskId];

          removePersistedBatchTaskMeta(taskId);




          setTimeout(() => {

            setBatchGenerateVisible(false);

            setBatchTaskId(null);

            setBatchProgress(null);

          }, 2000);

        }

      } catch (error) {

        console.error('闁哄鍎愰崰娑㈩敋濡ゅ懎绠ョ憸鐗堝笒濞呫倝鏌ｉ姀銏犳瀾闁搞劍宀搁幃鈺呮嚋绾版ê浜惧ù锝嗘偠娴滃ジ鎮?', error);

      }

    };




    poll();




    batchPollingIntervalRef.current = window.setInterval(poll, 2000);

  };




  const handleCancelBatchGenerate = async () => {

    if (!batchTaskId) return;



    try {

      await chapterBatchTaskApi.cancelBatchGenerateTask(batchTaskId, currentProject?.id);

      delete batchTaskMetaRef.current[batchTaskId];

      removePersistedBatchTaskMeta(batchTaskId);



      message.success('批量生成已取消。');




      await refreshChapters();

      await loadAnalysisTasks();




      if (currentProject?.id) {

        const updatedProject = await projectApi.getProject(currentProject.id);

        setCurrentProject(updatedProject);

      }

    } catch (error: unknown) {

      const err = error as Error;

      message.error('取消批量生成失败：' + (err.message || '未知错误'));

    }

  };




  const handleOpenBatchGenerate = async () => {

    if (batchGenerating) {

      message.info('批量生成进行中，请等待当前任务完成。');

      return;

    }





    if (!firstIncompleteChapter) {

      message.info('没有可生成的未完成章节。');

      return;

    }




    if (!canGenerateChapter(firstIncompleteChapter)) {

      const reason = getGenerateDisabledReason(firstIncompleteChapter);

      message.warning(reason);

      return;

    }




    const defaultModel = await loadAvailableModels();



    console.log('[闂佺懓鐏氶幐鍝ユ閹达箑绠ョ憸鐗堝笒濞呫倝鏌ｉ姀銏犳瀾闁搞劍鐡?defaultModel:', defaultModel);

    console.log('[闂佺懓鐏氶幐鍝ユ閹达箑绠ョ憸鐗堝笒濞呫倝鏌ｉ姀銏犳瀾闁搞劍鐡?selectedStyleId:', selectedStyleId);



    resetBatchStoryCreationCockpit();
    setBatchSelectedModel(defaultModel || undefined);
    setBatchSelectedPlotStage(projectDefaultPlotStage ?? inferCreationPlotStage({
      chapterNumber: firstIncompleteChapter.chapter_number,
      totalChapters: knownStructureChapterCount,
    }));



    batchForm.setFieldsValue({

      startChapterNumber: firstIncompleteChapter.chapter_number,

      count: 5,

      enableAnalysis: true,

      styleId: selectedStyleId,

      targetWordCount: getCachedWordCount(),

    });



    setBatchGenerateVisible(true);

  };




  const showManualCreateChapterModal = () => {


    const nextChapterNumber = chapters.length > 0

      ? Math.max(...chapters.map(c => c.chapter_number)) + 1

      : 1;



    modal.confirm({

      title: '手动创建章节',

      width: 600,

      centered: true,

      content: (

        <Form

          form={manualCreateForm}

          layout="vertical"

          initialValues={{

            chapter_number: nextChapterNumber,

            status: 'draft'

          }}

          style={{ marginTop: 16 }}

        >

          <Form.Item

            label="章节编号"

            name="chapter_number"

            rules={[{ required: true, message: '请输入章节编号' }]}

            tooltip="用于章节排序"

          >

            <InputNumber min={1} style={{ width: '100%' }} placeholder="请输入章节编号" />

          </Form.Item>



          <Form.Item

            label="章节标题"

            name="title"

            rules={[{ required: true, message: '请输入章节标题' }]}

          >

            <Input placeholder="请输入章节标题" />



          </Form.Item>



          <Form.Item

            label="大纲"

            name="outline_id"

            rules={[{ required: true, message: '请选择大纲' }]}

            tooltip="每章必须归属到某个大纲"

          >

            <Select placeholder="请选择大纲">


              {[...outlines]

                .sort((a, b) => a.order_index - b.order_index)

                .map(outline => (

                  <Select.Option key={outline.id} value={outline.id}>

                    {`#${outline.order_index} ${outline.title}`}

                  </Select.Option>

                ))}

            </Select>

          </Form.Item>



          <Form.Item

            label="梗概"

            name="summary"

            tooltip="章节的简要梗概。"

          >

            <TextArea

              rows={4}


              placeholder="请输入简要梗概"



            />






          </Form.Item>



          <Form.Item

            label="状态"

            name="status"

          >

            <Select>

              <Select.Option value="draft">草稿</Select.Option>

              <Select.Option value="writing">写作中</Select.Option>

              <Select.Option value="completed">已完成</Select.Option>

            </Select>

          </Form.Item>

        </Form>

      ),

      okText: '创建章节',

      cancelText: '取消',

      onOk: async () => {

        const values = await manualCreateForm.validateFields();




        const conflictChapter = chapters.find(

          ch => ch.chapter_number === values.chapter_number

        );



        if (conflictChapter) {


          modal.confirm({

            title: '章节编号冲突',

            icon: <InfoCircleOutlined style={{ color: '#ff4d4f' }} />,

            width: 500,

            centered: true,

            content: (

              <div>

                <p style={{ marginBottom: 12 }}>

                  章节编号 <strong>{values.chapter_number}</strong> 已被现有章节占用。

                </p>

                <div style={{

                  padding: 12,

                  background: '#fff7e6',

                  borderRadius: 4,

                  border: '1px solid #ffd591',

                  marginBottom: 12

                }}>

                  <div><strong>章节标题：</strong>{conflictChapter.title}</div>

                  <div><strong>当前状态：</strong>{getStatusText(conflictChapter.status)}</div>

                  <div><strong>当前字数：</strong>{conflictChapter.word_count || 0} 字</div>

                  {conflictChapter.outline_title && (

                    <div><strong>关联大纲：</strong>{conflictChapter.outline_title}</div>

                  )}

                </div>

                <p style={{ color: '#ff4d4f', marginBottom: 8 }}>

                  如果继续创建，系统会先删除当前章节，再使用该编号创建新章节。

                </p>

                <p style={{ fontSize: 12, color: '#666', marginBottom: 0 }}>

                  此操作不可撤销，请确认原章节内容已经不再需要。

                </p>

              </div>

            ),

            okText: '删除原章节并继续',

            okButtonProps: { danger: true },

            cancelText: '取消',

            onOk: async () => {

              try {


                await handleDeleteChapter(conflictChapter.id);




                await new Promise(resolve => setTimeout(resolve, 300));



                // 闂佸憡甯楃粙鎴犵磽閹捐妫橀柟娈垮枤瑜板潡鏌?

                await chapterApi.createChapter({

                  project_id: currentProject.id,

                  ...values

                });



                message.success('章节创建成功。');

                await refreshChapters();




                const updatedProject = await projectApi.getProject(currentProject.id);

                setCurrentProject(updatedProject);



                manualCreateForm.resetFields();

              } catch (error: unknown) {

                const err = error as Error;

          message.error('创建章节失败：' + (err.message || '未知错误'));

                throw error;

              }

            }

          });




          return Promise.reject();

        }




        try {

          await chapterApi.createChapter({

            project_id: currentProject.id,

            ...values

          });

          message.success('章节创建成功。');

          await refreshChapters();




          const updatedProject = await projectApi.getProject(currentProject.id);

          setCurrentProject(updatedProject);



          manualCreateForm.resetFields();

        } catch (error: unknown) {

          const err = error as Error;

        message.error('创建章节失败：' + (err.message || '未知错误'));

          throw error;

        }

      }

    });

  };




  const renderAnalysisStatus = (chapterId: string) => {

    const task = analysisTasksMap[chapterId];



    if (!task) {

      return null;

    }



    switch (task.status) {

      case 'pending':

        return (

          <Tag icon={<SyncOutlined spin />} color="processing">

            等待中

          </Tag>

        );

      case 'running': {


        const isRetrying = task.error_code === 'retrying';

        return (

          <Tag

            icon={<SyncOutlined spin />}

            color={isRetrying ? "warning" : "processing"}

            title={task.error_message || undefined}

          >

            {isRetrying ? `重试中 ${task.progress}%` : `分析中 ${task.progress}%`}

          </Tag>

        );

      }

      case 'completed':

        return (

          <Tag icon={<CheckCircleOutlined />} color="success">

            已完成

          </Tag>

        );

      case 'failed':

        return (

          <Tag icon={<CloseCircleOutlined />} color="error" title={task.error_message || undefined}>

            失败

          </Tag>

        );

      default:

        return null;

    }

  };




  const showExpansionPlanModal = (chapter: Chapter) => {

    if (!chapter.expansion_plan) return;



    try {

      const planData: ExpansionPlanData = JSON.parse(chapter.expansion_plan);



      modal.info({

        title: (

          <Space style={{ flexWrap: 'wrap' }}>

            <InfoCircleOutlined style={{ color: 'var(--color-primary)' }} />

            <span style={{ wordBreak: 'break-word' }}>第 {chapter.chapter_number} 章扩写计划</span>

          </Space>

        ),

        width: isMobile ? 'calc(100vw - 32px)' : 800,

        centered: true,

        style: isMobile ? {

          maxWidth: 'calc(100vw - 32px)',

          margin: '0 auto',

          padding: '0 16px'

        } : undefined,

        styles: {

          body: {

            maxHeight: isMobile ? 'calc(100vh - 200px)' : 'calc(80vh - 110px)',

            overflowY: 'auto'

          }

        },

        content: (

          <div style={{ marginTop: 16 }}>

            <Descriptions

              column={1}

              size="small"

              bordered

              labelStyle={{

                whiteSpace: 'normal',

                wordBreak: 'break-word',

                width: isMobile ? '80px' : '100px'

              }}

              contentStyle={{

                whiteSpace: 'normal',

                wordBreak: 'break-word',

                overflowWrap: 'break-word'

              }}

            >

              <Descriptions.Item label="章节标题">

                <strong style={{

                  wordBreak: 'break-word',

                  whiteSpace: 'normal',

                  overflowWrap: 'break-word'

                }}>

                  {chapter.title}

                </strong>

              </Descriptions.Item>

              <Descriptions.Item label="情感基调">

                <Tag

                  color="blue"

                  style={{

                    whiteSpace: 'normal',

                    wordBreak: 'break-word',

                    height: 'auto',

                    lineHeight: '1.5',

                    padding: '4px 8px'

                  }}

                >

                  {planData.emotional_tone}

                </Tag>

              </Descriptions.Item>

              <Descriptions.Item label="冲突类型">

                <Tag

                  color="orange"

                  style={{

                    whiteSpace: 'normal',

                    wordBreak: 'break-word',

                    height: 'auto',

                    lineHeight: '1.5',

                    padding: '4px 8px'

                  }}

                >

                  {planData.conflict_type}

                </Tag>

              </Descriptions.Item>

              <Descriptions.Item label="预计字数">

                <Tag color="green">{planData.estimated_words} 字</Tag>

              </Descriptions.Item>

              <Descriptions.Item label="叙事目标">

                <span style={{

                  wordBreak: 'break-word',

                  whiteSpace: 'normal',

                  overflowWrap: 'break-word'

                }}>

                  {planData.narrative_goal}

                </span>

              </Descriptions.Item>

              <Descriptions.Item label="关键事件">

                <Space direction="vertical" size="small" style={{ width: '100%' }}>

                  {planData.key_events.map((event, idx) => (

                    <div

                      key={idx}

                      style={{

                        padding: '4px 0',

                        wordBreak: 'break-word',

                        whiteSpace: 'normal',

                        overflowWrap: 'break-word'

                      }}

                    >

                      <Tag color="purple" style={{ flexShrink: 0 }}>{idx + 1}</Tag>{' '}

                      <span style={{

                        wordBreak: 'break-word',

                        whiteSpace: 'normal',

                        overflowWrap: 'break-word'

                      }}>

                        {event}

                      </span>

                    </div>

                  ))}

                </Space>

              </Descriptions.Item>

              <Descriptions.Item label="关注角色">

                <Space wrap style={{ maxWidth: '100%' }}>

                  {planData.character_focus.map((char, idx) => (

                    <Tag

                      key={idx}

                      color="cyan"

                      style={{

                        whiteSpace: 'normal',

                        wordBreak: 'break-word',

                        height: 'auto',

                        lineHeight: '1.5'

                      }}

                    >

                      {char}

                    </Tag>

                  ))}

                </Space>

              </Descriptions.Item>

              {planData.scenes && planData.scenes.length > 0 && (

                <Descriptions.Item label="场景列表">

                  <Space direction="vertical" size="small" style={{ width: '100%' }}>

                    {planData.scenes.map((scene, idx) => (

                      <Card

                        key={idx}

                        size="small"

                        style={{

                          backgroundColor: '#fafafa',

                          maxWidth: '100%',

                          overflow: 'hidden'

                        }}

                      >

                        <div style={{

                          marginBottom: 4,

                          wordBreak: 'break-word',

                          whiteSpace: 'normal',

                          overflowWrap: 'break-word'

                        }}>

                          <strong>场景地点：</strong>

                          <span style={{

                            wordBreak: 'break-word',

                            whiteSpace: 'normal',

                            overflowWrap: 'break-word'

                          }}>

                            {scene.location}

                          </span>

                        </div>

                        <div style={{ marginBottom: 4 }}>

                          <strong>涉及角色：</strong>

                          <Space

                            size="small"

                            wrap

                            style={{

                              marginLeft: isMobile ? 0 : 8,

                              marginTop: isMobile ? 4 : 0,

                              display: isMobile ? 'flex' : 'inline-flex'

                            }}

                          >

                            {scene.characters.map((char, charIdx) => (

                              <Tag

                                key={charIdx}

                                style={{

                                  whiteSpace: 'normal',

                                  wordBreak: 'break-word',

                                  height: 'auto'

                                }}

                              >

                                {char}

                              </Tag>

                            ))}

                          </Space>

                        </div>

                        <div style={{

                          wordBreak: 'break-word',

                          whiteSpace: 'normal',

                          overflowWrap: 'break-word'

                        }}>

                          <strong>场景目的：</strong>

                          <span style={{

                            wordBreak: 'break-word',

                            whiteSpace: 'normal',

                            overflowWrap: 'break-word'

                          }}>

                            {scene.purpose}

                          </span>

                        </div>

                      </Card>

                    ))}

                  </Space>

                </Descriptions.Item>

              )}

            </Descriptions>

            <Alert

              message="说明"

              description="扩写计划用于辅助章节创作，建议结合实际写作需要进一步调整场景、冲突与角色目标。"

              type="info"

              showIcon

              style={{ marginTop: 16 }}

            />

          </div>

        ),

        okText: '知道了',

      });

    } catch (error) {

      console.error('Failed to load expansion plan:', error);

      message.error('加载扩写计划失败。');

    }

  };




  const handleDeleteChapter = async (chapterId: string) => {

    try {

      await deleteChapter(chapterId);




      await refreshChapters();




      if (currentProject) {

        const updatedProject = await projectApi.getProject(currentProject.id);

        setCurrentProject(updatedProject);

      }



      message.success('章节已删除。');

    } catch (error: unknown) {

      const err = error as Error;

      message.error('删除章节失败：' + (err.message || '未知错误'));

    }

  };




  const handleOpenPlanEditor = (chapter: Chapter) => {


    setEditingPlanChapter(chapter);

    setPlanEditorVisible(true);

  };




  const handleSavePlan = async (planData: ExpansionPlanData) => {

    if (!editingPlanChapter) return;



    try {

      const response = await fetch(`/api/chapters/${editingPlanChapter.id}/expansion-plan`, {

        method: 'PUT',

        headers: {

          'Content-Type': 'application/json',

        },

        body: JSON.stringify(planData),

      });



      if (!response.ok) {

        const error = await response.json();

        throw new Error(error.detail || 'Save plan failed.');

      }




      await refreshChapters();



      message.success('章节规划已保存。');




      setPlanEditorVisible(false);

      setEditingPlanChapter(null);

    } catch (error: unknown) {

      const err = error as Error;

      message.error('保存章节规划失败：' + (err.message || '未知错误'));

      throw error;

    }

  };



  const handleChapterSelect = (chapterId: string) => {

    const element = document.getElementById(`chapter-item-${chapterId}`);

    if (element) {

      element.scrollIntoView({ behavior: 'smooth', block: 'center' });

      // Optional: add a visual highlight effect

      element.style.transition = 'background-color 0.5s ease';

      element.style.backgroundColor = '#e6f7ff';

      setTimeout(() => {

        element.style.backgroundColor = '';

      }, 1500);

    }

  };



  // 闂佺懓鐏氶幐鍝ユ閹达附鈷撻柛娑㈠亰閸ゃ垽鏌?

  const handleOpenReader = (chapter: Chapter) => {

    setReadingChapter(chapter);

    setReaderVisible(true);

  };




  const handleReaderChapterChange = async (chapterId: string) => {

    try {

      const response = await fetch(`/api/chapters/${chapterId}`);

      if (!response.ok) throw new Error('Failed to load chapter.');

      const newChapter = await response.json();

      setReadingChapter(newChapter);

    } catch {

      message.error('加载章节失败。');

    }

  };



  // 闂佺懓鐏氶幐鍝ユ閹寸姳娌柍褜鍓熼弻鍫ュΩ閳轰焦顏熼梺鍛婂姈閻熴儵鎳樻繝鍕幓?

  const handleOpenPartialRegenerate = () => {

    setPartialRegenerateToolbarVisible(false);

    setPartialRegenerateModalVisible(true);

  };




  const handleApplyPartialRegenerate = (newText: string, startPos: number, endPos: number) => {

    // 闂佸吋鍎抽崲鑼躲亹閸ヮ亗浜归柟鎯у暱椤ゅ懘鏌涢幇顒佸櫣妞?

    const currentContent = editorForm.getFieldValue('content') || '';

    


    const newContent = currentContent.substring(0, startPos) + newText + currentContent.substring(endPos);

    


    editorForm.setFieldsValue({ content: newContent });

    


    setPartialRegenerateModalVisible(false);

    

      message.success('局部重生成结果已应用。');

  };



  return (
    <>
    <div style={{ display: 'flex', flexDirection: 'column', height: '100%' }}>

      {contextHolder}

      <div style={{

        position: 'sticky',

        top: 0,

        zIndex: 10,

        backgroundColor: 'var(--color-bg-container)',

        padding: isMobile ? '12px 0' : '16px 0',

        marginBottom: isMobile ? 12 : 16,

        borderBottom: '1px solid #f0f0f0',

        display: 'flex',

        flexDirection: isMobile ? 'column' : 'row',

        gap: isMobile ? 12 : 0,

        justifyContent: 'space-between',

        alignItems: isMobile ? 'stretch' : 'center'

      }}>

        <h2 style={{ margin: 0, fontSize: isMobile ? 18 : 24 }}>

          <BookOutlined style={{ marginRight: 8 }} />

          章节

        </h2>

        <Space direction={isMobile ? 'vertical' : 'horizontal'} style={{ width: isMobile ? '100%' : 'auto' }}>

          {currentProject.outline_mode === 'one-to-many' && (

            <Button

              icon={<PlusOutlined />}

              onClick={showManualCreateChapterModal}

              block={isMobile}

              size={isMobile ? 'middle' : 'middle'}

            >

              创建章节

            </Button>

          )}

          <Button

            type="primary"

            icon={<RocketOutlined />}

            onClick={handleOpenBatchGenerate}

            disabled={chapters.length === 0}

            block={isMobile}

            size={isMobile ? 'middle' : 'middle'}

            style={{ background: '#722ed1', borderColor: '#722ed1' }}

          >

            批量生成

          </Button>

          <Button

            type="default"

            icon={<DownloadOutlined />}

            onClick={handleExport}

            disabled={chapters.length === 0}

            block={isMobile}

            size={isMobile ? 'middle' : 'middle'}

          >

            导出

          </Button>

          {!isMobile && (

            <Tag color="blue">

              {currentProject.outline_mode === 'one-to-one'

                ? '每章单独大纲'

                : '全书共用大纲'}

            </Tag>

          )}

        </Space>

      </div>



      <div style={{ flex: 1, overflowY: 'auto', minHeight: 0 }}>

        {chapters.length === 0 ? (

          <Empty description="暂无章节。" />

        ) : currentProject.outline_mode === 'one-to-one' ? (


          <List

            dataSource={sortedChapters}

            renderItem={(item) => (

              <List.Item

                id={`chapter-item-${item.id}`}

                style={{

                  padding: '16px',

                  marginBottom: 16,

                  background: '#fff',

                  borderRadius: 8,

                  border: '1px solid #f0f0f0',

                  flexDirection: isMobile ? 'column' : 'row',

                  alignItems: isMobile ? 'flex-start' : 'center',

                }}

                actions={isMobile ? undefined : [

                  <Button

                    type="text"

                    icon={<ReadOutlined />}

                    onClick={() => handleOpenReader(item)}

                    disabled={!item.content || item.content.trim() === ''}

                    title={!item.content || item.content.trim() === '' ? '暂无可阅读内容。' : '打开阅读器'}

                  >

                    阅读

                  </Button>,

                  <Button

                    type="text"

                    icon={<EditOutlined />}

                    onClick={() => handleOpenEditor(item.id)}

                  >

                    编辑

                  </Button>,

                  (() => {

                    const task = analysisTasksMap[item.id];

                    const isAnalyzing = task && (task.status === 'pending' || task.status === 'running');

                    const hasContent = item.content && item.content.trim() !== '';



                    return (

                      <Button

                        type="text"

                        icon={isAnalyzing ? <SyncOutlined spin /> : <FundOutlined />}

                        onClick={() => handleShowAnalysis(item.id)}

                        disabled={!hasContent || isAnalyzing}

                        loading={isAnalyzing}

                        title={

                          !hasContent ? '暂无可分析内容' :

                            isAnalyzing ? '分析中...' : ''



                        }

                      >

                        {isAnalyzing ? '分析中' : '分析'}

                      </Button>

                    );

                  })(),

                  <Button

                    type="text"

                    icon={<SettingOutlined />}

                    onClick={() => handleOpenModal(item.id)}

                  >

                    设置

                  </Button>,

                ]}

              >

                <div style={{ width: '100%' }}>

                  <List.Item.Meta

                    avatar={!isMobile && <FileTextOutlined style={{ fontSize: 32, color: 'var(--color-primary)' }} />}

                    title={

                      <div style={{

                        display: 'flex',

                        flexDirection: isMobile ? 'column' : 'row',

                        alignItems: isMobile ? 'flex-start' : 'center',

                        gap: isMobile ? 6 : 12,

                        width: '100%'

                      }}>

                        <span style={{ fontSize: isMobile ? 14 : 16, fontWeight: 500, flexShrink: 0 }}>

                          {`#${item.chapter_number} ${item.title}`}

                        </span>

                        <Space wrap size={isMobile ? 4 : 8}>

                          <Tag color={getStatusColor(item.status)}>{getStatusText(item.status)}</Tag>

                          <Badge count={`${item.word_count || 0}字`} style={{ backgroundColor: 'var(--color-success)' }} />

                          {renderAnalysisStatus(item.id)}

                          {!canGenerateChapter(item) && (

                            <Tag icon={<LockOutlined />} color="warning" title={getGenerateDisabledReason(item)}>

                              暂不可生成

                            </Tag>

                          )}

                        </Space>

                      </div>

                    }

                    description={

                      item.content ? (

                        <div style={{ marginTop: 8, color: 'rgba(0,0,0,0.65)', lineHeight: 1.6, fontSize: isMobile ? 12 : 14 }}>

                          {item.content.substring(0, isMobile ? 80 : 150)}

                          {item.content.length > (isMobile ? 80 : 150) && '...'}

                        </div>

                      ) : (

                        <span style={{ color: 'rgba(0,0,0,0.45)', fontSize: isMobile ? 12 : 14 }}>暂无内容</span>

                      )

                    }

                  />



                  {isMobile && (

                    <Space style={{ marginTop: 12, width: '100%', justifyContent: 'flex-end' }} wrap>

                      <Button

                        type="text"

                        icon={<ReadOutlined />}

                        onClick={() => handleOpenReader(item)}

                        size="small"

                        disabled={!item.content || item.content.trim() === ''}

                        title={!item.content || item.content.trim() === '' ? '暂无可阅读内容。' : '打开阅读器'}

                      />

                      <Button

                        type="text"

                        icon={<EditOutlined />}

                        onClick={() => handleOpenEditor(item.id)}

                        size="small"

                        title="编辑"

                      />

                      {(() => {

                        const task = analysisTasksMap[item.id];

                        const isAnalyzing = task && (task.status === 'pending' || task.status === 'running');

                        const hasContent = item.content && item.content.trim() !== '';



                        return (

                          <Button

                            type="text"

                            icon={isAnalyzing ? <SyncOutlined spin /> : <FundOutlined />}

                            onClick={() => handleShowAnalysis(item.id)}

                            size="small"

                            disabled={!hasContent || isAnalyzing}

                            loading={isAnalyzing}

                            title={!hasContent ? '暂无可分析内容。' : isAnalyzing ? '分析中...' : '分析'}









                          />

                        );

                      })()}

                      <Button

                        type="text"

                        icon={<SettingOutlined />}

                        onClick={() => handleOpenModal(item.id)}

                        size="small"

                        title="设置"

                      />

                    </Space>

                  )}

                </div>

              </List.Item>

            )}

          />

        ) : (


          <Collapse

            bordered={false}

            defaultActiveKey={groupedChapters.map((_, idx) => idx.toString())}

            expandIcon={({ isActive }) => <CaretRightOutlined rotate={isActive ? 90 : 0} />}

            style={{ background: 'transparent' }}

          >

            {groupedChapters.map((group, groupIndex) => (

              <Collapse.Panel

                key={groupIndex.toString()}

                header={

                  <div style={{ display: 'flex', alignItems: 'center', gap: 12 }}>

                    <Tag color={group.outlineId ? 'blue' : 'default'} style={{ margin: 0 }}>

                      {group.outlineId ? `大纲 ${group.outlineOrder}` : '未关联大纲'}

                    </Tag>

                    <span style={{ fontWeight: 600, fontSize: 16 }}>

                      {group.outlineTitle}

                    </span>

                    <Badge

                      count={`${group.chapters.length}章`}

                      style={{ backgroundColor: 'var(--color-success)' }}

                    />

                    <Badge

                      count={`${group.chapters.reduce((sum, ch) => sum + (ch.word_count || 0), 0)}字`}

                      style={{ backgroundColor: 'var(--color-primary)' }}

                    />

                  </div>

                }

                style={{

                  marginBottom: 16,

                  background: '#fff',

                  borderRadius: 8,

                  border: '1px solid #f0f0f0',

                }}

              >

                <List

                  dataSource={group.chapters}

                  renderItem={(item) => (

                    <List.Item

                      id={`chapter-item-${item.id}`}

                      style={{

                        padding: '16px 0',

                        borderRadius: 8,

                        transition: 'background 0.3s ease',

                        flexDirection: isMobile ? 'column' : 'row',

                        alignItems: isMobile ? 'flex-start' : 'center',

                      }}

                      actions={isMobile ? undefined : [

                        <Button

                          type="text"

                          icon={<ReadOutlined />}

                          onClick={() => handleOpenReader(item)}

                          disabled={!item.content || item.content.trim() === ''}

                          title={!item.content || item.content.trim() === '' ? '暂无内容' : '阅读'}

                        >

                          阅读

                        </Button>,

                        <Button

                          type="text"

                          icon={<EditOutlined />}

                          onClick={() => handleOpenEditor(item.id)}

                        >

                          编辑

                        </Button>,

                        (() => {

                          const task = analysisTasksMap[item.id];

                          const isAnalyzing = task && (task.status === 'pending' || task.status === 'running');

                          const hasContent = item.content && item.content.trim() !== '';



                          return (

                            <Button

                              type="text"

                              icon={isAnalyzing ? <SyncOutlined spin /> : <FundOutlined />}

                              onClick={() => handleShowAnalysis(item.id)}

                              disabled={!hasContent || isAnalyzing}

                              loading={isAnalyzing}

                              title={

                                !hasContent ? '暂无内容' :

                                  isAnalyzing ? '分析中...' :

                                    '查看分析'

                              }

                            >

                              {isAnalyzing ? '分析中' : '分析'}

                            </Button>

                          );

                        })(),

                        <Button

                          type="text"

                          icon={<SettingOutlined />}

                          onClick={() => handleOpenModal(item.id)}

                        >

                          设置

                        </Button>,


                        ...(currentProject.outline_mode === 'one-to-many' ? [

                          <Popconfirm

                            title="删除该章节？"

                            description="这会将该章节从列表中移除。"

                            onConfirm={() => handleDeleteChapter(item.id)}

                            okText="删除"

                            cancelText="取消"

                            okButtonProps={{ danger: true }}

                          >

                            <Button

                              type="text"

                              danger

                              icon={<DeleteOutlined />}

                            >

                              删除

                            </Button>

                          </Popconfirm>

                        ] : []),

                      ]}

                    >

                      <div style={{ width: '100%' }}>

                        <List.Item.Meta

                          avatar={!isMobile && <FileTextOutlined style={{ fontSize: 32, color: 'var(--color-primary)' }} />}

                          title={

                            <div style={{

                              display: 'flex',

                              flexDirection: isMobile ? 'column' : 'row',

                              alignItems: isMobile ? 'flex-start' : 'center',

                              gap: isMobile ? 6 : 12,

                              width: '100%'

                            }}>

                              <span style={{ fontSize: isMobile ? 14 : 16, fontWeight: 500, flexShrink: 0 }}>

                                第{item.chapter_number}章：{item.title}

                              </span>

                              <Space wrap size={isMobile ? 4 : 8}>

                                <Tag color={getStatusColor(item.status)}>{getStatusText(item.status)}</Tag>

                                <Badge count={`${item.word_count || 0}字`} style={{ backgroundColor: 'var(--color-success)' }} />

                                {renderAnalysisStatus(item.id)}

                                {!canGenerateChapter(item) && (

                                  <Tag icon={<LockOutlined />} color="warning" title={getGenerateDisabledReason(item)}>

                                    暂不可生成

                                  </Tag>

                                )}

                                <Space size={4}>

                                  {item.expansion_plan && (

                                    <InfoCircleOutlined

                                      title="查看扩写计划"

                                      style={{ color: 'var(--color-primary)', cursor: 'pointer', fontSize: 16 }}

                                      onClick={(e) => {

                                        e.stopPropagation();

                                        showExpansionPlanModal(item);

                                      }}

                                    />

                                  )}

                                  <FormOutlined

                                    title={item.expansion_plan ? "编辑扩写计划" : "创建扩写计划"}

                                    style={{ color: 'var(--color-success)', cursor: 'pointer', fontSize: 16 }}

                                    onClick={(e) => {

                                      e.stopPropagation();

                                      handleOpenPlanEditor(item);

                                    }}

                                  />

                                </Space>

                              </Space>

                            </div>

                          }

                          description={

                            item.content ? (

                              <div style={{ marginTop: 8, color: 'rgba(0,0,0,0.65)', lineHeight: 1.6, fontSize: isMobile ? 12 : 14 }}>

                                {item.content.substring(0, isMobile ? 80 : 150)}

                                {item.content.length > (isMobile ? 80 : 150) && '...'}

                              </div>

                            ) : (

                              <span style={{ color: 'rgba(0,0,0,0.45)', fontSize: isMobile ? 12 : 14 }}>暂无内容</span>

                            )

                          }

                        />



                        {isMobile && (

                          <Space style={{ marginTop: 12, width: '100%', justifyContent: 'flex-end' }} wrap>

                            <Button

                              type="text"

                              icon={<ReadOutlined />}

                              onClick={() => handleOpenReader(item)}

                              size="small"

                              disabled={!item.content || item.content.trim() === ''}

                              title={!item.content || item.content.trim() === '' ? '暂无内容' : '阅读'}

                            />

                            <Button

                              type="text"

                              icon={<EditOutlined />}

                              onClick={() => handleOpenEditor(item.id)}

                              size="small"

                              title="编辑"

                            />

                            {(() => {

                              const task = analysisTasksMap[item.id];

                              const isAnalyzing = task && (task.status === 'pending' || task.status === 'running');

                              const hasContent = item.content && item.content.trim() !== '';



                              return (

                                <Button

                                  type="text"

                                  icon={isAnalyzing ? <SyncOutlined spin /> : <FundOutlined />}

                                  onClick={() => handleShowAnalysis(item.id)}

                                  size="small"

                                  disabled={!hasContent || isAnalyzing}

                                  loading={isAnalyzing}

                                  title={

                                    !hasContent ? '暂无内容' :

                                      isAnalyzing ? '分析中...' :

                                        '查看分析'

                                  }

                                />

                              );

                            })()}

                            <Button

                              type="text"

                              icon={<SettingOutlined />}

                              onClick={() => handleOpenModal(item.id)}

                              size="small"

                              title="设置"

                            />


                            {currentProject.outline_mode === 'one-to-many' && (

                              <Popconfirm

                                title="删除该章节？"

                                description="这会将该章节从列表中移除。"

                                onConfirm={() => handleDeleteChapter(item.id)}

                                okText="删除"

                                cancelText="取消"

                                okButtonProps={{ danger: true }}

                              >

                                <Button

                                  type="text"

                                  danger

                                  icon={<DeleteOutlined />}

                                  size="small"

                                  title="删除"

                                />

                              </Popconfirm>

                            )}

                          </Space>

                        )}

                      </div>

                    </List.Item>

                  )}

                />

              </Collapse.Panel>

            ))}

          </Collapse>

        )}

      </div>



      <Modal

        title={editingId ? '编辑章节' : '创建章节'}

        open={isModalOpen}

        onCancel={() => setIsModalOpen(false)}

        footer={null}

        centered

        width={isMobile ? 'calc(100vw - 32px)' : 520}

        style={isMobile ? {

          maxWidth: 'calc(100vw - 32px)',

          margin: '0 auto',

          padding: '0 16px'

        } : undefined}

        styles={{

          body: {

            maxHeight: isMobile ? 'calc(100vh - 200px)' : 'calc(80vh - 110px)',

            overflowY: 'auto'

          }

        }}

      >

        <Form form={form} layout="vertical" onFinish={handleSubmit}>

          <Form.Item

            label="章节标题"

            name="title"

            tooltip={

              currentProject.outline_mode === 'one-to-one'

                ? "一对一模式下标题固定。"

                : "一对多模式下必须填写标题。"

            }

            rules={

              currentProject.outline_mode === 'one-to-many'

                ? [{ required: true, message: '请输入章节标题' }]

                : undefined

            }

          >

            <Input



              placeholder="请输入章节标题"



              disabled={currentProject.outline_mode === 'one-to-one'}



            />






          </Form.Item>



          <Form.Item

            label="章节编号"

            name="chapter_number"

            tooltip="用于章节排序"

          >

            <Input type="number" placeholder="请输入章节编号" />

          </Form.Item>



          <Form.Item label="状态" name="status">

            <Select placeholder="请选择状态">

              <Select.Option value="draft">草稿</Select.Option>

              <Select.Option value="writing">写作中</Select.Option>

              <Select.Option value="completed">已完成</Select.Option>

            </Select>

          </Form.Item>



          <Form.Item>

            <Space style={{ float: 'right' }}>

              <Button onClick={() => setIsModalOpen(false)}>取消</Button>

              <Button type="primary" htmlType="submit">

                保存

              </Button>

            </Space>

          </Form.Item>

        </Form>

      </Modal>



      <Modal

        title="编辑章节内容"

        open={isEditorOpen}

        onCancel={() => {

          setChapterQualityMetrics(null);

          setChapterQualityGeneratedAt(null);

          setIsEditorOpen(false);

        }}

        closable

        maskClosable={false}

        keyboard

        width={isMobile ? 'calc(100vw - 32px)' : '85%'}

        centered

        style={isMobile ? {

          maxWidth: 'calc(100vw - 32px)',

          margin: '0 auto',

          padding: '0 16px'

        } : undefined}

        styles={{

          body: {

            maxHeight: isMobile ? 'calc(100vh - 200px)' : 'calc(100vh - 110px)',

            overflowY: 'auto',

            padding: isMobile ? '16px 12px' : '8px'

          }

        }}

        footer={null}

      >

        <Form form={editorForm} layout="vertical" onFinish={handleEditorSubmit}>


          <Form.Item

            label="章节标题"

            tooltip="标题由系统生成，仅供展示"

            style={{ marginBottom: isMobile ? 16 : 12 }}

          >

            <Space.Compact style={{ width: '100%' }}>

              <Form.Item name="title" noStyle>

                <Input disabled style={{ flex: 1 }} />

              </Form.Item>

              {editingId && (() => {

                const currentChapter = chapters.find(c => c.id === editingId);

                const canGenerate = currentChapter ? canGenerateChapter(currentChapter) : false;

                const disabledReason = currentChapter ? getGenerateDisabledReason(currentChapter) : '';



                return (

                  <Button

                    type="primary"

                    icon={canGenerate ? <ThunderboltOutlined /> : <LockOutlined />}

                    onClick={() => currentChapter && showGenerateModal(currentChapter)}

                    loading={isContinuing}

                    disabled={!canGenerate}

                    danger={!canGenerate}

                    style={{ fontWeight: 'bold' }}

                    title={!canGenerate ? disabledReason : '智能续写当前章节'}

                  >

                    {isMobile ? '续写' : '智能续写'}

                  </Button>

                );

              })()}

            </Space.Compact>

          </Form.Item>




          <div style={{

            display: isMobile ? 'block' : 'flex',

            gap: isMobile ? 0 : 16,

            marginBottom: isMobile ? 0 : 12

          }}>

            <Form.Item

              label="写作风格"

              tooltip="选择写作风格会影响生成的语气、节奏和用词"

              required

              style={{ flex: 1, marginBottom: isMobile ? 16 : 0 }}

            >

              <Select

                placeholder="请选择写作风格"

                value={selectedStyleId}

                onChange={setSelectedStyleId}

                status={!selectedStyleId ? 'error' : undefined}

              >

                {writingStyles.map(style => (

                  <Select.Option key={style.id} value={style.id}>

                    {style.name}{style.is_default && ' (默认)'}

                  </Select.Option>

                ))}

              </Select>

              {!selectedStyleId && (

                <div style={{ color: '#ff4d4f', fontSize: 12, marginTop: 4 }}>请选择写作风格</div>

              )}

            </Form.Item>



            <Form.Item
              label="叙事视角"
              tooltip="可临时覆盖项目设置，用于控制第一/第三人称或全知视角"
              style={{ flex: 1, marginBottom: isMobile ? 16 : 0 }}
            >
              <Select

                placeholder={`当前项目默认：${getNarrativePerspectiveText(currentProject?.narrative_perspective)}`}

                value={temporaryNarrativePerspective}

                onChange={setTemporaryNarrativePerspective}

                allowClear

              >

                <Select.Option value="first_person">第一人称</Select.Option>

                <Select.Option value="third_person">第三人称</Select.Option>

                <Select.Option value="omniscient">全知视角</Select.Option>

              </Select>

              {temporaryNarrativePerspective && (

                <div style={{ color: 'var(--color-success)', fontSize: 12, marginTop: 4 }}>
                  当前视角：{getNarrativePerspectiveText(temporaryNarrativePerspective)}
                </div>

              )}
            </Form.Item>

            <Form.Item
              label="剧情阶段"
              tooltip="用于提示剧情推进阶段，帮助生成更符合节奏"
              style={{ flex: 1, marginBottom: isMobile ? 16 : 0 }}
            >
              <Select
                placeholder="请选择剧情阶段"
                value={selectedPlotStage}
                onChange={setSelectedPlotStage}
                allowClear
                optionLabelProp="label"
              >
                {CREATION_PLOT_STAGE_OPTIONS.map((option) => (
                  <Select.Option key={option.value} value={option.value} label={option.label}>
                    <div>{option.label}</div>
                    <div style={{ fontSize: 12, color: 'var(--color-text-tertiary)' }}>{option.description}</div>
                  </Select.Option>
                ))}
              </Select>
              <Space size={8} style={{ marginTop: 8 }}>
                <Button size="small" onClick={applyInferredSinglePlotStage}>应用推断阶段</Button>
                {selectedPlotStage && (
                  <span style={{ color: 'var(--color-success)', fontSize: 12 }}>
                    已选择： {CREATION_PLOT_STAGE_OPTIONS.find((item) => item.value === selectedPlotStage)?.label || selectedPlotStage}
                  </span>
                )}
              </Space>
            </Form.Item>
          </div>

          <Card
            size="small"
            title="创作预设"
            style={{ marginBottom: 12 }}
          >
            <Space wrap>
              {CREATION_PRESETS.map((preset) => (
                <Button
                  key={preset.id}
                  type={activeSingleCreationPreset?.id === preset.id ? 'primary' : 'default'}
                  onClick={() => applySingleCreationPreset(preset.id)}
                >
                  {preset.label}
                </Button>
              ))}
              <Button
                onClick={() => {
                  setSelectedCreativeMode(projectDefaultCreativeMode);
                  setSelectedStoryFocus(projectDefaultStoryFocus);
                }}
              >
                {"重置选择"}
              </Button>
            </Space>

            {activeSingleCreationPreset && (
              <Alert
                type="info"
                showIcon
                style={{ marginTop: 12 }}
                message={`当前预设：${activeSingleCreationPreset.label}`}
                description={activeSingleCreationPreset.description}
              />
            )}

            {recommendedCreationPresets.length > 0 && (
              <Alert
                type="success"
                showIcon
                style={{ marginTop: 12 }}
                message={"推荐预设"}
                description={(
                  <Space wrap>
                    {recommendedCreationPresets.map((item) => {
                      const preset = getCreationPresetById(item.id);
                      if (!preset) return null;
                      return (
                        <Button key={item.id} size="small" onClick={() => applySingleCreationPreset(item.id)}>
                          {preset.label}{item.reason ? ' - ' + item.reason : ''}
                        </Button>
                      );
                    })}
                  </Space>
                )}
              />
            )}

            {singleScoreDrivenRecommendationCard && (
              <Card size="small" title={singleScoreDrivenRecommendationCard.title} style={{ marginTop: 12 }}>
                <Space direction="vertical" size={10} style={{ display: 'flex' }}>
                  <Alert
                    type="info"
                    showIcon
                    message={singleScoreDrivenRecommendationCard.summary}
                    description={singleScoreDrivenRecommendationCard.applyHint}
                  />

                  {singleScoreDrivenRecommendationCard.recommendedPresetLabel && (
                    <div>
                      <div style={{ fontWeight: 600, marginBottom: 6 }}>推荐预设</div>
                      <Space wrap size={[8, 8]}>
                        <Tag color={singleScoreDrivenRecommendationCard.recommendedPresetId === activeSingleCreationPreset?.id ? 'blue' : 'processing'}>
                          {singleScoreDrivenRecommendationCard.recommendedPresetLabel}
                        </Tag>
                        {singleScoreDrivenRecommendationCard.recommendedPresetReason && (
                          <span style={{ color: 'var(--color-text-secondary)' }}>
                            {singleScoreDrivenRecommendationCard.recommendedPresetReason}
                          </span>
                        )}
                      </Space>
                    </div>
                  )}

                  <div>
                      <div style={{ fontWeight: 600, marginBottom: 6 }}>推荐阶段</div>
                    <Space wrap size={[8, 8]}>
                      <Tag color={singleScoreDrivenRecommendationCard.recommendedStage === selectedPlotStage ? 'blue' : 'purple'}>
                        {singleScoreDrivenRecommendationCard.recommendedStageLabel}
                      </Tag>
                      <span style={{ color: 'var(--color-text-secondary)' }}>
                        {singleScoreDrivenRecommendationCard.stageReason}
                      </span>
                    </Space>
                  </div>

                  {singleScoreDrivenRecommendationCard.alternatives.length > 0 && (
                    <div>
                      <div style={{ fontWeight: 600, marginBottom: 6 }}>备选方案</div>
                      <Space direction="vertical" size={4} style={{ display: 'flex' }}>
                        {singleScoreDrivenRecommendationCard.alternatives.map((item) => (
                          <div key={item.id} style={{ color: 'var(--color-text-secondary)' }}>
                            - <strong>{item.label}</strong>{item.reason ? ' - ' + item.reason : ''}
                          </div>
                        ))}
                      </Space>
                    </div>
                  )}

                  <Space wrap>
                    {singleScoreDrivenRecommendationCard.recommendedPresetId && (
                      <Button size="small" onClick={() => applySingleCreationPreset(singleScoreDrivenRecommendationCard.recommendedPresetId!)}>
                        Apply preset
                      </Button>
                    )}
                    {singleScoreDrivenRecommendationCard.recommendedStage && (
                      <Button size="small" onClick={() => setSelectedPlotStage(singleScoreDrivenRecommendationCard.recommendedStage)}>
                        Apply stage
                      </Button>
                    )}
                    {(singleScoreDrivenRecommendationCard.recommendedPresetId || singleScoreDrivenRecommendationCard.recommendedStage) && (
                      <Button
                        type="primary"
                        size="small"
                        onClick={() => {
                          if (singleScoreDrivenRecommendationCard.recommendedPresetId) {
                            applySingleCreationPreset(singleScoreDrivenRecommendationCard.recommendedPresetId!);
                          }
                          if (singleScoreDrivenRecommendationCard.recommendedStage) {
                            setSelectedPlotStage(singleScoreDrivenRecommendationCard.recommendedStage);
                          }
                        }}
                      >
                        Apply recommendations
                      </Button>
                    )}
                  </Space>
                </Space>
              </Card>
            )}

            {singleStoryCreationControlCard && (
              <Card
                size="small"
                title={singleStoryCreationControlCard.title}
                extra={(
                  <Space size={8}>
                    <Tag color={isSingleStoryCreationControlCustomized ? 'purple' : 'blue'}>
                      {isSingleStoryCreationControlCustomized ? '自定义' : '系统'}
                    </Tag>
                    <Button
                      size="small"
                      type="link"
                      onClick={() => setSingleStoryCreationBriefDraft(singleSystemStoryCreationBrief)}
                      disabled={!singleSystemStoryCreationBrief || singleStoryCreationBriefDraft === singleSystemStoryCreationBrief}
                    >
                      恢复系统建议
                    </Button>
                  </Space>
                )}
                style={{ marginTop: 12 }}
              >
                <Alert
                  type="info"
                  showIcon
                  style={{ marginBottom: 12 }}
                  message={singleStoryCreationControlCard.summary}
                  description={singleStoryCreationControlCard.directive}
                />
                <Space direction="vertical" size={8} style={{ display: 'flex' }}>
                  <div style={{ padding: '10px 12px', border: '1px solid #f0f0f0', borderRadius: 8 }}>
                    <div style={{ fontWeight: 600, marginBottom: 6 }}>故事简介</div>
                    <div style={{ color: 'var(--color-text-secondary)', fontSize: 12, marginBottom: 8 }}>
                      提供一段简短说明，帮助引导生成。
                    </div>
                    <TextArea
                      value={singleStoryCreationBriefDraft}
                      onChange={(event) => setSingleStoryCreationBriefDraft(event.target.value)}
                      autoSize={{ minRows: 4, maxRows: 8 }}
                      maxLength={600}
                      showCount
                      placeholder="请简要描述故事..."
                    />
                    <div style={{ color: 'var(--color-text-secondary)', fontSize: 12, marginTop: 8 }}>
                      {isSingleStoryCreationBriefCustomized
                        ? '当前使用自定义简介。'
                        : '当前使用系统简介。'
                      }</div>
                  </div>
                  <div style={{ padding: '10px 12px', border: '1px solid #f0f0f0', borderRadius: 8 }}>
                    <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', gap: 12, marginBottom: 8 }}>
                      <div>
                        <div style={{ fontWeight: 600, marginBottom: 4 }}>故事节拍</div>
                        <div style={{ color: 'var(--color-text-secondary)', fontSize: 12 }}>
                          规划故事的关键节拍。
                        </div>
                      </div>
                      <Button
                        size="small"
                        type="link"
                        onClick={() => setSingleStoryBeatPlannerDraft(singleSystemStoryBeatPlanner)}
                        disabled={
                          isStoryBeatPlannerDraftEmpty(singleSystemStoryBeatPlanner)
                          || areStoryBeatPlannerDraftsEqual(singleStoryBeatPlannerDraft, singleSystemStoryBeatPlanner)
                        }
                      >
                        恢复系统建议
                      </Button>
                    </div>
                    <Space direction="vertical" size={8} style={{ display: 'flex' }}>
                      {STORY_BEAT_PLANNER_FIELDS.map((field) => (
                        <div key={field.key}>
                          <div style={{ fontSize: 12, fontWeight: 600, marginBottom: 6 }}>{field.label}</div>
                          <Input
                            value={singleStoryBeatPlannerDraft[field.key]}
                            onChange={(event) => setSingleStoryBeatPlannerDraft((prev) => ({
                              ...prev,
                              [field.key]: event.target.value,
                            }))}
                            placeholder={field.placeholder}
                            maxLength={120}
                          />
                        </div>
                      ))}
                    </Space>
                    <div style={{ color: 'var(--color-text-secondary)', fontSize: 12, marginTop: 8 }}>
                      {isSingleStoryBeatPlannerCustomized
                        ? '当前使用自定义节拍。'
                        : '当前使用系统节拍。'
                      }</div>
                  </div>
                  <div style={{ padding: '10px 12px', border: '1px solid #f0f0f0', borderRadius: 8 }}>
                    <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', gap: 12, marginBottom: 8 }}>
                      <div>
                        <div style={{ fontWeight: 600, marginBottom: 4 }}>场景提纲</div>
                        <div style={{ color: 'var(--color-text-secondary)', fontSize: 12 }}>
                          为这个故事规划场景结构。
                        </div>
                      </div>
                      <Button
                        size="small"
                        type="link"
                        onClick={() => setSingleStorySceneOutlineDraft(singleSuggestedStorySceneOutline)}
                        disabled={
                          isStorySceneOutlineDraftEmpty(singleSuggestedStorySceneOutline)
                          || areStorySceneOutlineDraftsEqual(singleStorySceneOutlineDraft, singleSuggestedStorySceneOutline)
                        }
                      >
                        恢复系统建议
                      </Button>
                    </div>
                    <Space direction="vertical" size={8} style={{ display: 'flex' }}>
                      {STORY_SCENE_OUTLINE_FIELDS.map((field) => (
                        <div key={field.key}>
                          <div style={{ fontSize: 12, fontWeight: 600, marginBottom: 6 }}>{field.label}</div>
                          <TextArea
                            value={singleStorySceneOutlineDraft[field.key]}
                            onChange={(event) => setSingleStorySceneOutlineDraft((prev) => ({
                              ...prev,
                              [field.key]: event.target.value,
                            }))}
                            autoSize={{ minRows: 2, maxRows: 4 }}
                            maxLength={220}
                            showCount
                            placeholder={field.placeholder}
                          />
                        </div>
                      ))}
                    </Space>
                    <div style={{ color: 'var(--color-text-secondary)', fontSize: 12, marginTop: 8 }}>
                      {isSingleStorySceneOutlineCustomized
                        ? '当前使用自定义提纲。'
                        : '当前使用系统提纲。'
                      }</div>
                  </div>
                  <div style={{ padding: '10px 12px', border: '1px solid #f0f0f0', borderRadius: 8 }}>
                    <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'flex-start', gap: 12, marginBottom: 8 }}>
                      <div>
                        <div style={{ fontWeight: 600, marginBottom: 4 }}>提示词</div>
                        <div style={{ color: 'var(--color-text-secondary)', fontSize: 12 }}>
                          根据当前选择生成的提示词。
                        </div>
                      </div>
                      <Button
                        size="small"
                        type="link"
                        disabled={!resolvedSingleStoryCreationBrief}
                        onClick={() => void copyStoryCreationPrompt(resolvedSingleStoryCreationBrief, 'single')}
                      >
                        Copy prompt
                      </Button>
                    </div>
                    <Space wrap size={[8, 8]} style={{ marginBottom: 8 }}>
                      {singleStoryCreationPromptLayerLabels.map((item) => (
                        <Tag key={item} color="processing">{item}</Tag>
                      ))}
                      <Tag color={isSingleStoryCreationPromptVerbose ? 'gold' : 'blue'}>
                        {`${singleStoryCreationPromptCharCount} 字符`}
                      </Tag>
                    </Space>
                    {isSingleStoryCreationPromptVerbose && (
                      <Alert
                        type="warning"
                        showIcon
                        style={{ marginBottom: 8 }}
                        message="已启用详细提示词"
                        description="提示词包含更多细节，长度可能较长。"
                      />
                    )}
                    <TextArea
                      value={resolvedSingleStoryCreationBrief ?? ''}
                      autoSize={{ minRows: 6, maxRows: 12 }}
                      readOnly
                      placeholder="提示词将显示在此"
                    />
                  </div>
                  <StoryCreationSnapshotPanel
                    scopeLabel="single"
                    description="故事创作快照。"
                    emptyText="暂无快照。"
                    snapshots={singleStoryCreationSnapshots}
                    currentDraft={singleStoryCreationCurrentDraft}
                    canSave={canSaveSingleStoryCreationSnapshot}
                    onSave={() => void saveSingleStoryCreationSnapshot('manual')}
                    onApply={applySingleStoryCreationSnapshot}
                    onDelete={deleteSingleStoryCreationSnapshot}
                    onCopy={copyStoryCreationPrompt}
                    includeNarrativePerspective
                  />
                  <div style={{ padding: '10px 12px', border: '1px solid #f0f0f0', borderRadius: 8 }}>
                    <div style={{ fontWeight: 600, marginBottom: 6 }}>执行路径</div>
                    <Space direction="vertical" size={4} style={{ display: 'flex' }}>
                      {singleStoryCreationControlCard.executionPath.map((item) => (
                        <div key={item} style={{ color: 'var(--color-text-secondary)' }}>- {item}</div>
                      ))}
                    </Space>
                  </div>
                  <div style={{ padding: '10px 12px', border: '1px solid #f0f0f0', borderRadius: 8 }}>
                    <div style={{ fontWeight: 600, marginBottom: 6 }}>预期结果</div>
                    <Space direction="vertical" size={4} style={{ display: 'flex' }}>
                      {singleStoryCreationControlCard.expectedOutcomes.map((item) => (
                        <div key={item} style={{ color: 'var(--color-text-secondary)' }}>- {item}</div>
                      ))}
                    </Space>
                  </div>
                  <div style={{ padding: '10px 12px', border: '1px solid #f0f0f0', borderRadius: 8 }}>
                    <div style={{ fontWeight: 600, marginBottom: 6 }}>约束规则</div>
                    <Space direction="vertical" size={4} style={{ display: 'flex' }}>
                      {singleStoryCreationControlCard.guardrails.map((item) => (
                        <div key={item} style={{ color: 'var(--color-text-secondary)' }}>- {item}</div>
                      ))}
                    </Space>
                  </div>
                </Space>
              </Card>
            )}

            {singleStoryRepairTargetCard && (
              <Card
                size="small"
                title={singleStoryRepairTargetCard.title}
                extra={<Tag color="gold">修复重点</Tag>}
                style={{ marginTop: 12 }}
              >
                <Alert
                  type="warning"
                  showIcon
                  style={{ marginBottom: 12 }}
                  message={singleStoryRepairTargetCard.repairSummary}
                  description={singleStoryRepairTargetCard.applyHint}
                />
                <Space direction="vertical" size={8} style={{ display: 'flex' }}>
                  {[
                    ["优先修复项", singleStoryRepairTargetCard.priorityTarget],
                    ["反模式", singleStoryRepairTargetCard.antiPattern],
                  ].map(([label, value]) => (
                    <div
                      key={label}
                      style={{
                        padding: '10px 12px',
                        border: '1px solid #f0f0f0',
                        borderRadius: 8,
                      }}
                    >
                      <div style={{ fontWeight: 600, marginBottom: 4 }}>{label}</div>
                      <div style={{ color: 'var(--color-text-secondary)' }}>{value}</div>
                    </div>
                  ))}
                  <div style={{ padding: '10px 12px', border: '1px solid #f0f0f0', borderRadius: 8 }}>
                    <div style={{ fontWeight: 600, marginBottom: 6 }}>修复目标</div>
                    <Space direction="vertical" size={4} style={{ display: 'flex' }}>
                      {singleStoryRepairTargetCard.repairTargets.map((item) => (
                        <div key={item} style={{ color: 'var(--color-text-secondary)' }}>- {item}</div>
                      ))}
                    </Space>
                  </div>
                  <div style={{ padding: '10px 12px', border: '1px solid #f0f0f0', borderRadius: 8 }}>
                    <div style={{ fontWeight: 600, marginBottom: 6 }}>保留优势</div>
                    <Space direction="vertical" size={4} style={{ display: 'flex' }}>
                      {singleStoryRepairTargetCard.preserveStrengths.map((item) => (
                        <div key={item} style={{ color: 'var(--color-text-secondary)' }}>- {item}</div>
                      ))}
                    </Space>
                  </div>
                </Space>
              </Card>
            )}
          </Card>

          {singleCreationBlueprint && (
            <Card size="small" title="创作蓝图" style={{ marginBottom: 12 }}>
              <div style={{ color: 'var(--color-text-secondary)', marginBottom: 10 }}>
                {singleCreationBlueprint.summary}
              </div>
              <div style={{ fontWeight: 600, marginBottom: 8 }}>推荐节拍</div>
              <Space direction="vertical" size={6} style={{ display: 'flex' }}>
                {singleCreationBlueprint.beats.map((beat, index) => (
                  <div key={beat}>{index + 1}. {beat}</div>
                ))}
              </Space>
              {singleCreationBlueprint.risks.length > 0 && (
                <Alert
                  type="warning"
                  showIcon
                  style={{ marginTop: 12 }}
                  message="风险提示"
                  description={singleCreationBlueprint.risks.join(', ')}
                />
              )}
            </Card>
          )}

          {singleStoryObjectiveCard && (
            <Card size="small" title="故事目标" style={{ marginBottom: 12 }}>
              <div style={{ color: 'var(--color-text-secondary)', marginBottom: 10 }}>
                {singleStoryObjectiveCard.summary}
              </div>
              <Space direction="vertical" size={8} style={{ display: 'flex' }}>
                {[
                  ['目标', singleStoryObjectiveCard.objective],
                  ['阻碍', singleStoryObjectiveCard.obstacle],
                  ['转折', singleStoryObjectiveCard.turn],
                  ['钩子', singleStoryObjectiveCard.hook],
                ].map(([label, value]) => (
                  <div
                    key={label}
                    style={{
                      padding: '10px 12px',
                      border: '1px solid #f0f0f0',
                      borderRadius: 8,
                    }}
                  >
                    <div style={{ fontWeight: 600, marginBottom: 4 }}>{label}</div>
                    <div style={{ color: 'var(--color-text-secondary)' }}>{value}</div>
                  </div>
                ))}
              </Space>
            </Card>
          )}

          {singleStoryResultCard && (
            <Card size="small" title="故事结果" style={{ marginBottom: 12 }}>
              <div style={{ color: 'var(--color-text-secondary)', marginBottom: 10 }}>
                {singleStoryResultCard.summary}
              </div>
              <Space direction="vertical" size={8} style={{ display: 'flex' }}>
                {[
                  ['推进结果', singleStoryResultCard.progress],
                  ['揭示信息', singleStoryResultCard.reveal],
                  ['关系变化', singleStoryResultCard.relationship],
                  ['后续影响', singleStoryResultCard.fallout],
                ].map(([label, value]) => (
                  <div
                    key={label}
                    style={{
                      padding: '10px 12px',
                      border: '1px solid #f0f0f0',
                      borderRadius: 8,
                    }}
                  >
                    <div style={{ fontWeight: 600, marginBottom: 4 }}>{label}</div>
                    <div style={{ color: 'var(--color-text-secondary)' }}>{value}</div>
                  </div>
                ))}
              </Space>
            </Card>
          )}

          {singleStoryExecutionChecklist && (
            <Card size="small" title="执行清单" style={{ marginBottom: 12 }}>
              <div style={{ color: 'var(--color-text-secondary)', marginBottom: 10 }}>
                {singleStoryExecutionChecklist.summary}
              </div>
              <Space direction="vertical" size={8} style={{ display: 'flex' }}>
                {[
                  ['开篇', singleStoryExecutionChecklist.opening],
                  ['压力', singleStoryExecutionChecklist.pressure],
                  ['转折', singleStoryExecutionChecklist.pivot],
                  ['收束', singleStoryExecutionChecklist.closing],
                ].map(([label, value]) => (
                  <div
                    key={label}
                    style={{
                      padding: '10px 12px',
                      border: '1px solid #f0f0f0',
                      borderRadius: 8,
                    }}
                  >
                    <div style={{ fontWeight: 600, marginBottom: 4 }}>{label}</div>
                    <div style={{ color: 'var(--color-text-secondary)' }}>{value}</div>
                  </div>
                ))}
              </Space>
            </Card>
          )}

          {singleStoryRepetitionRiskCard && (
            <Card size="small" title="重复风险" style={{ marginBottom: 12 }}>
              <div style={{ color: 'var(--color-text-secondary)', marginBottom: 10 }}>
                {singleStoryRepetitionRiskCard.summary}
              </div>
              <Space direction="vertical" size={8} style={{ display: 'flex' }}>
                {[
                  ['开篇风险', singleStoryRepetitionRiskCard.openingRisk],
                  ['压力风险', singleStoryRepetitionRiskCard.pressureRisk],
                  ['转折风险', singleStoryRepetitionRiskCard.pivotRisk],
                  ['收束风险', singleStoryRepetitionRiskCard.closingRisk],
                ].map(([label, value]) => (
                  <div
                    key={label}
                    style={{
                      padding: '10px 12px',
                      border: '1px solid #f0f0f0',
                      borderRadius: 8,
                    }}
                  >
                    <div style={{ fontWeight: 600, marginBottom: 4 }}>{label}</div>
                    <div style={{ color: 'var(--color-text-secondary)' }}>{value}</div>
                  </div>
                ))}
              </Space>
            </Card>
          )}

          {singleStoryAcceptanceCard && (
            <Card size="small" title="验收检查" style={{ marginBottom: 12 }}>
              <div style={{ color: 'var(--color-text-secondary)', marginBottom: 10 }}>
                {singleStoryAcceptanceCard.summary}
              </div>
              <Space direction="vertical" size={8} style={{ display: 'flex' }}>
                {[
                  ['目标达成检查', singleStoryAcceptanceCard.missionCheck],
                  ['变化检查', singleStoryAcceptanceCard.changeCheck],
                  ['新鲜度检查', singleStoryAcceptanceCard.freshnessCheck],
                  ['收束检查', singleStoryAcceptanceCard.closingCheck],
                ].map(([label, value]) => (
                  <div
                    key={label}
                    style={{
                      padding: '10px 12px',
                      border: '1px solid #f0f0f0',
                      borderRadius: 8,
                    }}
                  >
                    <div style={{ fontWeight: 600, marginBottom: 4 }}>{label}</div>
                    <div style={{ color: 'var(--color-text-secondary)' }}>{value}</div>
                  </div>
                ))}
              </Space>
            </Card>
          )}

          {singleStoryCharacterArcCard && (
            <Card size="small" title="人物弧光" style={{ marginBottom: 12 }}>
              <div style={{ color: 'var(--color-text-secondary)', marginBottom: 10 }}>
                {singleStoryCharacterArcCard.summary}
              </div>
              <Space direction="vertical" size={8} style={{ display: 'flex' }}>
                {[
                  ['外在线', singleStoryCharacterArcCard.externalLine],
                  ['内在线', singleStoryCharacterArcCard.internalLine],
                  ['关系线', singleStoryCharacterArcCard.relationshipLine],
                  ['弧光落点', singleStoryCharacterArcCard.arcLanding],
                ].map(([label, value]) => (
                  <div
                    key={label}
                    style={{
                      padding: '10px 12px',
                      border: '1px solid #f0f0f0',
                      borderRadius: 8,
                    }}
                  >
                    <div style={{ fontWeight: 600, marginBottom: 4 }}>{label}</div>
                    <div style={{ color: 'var(--color-text-secondary)' }}>{value}</div>
                  </div>
                ))}
              </Space>
            </Card>
          )}

          {singleVolumePacingPlan && (
            <Card size="small" title="篇幅节奏规划" style={{ marginBottom: 12 }}>
              <div style={{ color: 'var(--color-text-secondary)', marginBottom: 10 }}>
                {singleVolumePacingPlan.summary}
              </div>
              <Space direction="vertical" size={8} style={{ display: 'flex' }}>
                {singleVolumePacingPlan.segments.map((segment) => (
                  <div key={`${segment.stage}-${segment.startChapter}`}>
                    <strong>第{segment.startChapter}-{segment.endChapter}章：{segment.label}</strong>
                    <div style={{ color: 'var(--color-text-secondary)', marginTop: 2 }}>
                      {segment.mission}
                    </div>
                  </div>
                ))}
              </Space>
            </Card>
          )}

          <Form.Item
            label="创作模式"
            tooltip="选择单章创作的模式"
            style={{ marginBottom: isMobile ? 16 : 12 }}
          >
            <Select
              placeholder="请选择创作模式"
              value={selectedCreativeMode}
              onChange={setSelectedCreativeMode}
              allowClear
              optionLabelProp="label"
            >
              {CREATIVE_MODE_OPTIONS.map((option) => (
                <Select.Option key={option.value} value={option.value} label={option.label}>
                  <div>{option.label}</div>
                  <div style={{ fontSize: 12, color: 'var(--color-text-tertiary)' }}>{option.description}</div>
                </Select.Option>
              ))}
            </Select>
          </Form.Item>

          <Form.Item
            label="故事聚焦"
            tooltip="选择本次创作的关注点与重心"
            style={{ marginBottom: isMobile ? 16 : 12 }}
          >
            <Select
              placeholder="请选择故事聚焦"
              value={selectedStoryFocus}
              onChange={setSelectedStoryFocus}
              allowClear
              optionLabelProp="label"
            >
              {STORY_FOCUS_OPTIONS.map((option) => (
                <Select.Option key={option.value} value={option.value} label={option.label}>
                  <div>{option.label}</div>
                  <div style={{ fontSize: 12, color: 'var(--color-text-tertiary)' }}>{option.description}</div>
                </Select.Option>
              ))}
            </Select>
          </Form.Item>

          <div style={{
            display: isMobile ? 'block' : 'flex',
            gap: isMobile ? 0 : 16,
            marginBottom: isMobile ? 16 : 12

          }}>

            <Form.Item

              label="目标字数"

              tooltip="设置 生成时的目标字数范围（约）"

              style={{ flex: 1, marginBottom: isMobile ? 16 : 0 }}

            >

              <InputNumber

                min={500}

                max={10000}

                step={100}

                value={targetWordCount}

                onChange={(value) => {

                  const newValue = value || DEFAULT_WORD_COUNT;

                  setTargetWordCount(newValue);

                  setCachedWordCount(newValue);

                }}

                style={{ width: '100%' }}

                formatter={(value) => (value ? String(value) + ' 字' : '')}
                parser={(value) => parseInt((value || '').replace(' 字', ''), 10)}


              />

            </Form.Item>



            <Form.Item

              label="AI 模型"

              tooltip="选择用于生成的模型"

              style={{ flex: 1, marginBottom: isMobile ? 16 : 0 }}

            >

              <Select

                placeholder={selectedModel ? '已选择：' + selectedModel : '请选择模型'}

                value={selectedModel}

                onChange={setSelectedModel}

                allowClear

                showSearch

                optionFilterProp="label"

              >

                {availableModels.map(model => (

                  <Select.Option key={model.value} value={model.value} label={model.label}>

                    {model.label}

                  </Select.Option>

                ))}

              </Select>

            </Form.Item>

          </div>



          <Card

            size="small"

            title="质量画像"

            style={{ marginBottom: 12 }}

          >

            {getQualityProfileDisplayItems(chapterQualityProfileSummary).length > 0 ? (

              <>

                <Alert

                  type="success"

                  showIcon

                  style={{ marginBottom: 12 }}

                  message="质量画像摘要"

                  description="该画像汇总了质量指标与优化建议。"

                />

                <Descriptions column={1} size="small">

                  {getQualityProfileDisplayItems(chapterQualityProfileSummary).map((item) => (

                    <Descriptions.Item key={item.key} label={item.label}>

                      {item.description}

                    </Descriptions.Item>

                  ))}

                </Descriptions>

              </>

            ) : (

              <Alert

                type="info"

                showIcon

                message="暂无质量画像"

                description="运行分析后可生成质量画像。"

              />

            )}

          </Card>



          <Card

            size="small"

            title="质量指标"

            loading={chapterQualityLoading}

            style={{ marginBottom: 12 }}

          >
            {chapterQualityMetrics ? (
              <>
                {singleAfterScorecard && (
                  <Card size="small" title="优化后评分卡" style={{ marginBottom: 12 }}>
                    <Alert
                      type={singleAfterScorecard.verdictColor as 'success' | 'info' | 'warning' | 'error'}
                      showIcon
                      style={{ marginBottom: 12 }}
                      message={singleAfterScorecard.verdict}
                      description={singleAfterScorecard.summary}
                    />
                    <Descriptions column={1} size="small" style={{ marginBottom: 12 }}>
                      <Descriptions.Item label="焦点检查">
                        {singleAfterScorecard.focusCheck}
                      </Descriptions.Item>
                      <Descriptions.Item label="下一步行动">
                        {singleAfterScorecard.nextAction}
                      </Descriptions.Item>
                    </Descriptions>
                    <div style={{ marginBottom: 8, fontWeight: 600 }}>Strengths</div>
                    <Space wrap size={[8, 8]} style={{ marginBottom: 12 }}>
                      {singleAfterScorecard.strengths.map((item) => (
                        <Tag key={item} color="success">{item}</Tag>
                      ))}
                    </Space>
                    <div style={{ marginBottom: 8, fontWeight: 600 }}>Gaps</div>
                    <Space direction="vertical" size={6} style={{ display: 'flex' }}>
                      {singleAfterScorecard.gaps.map((item) => (
                        <div key={item} style={{ color: 'var(--color-text-secondary)' }}>- {item}</div>
                      ))}
                    </Space>
                  </Card>
                )}

                <Descriptions column={isMobile ? 1 : 2} size="small">
                  <Descriptions.Item label="综合得分">
                    <Tag color={getOverallScoreColor(chapterQualityMetrics.overall_score)}>
                      {chapterQualityMetrics.overall_score}
                    </Tag>

                  </Descriptions.Item>

                  <Descriptions.Item label="冲突链命中率">

                    <Tag color={getMetricRateColor(chapterQualityMetrics.conflict_chain_hit_rate)}>{chapterQualityMetrics.conflict_chain_hit_rate}%</Tag>

                  </Descriptions.Item>

                  <Descriptions.Item label="规则锚定命中率">

                    <Tag color={getMetricRateColor(chapterQualityMetrics.rule_grounding_hit_rate)}>{chapterQualityMetrics.rule_grounding_hit_rate}%</Tag>

                  </Descriptions.Item>

                  <Descriptions.Item label="开篇钩子命中率">

                    <Tag color={getMetricRateColor(chapterQualityMetrics.opening_hook_rate)}>{chapterQualityMetrics.opening_hook_rate}%</Tag>

                  </Descriptions.Item>

                  <Descriptions.Item label="回收链命中率">

                    <Tag color={getMetricRateColor(chapterQualityMetrics.payoff_chain_rate)}>{chapterQualityMetrics.payoff_chain_rate}%</Tag>

                  </Descriptions.Item>

                  <Descriptions.Item label="悬念收尾率">

                    <Tag color={getMetricRateColor(chapterQualityMetrics.cliffhanger_rate)}>{chapterQualityMetrics.cliffhanger_rate}%</Tag>

                  </Descriptions.Item>

                  <Descriptions.Item label="对话自然度">

                    <Tag color={getMetricRateColor(chapterQualityMetrics.dialogue_naturalness_rate)}>{chapterQualityMetrics.dialogue_naturalness_rate}%</Tag>

                  </Descriptions.Item>

                  <Descriptions.Item label="大纲贴合度">

                    <Tag color={getMetricRateColor(chapterQualityMetrics.outline_alignment_rate)}>{chapterQualityMetrics.outline_alignment_rate}%</Tag>

                  </Descriptions.Item>

                  <Descriptions.Item label="生成时间">

                    {chapterQualityGeneratedAt ? new Date(chapterQualityGeneratedAt).toLocaleString() : '尚未生成'}

                  </Descriptions.Item>

                </Descriptions>

                <Alert

                  type={getWeakestQualityMetric(chapterQualityMetrics).value >= 60 ? 'info' : 'warning'}

                  showIcon

                  style={{ marginTop: 12 }}

                  message="最弱指标"

                  description="优先改善最弱指标以提升整体质量。"

                />

                <Card size="small" title="指标详情" style={{ marginTop: 12 }}>

                  <Space direction="vertical" style={{ width: '100%' }} size={10}>

                    {getQualityMetricItems(chapterQualityMetrics).map((item) => (

                      <div key={item.key}>

                        <div style={{ display: 'flex', justifyContent: 'space-between', marginBottom: 4, gap: 12 }}>

                          <Space size={4}>

                            <span>{item.label}</span>

                            <Tooltip title={item.tip}>

                              <InfoCircleOutlined style={{ color: '#8c8c8c' }} />

                            </Tooltip>

                          </Space>

                          <span style={{ color: '#595959' }}>{item.value}%</span>

                        </div>

                        <Progress percent={item.value} showInfo={false} size="small" strokeColor={getMetricStrokeColor(item.value)} />

                      </div>

                    ))}

                  </Space>

                </Card>

              </>

            ) : (

              <Alert

                type="info"

                showIcon

                message="暂无质量指标。"

                description="运行分析后可生成质量指标。"

              />

            )}

          </Card>



          <Form.Item label="章节内容" name="content">

            <TextArea

              ref={contentTextAreaRef}


              rows={isMobile ? 12 : 20}



              placeholder="请输入章节内容..."






              style={{ fontFamily: 'monospace', fontSize: isMobile ? 12 : 14 }}

            />

          </Form.Item>




          <div data-partial-regenerate-toolbar>

            <PartialRegenerateToolbar

              visible={partialRegenerateToolbarVisible}

              position={partialRegenerateToolbarPosition}

              selectedText={selectedTextForRegenerate}

              onRegenerate={handleOpenPartialRegenerate}

            />

          </div>



          <Form.Item>

            <Space style={{ width: '100%', justifyContent: 'flex-end', flexDirection: isMobile ? 'column' : 'row', alignItems: isMobile ? 'stretch' : 'center' }}>

              <Space style={{ width: isMobile ? '100%' : 'auto' }}>

                <Button

                  onClick={() => {

                    setChapterQualityMetrics(null);

                    setChapterQualityProfileSummary(null);

                    setChapterQualityGeneratedAt(null);

                    setIsEditorOpen(false);

                  }}

                  block={isMobile}

                >

                  取消

                </Button>

                <Button

                  type="primary"

                  htmlType="submit"

                  block={isMobile}

                >

                  保存内容

                </Button>

              </Space>

            </Space>

          </Form.Item>

        </Form>

      </Modal>



      {analysisChapterId ? (

        <Suspense fallback={null}>

          <LazyChapterAnalysis

            chapterId={analysisChapterId}

            visible={analysisVisible}

            onClose={() => {

              setAnalysisVisible(false);




            refreshChapters();




            if (currentProject) {

              projectApi.getProject(currentProject.id)

                .then(updatedProject => {

                  setCurrentProject(updatedProject);

                })

                .catch(error => {

                  console.error('闂佸憡甯￠弨閬嶅蓟婵犲啨浜滈柛锔诲幗缁愭菐閸ワ絽澧插ù鐓庢噺瀵板嫭娼忛銉?', error);

                });

            }




            if (analysisChapterId) {

              const chapterIdToRefresh = analysisChapterId;



              setTimeout(() => {

                refreshChapterAnalysisTask(chapterIdToRefresh)

                  .catch(error => {

                    console.error('闂佸憡甯￠弨閬嶅蓟婵犲洤绀嗛柛鈩冾焽閳ь兛绮欓幃鈺呮嚋绾版ê浜惧ù锝嗘偠娴滃ジ鎮?', error);


                    setTimeout(() => {

                      refreshChapterAnalysisTask(chapterIdToRefresh)

                        .catch(err => console.error('缂備焦顨忛崗娑氳姳閳轰讲鏋庨柍銉ュ暱閻撴洟鏌￠崒娑欑凡闁靛洦鍨归幏?', err));

                    }, 1000);

                  });

              }, 500);

            }



            setAnalysisChapterId(null);

          }}

          />

        </Suspense>

      ) : null}




      <Modal

        title={

          <Space>

            <RocketOutlined style={{ color: '#722ed1' }} />

            <span>批量生成章节</span>

          </Space>

        }

        open={batchGenerateVisible}

        onCancel={() => setBatchGenerateVisible(false)}

        footer={!batchGenerating ? (

          <Space style={{ width: '100%', justifyContent: 'flex-end', flexWrap: 'wrap' }}>

            <Button onClick={() => setBatchGenerateVisible(false)}>

              取消

            </Button>

            <Button type="primary" icon={<RocketOutlined />} onClick={() => batchForm.submit()}>

              开始批量生成

            </Button>

          </Space>

        ) : null}

        width={isMobile ? 'calc(100vw - 32px)' : 700}

        centered

        closable

        maskClosable

        style={isMobile ? {

          maxWidth: 'calc(100vw - 32px)',

          margin: '0 auto',

          padding: '0 16px'

        } : undefined}

        styles={{

          body: {

            maxHeight: isMobile ? 'calc(100vh - 200px)' : 'calc(100vh - 260px)',

            overflowY: 'auto',

            overflowX: 'hidden'

          }

        }}

      >

        {!batchGenerating ? (

          <Form

            form={batchForm}

            layout="vertical"

            onFinish={handleBatchGenerate}

            initialValues={{

              startChapterNumber: sortedChapters.find(ch => !ch.content || ch.content.trim() === '')?.chapter_number || 1,

              count: 5,

              enableAnalysis: true,

              styleId: selectedStyleId,

              targetWordCount: getCachedWordCount(),

              model: selectedModel,

            }}

          >

            <Alert

              message="批量生成会基于当前设置一次生成多个章节。"

              type="info"

              showIcon

              style={{ marginBottom: 16 }}

            />




            <div style={{ display: 'flex', flexDirection: isMobile ? 'column' : 'row', gap: isMobile ? 0 : 16 }}>

              <Form.Item

                label="起始章节"

                name="startChapterNumber"

                rules={[{ required: true, message: '请选择起始章节' }]}

                style={{ flex: 1, marginBottom: 12 }}

              >

                <Select placeholder="请选择章节">

                  {batchStartChapterOptions.map(ch => (

                    <Select.Option key={ch.id} value={ch.chapter_number}>

                      {'第' + ch.chapter_number + '章：' + ch.title}

                    </Select.Option>

                  ))}

                </Select>

              </Form.Item>



              <Form.Item

                label="章节数量"

                name="count"

                rules={[{ required: true, message: '请选择章节数量' }]}

                style={{ marginBottom: 12 }}

              >

                <Radio.Group buttonStyle="solid" size={isMobile ? 'small' : 'middle'}>

                  <Radio.Button value={5}>5章</Radio.Button>

                  <Radio.Button value={10}>10章</Radio.Button>

                  <Radio.Button value={15}>15章</Radio.Button>

                  <Radio.Button value={20}>20章</Radio.Button>

                </Radio.Group>

              </Form.Item>

            </div>




            <div style={{ display: 'flex', flexDirection: isMobile ? 'column' : 'row', gap: isMobile ? 0 : 16 }}>
              <Form.Item
                label="写作风格"
                name="styleId"
                rules={[{ required: true, message: '请选择写作风格' }]}

                style={{ flex: 1, marginBottom: 12 }}

              >



                <Select



                  placeholder="请选择写作风格"





                  showSearch

                  optionFilterProp="children"

                >

                  {writingStyles.map(style => (

                    <Select.Option key={style.id} value={style.id}>

                      {style.name}{style.is_default && ' (默认)'}

                    </Select.Option>

                  ))}

                </Select>

              </Form.Item>



              <Form.Item

                label="目标字数"

                name="targetWordCount"

                rules={[{ required: true, message: '请输入目标字数' }]}

                tooltip="用于控制批量生成的篇幅。"

                style={{ flex: 1, marginBottom: 12 }}

              >

                <InputNumber<number>

                  min={500}

                  max={10000}

                  step={100}

                  style={{ width: '100%' }}

                  formatter={(value) => (value ? String(value) + ' 字' : '')}

                  parser={(value) => parseInt((value || '').replace(' 字', ''), 10)}

                  onChange={(value) => {

                    if (value) {

                      setCachedWordCount(value);

                    }

                  }}

                />
              </Form.Item>
            </div>

            <Form.Item
              label="剧情阶段"



              tooltip="选择用于批量创作的剧情阶段"

              style={{ marginBottom: 12 }}

            >

              <Select

                placeholder="请选择剧情阶段"



                value={batchSelectedPlotStage}
                onChange={setBatchSelectedPlotStage}
                allowClear
                optionLabelProp="label"
              >
                {CREATION_PLOT_STAGE_OPTIONS.map((option) => (
                  <Select.Option key={option.value} value={option.value} label={option.label}>
                    <div>{option.label}</div>
                    <div style={{ fontSize: 12, color: 'var(--color-text-tertiary)' }}>{option.description}</div>
                  </Select.Option>
                ))}
              </Select>
              <Space size={8} style={{ marginTop: 8 }}>
                <Button size="small" onClick={applyInferredBatchPlotStage}>应用推断阶段</Button>
                {batchSelectedPlotStage && (
                  <span style={{ color: 'var(--color-success)', fontSize: 12 }}>
                    已选择： {CREATION_PLOT_STAGE_OPTIONS.find((item) => item.value === batchSelectedPlotStage)?.label || batchSelectedPlotStage}
                  </span>
                )}
              </Space>
            </Form.Item>

            <Card
              size="small"
              title="创作预设"
              style={{ marginBottom: 12 }}
            >
              <Space wrap>
                {CREATION_PRESETS.map((preset) => (
                  <Button
                    key={preset.id}
                    type={activeBatchCreationPreset?.id === preset.id ? 'primary' : 'default'}
                    onClick={() => applyBatchCreationPreset(preset.id)}
                  >
                    {preset.label}
                  </Button>
                ))}
                <Button
                  onClick={() => {
                    setBatchSelectedCreativeMode(projectDefaultCreativeMode);
                    setBatchSelectedStoryFocus(projectDefaultStoryFocus);
                  }}
                >
                  {"重置选择"}
                </Button>
              </Space>

              {activeBatchCreationPreset && (
                <div style={{ marginTop: 12, color: 'var(--color-text-secondary)' }}>
                   <strong>{activeBatchCreationPreset.label}</strong>: {activeBatchCreationPreset.description}
                </div>
              )}

              {batchScoreDrivenRecommendationCard && (
                <Card size="small" title={batchScoreDrivenRecommendationCard.title} style={{ marginTop: 12 }}>
                  <Space direction="vertical" size={10} style={{ display: 'flex' }}>
                    <Alert
                      type="info"
                      showIcon
                      message={batchScoreDrivenRecommendationCard.summary}
                      description={batchScoreDrivenRecommendationCard.applyHint}
                    />

                    {batchScoreDrivenRecommendationCard.recommendedPresetLabel && (
                      <div>
                        <div style={{ fontWeight: 600, marginBottom: 6 }}>推荐预设</div>
                        <Space wrap size={[8, 8]}>
                          <Tag color={batchScoreDrivenRecommendationCard.recommendedPresetId === activeBatchCreationPreset?.id ? 'blue' : 'processing'}>
                            {batchScoreDrivenRecommendationCard.recommendedPresetLabel}
                          </Tag>
                          {batchScoreDrivenRecommendationCard.recommendedPresetReason && (
                            <span style={{ color: 'var(--color-text-secondary)' }}>
                              {batchScoreDrivenRecommendationCard.recommendedPresetReason}
                            </span>
                          )}
                        </Space>
                      </div>
                    )}

                    <div>
                      <div style={{ fontWeight: 600, marginBottom: 6 }}>推荐阶段</div>
                      <Space wrap size={[8, 8]}>
                        <Tag color={batchScoreDrivenRecommendationCard.recommendedStage === batchSelectedPlotStage ? 'blue' : 'purple'}>
                          {batchScoreDrivenRecommendationCard.recommendedStageLabel}
                        </Tag>
                        <span style={{ color: 'var(--color-text-secondary)' }}>
                          {batchScoreDrivenRecommendationCard.stageReason}
                        </span>
                      </Space>
                    </div>

                    {batchScoreDrivenRecommendationCard.alternatives.length > 0 && (
                      <div>
                        <div style={{ fontWeight: 600, marginBottom: 6 }}>备选方案</div>
                        <Space direction="vertical" size={4} style={{ display: 'flex' }}>
                          {batchScoreDrivenRecommendationCard.alternatives.map((item) => (
                            <div key={item.id} style={{ color: 'var(--color-text-secondary)' }}>
                              - <strong>{item.label}</strong>{item.reason ? ' - ' + item.reason : ''}
                            </div>
                          ))}
                        </Space>
                      </div>
                    )}

                    <Space wrap>
                      {batchScoreDrivenRecommendationCard.recommendedPresetId && (
                        <Button size="small" onClick={() => applyBatchCreationPreset(batchScoreDrivenRecommendationCard.recommendedPresetId!)}>
                          {"应用预设"}
                        </Button>
                      )}
                      {batchScoreDrivenRecommendationCard.recommendedStage && (
                        <Button size="small" onClick={() => setBatchSelectedPlotStage(batchScoreDrivenRecommendationCard.recommendedStage)}>
                          {"应用阶段"}
                        </Button>
                      )}
                      {(batchScoreDrivenRecommendationCard.recommendedPresetId || batchScoreDrivenRecommendationCard.recommendedStage) && (
                        <Button
                          type="primary"
                          size="small"
                          onClick={() => {
                            if (batchScoreDrivenRecommendationCard.recommendedPresetId) {
                              applyBatchCreationPreset(batchScoreDrivenRecommendationCard.recommendedPresetId!);
                            }
                            if (batchScoreDrivenRecommendationCard.recommendedStage) {
                              setBatchSelectedPlotStage(batchScoreDrivenRecommendationCard.recommendedStage);
                            }
                          }}
                        >
                          {"应用建议"}
                        </Button>
                      )}
                    </Space>
                  </Space>
                </Card>
              )}
            </Card>

              {batchStoryCreationControlCard && (
                <Card
                  size="small"
                  title={batchStoryCreationControlCard.title}
                  extra={(
                    <Space size={8}>
                      <Tag color={isBatchStoryCreationControlCustomized ? 'purple' : 'blue'}>
                        {isBatchStoryCreationControlCustomized ? '自定义' : '系统'}
                      </Tag>
                      <Button
                        size="small"
                        type="link"
                        onClick={() => setBatchStoryCreationBriefDraft(batchSystemStoryCreationBrief)}
                        disabled={!batchSystemStoryCreationBrief || batchStoryCreationBriefDraft === batchSystemStoryCreationBrief}
                      >
                        恢复系统建议
                      </Button>
                    </Space>
                  )}
                  style={{ marginTop: 12 }}
                >
                  <Alert
                    type="info"
                    showIcon
                    style={{ marginBottom: 12 }}
                    message={batchStoryCreationControlCard.summary}
                    description={batchStoryCreationControlCard.directive}
                  />
                  <Space direction="vertical" size={8} style={{ display: 'flex' }}>
                    <div style={{ padding: '10px 12px', border: '1px solid #f0f0f0', borderRadius: 8 }}>
                      <div style={{ fontWeight: 600, marginBottom: 6 }}>故事简介</div>
                      <div style={{ color: 'var(--color-text-secondary)', fontSize: 12, marginBottom: 8 }}>
                        提供一段简短说明，帮助引导生成。
                      </div>
                      <TextArea
                        value={batchStoryCreationBriefDraft}
                        onChange={(event) => setBatchStoryCreationBriefDraft(event.target.value)}
                        autoSize={{ minRows: 4, maxRows: 8 }}
                        maxLength={600}
                        showCount
                        placeholder="请简要描述故事..."
                      />
                      <div style={{ color: 'var(--color-text-secondary)', fontSize: 12, marginTop: 8 }}>
                        {isBatchStoryCreationBriefCustomized
                          ? '当前使用自定义简介。'
                          : '当前使用系统简介。'
                        }</div>
                    </div>
                    <div style={{ padding: '10px 12px', border: '1px solid #f0f0f0', borderRadius: 8 }}>
                      <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', gap: 12, marginBottom: 8 }}>
                        <div>
                          <div style={{ fontWeight: 600, marginBottom: 4 }}>故事节拍</div>
                          <div style={{ color: 'var(--color-text-secondary)', fontSize: 12 }}>
                            规划故事的关键节拍。
                          </div>
                        </div>
                        <Button
                          size="small"
                          type="link"
                          onClick={() => setBatchStoryBeatPlannerDraft(batchSystemStoryBeatPlanner)}
                          disabled={
                            isStoryBeatPlannerDraftEmpty(batchSystemStoryBeatPlanner)
                            || areStoryBeatPlannerDraftsEqual(batchStoryBeatPlannerDraft, batchSystemStoryBeatPlanner)
                          }
                        >
                          恢复系统建议
                        </Button>
                      </div>
                      <Space direction="vertical" size={8} style={{ display: 'flex' }}>
                        {STORY_BEAT_PLANNER_FIELDS.map((field) => (
                          <div key={field.key}>
                            <div style={{ fontSize: 12, fontWeight: 600, marginBottom: 6 }}>{field.label}</div>
                            <Input
                              value={batchStoryBeatPlannerDraft[field.key]}
                              onChange={(event) => setBatchStoryBeatPlannerDraft((prev) => ({
                                ...prev,
                                [field.key]: event.target.value,
                              }))}
                              placeholder={field.placeholder}
                              maxLength={120}
                            />
                          </div>
                        ))}
                      </Space>
                      <div style={{ color: 'var(--color-text-secondary)', fontSize: 12, marginTop: 8 }}>
                        {isBatchStoryBeatPlannerCustomized
                          ? '当前使用自定义节拍。'
                          : '当前使用系统节拍。'
                        }</div>
                    </div>
                    <div style={{ padding: '10px 12px', border: '1px solid #f0f0f0', borderRadius: 8 }}>
                      <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', gap: 12, marginBottom: 8 }}>
                        <div>
                          <div style={{ fontWeight: 600, marginBottom: 4 }}>场景提纲</div>
                          <div style={{ color: 'var(--color-text-secondary)', fontSize: 12 }}>
                            为这个故事规划场景结构。
                          </div>
                        </div>
                        <Button
                          size="small"
                          type="link"
                          onClick={() => setBatchStorySceneOutlineDraft(batchSuggestedStorySceneOutline)}
                          disabled={
                            isStorySceneOutlineDraftEmpty(batchSuggestedStorySceneOutline)
                            || areStorySceneOutlineDraftsEqual(batchStorySceneOutlineDraft, batchSuggestedStorySceneOutline)
                          }
                        >
                          恢复系统建议
                        </Button>
                      </div>
                      <Space direction="vertical" size={8} style={{ display: 'flex' }}>
                        {STORY_SCENE_OUTLINE_FIELDS.map((field) => (
                          <div key={field.key}>
                            <div style={{ fontSize: 12, fontWeight: 600, marginBottom: 6 }}>{field.label}</div>
                            <TextArea
                              value={batchStorySceneOutlineDraft[field.key]}
                              onChange={(event) => setBatchStorySceneOutlineDraft((prev) => ({
                                ...prev,
                                [field.key]: event.target.value,
                              }))}
                              autoSize={{ minRows: 2, maxRows: 4 }}
                              maxLength={220}
                              showCount
                              placeholder={field.placeholder}
                            />
                          </div>
                        ))}
                      </Space>
                      <div style={{ color: 'var(--color-text-secondary)', fontSize: 12, marginTop: 8 }}>
                        {isBatchStorySceneOutlineCustomized
                          ? '当前使用自定义提纲。'
                          : '当前使用系统提纲。'
                        }</div>
                    </div>
                    <div style={{ padding: '10px 12px', border: '1px solid #f0f0f0', borderRadius: 8 }}>
                      <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'flex-start', gap: 12, marginBottom: 8 }}>
                        <div>
                          <div style={{ fontWeight: 600, marginBottom: 4 }}>提示词</div>
                          <div style={{ color: 'var(--color-text-secondary)', fontSize: 12 }}>
                            根据当前选择生成的提示词。
                          </div>
                        </div>
                        <Button
                          size="small"
                          type="link"
                          disabled={!resolvedBatchStoryCreationBrief}
                          onClick={() => void copyStoryCreationPrompt(resolvedBatchStoryCreationBrief, 'batch')}
                        >
                          复制提示词
                        </Button>
                      </div>
                      <Space wrap size={[8, 8]} style={{ marginBottom: 8 }}>
                        {batchStoryCreationPromptLayerLabels.map((item) => (
                          <Tag key={item} color="processing">{item}</Tag>
                        ))}
                        <Tag color={isBatchStoryCreationPromptVerbose ? 'gold' : 'blue'}>
                          {batchStoryCreationPromptCharCount + ' 字符'}
                        </Tag>
                      </Space>
                      {isBatchStoryCreationPromptVerbose && (
                        <Alert
                          type="warning"
                          showIcon
                          style={{ marginBottom: 8 }}
                          message="已启用详细提示词"
                          description="提示词包含更详细的生成指导。"
                        />
                      )}
                      <TextArea
                        value={resolvedBatchStoryCreationBrief ?? ''}
                        autoSize={{ minRows: 6, maxRows: 12 }}
                        readOnly
                        placeholder="提示词预览将显示在此。"
                      />
                    </div>
                    <StoryCreationSnapshotPanel
                      scopeLabel="batch"
                      description="保存提示词快照以便复用。"
                      emptyText="暂无快照。"
                      snapshots={batchStoryCreationSnapshots}
                      currentDraft={batchStoryCreationCurrentDraft}
                      canSave={canSaveBatchStoryCreationSnapshot}
                      onSave={() => void saveBatchStoryCreationSnapshot('manual')}
                      onApply={applyBatchStoryCreationSnapshot}
                      onDelete={deleteBatchStoryCreationSnapshot}
                      onCopy={copyStoryCreationPrompt}
                    />
                    <div style={{ padding: '10px 12px', border: '1px solid #f0f0f0', borderRadius: 8 }}>
                      <div style={{ fontWeight: 600, marginBottom: 6 }}>执行路径</div>
                      <Space direction="vertical" size={4} style={{ display: 'flex' }}>
                        {batchStoryCreationControlCard.executionPath.map((item) => (
                          <div key={item} style={{ color: 'var(--color-text-secondary)' }}>- {item}</div>
                        ))}
                      </Space>
                    </div>
                    <div style={{ padding: '10px 12px', border: '1px solid #f0f0f0', borderRadius: 8 }}>
                      <div style={{ fontWeight: 600, marginBottom: 6 }}>预期结果</div>
                      <Space direction="vertical" size={4} style={{ display: 'flex' }}>
                        {batchStoryCreationControlCard.expectedOutcomes.map((item) => (
                          <div key={item} style={{ color: 'var(--color-text-secondary)' }}>- {item}</div>
                        ))}
                      </Space>
                    </div>
                    <div style={{ padding: '10px 12px', border: '1px solid #f0f0f0', borderRadius: 8 }}>
                      <div style={{ fontWeight: 600, marginBottom: 6 }}>约束规则</div>
                      <Space direction="vertical" size={4} style={{ display: 'flex' }}>
                        {batchStoryCreationControlCard.guardrails.map((item) => (
                          <div key={item} style={{ color: 'var(--color-text-secondary)' }}>- {item}</div>
                        ))}
                      </Space>
                    </div>
                  </Space>
                </Card>
              )}

            {batchStoryRepairTargetCard && (
              <Card
                size="small"
                title={batchStoryRepairTargetCard.title}
                extra={<Tag color="gold">修复重点</Tag>}
                style={{ marginTop: 12 }}
              >
                <Alert
                  type="warning"
                  showIcon
                  style={{ marginBottom: 12 }}
                  message={batchStoryRepairTargetCard.repairSummary}
                  description={batchStoryRepairTargetCard.applyHint}
                />
                <Space direction="vertical" size={8} style={{ display: 'flex' }}>
                  {[
                    ['优先修复项', batchStoryRepairTargetCard.priorityTarget],
                    ['反模式', batchStoryRepairTargetCard.antiPattern],
                  ].map(([label, value]) => (
                    <div
                      key={label}
                      style={{
                        padding: '10px 12px',
                        border: '1px solid #f0f0f0',
                        borderRadius: 8,
                      }}
                    >
                      <div style={{ fontWeight: 600, marginBottom: 4 }}>{label}</div>
                      <div style={{ color: 'var(--color-text-secondary)' }}>{value}</div>
                    </div>
                  ))}
                  <div style={{ padding: '10px 12px', border: '1px solid #f0f0f0', borderRadius: 8 }}>
                    <div style={{ fontWeight: 600, marginBottom: 6 }}>修复目标</div>
                    <Space direction="vertical" size={4} style={{ display: 'flex' }}>
                      {batchStoryRepairTargetCard.repairTargets.map((item) => (
                        <div key={item} style={{ color: 'var(--color-text-secondary)' }}>- {item}</div>
                      ))}
                    </Space>
                  </div>
                  <div style={{ padding: '10px 12px', border: '1px solid #f0f0f0', borderRadius: 8 }}>
                    <div style={{ fontWeight: 600, marginBottom: 6 }}>保留优势</div>
                    <Space direction="vertical" size={4} style={{ display: 'flex' }}>
                      {batchStoryRepairTargetCard.preserveStrengths.map((item) => (
                        <div key={item} style={{ color: 'var(--color-text-secondary)' }}>- {item}</div>
                      ))}
                    </Space>
                  </div>
                </Space>
              </Card>
            )}

            {batchCreationBlueprint && (
              <Card size="small" title="批量创作蓝图" style={{ marginBottom: 12 }}>
                <div style={{ color: 'var(--color-text-secondary)', marginBottom: 10 }}>
                  {batchCreationBlueprint.summary}
                </div>
                <div style={{ fontWeight: 600, marginBottom: 8 }}>关键节拍</div>
                <Space direction="vertical" size={6} style={{ display: 'flex' }}>
                  {batchCreationBlueprint.beats.map((beat, index) => (
                    <div key={beat}>{index + 1}. {beat}</div>
                  ))}
                </Space>
                {batchCreationBlueprint.risks.length > 0 && (
                  <Alert
                    type="warning"
                    showIcon
                    style={{ marginTop: 12 }}
                    message="风险提示"
                    description={batchCreationBlueprint.risks.join(', ')}
                  />
                )}
              </Card>
            )}

            {batchStoryObjectiveCard && (
              <Card size="small" title="故事目标" style={{ marginBottom: 12 }}>
                <div style={{ color: 'var(--color-text-secondary)', marginBottom: 10 }}>
                  {batchStoryObjectiveCard.summary}
                </div>
                <Space direction="vertical" size={8} style={{ display: 'flex' }}>
                  {[
                    ['目标', batchStoryObjectiveCard.objective],
                    ['阻碍', batchStoryObjectiveCard.obstacle],
                    ['转折', batchStoryObjectiveCard.turn],
                    ['钩子', batchStoryObjectiveCard.hook],
                  ].map(([label, value]) => (
                    <div
                      key={label}
                      style={{
                        padding: '10px 12px',
                        border: '1px solid #f0f0f0',
                        borderRadius: 8,
                      }}
                    >
                      <div style={{ fontWeight: 600, marginBottom: 4 }}>{label}</div>
                      <div style={{ color: 'var(--color-text-secondary)' }}>{value}</div>
                    </div>
                  ))}
                </Space>
              </Card>
            )}

            {batchStoryResultCard && (
              <Card size="small" title="故事结果" style={{ marginBottom: 12 }}>
                <div style={{ color: 'var(--color-text-secondary)', marginBottom: 10 }}>
                  {batchStoryResultCard.summary}
                </div>
                <Space direction="vertical" size={8} style={{ display: 'flex' }}>
                  {[
                    ['推进结果', batchStoryResultCard.progress],
                    ['揭示信息', batchStoryResultCard.reveal],
                    ['关系变化', batchStoryResultCard.relationship],
                    ['后续影响', batchStoryResultCard.fallout],
                  ].map(([label, value]) => (
                    <div
                      key={label}
                      style={{
                        padding: '10px 12px',
                        border: '1px solid #f0f0f0',
                        borderRadius: 8,
                      }}
                    >
                      <div style={{ fontWeight: 600, marginBottom: 4 }}>{label}</div>
                      <div style={{ color: 'var(--color-text-secondary)' }}>{value}</div>
                    </div>
                  ))}
                </Space>
              </Card>
            )}

            {batchStoryExecutionChecklist && (
              <Card size="small" title="执行清单" style={{ marginBottom: 12 }}>
                <div style={{ color: 'var(--color-text-secondary)', marginBottom: 10 }}>
                  {batchStoryExecutionChecklist.summary}
                </div>
                <Space direction="vertical" size={8} style={{ display: 'flex' }}>
                  {[
                    ['开篇', batchStoryExecutionChecklist.opening],
                    ['压力', batchStoryExecutionChecklist.pressure],
                    ['转折', batchStoryExecutionChecklist.pivot],
                    ['收束', batchStoryExecutionChecklist.closing],
                  ].map(([label, value]) => (
                    <div
                      key={label}
                      style={{
                        padding: '10px 12px',
                        border: '1px solid #f0f0f0',
                        borderRadius: 8,
                      }}
                    >
                      <div style={{ fontWeight: 600, marginBottom: 4 }}>{label}</div>
                      <div style={{ color: 'var(--color-text-secondary)' }}>{value}</div>
                    </div>
                  ))}
                </Space>
              </Card>
            )}

            {batchStoryRepetitionRiskCard && (
              <Card size="small" title="重复风险" style={{ marginBottom: 12 }}>
                <div style={{ color: 'var(--color-text-secondary)', marginBottom: 10 }}>
                  {batchStoryRepetitionRiskCard.summary}
                </div>
                <Space direction="vertical" size={8} style={{ display: 'flex' }}>
                  {[
                    ['开篇风险', batchStoryRepetitionRiskCard.openingRisk],
                    ['压力风险', batchStoryRepetitionRiskCard.pressureRisk],
                    ['转折风险', batchStoryRepetitionRiskCard.pivotRisk],
                    ['收束风险', batchStoryRepetitionRiskCard.closingRisk],
                  ].map(([label, value]) => (
                    <div
                      key={label}
                      style={{
                        padding: '10px 12px',
                        border: '1px solid #f0f0f0',
                        borderRadius: 8,
                      }}
                    >
                      <div style={{ fontWeight: 600, marginBottom: 4 }}>{label}</div>
                      <div style={{ color: 'var(--color-text-secondary)' }}>{value}</div>
                    </div>
                  ))}
                </Space>
              </Card>
            )}

            {batchStoryAcceptanceCard && (
              <Card size="small" title="验收清单" style={{ marginBottom: 12 }}>
                <div style={{ color: 'var(--color-text-secondary)', marginBottom: 10 }}>
                  {batchStoryAcceptanceCard.summary}
                </div>
                <Space direction="vertical" size={8} style={{ display: 'flex' }}>
                  {[
                    ['目标达成检查', batchStoryAcceptanceCard.missionCheck],
                    ['变化检查', batchStoryAcceptanceCard.changeCheck],
                    ['新鲜度检查', batchStoryAcceptanceCard.freshnessCheck],
                    ['收束检查', batchStoryAcceptanceCard.closingCheck],
                  ].map(([label, value]) => (
                    <div
                      key={label}
                      style={{
                        padding: '10px 12px',
                        border: '1px solid #f0f0f0',
                        borderRadius: 8,
                      }}
                    >
                      <div style={{ fontWeight: 600, marginBottom: 4 }}>{label}</div>
                      <div style={{ color: 'var(--color-text-secondary)' }}>{value}</div>
                    </div>
                  ))}
                </Space>
              </Card>
            )}

            {batchStoryCharacterArcCard && (
              <Card size="small" title="人物弧光" style={{ marginBottom: 12 }}>
                <div style={{ color: 'var(--color-text-secondary)', marginBottom: 10 }}>
                  {batchStoryCharacterArcCard.summary}
                </div>
                <Space direction="vertical" size={8} style={{ display: 'flex' }}>
                  {[
                    ['外在线', batchStoryCharacterArcCard.externalLine],
                    ['内在线', batchStoryCharacterArcCard.internalLine],
                    ['关系线', batchStoryCharacterArcCard.relationshipLine],
                    ['弧光落点', batchStoryCharacterArcCard.arcLanding],
                  ].map(([label, value]) => (
                    <div
                      key={label}
                      style={{
                        padding: '10px 12px',
                        border: '1px solid #f0f0f0',
                        borderRadius: 8,
                      }}
                    >
                      <div style={{ fontWeight: 600, marginBottom: 4 }}>{label}</div>
                      <div style={{ color: 'var(--color-text-secondary)' }}>{value}</div>
                    </div>
                  ))}
                </Space>
              </Card>
            )}

            {batchVolumePacingPlan && (
              <Card size="small" title="篇幅节奏规划" style={{ marginBottom: 12 }}>
                <div style={{ color: 'var(--color-text-secondary)', marginBottom: 10 }}>
                  {batchVolumePacingPlan.summary}
                </div>
                <Space direction="vertical" size={8} style={{ display: 'flex' }}>
                  {batchVolumePacingPlan.segments.map((segment) => (
                    <div key={segment.stage + '-' + segment.startChapter}>
                      <strong>{'第 ' + segment.startChapter + '-' + segment.endChapter + ' 章：' + segment.label}</strong>
                      <div style={{ color: 'var(--color-text-secondary)', marginTop: 2 }}>
                        {segment.mission}
                      </div>
                    </div>
                  ))}
                </Space>
              </Card>
            )}

            <Form.Item
              label="创作模式"
              tooltip="选择用于批量创作的模式"
              style={{ marginBottom: 12 }}
            >
              <Select
                placeholder="请选择创作模式"
                value={batchSelectedCreativeMode}
                onChange={setBatchSelectedCreativeMode}
                allowClear
                optionLabelProp="label"
              >
                {CREATIVE_MODE_OPTIONS.map((option) => (
                  <Select.Option key={option.value} value={option.value} label={option.label}>
                    <div>{option.label}</div>
                    <div style={{ fontSize: 12, color: 'var(--color-text-tertiary)' }}>{option.description}</div>
                  </Select.Option>
                ))}
              </Select>
            </Form.Item>

            <Form.Item

              label="故事聚焦"

              tooltip="选择批量创作的关注点与重心"

              style={{ marginBottom: 12 }}

            >

              <Select

                placeholder="请选择故事聚焦"




                value={batchSelectedStoryFocus}
                onChange={setBatchSelectedStoryFocus}
                allowClear
                optionLabelProp="label"
              >
                {STORY_FOCUS_OPTIONS.map((option) => (
                  <Select.Option key={option.value} value={option.value} label={option.label}>
                    <div>{option.label}</div>
                    <div style={{ fontSize: 12, color: 'var(--color-text-tertiary)' }}>{option.description}</div>
                  </Select.Option>
                ))}
              </Select>
            </Form.Item>

            <div style={{ display: 'flex', flexDirection: isMobile ? 'column' : 'row', gap: isMobile ? 0 : 16 }}>
            <Form.Item
              label="AI 模型"
              tooltip="选择用于批量生成的模型。"
              style={{ flex: 1, marginBottom: 12 }}
            >
              <Select
                placeholder={batchSelectedModel
                  ? `已选择：${availableModels.find(m => m.value === batchSelectedModel)?.label || batchSelectedModel}`
                  : '请选择模型'}
                value={batchSelectedModel}
                onChange={setBatchSelectedModel}
                allowClear
                showSearch
                optionFilterProp="label"
              >
                {availableModels.map(model => (
                  <Select.Option key={model.value} value={model.value} label={model.label}>
                    {model.label}
                  </Select.Option>
                ))}
              </Select>
            </Form.Item>



              <Form.Item

                label="开启质量分析"

                name="enableAnalysis"

                tooltip="开启后将对生成内容进行质量分析与建议"

                style={{ marginBottom: 12 }}

              >

                <Radio.Group>

                  <Radio value={true}>

                    <span style={{ fontSize: 12, color: '#52c41a' }}>开启（生成后自动分析）</span>

                  </Radio>

                  <Radio value={false}>

                    <span style={{ fontSize: 12, color: '#8c8c8c' }}>关闭（不做分析）</span>

                  </Radio>

                </Radio.Group>

              </Form.Item>

            </div>

          </Form>

        ) : (

          <div>

            <Alert

              message="批量生成进行中说明"

              description={

                <ul style={{ margin: '8px 0 0 0', paddingLeft: 20 }}>

                  <li>系统会按章节顺序依次生成，并在完成后自动更新状态。</li>

                  <li>生成过程中可以关闭此窗口，任务会继续在后台执行。</li>

                  <li>如需停止任务，可点击下方“取消生成”。</li>

                  {batchProgress?.estimated_time_minutes && batchProgress.completed === 0 && (

                    <li>预计耗时约 {batchProgress.estimated_time_minutes} 分钟。</li>

                  )}

                  {batchProgress?.quality_metrics_summary?.avg_overall_score !== undefined && (

                    <li>

                      当前平均质量分 {batchProgress.quality_metrics_summary.avg_overall_score}，冲突链 / 规则锚定 / 开篇钩子 / 回收链 / 悬念收尾分别为 {batchProgress.quality_metrics_summary.avg_conflict_chain_hit_rate}% / {batchProgress.quality_metrics_summary.avg_rule_grounding_hit_rate}% / {batchProgress.quality_metrics_summary.avg_opening_hook_rate ?? 0}% / {batchProgress.quality_metrics_summary.avg_payoff_chain_rate ?? 0}% / {batchProgress.quality_metrics_summary.avg_cliffhanger_rate ?? 0}%。

                    </li>

                  )}

                </ul>

              }

              type="info"

              showIcon

              style={{ marginBottom: 16 }}

            />



            {batchProgress?.quality_profile_summary && getQualityProfileDisplayItems(batchProgress.quality_profile_summary).length > 0 && (

              <Card size="small" title="质量画像摘要" style={{ marginBottom: 16 }}>

                <Alert

                  type="success"

                  showIcon

                  style={{ marginBottom: 12 }}

                  message="质量画像摘要"

                  description="请参考以下建议。"

                />

                <Descriptions column={1} size="small">

                  {getQualityProfileDisplayItems(batchProgress.quality_profile_summary).map((item) => (

                    <Descriptions.Item key={item.key} label={item.label}>

                      {item.description}

                    </Descriptions.Item>

                  ))}

                </Descriptions>

              </Card>

            )}



            {batchProgress?.quality_metrics_summary?.avg_overall_score !== undefined && (
              <Card size="small" title="质量指标摘要" style={{ marginBottom: 16 }}>
                {batchAfterScorecard && (
                  <Alert
                    type={batchAfterScorecard.verdictColor as 'success' | 'info' | 'warning' | 'error'}
                    showIcon
                    style={{ marginBottom: 12 }}
                    message={batchAfterScorecard.verdict}
                    description={batchAfterScorecard.summary + ' ' + batchAfterScorecard.nextAction}
                  />
                )}
                <Space direction="vertical" style={{ width: '100%' }} size={10}>
                  {getBatchSummaryMetricItems(batchProgress.quality_metrics_summary).map((item) => (
                    <div key={item.key}>
                      <div style={{ display: 'flex', justifyContent: 'space-between', marginBottom: 4, gap: 12 }}>

                        <Space size={4}>

                          <span>{item.label}</span>

                          <Tooltip title={item.tip}>

                            <InfoCircleOutlined style={{ color: '#8c8c8c' }} />

                          </Tooltip>

                        </Space>

                        <span style={{ color: '#595959' }}>{item.value}%</span>

                      </div>

                      <Progress percent={item.value} showInfo={false} size="small" strokeColor={getMetricStrokeColor(item.value)} />

                    </div>

                  ))}

                </Space>

              </Card>

            )}



            <div style={{ textAlign: 'center' }}>

              <Button

                danger

                icon={<StopOutlined />}

                onClick={() => {

                  modal.confirm({

                    title: '确认取消批量生成？',

                    content: '取消后当前批量生成任务将停止，已生成的章节会保留。确定要取消吗？',

                    okText: '确认取消',

                    cancelText: '继续生成',

                    okButtonProps: { danger: true },

                    onOk: handleCancelBatchGenerate,

                  });

                }}

              >

                取消批量生成

              </Button>

            </div>

          </div>

        )}

      </Modal>




      {isGenerating ? (

        <Suspense fallback={null}>

          <LazySSELoadingOverlay

            loading={isGenerating}

            progress={singleChapterProgress}

            message={singleChapterProgressMessage}

            blocking={false}

          />

        </Suspense>

      ) : null}




      {batchGenerating ? (

        <Suspense fallback={null}>

          <LazySSEProgressModal

            visible={batchGenerating}

            progress={batchProgress ? Math.round((batchProgress.completed / batchProgress.total) * 100) : 0}

            message={

              batchProgress?.current_chapter_number

                ? `正在生成第 ${batchProgress.current_chapter_number} 章... (${batchProgress.completed}/${batchProgress.total})${

                    batchProgress.latest_quality_metrics?.overall_score !== undefined

                      ? ` 质量分：${batchProgress.latest_quality_metrics.overall_score}`

                      : ''

                  }`

                : `批量生成进行中... (${batchProgress?.completed || 0}/${batchProgress?.total || 0})${

                    batchProgress?.latest_quality_metrics?.overall_score !== undefined

                      ? ` 质量分：${batchProgress.latest_quality_metrics.overall_score}`

                      : ''

                  }`

            }

            title="批量生成进度"

            onCancel={() => {

              modal.confirm({

                title: '确认取消批量生成？',

                content: '取消后当前批量生成任务将停止，已生成的章节会保留。确定要取消吗？',

                okText: '确认取消',

                cancelText: '继续生成',

                okButtonProps: { danger: true },

                centered: true,

                onOk: handleCancelBatchGenerate,

              });

            }}

            cancelButtonText="取消生成"

            blocking={false}

          />

        </Suspense>

      ) : null}



      <FloatButton

        icon={<BookOutlined />}

        type="primary"

        tooltip="打开章节目录"

        onClick={() => setIsIndexPanelVisible(true)}

        style={{ right: isMobile ? 24 : 48, bottom: isMobile ? 80 : 48 }}

      />



      <FloatingIndexPanel

        visible={isIndexPanelVisible}

        onClose={() => setIsIndexPanelVisible(false)}

        groupedChapters={groupedChapters}

        onChapterSelect={handleChapterSelect}

      />




      {readingChapter && (

        <ChapterReader

          visible={readerVisible}

          chapter={readingChapter}

          onClose={() => {

            setReaderVisible(false);

            setReadingChapter(null);

          }}

          onChapterChange={handleReaderChapterChange}

        />

      )}




      {editingId && (

        <PartialRegenerateModal

          visible={partialRegenerateModalVisible}

          chapterId={editingId}

          selectedText={selectedTextForRegenerate}

          startPosition={selectionStartPosition}

          endPosition={selectionEndPosition}

          styleId={selectedStyleId}

          onClose={() => setPartialRegenerateModalVisible(false)}

          onApply={handleApplyPartialRegenerate}

        />

      )}




      {editingPlanChapter && currentProject && (() => {

        let parsedPlanData = null;

        try {

          if (editingPlanChapter.expansion_plan) {

            parsedPlanData = JSON.parse(editingPlanChapter.expansion_plan);

          }

        } catch (error) {

          console.error('闁荤喐鐟辩徊楣冩倵閻ｅ本鍠嗛柛鏇ㄥ亜閻忓﹪鏌℃担鍝勵暭鐎规挷鐒﹀鍕綇椤愩儛?', error);

        }



        return (

          <ExpansionPlanEditor

            visible={planEditorVisible}

            planData={parsedPlanData}

            chapterSummary={editingPlanChapter.summary || null}

            projectId={currentProject.id}

            onSave={handleSavePlan}

            onCancel={() => {

              setPlanEditorVisible(false);

              setEditingPlanChapter(null);

            }}

          />

        );

      })()}

    </div>
    </>
  );

}


