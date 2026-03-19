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
    label: '閻庢鍠掗崑鎾绘煕閿旀儳鍔嬪┑顔煎楠?',
    placeholder: '闁哄鏅滈悷銈囩博鐎靛摜鍗氶柣妯挎珪娴犳﹢鏌涜箛鎾跺濠殿喖绻樺畷娆撴惞鐟欏嫮鏆犻悗娈垮枛閸婃悂鎮ラ崼鏇炍ュù锝夋敱瀹曟煡鏌涢弬鍛閸嬫挻鎷呯粙璺ㄣ偒闂傚倸瀚ч弲婊堝垂閵娾晛绠氶柛娑卞灠瀹曟洟鏌熺紒銏犲闁?',
  },
  {
    key: 'chapterGoal',
    label: '闂佸搫鐗滈崜娑㈡偟閻戣姤鍎庢い鏃傛櫕閸?',
    placeholder: '闁荤喐鐟︾敮鐔哥珶婵犲啯浜ゆ繛鍡楁捣椤忚京绱掗弮鍌毿㈢紒鐑╁亾婵＄偑鍊楃划顖滄暜鐟欏嫭浜ゆ繛鎴炵懃閻忔鏌熷畡鎵虎闁糕晛鏈粋鎺楀焵椤掍胶鈻?',
  },
  {
    key: 'conflictPressure',
    label: '闂佸憡鍔樼亸娆撴偘婵犲洤绀夐柣妯诲絻缁?',
    placeholder: '闂佸搫鐗滈崜娑㈡偟闁垮鈻旈柡鍕禋濞诧綁姊婚崘顓у殭濠殿喖绻樻俊瀛樻媴缁涘娈╅梺鍛婂竾閸婃垿鍩€椤戞寧顦烽柛鐑嗗墯缁傛帡鏌ㄧ€ｎ亞浠愭繝銏ｅ煐閻楃娀宕曢幘顔煎偍閻庯綆鍘借ぐ銉╂煛婢跺牆鍔ラ柛銈嗙矒瀹曨偊顢旈崼婵囶仦',
  },
  {
    key: 'turningPoint',
    label: '婵炴垶鎼╅崢浠嬵敊鐏炵偓濮滄い鎺嶇椤?',
    placeholder: '婵炴垶鎼╅崢浠嬵敊瀹€鍕煑闁瑰濮甸弲绋棵归悩顐壕婵炴垶鏌ㄩ悧鍡氥亹婢舵劕绀岄柡宥囨暩缁€澶愭偣娴ｅ弶娅呴柣顏嶅墴瀹曟繈宕归鑲╋紦缂備胶瀚忔担鎻掍壕濞达絿顭堥崘鈧柡澶屽剱閸撴岸宕归妸鈺佹辈闁圭虎鍠楅懟鐔兼煟椤忓棗鏋旀繛?',
  },
  {
    key: 'endingHook',
    label: '缂傚倷鐒﹂幐鎼佹偄椤掑嫭鐓㈤柍杞拌兌閹?',
    placeholder: '缂備焦姊绘慨鎾偄椤掑嫭鍋╂繛鍡楁捣閻熸挸霉閻橆偄浜炬繛鎴炴煥閻楀﹪宕戦幘鍦杸闁绘劕鍘滈崑鎾存媴妞嬪海鈻忓┑鐐差槶閸ㄦ椽宕归妸锔锯枖閻庯絺鏅濋杈╃磼閺冨倸鈻堥柍鐟扮Ч瀹曟繈濡搁妷銉綕',
  },
];

const STORY_SCENE_OUTLINE_FIELDS: Array<{
  key: keyof StorySceneOutlineDraft;
  label: string;
  placeholder: string;
}> = [
  {
    key: 'setupScene',
    label: '闂侀潻濡囬崕銈呪枍?1闂佹寧绋掓穱娲箲閿濆绀夐柛顭戝枟缁?',
    placeholder: '闁哄鏅滈悷銈囩博閹绢喖鎹堕柛婵嗗鐢儵鏌熺捄鐚撮練闁汇劍绻堥獮鎺楀Ω閵堝洨鎲梺鍛婄懆閸╁洭鍩€椤戣法鍔嶅┑顔肩箻瀹曟瑦娼幍顔剧劶婵炴垶鏌ㄩ悧鍡欐閹捐埖鏆滈柛娑橈工閻忔霉閻樹警鍤欏┑?',
  },
  {
    key: 'confrontationScene',
    label: '闂侀潻濡囬崕銈呪枍?2闂佹寧绋掗懝楣冾敋椤曗偓楠炲骸螖閳ь剙鈹冮埀?',
    placeholder: '闂佸憡鍔樼亸娆撴偘婵犲啩绻嗛柛灞剧懅缁夊潡鏌涘Δ鈧ú銊︻殽閸ヮ剚鏅€光偓閸愮偓鍋ラ梺鑹邦潐瑜板啫锕㈤鍫濅紶妞ゅ繐鐗嗗▍锟犳煕濞嗘劦娈旈悽顖氭喘楠炲寮介鈶跨喖姊?',
  },
  {
    key: 'reversalScene',
    label: '闂侀潻濡囬崕銈呪枍?3闂佹寧绋掓穱鍝劽归崱娑樼婵☆垳鍎ょ花?',
    placeholder: '闁诲繒鍋愰崑鎾绘煕閺冣偓鐎笛囧焵椤掆偓鐎涒晠鎮ч柆宥呯煑鐎广儱鐗婄粊顕€鏌曢崱鏇犲妽婵絾宀稿Λ浣轰沪閸屾浜惧ù锝囩摂閸ゆ牠鎮楅棃娑樻倯闁搞劊鍔戝畷锝呂熼崹顔剧崺',
  },
  {
    key: 'payoffScene',
    label: '闂侀潻濡囬崕銈呪枍?4闂佹寧绋掔喊宥夊极瑜版帒绾ч柣鏃堟敱缁?',
    placeholder: '闁哄鏅滈悷銈囩博鐎靛摜鍗氶柣妯烘▕濞层倕霉閿濆棙绀冮柡浣告贡娴滄悂骞橀崨顖滎槷濡ょ姷鍋犻崺鏍ㄤ繆閸濄儲瀚氶柡鍕箚閸嬫捇宕ㄩ鑹板悅闂佸憡纰嶉崹宕囩箔閸涱喚鈻旈柍褜鍓涚划?',
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
    ? 'Batch story beat planner (fill in items)'
    : 'Story beat planner (fill in items)';
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
    ? 'Batch story scene outline (fill in items)'
    : 'Story scene outline (fill in items)';
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
  parts.summary?.trim() ? 'Summary' : '',
  parts.beat?.trim() ? 'Beat Planner' : '',
  parts.scene?.trim() ? 'Scene Outline' : '',
].filter(Boolean);




// localStorage 缂傚倸鍊归幐鎼佹偤閵娾晜鐓ユい鏂垮悑閸?

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
  conflict: 'Conflict chain coverage.',
  rule: 'Rule grounding.',
  opening: 'Opening hook.',
  payoff: 'Payoff chain.',
  cliffhanger: 'Cliffhanger strength.',
  dialogue: 'Dialogue naturalness.',
  outline: 'Outline alignment.',
};


const CREATIVE_MODE_OPTIONS: Array<{ value: CreativeMode; label: string; description: string }> = [
  { value: 'balanced', label: 'Balanced', description: 'A balanced mix of story beats.' },
  { value: 'hook', label: 'Hook', description: 'Emphasize the opening hook.' },
  { value: 'emotion', label: 'Emotion', description: 'Emphasize emotional resonance.' },
  { value: 'suspense', label: 'Suspense', description: 'Increase suspense and tension.' },
  { value: 'relationship', label: 'Relationship', description: 'Focus on relationship dynamics.' },
  { value: 'payoff', label: 'Payoff', description: 'Strengthen payoff and resolution.' },
];


const STORY_FOCUS_OPTIONS: Array<{ value: StoryFocus; label: string; description: string }> = [
  { value: 'advance_plot', label: 'Advance plot', description: 'Move the plot forward.' },
  { value: 'deepen_character', label: 'Deepen character', description: 'Develop character depth.' },
  { value: 'escalate_conflict', label: 'Escalate conflict', description: 'Increase stakes and conflict.' },
  { value: 'reveal_mystery', label: 'Reveal mystery', description: 'Reveal new information or clues.' },
  { value: 'relationship_shift', label: 'Relationship shift', description: 'Shift relationships or alliances.' },
  { value: 'foreshadow_payoff', label: 'Foreshadow payoff', description: 'Plant setup for future payoff.' },
];



const getWeakestQualityMetric = (metrics: ChapterQualityMetrics): { label: string; value: number } => {
  const items = [
    { label: 'Conflict', value: metrics.conflict_chain_hit_rate },
    { label: 'Rule', value: metrics.rule_grounding_hit_rate },
    { label: 'Opening', value: metrics.opening_hook_rate },
    { label: 'Payoff', value: metrics.payoff_chain_rate },
    { label: 'Cliffhanger', value: metrics.cliffhanger_rate },
    { label: 'Dialogue', value: metrics.dialogue_naturalness_rate },
    { label: 'Outline', value: metrics.outline_alignment_rate },
  ];
  return items.reduce((min, item) => (item.value < min.value ? item : min), items[0]);
};




const getQualityMetricItems = (metrics: ChapterQualityMetrics) => [
  { key: 'conflict', label: 'Conflict', value: metrics.conflict_chain_hit_rate, tip: QUALITY_METRIC_TIPS.conflict },
  { key: 'rule', label: 'Rule', value: metrics.rule_grounding_hit_rate, tip: QUALITY_METRIC_TIPS.rule },
  { key: 'opening', label: 'Opening', value: metrics.opening_hook_rate, tip: QUALITY_METRIC_TIPS.opening },
  { key: 'payoff', label: 'Payoff', value: metrics.payoff_chain_rate, tip: QUALITY_METRIC_TIPS.payoff },
  { key: 'cliffhanger', label: 'Cliffhanger', value: metrics.cliffhanger_rate, tip: QUALITY_METRIC_TIPS.cliffhanger },
  { key: 'dialogue', label: 'Dialogue', value: metrics.dialogue_naturalness_rate, tip: QUALITY_METRIC_TIPS.dialogue },
  { key: 'outline', label: 'Outline', value: metrics.outline_alignment_rate, tip: QUALITY_METRIC_TIPS.outline },
];




const getBatchSummaryMetricItems = (summary?: {
  avg_conflict_chain_hit_rate?: number;
  avg_rule_grounding_hit_rate?: number;
  avg_opening_hook_rate?: number;
  avg_payoff_chain_rate?: number;
  avg_cliffhanger_rate?: number;
}) => [
  { key: 'conflict', label: 'Conflict', value: summary?.avg_conflict_chain_hit_rate ?? 0, tip: QUALITY_METRIC_TIPS.conflict },
  { key: 'rule', label: 'Rule', value: summary?.avg_rule_grounding_hit_rate ?? 0, tip: QUALITY_METRIC_TIPS.rule },
  { key: 'opening', label: 'Opening', value: summary?.avg_opening_hook_rate ?? 0, tip: QUALITY_METRIC_TIPS.opening },
  { key: 'payoff', label: 'Payoff', value: summary?.avg_payoff_chain_rate ?? 0, tip: QUALITY_METRIC_TIPS.payoff },
  { key: 'cliffhanger', label: 'Cliffhanger', value: summary?.avg_cliffhanger_rate ?? 0, tip: QUALITY_METRIC_TIPS.cliffhanger },
];




const QUALITY_PROFILE_BLOCK_ORDER: Array<keyof Pick<ChapterQualityProfileSummary, 'generation' | 'checker' | 'reviser' | 'mcp_guard' | 'external_assets_block'>> = [

  'generation',

  'checker',

  'reviser',

  'mcp_guard',

  'external_assets_block',

];



const QUALITY_PROFILE_BLOCK_LABELS: Record<typeof QUALITY_PROFILE_BLOCK_ORDER[number], string> = {
  generation: 'Generation',
  checker: 'Checker',
  reviser: 'Reviser',
  mcp_guard: 'MCP guard',
  external_assets_block: 'External assets',
};




const getQualityProfileDisplayItems = (summary?: ChapterQualityProfileSummary | null) => {
  if (!summary) {
    return [];
  }

  const items: Array<{ key: string; label: string; description: string }> = [];

  if (summary.baseline_id) {
    items.push({ key: 'baseline', label: 'Baseline', description: summary.baseline_id });
  }

  if (summary.version) {
    items.push({ key: 'version', label: 'Version', description: summary.version });
  }

  if (summary.style_profile) {
    items.push({ key: 'style', label: 'Style', description: summary.style_profile });
  }

  if (summary.genre_profiles?.length) {
    items.push({ key: 'genres', label: 'Genres', description: summary.genre_profiles.join(' / ') });
  }

  if (summary.quality_dimensions?.length) {
    items.push({ key: 'dimensions', label: 'Dimensions', description: summary.quality_dimensions.join(' / ') });
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



// 婵烇絽娲︾换鍌炴偤閵娧€鍋撳☉娆樻畷闁哄棛鍠栧畷?localStorage

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
        ? '閻㈢喐鍨氶崜宥囨殌濡?'
        : '閹靛濮╄箛顐ゅ弾',
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
    labels.push('Brief');
  }

  if (!areStoryBeatPlannerDraftsEqual(snapshot.beatPlannerDraft, currentDraft.beatPlannerDraft)) {
    labels.push('Beat Planner');
  }

  if (!areStorySceneOutlineDraftsEqual(snapshot.sceneOutlineDraft, currentDraft.sceneOutlineDraft)) {
    labels.push('Scene Outline');
  }

  if (!areStoryCreationDraftMetaFieldsEqual(snapshot, currentDraft, { includeNarrativePerspective })) {
    labels.push('Settings');
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
          <div style={{ fontWeight: 600, marginBottom: 4 }}>{'Snapshots'}</div>
          <div style={{ color: 'var(--color-text-secondary)', fontSize: 12 }}>{description}</div>
        </div>
        <Space size={[8, 8]} wrap>
          {snapshots.length > 0 && <Tag color="purple">{`Total: ${snapshots.length}`}</Tag>}
          <Button size="small" onClick={onSave} disabled={!canSave}>
            Save snapshot
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
                      {snapshot.reason === 'manual' ? 'Manual' : 'Auto'}
                    </Tag>
                    <Tag color={(snapshot.promptCharCount ?? 0) >= STORY_CREATION_PROMPT_WARN_THRESHOLD ? 'gold' : 'blue'}>
                      {`Chars: ${snapshot.promptCharCount ?? 0}`}
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
                    ? 'Includes prompt text for reuse.'
                    : 'No prompt text saved for this snapshot.'}
                </div>
                <Space wrap size={[8, 8]}>
                  <Button size="small" onClick={() => onApply(snapshot)}>
                    Apply
                  </Button>
                  <Button
                    size="small"
                    type="link"
                    disabled={!snapshot.prompt}
                    onClick={() => void onCopy(snapshot.prompt, scopeLabel)}
                  >
                    Copy
                  </Button>
                  <Popconfirm
                    title="Delete this snapshot?"
                    okText="Delete"
                    cancelText="Cancel"
                    onConfirm={() => onDelete(snapshot.id)}
                  >
                    <Button size="small" type="link" danger>
                      Delete
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

  // 闂佸憡甯掑Λ娆撴倵閼恒儳顩烽悹鍥ㄥ絻椤倝鏌ｅΟ鍨厫闁逞屽厸閼冲爼顢橀幖浣瑰仩?

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



  // 闂傚倸鍟幊鎾活敋娴兼潙闂柕濞у唭锕傛煙?

  const [readerVisible, setReaderVisible] = useState(false);

  const [readingChapter, setReadingChapter] = useState<Chapter | null>(null);



  // 闁荤喐鐟ョ€氼剟宕瑰┑鍫㈢＝闁哄稁鍓涚敮鍡涙煟濡灝鐓愰柍?

  const [planEditorVisible, setPlanEditorVisible] = useState(false);

  const [editingPlanChapter, setEditingPlanChapter] = useState<Chapter | null>(null);



  // 闁诲繒鍋愰崑鎾绘⒑椤斿搫濮傞柛锝嗘倐瀹曟ê鈻庨幋婢箓鏌?

  const [partialRegenerateToolbarVisible, setPartialRegenerateToolbarVisible] = useState(false);

  const [partialRegenerateToolbarPosition, setPartialRegenerateToolbarPosition] = useState({ top: 0, left: 0 });

  const [selectedTextForRegenerate, setSelectedTextForRegenerate] = useState('');

  const [selectionStartPosition, setSelectionStartPosition] = useState(0);

  const [selectionEndPosition, setSelectionEndPosition] = useState(0);

  const [partialRegenerateModalVisible, setPartialRegenerateModalVisible] = useState(false);



  // 闂佸憡顨嗗ú婊堟偟閻戣姤鍤嶉柛灞剧矋閺呮悂鏌熺€涙ê濮岀紒缁樕戦幆鏃堟晜閹灝锕傛煙?

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

  // 闂佸綊娼х紞濠囧闯濞差亝鍋ㄩ柣鏃傤焾閻忓洭鏌ｉ埡鍐剧劸闁告鍥ㄥ亹闁煎摜顣介崑?
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
    setSelectedCreativeMode(undefined);
    setSelectedStoryFocus(undefined);
    setSelectedPlotStage(inferCreationPlotStage({
      chapterNumber: chapterNumber ?? undefined,
      totalChapters: knownStructureChapterCount,
    }));
    setSingleStoryCreationBriefDraft('');
    setSingleStoryBeatPlannerDraft({ ...EMPTY_STORY_BEAT_PLANNER_DRAFT });
    setSingleStorySceneOutlineDraft({ ...EMPTY_STORY_SCENE_OUTLINE_DRAFT });
  }, [knownStructureChapterCount]);

  const resetBatchStoryCreationCockpit = useCallback(() => {
    batchStoryCreationAutoBriefRef.current = '';
    batchStoryBeatPlannerAutoRef.current = { ...EMPTY_STORY_BEAT_PLANNER_DRAFT };
    batchStorySceneOutlineAutoRef.current = { ...EMPTY_STORY_SCENE_OUTLINE_DRAFT };
    setBatchSelectedCreativeMode(undefined);
    setBatchSelectedStoryFocus(undefined);
    setBatchSelectedPlotStage(undefined);
    setBatchStoryCreationBriefDraft('');
    setBatchStoryBeatPlannerDraft({ ...EMPTY_STORY_BEAT_PLANNER_DRAFT });
    setBatchStorySceneOutlineDraft({ ...EMPTY_STORY_SCENE_OUTLINE_DRAFT });
  }, []);

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

  const singleStoryCreationBaseBrief = normalizedSingleStoryCreationBriefDraft || singleSystemStoryCreationBrief || undefined;

  const batchStoryCreationBaseBrief = normalizedBatchStoryCreationBriefDraft || batchSystemStoryCreationBrief || undefined;

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
    && normalizedSingleStoryCreationBriefDraft !== singleSystemStoryCreationBrief.trim(),
  );

  const isBatchStoryCreationBriefCustomized = Boolean(
    normalizedBatchStoryCreationBriefDraft
    && normalizedBatchStoryCreationBriefDraft !== batchSystemStoryCreationBrief.trim(),
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
      : persistedDraft.storyCreationBriefDraft ?? '';
    singleStoryBeatPlannerAutoRef.current = persistedDraft.isBeatPlannerCustomized
      ? { ...EMPTY_STORY_BEAT_PLANNER_DRAFT }
      : normalizeStoryBeatPlannerDraft(persistedDraft.beatPlannerDraft);
    singleStorySceneOutlineAutoRef.current = persistedDraft.isSceneOutlineCustomized
      ? { ...EMPTY_STORY_SCENE_OUTLINE_DRAFT }
      : normalizeStorySceneOutlineDraft(persistedDraft.sceneOutlineDraft);

    setTemporaryNarrativePerspective(persistedDraft.narrativePerspective);
    setSelectedCreativeMode(persistedDraft.creativeMode);
    setSelectedStoryFocus(persistedDraft.storyFocus);
    setSelectedPlotStage(
      persistedDraft.plotStage
      ?? inferCreationPlotStage({
        chapterNumber: currentEditingChapter.chapter_number,
        totalChapters: knownStructureChapterCount,
      }),
    );
    setSingleStoryCreationBriefDraft(persistedDraft.storyCreationBriefDraft ?? '');
    setSingleStoryBeatPlannerDraft(normalizeStoryBeatPlannerDraft(persistedDraft.beatPlannerDraft));
    setSingleStorySceneOutlineDraft(normalizeStorySceneOutlineDraft(persistedDraft.sceneOutlineDraft));
  }, [
    currentEditingChapter?.chapter_number,
    currentEditingChapter?.id,
    knownStructureChapterCount,
    resetSingleStoryCreationCockpit,
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
      : persistedDraft.storyCreationBriefDraft ?? '';
    batchStoryBeatPlannerAutoRef.current = persistedDraft.isBeatPlannerCustomized
      ? { ...EMPTY_STORY_BEAT_PLANNER_DRAFT }
      : normalizeStoryBeatPlannerDraft(persistedDraft.beatPlannerDraft);
    batchStorySceneOutlineAutoRef.current = persistedDraft.isSceneOutlineCustomized
      ? { ...EMPTY_STORY_SCENE_OUTLINE_DRAFT }
      : normalizeStorySceneOutlineDraft(persistedDraft.sceneOutlineDraft);

    setBatchSelectedCreativeMode(persistedDraft.creativeMode);
    setBatchSelectedStoryFocus(persistedDraft.storyFocus);
    setBatchSelectedPlotStage(persistedDraft.plotStage);
    setBatchStoryCreationBriefDraft(persistedDraft.storyCreationBriefDraft ?? '');
    setBatchStoryBeatPlannerDraft(normalizeStoryBeatPlannerDraft(persistedDraft.beatPlannerDraft));
    setBatchStorySceneOutlineDraft(normalizeStorySceneOutlineDraft(persistedDraft.sceneOutlineDraft));
  }, [batchStoryCreationDraftStorageKey, resetBatchStoryCreationCockpit]);

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
        message.warning('???????????????');
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
        message.info('?????????????????????');
      }
      return latestSnapshot;
    }

    const createdAt = new Date().toISOString();
    const chapterLabel = currentEditingChapter.chapter_number ? `?${currentEditingChapter.chapter_number}?` : '????';
    const snapshot: StoryCreationSnapshot = {
      ...singleStoryCreationCurrentDraft,
      id: buildStoryCreationSnapshotId(),
      scope: 'single',
      createdAt,
      updatedAt: createdAt,
      reason,
      label: options?.label?.trim() || `${chapterLabel} ? ${reason === 'generate' ? '?????' : '????'}`,
      prompt: prompt || undefined,
      promptLayerLabels: [...singleStoryCreationPromptLayerLabels],
      promptCharCount: prompt?.length ?? 0,
    };

    const nextSnapshots = persistStoryCreationSnapshot(singleStoryCreationDraftStorageKey, snapshot);
    setSingleStoryCreationSnapshots(nextSnapshots);

    if (!options?.silent) {
      message.success(reason === 'generate' ? '??????????' : '?????????');
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
        message.warning('???????????????');
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
        message.info('?????????????????????');
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
      label: options?.label?.trim() || `???? ? ${reason === 'generate' ? '?????' : '????'}`,
      prompt: prompt || undefined,
      promptLayerLabels: [...batchStoryCreationPromptLayerLabels],
      promptCharCount: prompt?.length ?? 0,
    };

    const nextSnapshots = persistStoryCreationSnapshot(batchStoryCreationDraftStorageKey, snapshot);
    setBatchStoryCreationSnapshots(nextSnapshots);

    if (!options?.silent) {
      message.success(reason === 'generate' ? '??????????' : '?????????');
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
    message.success(`??????${snapshot.label}`);
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
    message.success(`Loaded snapshot: ${snapshot.label}`);
  }, []);

  const deleteSingleStoryCreationSnapshot = useCallback((snapshotId: string) => {
    if (!singleStoryCreationDraftStorageKey) {
      return;
    }

    const nextSnapshots = removePersistedStoryCreationSnapshot(singleStoryCreationDraftStorageKey, snapshotId);
    setSingleStoryCreationSnapshots(nextSnapshots);
    message.success('Snapshot deleted.');
  }, [singleStoryCreationDraftStorageKey]);

  const deleteBatchStoryCreationSnapshot = useCallback((snapshotId: string) => {
    if (!batchStoryCreationDraftStorageKey) {
      return;
    }

    const nextSnapshots = removePersistedStoryCreationSnapshot(batchStoryCreationDraftStorageKey, snapshotId);
    setBatchStoryCreationSnapshots(nextSnapshots);
    message.success('Snapshot deleted.');
  }, [batchStoryCreationDraftStorageKey]);

  const copyStoryCreationPrompt = useCallback(async (
    content: string | undefined,
    scopeLabel: 'single' | 'batch',
  ) => {
    const normalizedContent = content?.trim();
    if (!normalizedContent) {
      message.warning(`No prompt content to copy for ${scopeLabel}.`);

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

      message.success(`Copied ${scopeLabel} prompt to clipboard.`);
    } catch (error) {
      console.error('Failed to copy prompt.', error);
      message.error('Copy failed. Please try again.');
    }
  }, []);
  useEffect(() => {
    const previousAutoBrief = singleStoryCreationAutoBriefRef.current;

    if (!singleSystemStoryCreationBrief) {
      singleStoryCreationAutoBriefRef.current = '';
      setSingleStoryCreationBriefDraft(prev => (prev ? '' : prev));
      return;
    }

    setSingleStoryCreationBriefDraft(prev => {
      if (!prev.trim() || prev === previousAutoBrief) {
        return singleSystemStoryCreationBrief;
      }

      return prev;
    });

    singleStoryCreationAutoBriefRef.current = singleSystemStoryCreationBrief;
  }, [singleSystemStoryCreationBrief]);

  useEffect(() => {
    const previousAutoBrief = batchStoryCreationAutoBriefRef.current;

    if (!batchSystemStoryCreationBrief) {
      batchStoryCreationAutoBriefRef.current = '';
      setBatchStoryCreationBriefDraft(prev => (prev ? '' : prev));
      return;
    }

    setBatchStoryCreationBriefDraft(prev => {
      if (!prev.trim() || prev === previousAutoBrief) {
        return batchSystemStoryCreationBrief;
      }

      return prev;
    });

    batchStoryCreationAutoBriefRef.current = batchSystemStoryCreationBrief;
  }, [batchSystemStoryCreationBrief]);

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



  // 婵犮垼娉涚€氼噣骞冩繝鍥ф闁搞儯鍔嶉幏閬嶆⒑椤愩埄妲烽柤?- 濠碘槅鍋€閸嬫挻绻涢弶鎴剶闁逞屽墮椤︻噣鎳欓幋锕€妫橀柛銉ｅ妽閹疯鲸顨ラ悙璺虹厫婵☆垰顦辩划鍫ユ倻濡法妾ㄩ梺鍛婃煟閸斿本瀵奸幇鏉跨闂佸灝顑囬崺?

  const handleTextSelection = useCallback(() => {

    // 闂佸憡鐟禍婊冿耿椤忓棛纾介柡宥庡墰鐢棝鏌涢敐鍐ㄥ濠⒀嶇畱椤曪綁鍩€椤掑嫬绫嶉悹杞拌濡查亶鏌ｉ悙鍙夘棦闁逞屽墮椤︻噣鎳?

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

    

    // 闂佺厧鍢查崯鍧楁儍椤栫偞鐒诲璺侯槼閸?0婵炴垶鎼╂禍婊堟偤瑜忕划顓㈡晜閽樺鏋€闂佸搫瀚晶浠嬪Φ濮橆剦鍟呴柕澶堝劚瀵版棃鏌?

    if (selectedText.length < 10) {

      setPartialRegenerateToolbarVisible(false);

      return;

    }



    // 濠碘槅鍋€閸嬫捇鏌＄仦璇插姦闁逞屽墮椤︻噣鎳欓幋锕€鍙婃い鏍ㄧ閸庡﹪鏌?TextArea 闂?

    const textArea = contentTextAreaRef.current?.resizableTextArea?.textArea;

    if (!textArea) {

      setPartialRegenerateToolbarVisible(false);

      return;

    }

    

    // 濠碘槅鍋€閸嬫捇鏌＄仦璇插姦闁逞屽墮椤︻噣鎳欓幋锕€鍙婃い鏍ㄧ閸庡﹪鏌?textarea 闂佸憡鍔曢幏鎴犳濞嗘挻顥嗛柍褜鍓涢幉鐗堟媴閸濆嫷妫楀┑鐐茬墕閿曘倝藝閳哄懏鍋犻柛鈽嗗幘缁€澶愭煕閵壯冃￠悹?textarea 闂佹眹鍔岀€氫即鍩€椤掆偓椤︻噣鎳欓幋鐐碘枖鐎广儱瀚粣妤呮煕閹烘挾鈽夌紓?range闂?

    if (document.activeElement !== textArea) {

      setPartialRegenerateToolbarVisible(false);

      return;

    }



    // 闂佸吋鍎抽崲鑼躲亹?textarea 婵炴垶鎼╅崢鎯р枔閹达附鐒诲璺侯槼閸橆剙霉閿濆懐肖闁?

    const start = textArea.selectionStart;

    const end = textArea.selectionEnd;

    const textContent = textArea.value;

    const selectedInTextArea = textContent.substring(start, end);



    if (selectedInTextArea.trim().length < 10) {

      setPartialRegenerateToolbarVisible(false);

      return;

    }



    // 闁荤姳绶ょ槐鏇㈡偩鐠囪褰掝敊閻撳巩妤冣偓瑙勬偠閸庨亶宕ｉ崸妤€鍐€闊洤娴风粔瀵哥磽?

    const rect = textArea.getBoundingClientRect();

    const computedStyle = window.getComputedStyle(textArea);

    const lineHeight = parseFloat(computedStyle.lineHeight) || 24;

    const paddingTop = parseFloat(computedStyle.paddingTop) || 0;

    

    // 闁荤姳绶ょ槐鏇㈡偩婵犳碍鐒诲璺侯槼閸橆剟鏌￠崒姘婵犫偓閹殿喗灏庣€瑰嫰鍋婂妤€霉閿濆懐肖闁汇倕妫濋獮宥夊焵椤掑嫬鎹堕柕濞у嫮鏆犻柣鐐寸☉閼活垵銇?

    const textBeforeSelection = textContent.substring(0, start);

    const startLine = textBeforeSelection.split('\n').length - 1;

    

    // 闁荤姳绶ょ槐鏇㈡偩婵犳碍鐒诲璺侯槼閸橆剟鏌￠崒姘婵犫偓娴兼潙鎹?textarea 婵炴垶鎼╅崢鎯р枔閹寸姵鍠嗛柛鈩冨嚬濞兼洖霉閿濆懐肖闁?

    // 闂傚倸娲犻崑鎾绘偡閺囨碍绁伴柍褜鍓欓崯鍐差瀶?scrollTop闂佹寧绋戝鍗恱tarea 闂佸憡鍔曢幊姗€宕曢弶鎴叆婵﹩鍓欒闂佺顑呯换鎺嶇昂闂?

    const scrollTop = textArea.scrollTop;

    const visualTop = (startLine * lineHeight) + paddingTop - scrollTop;

    

    // 閻庤鎮堕崕閬嶅矗閸ф鍐€闊洤娴风粔瀵哥磽閸愭儳娅欑紒杈╂疄extarea 婵＄偑鍊曢悥濂稿磿?+ 闂備緡鍋勯ˇ顕€鎳欓幋锕€妫橀柛銉ｅ妽閹烽亶鏌ｉ妸銉ヮ伂妞ゎ偄顑囬幉瀛樺緞婢跺瞼孝缂?- 閻庤鎮堕崕閬嶅矗閸ф鍐€闊洦绋撹ぐ顖炲箹鏉堝墽鐣卞ù婊冩憸缁?

    const toolbarTop = rect.top + visualTop - 45;

    

    // 濠殿喗蓱濞兼瑩鏌﹂埡鍌涘鐎广儱娲ㄩ弸鍌炴煥濞戞瑧顣查柡鍌欑窔瀹?textarea 闂佹眹鍔岀€氼剝銇愰崨濠勭懝鐟滃秶浜搁鐐叉槬闁绘洖鍊荤粈澶愭⒑椤掆偓閻忔繈宕㈤妶澶嬬劶妞ゆ棁妫勯惃锟犳煛閸屾碍澶勬繝鈧?

    const toolbarLeft = rect.right - 180;



    setSelectedTextForRegenerate(selectedInTextArea);

    setSelectionStartPosition(start);

    setSelectionEndPosition(end);

    

    // 闁荤姳绶ょ槐鏇㈡偩鐠囧樊鍟呴柕澶堝劚瀵版棃鏌″鍛缂傚秴鎳愮槐鏃堫敋閸℃瑧顦繝纰樷偓鍐测偓褰掓倶婢舵劖鐒诲璺侯槼閸橆剙霉閿濆懐肖闁汇倕妫欑粙澶婎吋閸涱喛鍚梺鍛婄懐閸ㄧ敻锝炵€ｎ喖绀岄柛婵嗗閸樼敻鏌涢幇顒佸珔缂佽鲸绻堝畷鍫曞传閸曨厽姣庨梺闈╄礋閸斿繒绮╅悢铏圭＝?

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



  // 闂佸搫娲ら悺銊╁蓟婵犲偆鍟呴柕澶堝劚瀵版棃鏌″鍛缂傚秴鎳愮槐鏃堫敊閻愵剛鏆犻梺鍛婂灱婵倝寮抽悢鍏兼櫖闁割偅绮庨悷婵囦繆椤愮喎浜惧┑鐐存綑椤戝鍩€椤掆偓椤︻噣鎳欓幋锔芥櫖閻忕偠妫勫☉褔鏌￠崶褏鎽犻柡灞斤攻閹峰懎顓奸崶鈺傜€梺?

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

    // 闂佹悶鍎遍幖顐︽偩妤ｅ啫鎹?textarea 闂佸憡鐟ラ崢鏍箔閸屾粍鍠嗛柟铏瑰仧缁€澶娾槈閹惧磭效婵炲牄鍨介弻鍛緞婢跺骸骞€婵炶揪绲界粔鍫曟偪閸℃稑鐭楁俊顖氭惈椤?

    const toolbarLeft = rect.right - 180;

    

    // 閻庤鎮堕崕閬嶅矗閸ф鍐€闊洦鎸荤粊濂告倵鐟欏嫯澹樻繝鈧?textarea 闂佸憡鐟崹鐢革綖鐎ｎ喖绀岄柛婵嗗閸樼敻鏌涢幇顒佸珔缂佽鲸绻堝畷锟犲礂閸涱厸鏋忛梻渚囧亜椤︻噣鎳欓幋锕€妫橀柛銉ｅ妽閹疯鲸绻濇繝鍐闁搞値鍘鹃幉鎾幢濞戞ɑ顏犳繛鎴炴⒒閸犲秶鎹㈠璺虹濞达綀顫夐埢鏃傜磼閳?

    // 婵犵鈧啿鈧綊鎮樻径鎰劵濠㈣泛顦抽崢顒€霉閿濆懐肖闁汇倕妫濆畷鐑藉Ω閵夈儴顔夐柣鐔哥懃濡浜搁鐐叉槬闁绘柨鍢查弫鍫曟煥濞戞鐒风紒鎰剁節濮婃崘绠涘☉鎺戜壕濠㈣泛顦抽崢顒€霉閿濆懐肖闁?

    // 婵犵鈧啿鈧綊鎮樻径濠庣叆婵﹩鍓欏В澶愭偡濞嗗繑顥㈤柛锝呯秺閺佸秶浠﹂懞銉с偧闁诲氦顫夐懝鎯э耿椤忓懌浜滈柛顐ｆ礀閸斻儵鏌熺€涙澧紒銊﹀▕閺屽牓濡搁妸褏褰剧紓?

    let finalTop = toolbarTop;

    if (visualTop < 0) {

      // 闂備緡鍋勯ˇ顕€鎳欓幋鐐村鐎广儱娲ㄩ弸鍌炴煕閿斿搫濡虹紒妤€鍊垮顒傛兜閸滀焦缍婇梻浣瑰絻妤犳悂藝婵犳碍鏅悘鐐村灊缁憋綁鏌涜箛娑欐暠闁绘牬鍣ｅ畷鍫曞传閸曨厽姣庨梺闈╄礋閸旀垿濡存繝鍥ㄧ劸?

      finalTop = rect.top + 10;

    } else if (visualTop > textArea.clientHeight) {

      // 闂備緡鍋勯ˇ顕€鎳欓幋鐐村鐎广儱娲ㄩ弸鍌炴煕閿斿搫濡虹紒妤€鎳樺顒傛兜閸滀焦缍婇梻浣瑰絻妤犳悂藝婵犳碍鏅悘鐐村灊缁憋綁鏌涜箛娑欐暠闁绘牬鍣ｅ畷鍫曞传閸曨厽姣庨梺闈╄礋閸斿瞼鑺遍幎鑺ョ劸?

      finalTop = rect.bottom - 50;

    }

    

    setPartialRegenerateToolbarPosition({

      top: Math.max(rect.top + 10, Math.min(finalTop, rect.bottom - 50)),

      left: Math.min(Math.max(rect.left + 20, toolbarLeft), window.innerWidth - 200),

    });

  }, [partialRegenerateToolbarVisible, selectedTextForRegenerate, selectionStartPosition]);



  // 闂佺儵鏅滈崹鐢稿箚婢舵劖鐒诲璺侯槼閸橆剙霉濠婂喚鍎庢繛?

  useEffect(() => {

    if (!isEditorOpen) return;



    const textArea = contentTextAreaRef.current?.resizableTextArea?.textArea;

    if (!textArea) return;



    const handleMouseUp = () => {

      // 婵崿鍛ｉ柣鏍电秮閺屽本绻濋崘鈺傛緬闂佸搫鍟抽崺鏍夐崨鏉戣摕闁靛鏂侀崑鎾村緞婢跺骸骞€

      setTimeout(handleTextSelection, 50);

    };



    const handleKeyUp = (e: KeyboardEvent) => {

      // Shift + 闂佸搫鍊婚幊鎾诲箖濠婂牊鐓ユい鏇楀亾闁逞屽墮椤︻噣鎳欓幋锕€绫嶉柤鍛婎問濮婇箖鏌?

      if (e.shiftKey && ['ArrowLeft', 'ArrowRight', 'ArrowUp', 'ArrowDown'].includes(e.key)) {

        setTimeout(handleTextSelection, 50);

      }

    };



    const handleScroll = () => {

      // 濠电姴锕ラ懝鐐叏閳哄懎绫嶉柤绋跨仛缁绢垶鏌￠崒鐑嗘殥缂傚秴鎳愮槐鏃堫敋閸℃瑧顦╂繛杈剧秬濞夋洟寮?requestAnimationFrame 婵炴潙鍚嬮敋閻庡灚鐓￠獮鈧憸鎴﹀礂濮椻偓閺?

      requestAnimationFrame(updateToolbarPosition);

    };



    // 闂佺儵鏅滈崹鐢稿箚?textarea 濠电姴锕ラ懝鐐叏?

    textArea.addEventListener('mouseup', handleMouseUp);

    textArea.addEventListener('keyup', handleKeyUp);

    textArea.addEventListener('scroll', handleScroll);



    // 闂佸憡鑹鹃張顒€顪冮崒鐐村剮闁瑰瓨绻冮崕?Modal body 濠电姴锕ラ懝鐐叏閳哄懏鏅柛锔绘懓dal 闂佸憡鍔曢幊搴敊閹版澘鐭楁い鏍ㄧ箓閸樻挳鏌涢敂鍝勫妞わ箒宕垫禒锕傚磼濮樼厧鏅ｉ梺闈╃祷閸斿秶鍒掗幘顔肩妞ゎ厽甯炵粈?

    const modalBody = textArea.closest('.ant-modal-body');

    if (modalBody) {

      modalBody.addEventListener('scroll', handleScroll);

    }



    // 闂佺儵鏅滈崹鐢稿箚婢跺瞼鐜绘俊銈傚亾鐟滅増绋掑鍕槻闁活煈鍓熷畷锝呂熼崫鍕靛殭

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

      

      // 婵犵鈧啿鈧綊鎮樻径鎰€烽柣鐔告緲濮ｅ﹪鏌ｉ妸銉ヮ仾婵¤尙顭堥蹇涘Ψ閵夈儱绶梺鍝勭Т妤犲繒妲愬┑鍥┾枖鐎广儱顦伴娲煢?

      if (target.closest('[data-partial-regenerate-toolbar]')) {

        return;

      }

      

      // 婵犵鈧啿鈧綊鎮樻径鎰€烽柣鐔告緲濮ｅ﹪鏌ｉ妸銉ヮ仾婵?textarea闂佹寧绋戞總鏃傜箔婢舵劖鈷曢柟閭﹀灡椤?

      if (target.tagName === 'TEXTAREA') {

        return;

      }

      

      // 婵犵鈧啿鈧綊鎮樻径鎰€烽柣鐔告緲濮ｅ﹪鏌ｉ妸銉ヮ仾婵?Modal 闂佸憡鍔曢幊姗€宕曢幘顔芥櫖闁割偅绻傞惁鍫曟煙婵傚澧紒顔芥尦瀹曟繈濡搁敂鍊熸嫬闂佹寧绋戦¨鈧紒杈ㄧ箖缁嬪顓兼径瀣靛悈闂?

      if (target.closest('.ant-modal-content')) {

        return;

      }

      

      // 闂佺粯鍔楅幊鎾诲吹?Modal 婵犮垼鍩栭悧鐘诲磿閹绢喖绠ョ€广儱顦伴娲煢濡櫣绠板ù鍏煎姍瀹曟鏌ㄧ€ｎ剙鐒?

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



  // 濠电偞鎸搁幊鎰板箖婵犲啯濮滄い鏃€顑欓崵鍕倵鐟欏嫮顣叉俊鐐插€垮畷?

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



  // 闂佸憡姊绘慨鎯归崶顒€绠ラ柍褜鍓熷鍨緞瀹€鈧ぐ鍧楁煠閸濆嫬鈧鈻撻幋锕€绀嗛柛鈩冾焽閳ь剝濮ょ粋鎺旀嫚閹绘帩娼抽梺缁橆焾閸╂牠鍩€?

  // 闂佽浜介崕杈亹閸儱鐭楁い鏍亹閸嬫挻寰勭仦鍓ф殸 chaptersToLoad 闂佸憡鐟ラ崐褰掑汲閻斿吋鏅€光偓閸愬啯甯″畷?React 闂佺粯顭堥崺鏍焵椤戣法鍔嶆繛鎻掓健瀵剟寮堕幐搴仺闁哄鏅濋崰搴敋闁秵鍤婇悗闈涙啞閻ｉ亶姊婚崒銈呮珝妞?

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



  // 闂佸憡鍑归崹鐗堟叏閳哄懎纭€闁哄洦宀搁崵瀣磼閺冨倸鞋濠碘槅鍙冮幆鍐礋椤忓懎搴婇梺鍛婃瀫閵堝洦鐎柣?

  const startPollingTask = (chapterId: string) => {

    // 婵犵鈧啿鈧綊鎮樻径濠庡晠闁肩⒈鍓涢惀鍛存煕閿斿搫濮€闁汇倕妫涢幏鐘伙綖椤斿墽顦梺绋跨箰閻楀﹦绮╂繝姘挃?

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

            message.success('Chapter analysis completed.');

          } else if (task.status === 'failed') {

            message.error(`Chapter analysis failed: ${task.error_message || 'Unknown error'}`);

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

        // 缂備焦姊绘慨鐐繆椤撱垹姹查柛灞剧⊕閿熴儵鏌涢幒鎴烆棡闁诲氦濮ょ粋鎺旀嫚閹绘帩娼抽梺鍝勫暞濠€鍦閹殿喚纾肩憸蹇涙偨閼姐倖鍠嗛柨鏇楀亾鐟滄澘鍊垮畷姘槈濡偐澶?

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

      message.info(`Queued ${queuedCount} chapters for analysis.`);

    } else if (skippedCount > 0 && failedCount === 0) {

      message.info('Skipped chapters that were already analyzed.');

    }



    if (failedCount > 0) {

      message.warning(`${failedCount} chapter analyses failed.`);

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

        message.error('Failed to load writing styles.');

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

      // 婵炲濮村锕傤敊閺囩姷纾鹃柣锛勵檮I闂佸吋鍎抽崲鑼躲亹閸ヮ剚鍋ㄩ柕濠忕畱閻撴洟姊洪弶璺ㄐら柣銈呮閹啴宕熼鐔剁窔瀹曞湱鈧綆浜滈悘娆撴偠?

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

          message.info('Batch generation restored and running.');

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



      // 闂佺粯鍔楅幊鎾诲吹椤曗偓閺屽懎顫濇潏鈺佸绩闂佸搫鍟崕鍏肩濞戙垺鍊块柨鏇楀亾闁糕晛鐬肩划锝呂旈埀顒冦亹?

      notification.onclick = () => {

        window.focus();

        notification.close();

      };



      // 5缂備礁顦扮敮鎺楀箖濡ゅ懏鍤婃い蹇撳琚熼梺绋跨箲婵炲﹤螞?

      setTimeout(() => {

        notification.close();

      }, 5000);

    } else if (Notification.permission !== 'denied') {

      // 婵犵鈧啿鈧綊鎮樻径鎰骇闁告劦鍠楅娆撴煛閸偂娴锋い顐畵瀵増鎯旈敐鍌楀亾濮椻偓楠炲繘骞掗弮鍌氬椽闂佹寧绋戦懟顖炴儍閸撗勫珰闁哄洠妲呴崵鐐存叏閻熸澘鈧懓顭囬崼銉︹挃?

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

          outlineTitle: chapter.outline_title || 'Uncategorized',

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

      first_person: 'First person',
      third_person: 'Third person',
      omniscient: 'Omniscient',
    };

    return texts[perspective || ''] || 'Third person';

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



      // 闂佸憡甯￠弨閬嶅蓟婵犲嫮鍗氶柣妯烘惈铻￠梺鍛婂笚椤ㄥ濡撮崘鈺冾浄闁靛鍔岀粻顖炴煕濞嗘劗澧柣锝庡墴瀵偆鈧潧鎲￠悾杈╃磼閺冨倸鞋濠碘槅鍙冨顐︽偋閸繄銈﹂梺鎸庣☉閻楀棛鈧灚锕㈤獮蹇涙偠缁茬憙line_title缂備焦绋戦ˇ铏閸儱钃熼柕澶堝劤閹界喐绻涢崼銏╂殰缂?

      await refreshChapters();



      message.success('Chapter updated.');

      setIsModalOpen(false);

      form.resetFields();

    } catch {

      message.error('Failed to update chapter.');

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
      setEditingId(id);
      setTemporaryNarrativePerspective(undefined); // 闂備焦褰冪粔鍫曟偪閸℃顩查柧蹇撳ⅲ閻愮儤鐒诲璺侯儏椤?
      setSelectedCreativeMode(undefined);
      setSelectedStoryFocus(undefined);
      setSelectedPlotStage(inferCreationPlotStage({
        chapterNumber: chapter.chapter_number,
        totalChapters: knownStructureChapterCount,
      }));
      setIsEditorOpen(true);
      setChapterQualityMetrics(null);

      setChapterQualityProfileSummary(null);

      setChapterQualityGeneratedAt(null);

      // 闂佺懓鐏氶幐鍝ユ閹寸姷纾介柡宥庡墰鐢棛绱掗幇顓ф當鐟滅増鐩顔炬崉閸濆嫷娼遍柡澶屽仩婵倛鍟梺鎼炲妼椤戝懘宕归鍡樺仒?

      loadAvailableModels();

      // 闂佸憡鑹鹃張顒勵敆閻愬搫绀夐柣妯煎劋缁佷即鎮归崶銉ュ姢闁绘繄鍏橀幊鐐哄磼濞戞瑤绮柡澶嗘櫆閸ㄥ磭绮╅弶鎴旀瀻闁炽儱鍟块埛鏃堟煙椤栨碍鍤€闁伙箑閰ｅ畷?

      void loadChapterQualityMetrics(chapter.id);

    }

  };



  const handleEditorSubmit = async (values: ChapterUpdate) => {

    if (!editingId || !currentProject) return;



    try {

      await updateChapter(editingId, values);



      // 闂佸憡甯￠弨閬嶅蓟婵犲啨浜滈柛锔诲幗缁愭菐閸ワ絽澧插ù鐓庢噺缁傛帡濡烽敂鐣屽嚱闂佸搫鍊绘晶妤呭焵椤掑喚鍤欓柣鈯欏洤鏋侀柟娈垮枤閸╃娀鎮?

      const updatedProject = await projectApi.getProject(currentProject.id);

      setCurrentProject(updatedProject);



      message.success('Project updated.');

      setIsEditorOpen(false);

    } catch {

      message.error('Failed to update project.');

    }

  };



  const handleGenerate = async () => {

    if (!editingId) return;

    const chapterId = editingId;

    if (runningSingleChapterTasks[chapterId]) {

      message.info('This chapter is already generating.');

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

          // 闁哄鏅滅粙鎴犫偓瑙勫▕瀹曞爼鎮欓崜浣诡啀

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

        content: '闂佸憡鑹炬姝屻亹閹绢喖绀嗘繛鎴烆殘缁嬪﹪寮堕埡鍌滎灱妞ゃ垺鍨剁粙澶愵敇閵娧咁槷闂佸憡鐟崹鎶藉箣妞嬪海纾兼い鎾跺仜瀵版挸霉閻樺磭澧柟濂告敱閹?',

        duration: 0,

      });



      // 闂佸憡鑹炬姝屻亹鐎靛摜纾肩憸蹇涙偨婵犳艾绠ョ憸鎴︺€侀幋锔芥櫖婵﹩鍓涢弳姘舵煙鐎涙ê濮囬柟顔筋殜閹虫盯顢旈崟顐嶆鏌￠崶褏鎽犻柡灞斤躬瀵剟宕堕…鎴炴暤闂佹寧绋掔粙鎴﹀Φ閹寸姵瀚婚柕澶涢檮椤ρ囨煙缂佹ê濮夐柕鍥ㄥ哺閺屻劌鈻庨幒婵嗘

      result.completion

        .then(async (finalResult) => {

          if (isEditorOpenRef.current && editingChapterIdRef.current === chapterId) {

            const hasContentTouched = editorForm.isFieldsTouched(['content']);

            if (!hasContentTouched && finalResult?.content) {

              editorForm.setFieldsValue({ content: finalResult.content });

            } else if (hasContentTouched) {

              message.info('Content was edited; keeping your changes.');

            }

          }



          message.open({

            key: progressMessageKey,

            type: 'success',

            content: 'Generation completed.',

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

            content: 'Chapter analysis failed: ' + (completionError.response?.data?.detail || completionError.message || 'Unknown error'),

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



      message.success('Chapter analysis task created.');

    } catch (error) {

      const apiError = error as ApiError;

      message.error('AI generation failed: ' + (apiError.response?.data?.detail || apiError.message || 'Unknown error'));

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
      : 'Not selected';
    const storyFocusLabel = selectedStoryFocus
      ? (STORY_FOCUS_OPTIONS.find((item) => item.value === selectedStoryFocus)?.label || selectedStoryFocus)
      : 'Not selected';
    const plotStageLabel = selectedPlotStage
      ? (CREATION_PLOT_STAGE_OPTIONS.find((item) => item.value === selectedPlotStage)?.label || selectedPlotStage)
      : 'Not selected';

    const instance = modal.confirm({
      title: 'Confirm generation',
      width: 700,
      centered: true,
      content: (
        <div style={{ marginTop: 16 }}>
          <p>This will continue generating the chapter with current settings.</p>
          <ul>
            <li>Style: {selectedStyle?.name ?? 'Not selected'}</li>
            <li>Creative mode: {creativeModeLabel}</li>
            <li>Story focus: {storyFocusLabel}</li>
            <li>Plot stage: {plotStageLabel}</li>
            <li>Target words: {targetWordCount}</li>
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
                Generated {previousChapters.length} chapters will be used as reference:
              </div>
              <div style={{ maxHeight: 150, overflowY: 'auto' }}>
                {previousChapters.map((ch) => (
                  <div key={ch.id} style={{ padding: '4px 0', fontSize: 13 }}>
                    Chapter {ch.chapter_number}: {ch.title} ({ch.word_count || 0} words)
                  </div>
                ))}
              </div>
              <div style={{ marginTop: 8, fontSize: 12, color: '#666' }}>
                Continuing will overwrite the current chapter content.
              </div>
            </div>
          )}
          <p style={{ color: '#ff4d4f', marginTop: 16, marginBottom: 0 }}>
            Please make sure important content is saved before continuing.
          </p>
        </div>
      ),
      okText: 'Continue',
      okButtonProps: { danger: true },
      cancelText: 'Cancel',
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

            message.error('Please select a writing style first.');

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

      'draft': 'Draft',

      'writing': 'Writing',

      'completed': 'Completed',

    };

    return texts[status] || status;

  };



  const handleExport = () => {

    if (chapters.length === 0) {

      message.warning('No chapters to export.');

      return;

    }



    modal.confirm({

      title: 'Export project',

      content: `Export project "${currentProject.title}"?`,

      centered: true,

      okText: 'Export',

      cancelText: 'Cancel',

      onOk: () => {

        try {

          projectApi.exportProject(currentProject.id);

          message.success('Export started.');

        } catch {

          message.error('Export failed.');

        }

      },

    });

  };



  const handleShowAnalysis = (chapterId: string) => {

    setAnalysisChapterId(chapterId);

    setAnalysisVisible(true);

  };



  // 闂佸綊娼х紞濠囧闯濞差亝鍋ㄩ柣鏃傤焾閻忓洭鏌涢幋锝呅撻柡?

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



    // 闁荤姴顑呴崯鎶芥儊椤栫偛绫嶉柕澶堝劤缁?

    console.log('[闂佸綊娼х紞濠囧闯濞差亝鍋ㄩ柣鏃傤焾閻忓槼 闁荤偞绋忛崝灞界暦閻掋倹lues:', values);

    console.log('[闂佸綊娼х紞濠囧闯濞差亝鍋ㄩ柣鏃傤焾閻忓槼 batchSelectedModel闂佺粯顭堥崺鏍焵?', batchSelectedModel);



    // 婵炶揪缍€濞夋洟寮妶澶婄鐟滅増甯掑▍銈夋煟閵忋垹鏋戦柛銊︽皑閳ь剛鏁搁、濠囨儊閽樺娴栭柛鈩冩礉閸橆剟姊洪銏╂Ч閻庢哎鍔戦幆鍐礋椤栵絾顥栭梺鍝勭Ф閸樠囧箯閹殿喒鍋撳☉娆樻畷闁哄棛鍠栭弫宥囦沪缁涘鎼愰梺鍝勵儐缁秹鎯€閸涙潙瀚夊鑸靛姀閸嬫挻寰勭€ｎ亶浠撮梺鍛婂笚閻熴倖绻涢崶顒佸仺闁靛ň鏅濈敮娑㈡偣娴ｉ潧鈧洟鍩€?

    const styleId = values.styleId || selectedStyleId;

    const wordCount = values.targetWordCount || targetWordCount;



    // 婵炶揪缍€濞夋洟寮妶澶婄鐟滅増甯掑▍銈夋煟閵忋垹鏋戦柛銊︾缁嬪骞橀懜鍨闂佹眹鍔岀€氼叀鍟梺鎼炲妼椤戝洦鎱ㄩ幖浣哥畱?
    const model = batchSelectedModel;
    const creativeMode = batchSelectedCreativeMode;
    const storyFocus = batchSelectedStoryFocus;
    const plotStage = batchSelectedPlotStage;


    console.log('[闂佸綊娼х紞濠囧闯濞差亝鍋ㄩ柣鏃傤焾閻忓槼 闂佸搫鐗冮崑鎾剁磽娴ｅ摜澧斿┑鐐叉喘閹粙濡歌閻ｇ湌odel:', model);



    if (!styleId) {

      message.error('Please select a writing style first.');

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


      // 婵犵鈧啿鈧綊鎮樻径鎰珘濠㈣泛鐟旀笟鈧畷鍦偓锝庝簻濡﹢鏌℃担绋跨盎缂佽鲸绻傝彁閻犲洦褰冮～锝夋煕閹烘挸顎滄い鏇ㄥ墮鏁堥柛灞剧懅缁夌厧鈽?

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



      message.success(`Batch generation started. Estimated time: ${result.estimated_time_minutes} min.`);



      // 濡絽鍟弲?闁荤喐鐟辩粻鎴ｃ亹閸屾纭呯疀濮樺吋缍岄梺闈╃祷閸旀垿鍩€椤掍焦鐨戦柣鎿勭節閺佸秹宕煎鍕簥闂佸憡鏌￠埀顒€纾壕璇测攽椤旂⒈鍎滅紒?

      showBrowserNotification(

        'Batch generation started',

        `Chapters: ${result.chapters_to_generate.length}, estimated time: ${result.estimated_time_minutes} min.`,

        'info'

      );



      // 閻庢鍠掗崑鎾斥攽椤旂⒈鍎撻柣銈呮閹风娀锝為鐔峰簥闂佸憡妫戠槐鏇熸叏閹间礁绠?

      startBatchPolling(result.batch_id);



    } catch (error: unknown) {

      const err = error as Error;

      message.error('Batch generation failed: ' + (err.message || 'Unknown error')) ;

      setBatchGenerating(false);

      setBatchGenerateVisible(false);

    }

  };



  // 闁哄鍎愰崰娑㈩敋濡ゅ懎绠ョ憸鐗堝笒濞呫倝鏌ｉ姀銏犳瀾闁搞劍纰嶇粋鎺旀嫚閹绘帩娼抽梺缁橆焾閸╂牠鍩€?

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



        // 濠殿噯绲界换鎴︻敃閸忓吋濮滄い鏃€顑欓崵鍕煛閸愵厽纭鹃柛鈺傜洴瀵剟骞嶉鎯у▏闂佺厧鎼崐鎼佸垂椤忓棙鍋橀柕濞垮劜鐎氭煡鏌涢幒鎴烆棡闁诲簼绮欓幃鈺呮嚋绾版ê浜惧〒姘功缁€澶愭倵閸︻厼浠︽俊鐐插€垮浼村礈瑜嬫禒娑㈡煛閸屾稑顥嬮柡浣规崌楠炲骞囬鐣屾殸缂備焦姊绘慨鐐繆椤撱垹妞介悘鐐舵閻庡鏌＄€ｎ偄濮岀紒缁樕戦幆?

        // 婵炶揪缍€濞夋洟寮?await 缂佺虎鍙庨崰鏇犳崲濮樿埖鍤旂€瑰嫭婢樼徊鍧楁煛閸艾浜鹃梺鍝勫€规竟鍡涙偟閻戣姤鍤嶉柛灞捐壘閻忔瑩鎮跺☉鏍у闁诡喗顨婂畷妯侯吋閸涱収娼遍柡澶屽仩濡嫰宕规惔銊ュ嚑闁归偊浜濆畷鏌ユ煕閺冩挾纾垮┑顔芥倐楠炩偓?

        if (status.completed > 0) {

          const latestChapters = await refreshChapters();

          await loadAnalysisTasks(latestChapters);



          // 闂佸憡甯￠弨閬嶅蓟婵犲啨浜滈柛锔诲幗缁愭菐閸ワ絽澧插ù鐓庢噺缁傛帡濡烽妷褎婢栭梺鍝勫暢閸╂牕煤閸ф妫橀柣妤€鐗冮崑鎾舵嫚閼碱剚鎲婚梺杞扮劍婢瑰棛鍒掗搹瑙勫?

          if (currentProject?.id) {

            const updatedProject = await projectApi.getProject(currentProject.id);

            setCurrentProject(updatedProject);

          }

        }



        // 婵炲濮鹃褎鎱ㄩ悢琛″亾閻熺増婀伴柛銊﹀哺楠炲寮借娴滃ジ鎮归幇鈺佸姷缂佽鲸绻堝畷鎴濐煥閸曢潧澹橀柡澶屽剱閸犳盯顢?

        if (status.status === 'completed' || status.status === 'failed' || status.status === 'cancelled') {

          if (batchPollingIntervalRef.current) {

            clearInterval(batchPollingIntervalRef.current);

            batchPollingIntervalRef.current = null;

          }



          setBatchGenerating(false);

          const taskMeta = batchTaskMetaRef.current[taskId] ?? getPersistedBatchTaskMeta(taskId, currentProject?.id);



          // 缂備焦鏌ㄩ鍛暤閸℃稑绀嗛梺鍨儐閻撯偓缂備焦姊绘慨鐐繆椤撱垹绀嗘俊銈呭閳ь剙鍟村畷顏嗕沪閽樺鈧鏌＄€ｎ偄濮冮柟骞垮灲瀹曟繈鏁嶉崟顐嶏箓鏌熼璺ㄧ瓘缂佽鲸鐟╁畷鐑藉Ω閿旇В鏋栫紓浣插亾闁绘垶顭囧暩闂佽鍙庨崹鐗堟櫠閻樼粯鏅?

          // 婵炶揪缍€濞夋洟寮?refreshChapters 闁哄鏅滈弻銊ッ洪弽顓熷剭闁告洦鍓氭禒姗€鏌￠崒娑橆棆闁绘繄鍏橀幊鐐哄磼濮橆剛浠氶柣鐐寸◤閸斿妲愰崼鏇熺劵闁圭儤姊婚懜?loadAnalysisTasks

          const finalChapters = await refreshChapters();

          await loadAnalysisTasks(finalChapters);



          // 闂佸憡甯￠弨閬嶅蓟婵犲啨浜滈柛锔诲幗缁愭菐閸ワ絽澧插ù鐓庢噺缁傛帡濡烽敂鐣屽嚱闂佸搫鍊绘晶妤呭焵椤掑喚鍤欓柣鈯欏洤鏋侀柟娈垮枤閸╃娀鎮?

          if (currentProject?.id) {

            const updatedProject = await projectApi.getProject(currentProject.id);

            setCurrentProject(updatedProject);

          }



          if (status.status === 'completed') {

            message.success(`Batch generation completed. Chapters: ${status.completed}.`);

            // 濡絽鍟弲?闁荤喐鐟辩粻鎴ｃ亹閸屾纭呯疀濮樺吋缍岄梺闈╃祷閸旀垿鍩€椤掍焦鐨戦柣?

            showBrowserNotification(

              'Batch generation completed',

              `Project "${currentProject?.title || 'Untitled'}": ${status.completed} chapters completed.`,

              'success'

            );



            if (taskMeta?.autoAnalyze) {

              void triggerDeferredBatchAnalysis(taskMeta.startChapterNumber, taskMeta.count, finalChapters);

            }

          } else if (status.status === 'failed') {

            message.error(`Batch generation failed: ${status.error_message || 'Unknown error'}`);

            // 濡絽鍟弲?闁荤喐鐟辩粻鎴ｃ亹閸屾纭呯疀濮樺吋缍岄梺闈╃祷閸旀垿鍩€椤掍焦鐨戦柣?

            showBrowserNotification(

              'Batch generation failed',

              status.error_message || 'Unknown error',

              'error'

            );

          } else if (status.status === 'cancelled') {

            message.warning('Batch generation cancelled.');

          }



          delete batchTaskMetaRef.current[taskId];

          removePersistedBatchTaskMeta(taskId);



          // 閻庣偣鍊栭崕鑲╂崲濠婂牆绀傞柟鎯板Г閿涙棃鎮楅悽娈挎敯闁伙箒妫勯々濂稿幢椤撶姷顦柣鐘辫閺呮繈寮妶澶婄濡炲瀛╃粻娆撴煕閹烘柨顣兼繛鎾冲缁辨帡宕奸姀鐘橈箓鏌?

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



    // 缂備焦鏌ㄩ鍛暤閸℃稑绠ョ憸鎴︺€侀幋鐐碘枖闁逞屽墮閳?

    poll();



    // 濠?缂備礁顦扮敮鐔兼偪閸℃瑦瀚氭い顐幘椤忚鲸绻?

    batchPollingIntervalRef.current = window.setInterval(poll, 2000);

  };



  // 闂佸憡鐟﹂悧妤冪矓閻戣棄绠ョ憸鐗堝笒濞呫倝鏌ｉ姀銏犳瀾闁?

  const handleCancelBatchGenerate = async () => {

    if (!batchTaskId) return;



    try {

      await chapterBatchTaskApi.cancelBatchGenerateTask(batchTaskId, currentProject?.id);

      delete batchTaskMetaRef.current[batchTaskId];

      removePersistedBatchTaskMeta(batchTaskId);



      message.success('Batch generation cancelled.');



      // 闂佸憡鐟﹂悧妤冪矓閻戣棄瑙﹂幖杈剧悼瑜板矂鏌涘Δ鈧崯鍧楀春濞戙垹妫橀柟娈垮枤瑜板潡鏌ら崫鍕偓鎼佸垂椤忓棙鍋橀柕濞垮劜鐎氭煡鏌涢幒鎴烆棡闁诲氦濮ょ粋鎺旀嫚閹绘帩娼抽梺鎸庣☉閺堫剙螣婢跺瞼鐭嗛柛婵嗗閸ゆ帡鏌ｉ姀銏犳瀾闁搞劍宀搁幆鍐礋椤撶姴濞囬梺?

      await refreshChapters();

      await loadAnalysisTasks();



      // 闂佸憡甯￠弨閬嶅蓟婵犲啨浜滈柛锔诲幗缁愭菐閸ワ絽澧插ù鐓庢噺缁傛帡濡烽敂鐣屽嚱闂佸搫鍊绘晶妤呭焵椤掑喚鍤欓柣鈯欏洤鏋侀柟娈垮枤閸╃娀鎮?

      if (currentProject?.id) {

        const updatedProject = await projectApi.getProject(currentProject.id);

        setCurrentProject(updatedProject);

      }

    } catch (error: unknown) {

      const err = error as Error;

      message.error('Cancel batch generation failed: ' + (err.message || 'Unknown error'));

    }

  };



  // 闂佺懓鐏氶幐鍝ユ閹达箑绠ョ憸鐗堝笒濞呫倝鏌ｉ姀銏犳瀾闁搞劍姘ㄩ埀顒傛暩椤㈠﹪鎯侀挊澶樻禆?

  const handleOpenBatchGenerate = async () => {

    if (batchGenerating) {

      message.info('Batch generation is running. Please wait for it to finish.');

      return;

    }





    if (!firstIncompleteChapter) {

      message.info('No incomplete chapters to generate.');

      return;

    }



    // 濠碘槅鍋€閸嬫捇鏌＄仦璇插姤妞ゆ洘姘ㄧ划鈺呮偐閸濆嫀婵嬫煛閸曢潧鐏犻柟顖欑窔瀹曪綁顢涘▎搴ｉ瀺闂佹眹鍨婚崰鎰板垂?

    if (!canGenerateChapter(firstIncompleteChapter)) {

      const reason = getGenerateDisabledReason(firstIncompleteChapter);

      message.warning(reason);

      return;

    }



    // 闂佺懓鐏氶幐鍝ユ閹寸姭鍋撻悽娈挎敯闁伙箒妫勯々濂稿幢濡椿妲梺鍛婃⒒婵儳霉閸ヮ灛鐔煎灳瀹曞洠鍋撻悜钘夌婵°倕瀚ㄩ埀顒€鍟撮弫宥呯暆閳ь剟鎮洪幋婵愬殫闁告侗鍘鹃弳姘舵煙?

    const defaultModel = await loadAvailableModels();



    console.log('[闂佺懓鐏氶幐鍝ユ閹达箑绠ョ憸鐗堝笒濞呫倝鏌ｉ姀銏犳瀾闁搞劍鐡?defaultModel:', defaultModel);

    console.log('[闂佺懓鐏氶幐鍝ユ閹达箑绠ョ憸鐗堝笒濞呫倝鏌ｉ姀銏犳瀾闁搞劍鐡?selectedStyleId:', selectedStyleId);



    // 闁荤姳绀佹晶浠嬫偪閸℃稑绠ョ憸鐗堝笒濞呫倝鏌ｉ姀銏犳瀾闁搞劍宀搁幆鍐礋椤戠喍绶氬畷鍦偓锝庡枓閸嬫挻寰勭€ｎ亶浠撮梺缁橆焾閸╂牠鍩€?
    setBatchSelectedModel(defaultModel || undefined);
    setBatchSelectedCreativeMode(undefined);
    setBatchSelectedStoryFocus(undefined);
    setBatchSelectedPlotStage(inferCreationPlotStage({
      chapterNumber: firstIncompleteChapter.chapter_number,
      totalChapters: knownStructureChapterCount,
    }));


    // 闂備焦褰冪粔鍫曟偪閸℃瑦鍋橀柕濞垮劚缁€瀣殽閻愭潙鍔舵い鏃€娲滅槐鏃堫敊閻撳海浠存繝娈垮枛椤戝懘鍩€椤掑倶鈧妲愬▎鎰閻犳亽鍔嶉弳蹇曠磽閸屾稒灏柣掳鍔戦幆鍐礋椤愩倖鎲婚梺杞扮鎼存粎妲?

    batchForm.setFieldsValue({

      startChapterNumber: firstIncompleteChapter.chapter_number,

      count: 5,

      enableAnalysis: true,

      styleId: selectedStyleId,

      targetWordCount: getCachedWordCount(),

    });



    setBatchGenerateVisible(true);

  };



  // 闂佸綊娼ч鍛叏閳哄懎绀嗘繛鎴烆焽缁憋妇绱掗弮鍌毿┑?婵炲濮村畵鈧琻e-to-many濠碘槅鍨埀顒€纾涵鈧?

  const showManualCreateChapterModal = () => {

    // 闁荤姳绶ょ槐鏇㈡偩缂佹鈻旈悗锝傛櫇椤忓崬鈽夐幙鍐х敖闁绘繄鍏橀幊鐐哄磼濮橆剙鈻?

    const nextChapterNumber = chapters.length > 0

      ? Math.max(...chapters.map(c => c.chapter_number)) + 1

      : 1;



    modal.confirm({

      title: 'Create chapter manually',

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

            label="Chapter number"

            name="chapter_number"

            rules={[{ required: true, message: 'Please enter chapter number.' }]}

            tooltip="Chapter number must be unique."

          >

            <InputNumber min={1} style={{ width: '100%' }} placeholder="Enter chapter number" />

          </Form.Item>



          <Form.Item

            label="Chapter title"

            name="title"

            rules={[{ required: true, message: 'Please enter chapter title.' }]}

          >

            <Input placeholder="Enter chapter title" />

          </Form.Item>



          <Form.Item

            label="Outline"

            name="outline_id"

            rules={[{ required: true, message: 'Please select an outline.' }]}

            tooltip="Each chapter must belong to an outline."

          >

            <Select placeholder="Select an outline">

              {/* 闂佺儵鏅涢悺銊ф暜鐎涙ɑ濯撮悹鎭掑妽閺?store 婵炴垶鎼╅崢鎯р枔?outlines 闂佽桨鑳舵晶妤€鐣垫笟鈧弫宥呯暆閸愶絽浜鹃悘鐐跺亹閻熸繈鏌￠崟闈涚仧缂侇喚濞€閹娊鎮ч崼鐔虹暢缂備焦姊绘慨鐐繆椤撶喓鈻旀い鎾跺枎缁插綊鏌?*/}

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

            label="Summary"

            name="summary"

            tooltip="Short summary of the chapter."

          >

            <TextArea

              rows={4}

              placeholder="Enter a short summary"

            />

          </Form.Item>



          <Form.Item

            label="Status"

            name="status"

          >

            <Select>

              <Select.Option value="draft">Draft</Select.Option>

              <Select.Option value="writing">Writing</Select.Option>

              <Select.Option value="completed">Completed</Select.Option>

            </Select>

          </Form.Item>

        </Form>

      ),

      okText: 'Create',

      cancelText: 'Cancel',

      onOk: async () => {

        const values = await manualCreateForm.validateFields();



        // 濠碘槅鍋€閸嬫捇鏌＄仦璇插姢闁绘繄鍏橀幊鐐哄磼濮樿京顣查梺鍛婄懇閺€鍗炍ｉ幖浣歌Е闁挎洍鍋撻柛鎴磿閳ь剚绋掗敋婵犫偓?

        const conflictChapter = chapters.find(

          ch => ch.chapter_number === values.chapter_number

        );



        if (conflictChapter) {

          // 闂佸搫瀚晶浠嬪Φ濮樿泛绀冮柤纰卞墰瀹曟劙鏌熺紒妯哄闁靛洦鐡塷dal

          modal.confirm({

            title: '缂備焦姊绘慨鐐繆椤撶喐鍎熼煫鍥ㄦ尭婵炲洭鏌涢幇顖氱毢闁?',

            icon: <InfoCircleOutlined style={{ color: '#ff4d4f' }} />,

            width: 500,

            centered: true,

            content: (

              <div>

                <p style={{ marginBottom: 12 }}>

                  缂?<strong>{values.chapter_number}</strong> 缂備焦姊绘慨鎾礄閿涘嫧鍋撳☉娅亜锕㈤鍫熸櫖?

                </p>

                <div style={{

                  padding: 12,

                  background: '#fff7e6',

                  borderRadius: 4,

                  border: '1px solid #ffd591',

                  marginBottom: 12

                }}>

                  <div><strong>闂佸搫绉村ú顓€傛禒瀣櫖</strong>{conflictChapter.title}</div>

                  <div><strong>闂佺粯顭堥崺鏍焵椤戣法绛忕紒</strong>{getStatusText(conflictChapter.status)}</div>

                  <div><strong>闁诲孩绋掗〃鍡涘汲閻斿吋鏅</strong>{conflictChapter.word_count || 0}闁</div>

                  {conflictChapter.outline_title && (

                    <div><strong>闂佸湱顣介崑鎾绘倶閻愰潧浠滈柕鍥ф川閻ヮ亞鎷犺缁</strong>{conflictChapter.outline_title}</div>

                  )}

                </div>

                <p style={{ color: '#ff4d4f', marginBottom: 8 }}>

                  闂佸疇娉曟刊瀵哥箔?闂佸搫瀚烽崹浼村箚娓氣偓瀹曟岸鎮╃紒妯煎綉闂佸搫鍞查崨顖氬▏闂佺厧鎼崐鎼佹嚐閻旂厧绀嗘繛鎴烆焽缁憋箓鏌￠崒娑橆棆闁绘繄鍏橀幊鐐哄磼閿旀儳鎯?

                </p>

                <p style={{ fontSize: 12, color: '#666', marginBottom: 0 }}>

                  闂佸憡甯炴繛鈧繛鍛叄瀹曘儲鎯旈敍鍕啈闂佸搫鍟版慨鐢垫兜閸洖绠掗柕蹇曞濡插鏌ㄥ☉妯肩伇闁绘繄鍏橀幊鐐哄磼濮橆剚鏆ラ柣搴℃贡閹虫捇骞忔导鏉戠闁糕剝顭囬埀顒傛櫕缁辨帡骞樼€甸晲鍑介梻渚囧枦濡嫰鎯冮姀銏″仏妞ゆ劑鍨归悘鈺呮⒒閸曗晛鈧垿鍩€?

                </p>

              </div>

            ),

            okText: '闂佸憡甯炴繛鈧繛鍛叄閻涱喚鎹勯崫鍕画閻?',

            okButtonProps: { danger: true },

            cancelText: 'Cancel',

            onOk: async () => {

              try {

                // 闂佺绻愰悧鍡涘垂瑜版帗鈷旈柕鍫濇閿涘绱掗弮鍌毿┑?

                await handleDeleteChapter(conflictChapter.id);



                // 缂備焦绋戦ˇ顖滄閻斿摜鈻旈柍褜鍓涙禍姝岀疀閺冩垵鏂€闂佸搫鍟悥鐓幬涚捄銊﹀厹妞ゆ棁宕电粻浠嬫煕閹烘柨鈻堟繛鍛捣閳ь剛鎳撻張顒勫垂?

                await new Promise(resolve => setTimeout(resolve, 300));



                // 闂佸憡甯楃粙鎴犵磽閹捐妫橀柟娈垮枤瑜板潡鏌?

                await chapterApi.createChapter({

                  project_id: currentProject.id,

                  ...values

                });



                message.success('Chapter created.');

                await refreshChapters();



                // 闂佸憡甯￠弨閬嶅蓟婵犲啨浜滈柛锔诲幗缁愭菐閸ワ絽澧插ù鐓庢噺缁傛帡濡烽敂鐣屽嚱闂佸搫鍊瑰姗€鎮鸿瀵偊骞嶉鎯х厷闁?

                const updatedProject = await projectApi.getProject(currentProject.id);

                setCurrentProject(updatedProject);



                manualCreateForm.resetFields();

              } catch (error: unknown) {

                const err = error as Error;

                message.error('Create chapter failed: ' + (err.message || 'Unknown error'));

                throw error;

              }

            }

          });



          // 闂傚倸鍟扮划顖烆敆濞戞瑥绶為柡宓懏鍕綧odal闂佺绻戞繛濠偽?

          return Promise.reject();

        }



        // 濠电偛澶囬崜婵嗭耿娓氣偓瀹曟﹢鎳犻鍌氱９闂佹寧绋戦惉鐓幟洪崸妤€绠抽柕澶堝劚閻忥紕鈧?

        try {

          await chapterApi.createChapter({

            project_id: currentProject.id,

            ...values

          });

          message.success('Chapter created.');

          await refreshChapters();



          // 闂佸憡甯￠弨閬嶅蓟婵犲啨浜滈柛锔诲幗缁愭菐閸ワ絽澧插ù鐓庢噺缁傛帡濡烽敂鐣屽嚱闂佸搫鍊瑰姗€鎮鸿瀵偊骞嶉鎯х厷闁?

          const updatedProject = await projectApi.getProject(currentProject.id);

          setCurrentProject(updatedProject);



          manualCreateForm.resetFields();

        } catch (error: unknown) {

          const err = error as Error;

          message.error('Create chapter failed: ' + (err.message || 'Unknown error'));

          throw error;

        }

      }

    });

  };



  // 濠电偞鎸稿鍫曟偂鐎ｎ喖绀嗛柛鈩冾焽閳ь兛绮欓幃鈺呮嚋绾版ê浜惧ù锝呮贡閸ㄨ偐绱?

  const renderAnalysisStatus = (chapterId: string) => {

    const task = analysisTasksMap[chapterId];



    if (!task) {

      return null;

    }



    switch (task.status) {

      case 'pending':

        return (

          <Tag icon={<SyncOutlined spin />} color="processing">

            Pending

          </Tag>

        );

      case 'running': {

        // 濠碘槅鍋€閸嬫捇鏌＄仦璇插姕婵″弶鎮傚畷銉╂晜缁涘濡ч梺闈╄礋閸旀垿宕抽崫銉﹀珰闁哄浂浜炵粈鍕煕濮橆剚鎹ｆい蹇ｅ墯鐎电厧顫濋浣藉惈error_message婵炴垶鎼╅崢鑲┾偓鍨耿瀹?闂備焦褰冪粔鐑芥儊?婵烇絽娲犻崜婵囧閸涘瓨鏅?

        const isRetrying = task.error_code === 'retrying';

        return (

          <Tag

            icon={<SyncOutlined spin />}

            color={isRetrying ? "warning" : "processing"}

            title={task.error_message || undefined}

          >

            {isRetrying ? `Retrying ${task.progress}%` : `Running ${task.progress}%`}

          </Tag>

        );

      }

      case 'completed':

        return (

          <Tag icon={<CheckCircleOutlined />} color="success">

            Completed

          </Tag>

        );

      case 'failed':

        return (

          <Tag icon={<CloseCircleOutlined />} color="error" title={task.error_message || undefined}>

            Failed

          </Tag>

        );

      default:

        return null;

    }

  };



  // 闂佸搫瀚晶浠嬪Φ濮樺彉娌柡鍥╁仧绾惧鎮峰▎蹇擃仼闁搞劍绻勯幏鐘绘晜閽樺澹?

  const showExpansionPlanModal = (chapter: Chapter) => {

    if (!chapter.expansion_plan) return;



    try {

      const planData: ExpansionPlanData = JSON.parse(chapter.expansion_plan);



      modal.info({

        title: (

          <Space style={{ flexWrap: 'wrap' }}>

            <InfoCircleOutlined style={{ color: 'var(--color-primary)' }} />

            <span style={{ wordBreak: 'break-word' }}>Chapter ${chapter.chapter_number} expansion plan</span>

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

              <Descriptions.Item label="缂備焦姊绘慨鐐繆椤撱垹鍐€闁搞儺鍓﹂弳?">

                <strong style={{

                  wordBreak: 'break-word',

                  whiteSpace: 'normal',

                  overflowWrap: 'break-word'

                }}>

                  {chapter.title}

                </strong>

              </Descriptions.Item>

              <Descriptions.Item label="闂佽鍨伴幊蹇涘礉閸涙潙鏄ュΔ锕佹硶濞?">

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

              <Descriptions.Item label="闂佸憡鍔樼亸娆撴偘婵犲嫮灏甸悹鍥皺閳?">

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

              <Descriptions.Item label="婵☆偅婢樼€氼亪宕ｆ繝鍕ㄥ亾濞戞瑯娈曢柡?">

                <Tag color="green">{planData.estimated_words}闁</Tag>

              </Descriptions.Item>

              <Descriptions.Item label="闂佸憡鐟﹂悷銈囪姳閵娾晜鍎庢い鏃傛櫕閸?">

                <span style={{

                  wordBreak: 'break-word',

                  whiteSpace: 'normal',

                  overflowWrap: 'break-word'

                }}>

                  {planData.narrative_goal}

                </span>

              </Descriptions.Item>

              <Descriptions.Item label="闂佺绻戞繛濠囧极椤撶喓顩查悗锝傛櫆椤?">

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

              <Descriptions.Item label="濠电偞鍨甸ˇ顖氼嚕妞嬪孩鍠嗛柟鐑樻礀椤?">

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

                <Descriptions.Item label="闂侀潻濡囬崕銈呪枍濞嗘垶鍠嗛柛鏇ㄥ亜閻?">

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

                          <strong>濡絽鍟幆?闂侀潻闄勬竟鍡涘磻閿濆鏅</strong>

                          <span style={{

                            wordBreak: 'break-word',

                            whiteSpace: 'normal',

                            overflowWrap: 'break-word'

                          }}>

                            {scene.location}

                          </span>

                        </div>

                        <div style={{ marginBottom: 4 }}>

                          <strong>濡絽鍟崳?闁荤喐鐟︾敮鐔哥珶婵犲洦鏅</strong>

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

                          <strong>濡絽鍟粻?闂佺儵鏅╅崰姘枔閹达附鏅</strong>

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

              message="闂佸湱绮崝妤呭Φ?"

              description="闁哄鏅滈悷銈囪姳濞差亜鍙婇柣銈咁攳闂侀潻璐熼崝宀勫Φ閸モ晝妫柟缁樺笧濞兼梻鈧鍠掗崑鎾绘煛閸愩劌顣抽柡浣规崌楠炲骞囬鐣屾殸闁荤喐鐟ョ€氼剟宕瑰┑鍥┾攳闁斥晛鍟╃槐鏍煥濞戞瀚扮憸鏉垮级缁傛帡濡烽妶鍥┾枙婵炴垶鎸搁幖顐﹀垂鏉堛劍濯存繝濠傛噽瑜板潡鏌ら崫鍕偓鎼佸船鐎电硶鍋撶涵鍜佹綈婵＄偛鍊块幆鍐礋椤愩垺顥濋梺鍏兼緲閸熴劑鍩€?"

              type="info"

              showIcon

              style={{ marginTop: 16 }}

            />

          </div>

        ),

        okText: 'OK',

      });

    } catch (error) {

      console.error('Failed to load expansion plan:', error);

      message.error('Failed to load expansion plan.');

    }

  };



  // 闂佸憡甯炴繛鈧繛鍛捣缁晠鎮╅崫鍕庢繂顭跨捄鍝勵伀闁诡喖锕畷娆撴嚍閵夛附顔?

  const handleDeleteChapter = async (chapterId: string) => {

    try {

      await deleteChapter(chapterId);



      // 闂佸憡甯￠弨閬嶅蓟婵犲嫮鍗氶柣妯烘惈铻￠梺鍛婂笚椤ㄥ濡?

      await refreshChapters();



      // 闂佸憡甯￠弨閬嶅蓟婵犲啨浜滈柛锔诲幗缁愭菐閸ワ絽澧插ù鐓庢噺缁傛帡濡烽敂鐣屽嚱闂佸搫鍊绘晶妤呭焵椤掑喚鍤欓柣鈯欏洤鏋侀柟娈垮枤閸╃娀鎮?

      if (currentProject) {

        const updatedProject = await projectApi.getProject(currentProject.id);

        setCurrentProject(updatedProject);

      }



      message.success('Chapter deleted.');

    } catch (error: unknown) {

      const err = error as Error;

      message.error('Delete chapter failed: ' + (err.message || 'Unknown error'));

    }

  };



  // 闂佺懓鐏氶幐鍝ユ閹寸姵鍠嗛柛鏇ㄥ亜閻忓﹦绱撻崒娑氬⒊缂侀鍋婂畷?

  const handleOpenPlanEditor = (chapter: Chapter) => {

    // 闂佺儵鏅涢悺銊ф暜閹绢喖绠ラ柟鎯х－绾捐崵绱撻崒娑氬⒊缂侀鍋婂畷?婵犵鈧啿鈧綊鎮樻径濞炬煢闁斥晛鍟粻鎺楁偡濞嗗繐顏╅柛銊︾箞瀵偊鎮ч崼婵堛偊闂佸憡甯楅悷銉╁垂閸楃儐鍤堥柣鎴炆戦悡鈧梺?

    setEditingPlanChapter(chapter);

    setPlanEditorVisible(true);

  };



  // 婵烇絽娲︾换鍌炴偤閵娧勫枂闁告洦鍋勯悘濠偳庨崶锝呭⒉濞?

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



      // 闂佸憡甯￠弨閬嶅蓟婵犲嫮鍗氶柣妯烘惈铻￠梺鍛婂笚椤ㄥ濡?

      await refreshChapters();



      message.success('Plan saved.');



      // 闂佺绻戞繛濠偽涚€靛摜纾介柡宥庡墰鐢棝鏌?

      setPlanEditorVisible(false);

      setEditingPlanChapter(null);

    } catch (error: unknown) {

      const err = error as Error;

      message.error('Save plan failed: ' + (err.message || 'Unknown error'));

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



  // 闂傚倸鍟幊鎾活敋娴兼潙闂柕濞垮劚閻庡ジ鏌熼獮鍨伄闁绘繄鍏橀幊?

  const handleReaderChapterChange = async (chapterId: string) => {

    try {

      const response = await fetch(`/api/chapters/${chapterId}`);

      if (!response.ok) throw new Error('Failed to load chapter.');

      const newChapter = await response.json();

      setReadingChapter(newChapter);

    } catch {

      message.error('Failed to load chapter.');

    }

  };



  // 闂佺懓鐏氶幐鍝ユ閹寸姳娌柍褜鍓熼弻鍫ュΩ閳轰焦顏熼梺鍛婂姈閻熴儵鎳樻繝鍕幓?

  const handleOpenPartialRegenerate = () => {

    setPartialRegenerateToolbarVisible(false);

    setPartialRegenerateModalVisible(true);

  };



  // 闁圭厧鐡ㄥ濠氬极閵堝洣娌柍褜鍓熼弻鍫ュΩ閳轰焦顏熼梺鍛婂姈閻熝呭垝閵娾晛鍑?

  const handleApplyPartialRegenerate = (newText: string, startPos: number, endPos: number) => {

    // 闂佸吋鍎抽崲鑼躲亹閸ヮ亗浜归柟鎯у暱椤ゅ懘鏌涢幇顒佸櫣妞?

    const currentContent = editorForm.getFieldValue('content') || '';

    

    // 闂佸搫娲︾€笛冪暦閺屻儲鐒诲璺侯槼閸橆剟姊洪鍝勫闁?

    const newContent = currentContent.substring(0, startPos) + newText + currentContent.substring(endPos);

    

    // 闂佸搫娲ら悺銊╁蓟婵犲嫭鍋橀柕濞垮劚缁€?

    editorForm.setFieldsValue({ content: newContent });

    

    // 闂佺绻戞繛濠偽涢弶鎴殨闁革富鍘惧畷?

    setPartialRegenerateModalVisible(false);

    

    message.success('Partial regeneration applied.');

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

          Chapters

        </h2>

        <Space direction={isMobile ? 'vertical' : 'horizontal'} style={{ width: isMobile ? '100%' : 'auto' }}>

          {currentProject.outline_mode === 'one-to-many' && (

            <Button

              icon={<PlusOutlined />}

              onClick={showManualCreateChapterModal}

              block={isMobile}

              size={isMobile ? 'middle' : 'middle'}

            >

              Create chapter

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

            Batch generate

          </Button>

          <Button

            type="default"

            icon={<DownloadOutlined />}

            onClick={handleExport}

            disabled={chapters.length === 0}

            block={isMobile}

            size={isMobile ? 'middle' : 'middle'}

          >

            Export

          </Button>

          {!isMobile && (

            <Tag color="blue">

              {currentProject.outline_mode === 'one-to-one'

                ? 'One outline per chapter'

                : 'One outline for all chapters'}

            </Tag>

          )}

        </Space>

      </div>



      <div style={{ flex: 1, overflowY: 'auto', minHeight: 0 }}>

        {chapters.length === 0 ? (

          <Empty description="No chapters yet." />

        ) : currentProject.outline_mode === 'one-to-one' ? (

          // one-to-one 濠碘槅鍨埀顒€纾涵鈧梺鎸庣⊕濮樸劌煤閸ф绠抽柕澶涢檮閳绘梻绱掗埀顒勬倻濡警鏆㈠Δ鐘靛仜閸熷潡宕归鍡樺仒?

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

                    title={!item.content || item.content.trim() === '' ? 'No content to read.' : 'Open reader'}

                  >

                    Read

                  </Button>,

                  <Button

                    type="text"

                    icon={<EditOutlined />}

                    onClick={() => handleOpenEditor(item.id)}

                  >

                    Edit

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

                          !hasContent ? 'No content to analyze.' :

                            isAnalyzing ? 'Analyzing...' : ''



                        }

                      >

                        {isAnalyzing ? 'Analyzing' : 'Analyze'}

                      </Button>

                    );

                  })(),

                  <Button

                    type="text"

                    icon={<SettingOutlined />}

                    onClick={() => handleOpenModal(item.id)}

                  >

                    Settings

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

                          <Badge count={`${item.word_count || 0} words`} style={{ backgroundColor: 'var(--color-success)' }} />

                          {renderAnalysisStatus(item.id)}

                          {!canGenerateChapter(item) && (

                            <Tag icon={<LockOutlined />} color="warning" title={getGenerateDisabledReason(item)}>

                              闂傚倸娲犻崑鎾绘煕閹惧磭肖闁汇倕妫涚划鈺呮偐閸濆嫀?

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

                        <span style={{ color: 'rgba(0,0,0,0.45)', fontSize: isMobile ? 12 : 14 }}>闂佸搫妫楅崐鐟拔涢妶澶婄闁告侗鍙庨崯</span>

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

                        title={!item.content || item.content.trim() === '' ? 'No content to read.' : 'Open reader'}

                      />

                      <Button

                        type="text"

                        icon={<EditOutlined />}

                        onClick={() => handleOpenEditor(item.id)}

                        size="small"

                        title="Edit"

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

                            title={!hasContent ? 'No content to analyze.' : isAnalyzing ? 'Analyzing...' : 'Analyze'}









                          />

                        );

                      })()}

                      <Button

                        type="text"

                        icon={<SettingOutlined />}

                        onClick={() => handleOpenModal(item.id)}

                        size="small"

                        title="Settings"

                      />

                    </Space>

                  )}

                </div>

              </List.Item>

            )}

          />

        ) : (

          // one-to-many 濠碘槅鍨埀顒€纾涵鈧梺鎸庣⊕绾板秶鈧灚绮嶅鍕槾缂傚牅鍗冲畷姘跺幢濞嗘垹鐓侀梺鍝勫婢т粙濡?

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

                      {group.outlineId ? `濡絽鍟幉?婵犮垹鐖㈤崨顖氱墯 ${group.outlineOrder}` : '濡絽鍟幉?闂佸搫鐗滄禍婊堝垂鎼达絿灏?'}

                    </Tag>

                    <span style={{ fontWeight: 600, fontSize: 16 }}>

                      {group.outlineTitle}

                    </span>

                    <Badge

                      count={`${group.chapters.length} chapters`}

                      style={{ backgroundColor: 'var(--color-success)' }}

                    />

                    <Badge

                      count={`${group.chapters.reduce((sum, ch) => sum + (ch.word_count || 0), 0)} words`}

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

                          title={!item.content || item.content.trim() === '' ? 'No content' : 'Read'}

                        >

                          Read

                        </Button>,

                        <Button

                          type="text"

                          icon={<EditOutlined />}

                          onClick={() => handleOpenEditor(item.id)}

                        >

                          Edit

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

                                !hasContent ? 'No content' :

                                  isAnalyzing ? 'Analyzing...' :

                                    'Show analysis'

                              }

                            >

                              {isAnalyzing ? 'Analyzing' : 'Analyze'}

                            </Button>

                          );

                        })(),

                        <Button

                          type="text"

                          icon={<SettingOutlined />}

                          onClick={() => handleOpenModal(item.id)}

                        >

                          Settings

                        </Button>,

                        // 闂佸憡鐟禍婊冿耿?one-to-many 濠碘槅鍨埀顒€纾涵鈧繛鎴炴尭椤戝棗螣婢跺瞼鐭嗛柛婵嗗閻忊晠姊婚崟鈺佲偓鏍偓鍨矒閺?

                        ...(currentProject.outline_mode === 'one-to-many' ? [

                          <Popconfirm

                            title="Delete chapter?"

                            description="This will remove the chapter from the list."

                            onConfirm={() => handleDeleteChapter(item.id)}

                            okText="Delete"

                            cancelText="Cancel"

                            okButtonProps={{ danger: true }}

                          >

                            <Button

                              type="text"

                              danger

                              icon={<DeleteOutlined />}

                            >

                              Delete

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

                                Chapter {item.chapter_number}: {item.title}

                              </span>

                              <Space wrap size={isMobile ? 4 : 8}>

                                <Tag color={getStatusColor(item.status)}>{getStatusText(item.status)}</Tag>

                                <Badge count={`${item.word_count || 0} words`} style={{ backgroundColor: 'var(--color-success)' }} />

                                {renderAnalysisStatus(item.id)}

                                {!canGenerateChapter(item) && (

                                  <Tag icon={<LockOutlined />} color="warning" title={getGenerateDisabledReason(item)}>

                                    Generation disabled

                                  </Tag>

                                )}

                                <Space size={4}>

                                  {item.expansion_plan && (

                                    <InfoCircleOutlined

                                      title="View expansion plan"

                                      style={{ color: 'var(--color-primary)', cursor: 'pointer', fontSize: 16 }}

                                      onClick={(e) => {

                                        e.stopPropagation();

                                        showExpansionPlanModal(item);

                                      }}

                                    />

                                  )}

                                  <FormOutlined

                                    title={item.expansion_plan ? "Edit expansion plan" : "Create expansion plan"}

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

                              <span style={{ color: 'rgba(0,0,0,0.45)', fontSize: isMobile ? 12 : 14 }}>No content yet.</span>

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

                              title={!item.content || item.content.trim() === '' ? 'No content' : 'Read'}

                            />

                            <Button

                              type="text"

                              icon={<EditOutlined />}

                              onClick={() => handleOpenEditor(item.id)}

                              size="small"

                              title="Edit"

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

                                    !hasContent ? 'No content' :

                                      isAnalyzing ? 'Analyzing...' :

                                        'Show analysis'

                                  }

                                />

                              );

                            })()}

                            <Button

                              type="text"

                              icon={<SettingOutlined />}

                              onClick={() => handleOpenModal(item.id)}

                              size="small"

                              title="Settings"

                            />

                            {/* 闂佸憡鐟禍婊冿耿?one-to-many 濠碘槅鍨埀顒€纾涵鈧繛鎴炴尭椤戝棗螣婢跺瞼鐭嗛柛婵嗗閻忊晠姊婚崟鈺佲偓鏍偓鍨矒閺?*/}

                            {currentProject.outline_mode === 'one-to-many' && (

                              <Popconfirm

                                title="Delete chapter?"

                                description="This will remove the chapter from the list."

                                onConfirm={() => handleDeleteChapter(item.id)}

                                okText="Delete"

                                cancelText="Cancel"

                                okButtonProps={{ danger: true }}

                              >

                                <Button

                                  type="text"

                                  danger

                                  icon={<DeleteOutlined />}

                                  size="small"

                                  title="Delete"

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

        title={editingId ? 'Edit chapter' : 'Create chapter'}

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

            label="Chapter title"

            name="title"

            tooltip={

              currentProject.outline_mode === 'one-to-one'

                ? "Title is fixed in one-to-one mode."

                : "Title is required in one-to-many mode."

            }

            rules={

              currentProject.outline_mode === 'one-to-many'

                ? [{ required: true, message: 'Title is required.' }]

                : undefined

            }

          >

            <Input

              placeholder="Enter chapter title"

              disabled={currentProject.outline_mode === 'one-to-one'}

            />

          </Form.Item>



          <Form.Item

            label="Chapter number"

            name="chapter_number"

            tooltip="Used for ordering chapters."

          >

            <Input type="number" placeholder="Enter chapter number" />

          </Form.Item>



          <Form.Item label="Status" name="status">

            <Select placeholder="Select status">

              <Select.Option value="draft">Draft</Select.Option>

              <Select.Option value="writing">Writing</Select.Option>

              <Select.Option value="completed">Completed</Select.Option>

            </Select>

          </Form.Item>



          <Form.Item>

            <Space style={{ float: 'right' }}>

              <Button onClick={() => setIsModalOpen(false)}>Cancel</Button>

              <Button type="primary" htmlType="submit">

                Save

              </Button>

            </Space>

          </Form.Item>

        </Form>

      </Modal>



      <Modal

        title="Edit chapter content"

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

          {/* 缂備焦姊绘慨鐐繆椤撱垹鍐€闁搞儺鍓﹂弳顖炴煕濠婂啰鎼糏闂佸憡甯楃粙鎰礊閺冨牆绠板鑸靛姈鐏?*/}

          <Form.Item

            label="缂備焦姊绘慨鐐繆椤撱垹鍐€闁搞儺鍓﹂弳?"

            tooltip="闂?-1濠碘槅鍨埀顒€纾涵鈧柣鐘叉搐閸㈡彃锕㈤鍛窞鐟滃繒绱欓悧鍫⑩攳妞ゆ棁濮ら弳顓㈡煥?-N濠碘槅鍨埀顒€纾涵鈧柣鐘叉穿濞撹绻涢崶顒佸仺闁靛鍊栭崣蹇涙煛閳ь剛鎲撮崟顐ゆ▎闂備胶鐡旈崰姘辨椤忓懏缍囬柟鎼灣缁€?"

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

                    title={!canGenerate ? disabledReason : '闂佸搫绉烽～澶婄暤娴ｇ懓绶炵憸蹇曠礄娴兼潙妞介悘鐐舵椤ゅ懐绱撻崘鎯ф灓闁绘繄鍏橀幊鐐哄磼濮橆剚鏆ラ柣搴℃贡閹虫捇宕规潏銊﹀?'}

                  >

                    {isMobile ? 'AI' : 'AI闂佸憡甯楃粙鎰礊?'}

                  </Button>

                );

              })()}

            </Space.Compact>

          </Form.Item>



          {/* 缂備焦顨忛崗娑氱博鐎靛憡鍋樼€光偓鐎ｎ剛鐛ラ梺鍛婂姈閻熴倗绱為弮鈧ˇ鐗堟償閵忋垹顥?+ 闂佸憡鐟﹂悷銈囪姳閵娧勫枂闁圭儤鍨甸?*/}

          <div style={{

            display: isMobile ? 'block' : 'flex',

            gap: isMobile ? 0 : 16,

            marginBottom: isMobile ? 0 : 12

          }}>

            <Form.Item

              label="闂佸憡鍔栭悷銈囩礊閺冣偓椤︾増鎯旈姀銏狀棔"

              tooltip="闂備緡鍋勯ˇ鎵偓姣稿┉闂佸憡甯楃粙鎰礊閺冨牆绫嶉柡鍫ユ涧閳诲繘鏌ｉ～顒€濡挎繛鍫熷灴瀹曟ê鈻庢惔锝団枙婵＄偛顑呯€涒晠鎮?"

              required

              style={{ flex: 1, marginBottom: isMobile ? 16 : 0 }}

            >

              <Select

                placeholder="闁荤姴娲ㄩ崗姗€鍩€椤掆偓椤︽壆鈧哎鍔戝畷妯衡枎鎼达絿鈻曟俊鐐差儏鐎涒晠鎮?"

                value={selectedStyleId}

                onChange={setSelectedStyleId}

                status={!selectedStyleId ? 'error' : undefined}

              >

                {writingStyles.map(style => (

                  <Select.Option key={style.id} value={style.id}>

                    {style.name}{style.is_default && ' (婵帗绋掗…鍫ヮ敇?'}

                  </Select.Option>

                ))}

              </Select>

              {!selectedStyleId && (

                <div style={{ color: '#ff4d4f', fontSize: 12, marginTop: 4 }}>闁荤姴娲ㄩ崗姗€鍩€椤掆偓椤︽壆鈧哎鍔戝畷妯衡枎鎼达絿鈻曟俊鐐差儏鐎涒晠鎮</div>

              )}

            </Form.Item>



            <Form.Item
              label="闂佸憡鐟﹂悷銈囪姳閵娧勫枂闁圭儤鍨甸?"
              tooltip="缂備焦顨忛崗娑氱博鐎涙顩查柧蹇撳ⅲ?闂?婵炲濯寸徊浠嬪矗閸℃稑绠涢柣鏂垮槻缁讳線鏌ㄥ☉娆戙€掓い鎴濇处缁嬪寰勬径瀣簞缂?婵?婵?闂佸搫娲﹀娆擃敇閸︻厽鍠嗛柛宀嬪楠炪垽鏌涜箛瀣闁绘搫绱曢幉鎾幢濮樺吋鍋ュ┑鐐跺皺閸嬬偤宕愬┑鍥┾枖闁逞屽墴瀹?"
              style={{ flex: 1, marginBottom: isMobile ? 16 : 0 }}
            >
              <Select

                placeholder={`婵＄偑鍊曞﹢鍗灻洪悧鍫付婵☆垱顑欓崥? ${getNarrativePerspectiveText(currentProject?.narrative_perspective)}`}

                value={temporaryNarrativePerspective}

                onChange={setTemporaryNarrativePerspective}

                allowClear

              >

                <Select.Option value="first_person">First person</Select.Option>

                <Select.Option value="third_person">Third person</Select.Option>

                <Select.Option value="omniscient">Omniscient</Select.Option>

              </Select>

              {temporaryNarrativePerspective && (

                <div style={{ color: 'var(--color-success)', fontSize: 12, marginTop: 4 }}>

                  闂?{getNarrativePerspectiveText(temporaryNarrativePerspective)}

                </div>

              )}
            </Form.Item>

            <Form.Item
              label="闂佽鍨伴幊鎾翠繆椤撱垺鈷撻柤鍛婎問閸?"
              tooltip="闁汇埄鍨奸崰鏍ㄦ叏?AI 闂佸憡甯囬崐鏍蓟閸ヮ剙瀚夋い鎺戝€昏ぐ鍧楁煛閸モ晩妫庢い鏇熷哺閺屟囧传閸曨厾锛涢梺闈涙閼冲爼锝為锕€绠婚柣鎴濇川缁€澶愬级閳哄伒鎴澪ｉ幖浣哥倞闁绘劕鐡ㄩ弳顏堟煛閳ь剟鎳滈崹顐㈢殺"
              style={{ flex: 1, marginBottom: isMobile ? 16 : 0 }}
            >
              <Select
                placeholder="闂佸憡鐟崹杈ㄦ櫠濠婂牆绀夐柕濠忕畱閻﹀綊鎮楃憴鍕暡缂佽鲸绻冪粙濠囨偄瀹勬媽顔夐柣鐘辫閺呮繈鏌堢€靛摜纾奸柣鏂挎啞椤忥繝鏌ょ€圭姵顥夐柛銊ょ矙瀵?"
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
                <Button size="small" onClick={applyInferredSinglePlotStage}>Apply inferred stage</Button>
                {selectedPlotStage && (
                  <span style={{ color: 'var(--color-success)', fontSize: 12 }}>
                    Selected: {CREATION_PLOT_STAGE_OPTIONS.find((item) => item.value === selectedPlotStage)?.label || selectedPlotStage}
                  </span>
                )}
              </Space>
            </Form.Item>
          </div>

          <Card
            size="small"
            title="Creation presets"
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
                  setSelectedCreativeMode(undefined);
                  setSelectedStoryFocus(undefined);
                }}
              >
                {"Reset selections"}
              </Button>
            </Space>

            {activeSingleCreationPreset && (
              <Alert
                type="info"
                showIcon
                style={{ marginTop: 12 }}
                message={`Preset: ${activeSingleCreationPreset.label}`}
                description={activeSingleCreationPreset.description}
              />
            )}

            {recommendedCreationPresets.length > 0 && (
              <Alert
                type="success"
                showIcon
                style={{ marginTop: 12 }}
                message={"Recommended presets"}
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
                      <div style={{ fontWeight: 600, marginBottom: 6 }}>Recommended preset</div>
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
                      <div style={{ fontWeight: 600, marginBottom: 6 }}>Recommended stage</div>
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
                      <div style={{ fontWeight: 600, marginBottom: 6 }}>Alternatives</div>
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
                      {isSingleStoryCreationControlCustomized ? 'Customized' : 'System'}
                    </Tag>
                    <Button
                      size="small"
                      type="link"
                      onClick={() => setSingleStoryCreationBriefDraft(singleSystemStoryCreationBrief)}
                      disabled={!singleSystemStoryCreationBrief || singleStoryCreationBriefDraft === singleSystemStoryCreationBrief}
                    >
                      Reset to system
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
                    <div style={{ fontWeight: 600, marginBottom: 6 }}>Story brief</div>
                    <div style={{ color: 'var(--color-text-secondary)', fontSize: 12, marginBottom: 8 }}>
                      Provide a short brief to guide generation.
                    </div>
                    <TextArea
                      value={singleStoryCreationBriefDraft}
                      onChange={(event) => setSingleStoryCreationBriefDraft(event.target.value)}
                      autoSize={{ minRows: 4, maxRows: 8 }}
                      maxLength={600}
                      showCount
                      placeholder="Describe the story briefly..."
                    />
                    <div style={{ color: 'var(--color-text-secondary)', fontSize: 12, marginTop: 8 }}>
                      {isSingleStoryCreationBriefCustomized
                        ? 'Using customized brief.'
                        : 'Using system brief.'
                      }</div>
                  </div>
                  <div style={{ padding: '10px 12px', border: '1px solid #f0f0f0', borderRadius: 8 }}>
                    <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', gap: 12, marginBottom: 8 }}>
                      <div>
                        <div style={{ fontWeight: 600, marginBottom: 4 }}>Story beats</div>
                        <div style={{ color: 'var(--color-text-secondary)', fontSize: 12 }}>
                          Plan key beats for the story.
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
                        Reset to system
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
                        ? 'Using customized beats.'
                        : 'Using system beats.'
                      }</div>
                  </div>
                  <div style={{ padding: '10px 12px', border: '1px solid #f0f0f0', borderRadius: 8 }}>
                    <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', gap: 12, marginBottom: 8 }}>
                      <div>
                        <div style={{ fontWeight: 600, marginBottom: 4 }}>Scene outline</div>
                        <div style={{ color: 'var(--color-text-secondary)', fontSize: 12 }}>
                          Outline scenes for this story.
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
                        Reset to system
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
                        ? 'Using customized outline.'
                        : 'Using system outline.'
                      }</div>
                  </div>
                  <div style={{ padding: '10px 12px', border: '1px solid #f0f0f0', borderRadius: 8 }}>
                    <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'flex-start', gap: 12, marginBottom: 8 }}>
                      <div>
                        <div style={{ fontWeight: 600, marginBottom: 4 }}>Prompt</div>
                        <div style={{ color: 'var(--color-text-secondary)', fontSize: 12 }}>
                          Generated prompt based on selections.
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
                        {`${singleStoryCreationPromptCharCount} chars`}
                      </Tag>
                    </Space>
                    {isSingleStoryCreationPromptVerbose && (
                      <Alert
                        type="warning"
                        showIcon
                        style={{ marginBottom: 8 }}
                        message="Verbose prompt enabled"
                        description="The prompt includes extra details and may be long."
                      />
                    )}
                    <TextArea
                      value={resolvedSingleStoryCreationBrief ?? ''}
                      autoSize={{ minRows: 6, maxRows: 12 }}
                      readOnly
                      placeholder="Prompt will appear here"
                    />
                  </div>
                  <StoryCreationSnapshotPanel
                    scopeLabel="single"
                    description="Snapshots for story creation."
                    emptyText="No snapshots yet."
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
                    <div style={{ fontWeight: 600, marginBottom: 6 }}>Execution path</div>
                    <Space direction="vertical" size={4} style={{ display: 'flex' }}>
                      {singleStoryCreationControlCard.executionPath.map((item) => (
                        <div key={item} style={{ color: 'var(--color-text-secondary)' }}>- {item}</div>
                      ))}
                    </Space>
                  </div>
                  <div style={{ padding: '10px 12px', border: '1px solid #f0f0f0', borderRadius: 8 }}>
                    <div style={{ fontWeight: 600, marginBottom: 6 }}>Expected outcomes</div>
                    <Space direction="vertical" size={4} style={{ display: 'flex' }}>
                      {singleStoryCreationControlCard.expectedOutcomes.map((item) => (
                        <div key={item} style={{ color: 'var(--color-text-secondary)' }}>- {item}</div>
                      ))}
                    </Space>
                  </div>
                  <div style={{ padding: '10px 12px', border: '1px solid #f0f0f0', borderRadius: 8 }}>
                    <div style={{ fontWeight: 600, marginBottom: 6 }}>Guardrails</div>
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
                extra={<Tag color="gold">Repair target</Tag>}
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
                    ["Priority target", singleStoryRepairTargetCard.priorityTarget],
                    ["Anti-pattern", singleStoryRepairTargetCard.antiPattern],
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
                    <div style={{ fontWeight: 600, marginBottom: 6 }}>Repair targets</div>
                    <Space direction="vertical" size={4} style={{ display: 'flex' }}>
                      {singleStoryRepairTargetCard.repairTargets.map((item) => (
                        <div key={item} style={{ color: 'var(--color-text-secondary)' }}>- {item}</div>
                      ))}
                    </Space>
                  </div>
                  <div style={{ padding: '10px 12px', border: '1px solid #f0f0f0', borderRadius: 8 }}>
                    <div style={{ fontWeight: 600, marginBottom: 6 }}>Preserve strengths</div>
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
            <Card size="small" title="Creation blueprint" style={{ marginBottom: 12 }}>
              <div style={{ color: 'var(--color-text-secondary)', marginBottom: 10 }}>
                {singleCreationBlueprint.summary}
              </div>
              <div style={{ fontWeight: 600, marginBottom: 8 }}>Recommended beats</div>
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
                  message="Risks"
                  description={singleCreationBlueprint.risks.join(', ')}
                />
              )}
            </Card>
          )}

          {singleStoryObjectiveCard && (
            <Card size="small" title="Story objective" style={{ marginBottom: 12 }}>
              <div style={{ color: 'var(--color-text-secondary)', marginBottom: 10 }}>
                {singleStoryObjectiveCard.summary}
              </div>
              <Space direction="vertical" size={8} style={{ display: 'flex' }}>
                {[
                  ['Objective', singleStoryObjectiveCard.objective],
                  ['Obstacle', singleStoryObjectiveCard.obstacle],
                  ['Turn', singleStoryObjectiveCard.turn],
                  ['Hook', singleStoryObjectiveCard.hook],
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
            <Card size="small" title="Story result" style={{ marginBottom: 12 }}>
              <div style={{ color: 'var(--color-text-secondary)', marginBottom: 10 }}>
                {singleStoryResultCard.summary}
              </div>
              <Space direction="vertical" size={8} style={{ display: 'flex' }}>
                {[
                  ['Progress', singleStoryResultCard.progress],
                  ['Reveal', singleStoryResultCard.reveal],
                  ['Relationship', singleStoryResultCard.relationship],
                  ['Fallout', singleStoryResultCard.fallout],
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
            <Card size="small" title="Execution checklist" style={{ marginBottom: 12 }}>
              <div style={{ color: 'var(--color-text-secondary)', marginBottom: 10 }}>
                {singleStoryExecutionChecklist.summary}
              </div>
              <Space direction="vertical" size={8} style={{ display: 'flex' }}>
                {[
                  ['Opening', singleStoryExecutionChecklist.opening],
                  ['Pressure', singleStoryExecutionChecklist.pressure],
                  ['Pivot', singleStoryExecutionChecklist.pivot],
                  ['Closing', singleStoryExecutionChecklist.closing],
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
            <Card size="small" title="Repetition risk" style={{ marginBottom: 12 }}>
              <div style={{ color: 'var(--color-text-secondary)', marginBottom: 10 }}>
                {singleStoryRepetitionRiskCard.summary}
              </div>
              <Space direction="vertical" size={8} style={{ display: 'flex' }}>
                {[
                  ['Opening risk', singleStoryRepetitionRiskCard.openingRisk],
                  ['Pressure risk', singleStoryRepetitionRiskCard.pressureRisk],
                  ['Pivot risk', singleStoryRepetitionRiskCard.pivotRisk],
                  ['Closing risk', singleStoryRepetitionRiskCard.closingRisk],
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
            <Card size="small" title="Acceptance checks" style={{ marginBottom: 12 }}>
              <div style={{ color: 'var(--color-text-secondary)', marginBottom: 10 }}>
                {singleStoryAcceptanceCard.summary}
              </div>
              <Space direction="vertical" size={8} style={{ display: 'flex' }}>
                {[
                  ['Mission check', singleStoryAcceptanceCard.missionCheck],
                  ['Change check', singleStoryAcceptanceCard.changeCheck],
                  ['Freshness check', singleStoryAcceptanceCard.freshnessCheck],
                  ['Closing check', singleStoryAcceptanceCard.closingCheck],
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
            <Card size="small" title="Character arc" style={{ marginBottom: 12 }}>
              <div style={{ color: 'var(--color-text-secondary)', marginBottom: 10 }}>
                {singleStoryCharacterArcCard.summary}
              </div>
              <Space direction="vertical" size={8} style={{ display: 'flex' }}>
                {[
                  ['External line', singleStoryCharacterArcCard.externalLine],
                  ['Internal line', singleStoryCharacterArcCard.internalLine],
                  ['Relationship line', singleStoryCharacterArcCard.relationshipLine],
                  ['Arc landing', singleStoryCharacterArcCard.arcLanding],
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
            <Card size="small" title="Volume pacing plan" style={{ marginBottom: 12 }}>
              <div style={{ color: 'var(--color-text-secondary)', marginBottom: 10 }}>
                {singleVolumePacingPlan.summary}
              </div>
              <Space direction="vertical" size={8} style={{ display: 'flex' }}>
                {singleVolumePacingPlan.segments.map((segment) => (
                  <div key={`${segment.stage}-${segment.startChapter}`}>
                    <strong>Chapters {segment.startChapter}-{segment.endChapter}: {segment.label}</strong>
                    <div style={{ color: 'var(--color-text-secondary)', marginTop: 2 }}>
                      {segment.mission}
                    </div>
                  </div>
                ))}
              </Space>
            </Card>
          )}

          <Form.Item
            label="Creative mode"
            tooltip="Choose a creative mode for the single story creation."
            style={{ marginBottom: isMobile ? 16 : 12 }}
          >
            <Select
              placeholder="Select a mode"
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
            label="缂傚倷鐒﹂幐濠氭倵椤栨稓鐟圭憸鏃堝闯閹间焦鍊?"
            tooltip="闂佸憡鐟崹鍫曞焵椤掆偓椤р偓缂佸彉鍗抽獮鎰板炊瑜忛弳浼村级閳哄倻鎳呴柣婵堝厴瀵挳寮堕幋婊呭墾婵炴垶鎹佸畷鐢稿吹鎼淬劌绠抽柕濞垮妿缁犲鏌曢崱鏇燁樂婵懓顦甸幃褔鍩℃笟鍥ㄦ櫈閻熸粓鍋婂鍧楀焵椤戣法顦﹂柛鐔绘硶缁絾鎷呯粙璺紦缂備胶瀚忛崨顖涙儯闂佸憡鐟﹂悷銈囪姳閵婏妇顩烽悹鍥ㄥ絻椤?"
            style={{ marginBottom: isMobile ? 16 : 12 }}
          >
            <Select
              placeholder="婵帗绋掗…鍫ヮ敇婵犳艾閿ら柛銉簵閳ь兘鍋撻梺鐟扮仛鐎笛勫垔鐎涙ê绶炴慨姗€纭稿姘舵煕閺冨倸鏋欓柛蹇旓耿閺佸秶浠﹂挊澶庮唹闂佸湱顭堥ˇ鏉匡耿閹殿喚鍗氶柣妯块哺瀹曟煡鏌涢弬鍛€撶划鐢告煟?"
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

          {/* 缂備焦顨忛崗娑氳姳閳哄啯鍋樼€光偓鐎ｎ剛鐛ラ梺鐑╂櫓閸犳鎮ラ敐鍥ｅ亾濞戞瑯娈曢柡?+ AI濠碘槅鍨埀顒€纾埀?*/}
          <div style={{
            display: isMobile ? 'block' : 'flex',
            gap: isMobile ? 0 : 16,
            marginBottom: isMobile ? 16 : 12

          }}>

            <Form.Item

              label="闂佺儵鏅╅崰妤呮偉閿濆洠鍋撳☉娆樻畷闁?"

              tooltip="AI闂佹眹鍨婚崰鎰板垂濮樿京鍗氶柣妯烘惈铻￠梺鍝勫暙婢у骸鈻撻幋锔藉剮妞ゆ梻鏅崹濂告倵濞戞瑯娈曢柡鍡欏枛閺佸秶浠﹂悾灞炬緰闂傚倸瀚幊搴ゃ亹閺屻儲鍤勯柣锝呮湰濞堬綁鏌￠崼婵愭Ц濞存粎顭堥蹇涱敋閸℃瑧顦╂繛锝呮祩閸犳寮總绋胯Е閹煎瓨绻勭粣妤呮煠婵傚绨诲┑顔规櫇閹峰锛愭担铏剐ら梺?"

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

                formatter={(value) => (value ? String(value) + ' words' : '')}
                parser={(value) => parseInt((value || '').replace(' words', ''), 10)}


              />

            </Form.Item>



            <Form.Item

              label="AI model"

              tooltip="Select a model for generation."

              style={{ flex: 1, marginBottom: isMobile ? 16 : 0 }}

            >

              <Select

                placeholder={selectedModel ? 'Selected: ' + selectedModel : 'Select a model'}

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

            title="Quality profile"

            style={{ marginBottom: 12 }}

          >

            {getQualityProfileDisplayItems(chapterQualityProfileSummary).length > 0 ? (

              <>

                <Alert

                  type="success"

                  showIcon

                  style={{ marginBottom: 12 }}

                  message="Quality profile summary"

                  description="This profile summarizes quality metrics and recommendations."

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

                message="No quality profile yet"

                description="Run analysis to generate the quality profile."

              />

            )}

          </Card>



          <Card

            size="small"

            title="Quality metrics"

            loading={chapterQualityLoading}

            style={{ marginBottom: 12 }}

          >
            {chapterQualityMetrics ? (
              <>
                {singleAfterScorecard && (
                  <Card size="small" title="After scorecard" style={{ marginBottom: 12 }}>
                    <Alert
                      type={singleAfterScorecard.verdictColor as 'success' | 'info' | 'warning' | 'error'}
                      showIcon
                      style={{ marginBottom: 12 }}
                      message={singleAfterScorecard.verdict}
                      description={singleAfterScorecard.summary}
                    />
                    <Descriptions column={1} size="small" style={{ marginBottom: 12 }}>
                      <Descriptions.Item label="Focus check">
                        {singleAfterScorecard.focusCheck}
                      </Descriptions.Item>
                      <Descriptions.Item label="Next action">
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
                  <Descriptions.Item label="Overall score">
                    <Tag color={getOverallScoreColor(chapterQualityMetrics.overall_score)}>
                      {chapterQualityMetrics.overall_score}
                    </Tag>

                  </Descriptions.Item>

                  <Descriptions.Item label="Conflict chain hit rate">

                    <Tag color={getMetricRateColor(chapterQualityMetrics.conflict_chain_hit_rate)}>{chapterQualityMetrics.conflict_chain_hit_rate}%</Tag>

                  </Descriptions.Item>

                  <Descriptions.Item label="Rule grounding hit rate">

                    <Tag color={getMetricRateColor(chapterQualityMetrics.rule_grounding_hit_rate)}>{chapterQualityMetrics.rule_grounding_hit_rate}%</Tag>

                  </Descriptions.Item>

                  <Descriptions.Item label="Opening hook rate">

                    <Tag color={getMetricRateColor(chapterQualityMetrics.opening_hook_rate)}>{chapterQualityMetrics.opening_hook_rate}%</Tag>

                  </Descriptions.Item>

                  <Descriptions.Item label="Payoff chain rate">

                    <Tag color={getMetricRateColor(chapterQualityMetrics.payoff_chain_rate)}>{chapterQualityMetrics.payoff_chain_rate}%</Tag>

                  </Descriptions.Item>

                  <Descriptions.Item label="Cliffhanger rate">

                    <Tag color={getMetricRateColor(chapterQualityMetrics.cliffhanger_rate)}>{chapterQualityMetrics.cliffhanger_rate}%</Tag>

                  </Descriptions.Item>

                  <Descriptions.Item label="Dialogue naturalness rate">

                    <Tag color={getMetricRateColor(chapterQualityMetrics.dialogue_naturalness_rate)}>{chapterQualityMetrics.dialogue_naturalness_rate}%</Tag>

                  </Descriptions.Item>

                  <Descriptions.Item label="Outline alignment rate">

                    <Tag color={getMetricRateColor(chapterQualityMetrics.outline_alignment_rate)}>{chapterQualityMetrics.outline_alignment_rate}%</Tag>

                  </Descriptions.Item>

                  <Descriptions.Item label="Generated at">

                    {chapterQualityGeneratedAt ? new Date(chapterQualityGeneratedAt).toLocaleString() : 'Not generated yet'}

                  </Descriptions.Item>

                </Descriptions>

                <Alert

                  type={getWeakestQualityMetric(chapterQualityMetrics).value >= 60 ? 'info' : 'warning'}

                  showIcon

                  style={{ marginTop: 12 }}

                  message="Weakest metric"

                  description="Improve the weakest metric to raise overall quality."

                />

                <Card size="small" title="Metric details" style={{ marginTop: 12 }}>

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

                message="No quality metrics yet."

                description="Run analysis to generate quality metrics."

              />

            )}

          </Card>



          <Form.Item label="Chapter content" name="content">

            <TextArea

              ref={contentTextAreaRef}

              rows={isMobile ? 12 : 20}

              placeholder="Enter chapter content..."

              style={{ fontFamily: 'monospace', fontSize: isMobile ? 12 : 14 }}

            />

          </Form.Item>



          {/* 闁诲繒鍋愰崑鎾绘⒑椤斿搫濮傞柛锝嗘倐瀹曟ê鈻庨幇顖滄闂佸憡鏌ｉ崝灞惧閹版澘绀傞梺鍨儑閸?*/}

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

                  闂佸憡鐟﹂悧妤冪矓?

                </Button>

                <Button

                  type="primary"

                  htmlType="submit"

                  block={isMobile}

                >

                  婵烇絽娲︾换鍌炴偤閵娧呭崥闁绘ê鎼灐

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



            // 闂佸憡甯￠弨閬嶅蓟婵犲嫮鍗氶柣妯烘惈铻￠梺鍛婂笚椤ㄥ濡撮崘鈺冾浄闁靛闄勯埢鏃傜磼閳ь剟鎮滃Ο璁崇帛闂佸搫鍊瑰姗€宕€电硶鍋?

            refreshChapters();



            // 闂佸憡甯￠弨閬嶅蓟婵犲啨浜滈柛锔诲幗缁愭菐閸ワ絽澧插ù鐓庢噺缁傛帡濡烽敂鐣屽嚱闂佸搫鍊瑰姗€鎮鸿瀵偊骞嶉鎯х厷闁?

            if (currentProject) {

              projectApi.getProject(currentProject.id)

                .then(updatedProject => {

                  setCurrentProject(updatedProject);

                })

                .catch(error => {

                  console.error('闂佸憡甯￠弨閬嶅蓟婵犲啨浜滈柛锔诲幗缁愭菐閸ワ絽澧插ù鐓庢噺瀵板嫭娼忛銉?', error);

                });

            }



            // 閻庣偣鍊栭崕鑲╂崲?00ms闂佸憡鑹炬鎼佸春濞戙垹妫橀柟宄扮焾閸ゅ绱掗弮鍌毿┑鈽嗗弮閹啴宕熼銏⑩偓濠氭煛鐎ｎ偄濮夊┑顔芥倐楠炩偓濞撴艾锕︾粈澶岀磽娴ｅ湱鎳冮柟顔筋殘缁晠顢涘┑鍡楁灆婵犮垹澧庨崰鎰渻閸岀偞鈷掗柡澶嬪灩閺嗘岸鏌熺€涙ê濮堥柡鍡欏枛楠炴垿顢欓懖鈺傜殤闂佸憡鍔栭悷銉╁矗?

            if (analysisChapterId) {

              const chapterIdToRefresh = analysisChapterId;



              setTimeout(() => {

                refreshChapterAnalysisTask(chapterIdToRefresh)

                  .catch(error => {

                    console.error('闂佸憡甯￠弨閬嶅蓟婵犲洤绀嗛柛鈩冾焽閳ь兛绮欓幃鈺呮嚋绾版ê浜惧ù锝嗘偠娴滃ジ鎮?', error);

                    // 婵犵鈧啿鈧綊鎮樻径鎰摕闁靛鐓堥崵鍕熆閹壆绨块悷娆欑畵閺佸秶浠﹂挊澶嬫珒閻庣偣鍊栭崕鑲╂崲濠婂懍鐒婃繝闈涳功濡茬鈽夐幘顖氫壕濠?

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



      {/* 闂佸綊娼х紞濠囧闯濞差亝鍋ㄩ柣鏃傤焾閻忓洭鎮楅悽娈挎敯闁伙箒妫勯々?*/}

      <Modal

        title={

          <Space>

            <RocketOutlined style={{ color: '#722ed1' }} />

            <span>闂佸綊娼х紞濠囧闯濞差亝鍋ㄩ柣鏃傤焾閻忓洨绱掗弮鍌毿┑鈽嗗弮瀹曟﹢宕ㄩ褍鏅</span>

          </Space>

        }

        open={batchGenerateVisible}

        onCancel={() => setBatchGenerateVisible(false)}

        footer={!batchGenerating ? (

          <Space style={{ width: '100%', justifyContent: 'flex-end', flexWrap: 'wrap' }}>

            <Button onClick={() => setBatchGenerateVisible(false)}>

              闂佸憡鐟﹂悧妤冪矓?

            </Button>

            <Button type="primary" icon={<RocketOutlined />} onClick={() => batchForm.submit()}>

              閻庢鍠掗崑鎾斥攽椤旂⒈鍎愬瑙勫浮閺屽矁绠涢弴鐔告闂?

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

              message="Batch generation will create multiple chapters based on the current settings."

              type="info"

              showIcon

              style={{ marginBottom: 16 }}

            />



            {/* 缂備焦顨忛崗娑氱博鐎靛憡鍋樼€光偓鐎ｎ剛鐛ラ柣鐘欏啫妲绘い顐ｅ姉缁晠鎮╅崫鍕?+ 闂佹眹鍨婚崰鎰板垂濮樿泛鏋佸ù鍏兼綑濞?*/}

            <div style={{ display: 'flex', flexDirection: isMobile ? 'column' : 'row', gap: isMobile ? 0 : 16 }}>

              <Form.Item

                label="Start chapter"

                name="startChapterNumber"

                rules={[{ required: true, message: 'Start chapter is required.' }]}

                style={{ flex: 1, marginBottom: 12 }}

              >

                <Select placeholder="Select a chapter">

                  {batchStartChapterOptions.map(ch => (

                    <Select.Option key={ch.id} value={ch.chapter_number}>

                      {'Chapter ' + ch.chapter_number + ': ' + ch.title}

                    </Select.Option>

                  ))}

                </Select>

              </Form.Item>



              <Form.Item

                label="Chapter count"

                name="count"

                rules={[{ required: true, message: 'Chapter count is required.' }]}

                style={{ marginBottom: 12 }}

              >

                <Radio.Group buttonStyle="solid" size={isMobile ? 'small' : 'middle'}>

                  <Radio.Button value={5}>5 chapters</Radio.Button>

                  <Radio.Button value={10}>10 chapters</Radio.Button>

                  <Radio.Button value={15}>15 chapters</Radio.Button>

                  <Radio.Button value={20}>20 chapters</Radio.Button>

                </Radio.Group>

              </Form.Item>

            </div>



            {/* 缂備焦顨忛崗娑氳姳閳哄啯鍋樼€光偓鐎ｎ剛鐛ラ梺鍛婂姈閻熴倗绱為弮鈧ˇ鐗堟償閵忋垹顥?+ 闂佺儵鏅╅崰妤呮偉閿濆洠鍋撳☉娆樻畷闁?*/}

            <div style={{ display: 'flex', flexDirection: isMobile ? 'column' : 'row', gap: isMobile ? 0 : 16 }}>
              <Form.Item
                label="Writing style"
                name="styleId"
                rules={[{ required: true, message: 'Writing style is required.' }]}

                style={{ flex: 1, marginBottom: 12 }}

              >

                <Select

                  placeholder="Select a writing style"

                  showSearch

                  optionFilterProp="children"

                >

                  {writingStyles.map(style => (

                    <Select.Option key={style.id} value={style.id}>

                      {style.name}{style.is_default && ' (Default)'}

                    </Select.Option>

                  ))}

                </Select>

              </Form.Item>



              <Form.Item

                label="Target word count"

                name="targetWordCount"

                rules={[{ required: true, message: 'Target word count is required.' }]}

                tooltip="Used to guide batch generation length."

                style={{ flex: 1, marginBottom: 12 }}

              >

                <InputNumber<number>

                  min={500}

                  max={10000}

                  step={100}

                  style={{ width: '100%' }}

                  formatter={(value) => (value ? String(value) + ' words' : '')}

                  parser={(value) => parseInt((value || '').replace(' words', ''), 10)}

                  onChange={(value) => {

                    if (value) {

                      setCachedWordCount(value);

                    }

                  }}

                />
              </Form.Item>
            </div>

            <Form.Item
              label="Plot stage"
              tooltip="Choose a plot stage for batch creation."
              style={{ marginBottom: 12 }}
            >
              <Select
                placeholder="Select a plot stage"
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
                <Button size="small" onClick={applyInferredBatchPlotStage}>Apply inferred stage</Button>
                {batchSelectedPlotStage && (
                  <span style={{ color: 'var(--color-success)', fontSize: 12 }}>
                    Selected: {CREATION_PLOT_STAGE_OPTIONS.find((item) => item.value === batchSelectedPlotStage)?.label || batchSelectedPlotStage}
                  </span>
                )}
              </Space>
            </Form.Item>

            <Card
              size="small"
              title="Creation presets"
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
                    setBatchSelectedCreativeMode(undefined);
                    setBatchSelectedStoryFocus(undefined);
                  }}
                >
                  {"Reset selections"}
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
                        <div style={{ fontWeight: 600, marginBottom: 6 }}>Recommended preset</div>
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
                      <div style={{ fontWeight: 600, marginBottom: 6 }}>Recommended stage</div>
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
                        <div style={{ fontWeight: 600, marginBottom: 6 }}>Alternatives</div>
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
                          {"Apply preset"}
                        </Button>
                      )}
                      {batchScoreDrivenRecommendationCard.recommendedStage && (
                        <Button size="small" onClick={() => setBatchSelectedPlotStage(batchScoreDrivenRecommendationCard.recommendedStage)}>
                          {"Apply stage"}
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
                          {"Apply recommendations"}
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
                        {isBatchStoryCreationControlCustomized ? 'Customized' : 'System'}
                      </Tag>
                      <Button
                        size="small"
                        type="link"
                        onClick={() => setBatchStoryCreationBriefDraft(batchSystemStoryCreationBrief)}
                        disabled={!batchSystemStoryCreationBrief || batchStoryCreationBriefDraft === batchSystemStoryCreationBrief}
                      >
                        Reset to system
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
                      <div style={{ fontWeight: 600, marginBottom: 6 }}>Story brief</div>
                      <div style={{ color: 'var(--color-text-secondary)', fontSize: 12, marginBottom: 8 }}>
                        Provide a short brief to guide generation.
                      </div>
                      <TextArea
                        value={batchStoryCreationBriefDraft}
                        onChange={(event) => setBatchStoryCreationBriefDraft(event.target.value)}
                        autoSize={{ minRows: 4, maxRows: 8 }}
                        maxLength={600}
                        showCount
                        placeholder="Describe the story briefly..."
                      />
                      <div style={{ color: 'var(--color-text-secondary)', fontSize: 12, marginTop: 8 }}>
                        {isBatchStoryCreationBriefCustomized
                          ? 'Using customized brief.'
                          : 'Using system brief.'
                        }</div>
                    </div>
                    <div style={{ padding: '10px 12px', border: '1px solid #f0f0f0', borderRadius: 8 }}>
                      <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', gap: 12, marginBottom: 8 }}>
                        <div>
                          <div style={{ fontWeight: 600, marginBottom: 4 }}>Story beats</div>
                          <div style={{ color: 'var(--color-text-secondary)', fontSize: 12 }}>
                            Plan key beats for the story.
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
                          Reset to system
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
                          ? 'Using customized beats.'
                          : 'Using system beats.'
                        }</div>
                    </div>
                    <div style={{ padding: '10px 12px', border: '1px solid #f0f0f0', borderRadius: 8 }}>
                      <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', gap: 12, marginBottom: 8 }}>
                        <div>
                          <div style={{ fontWeight: 600, marginBottom: 4 }}>Scene outline</div>
                          <div style={{ color: 'var(--color-text-secondary)', fontSize: 12 }}>
                            Outline scenes for this story.
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
                          Reset to system
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
                          ? 'Using customized outline.'
                          : 'Using system outline.'
                        }</div>
                    </div>
                    <div style={{ padding: '10px 12px', border: '1px solid #f0f0f0', borderRadius: 8 }}>
                      <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'flex-start', gap: 12, marginBottom: 8 }}>
                        <div>
                          <div style={{ fontWeight: 600, marginBottom: 4 }}>Prompt</div>
                          <div style={{ color: 'var(--color-text-secondary)', fontSize: 12 }}>
                            Generated prompt based on selections.
                          </div>
                        </div>
                        <Button
                          size="small"
                          type="link"
                          disabled={!resolvedBatchStoryCreationBrief}
                          onClick={() => void copyStoryCreationPrompt(resolvedBatchStoryCreationBrief, 'batch')}
                        >
                          Copy prompt
                        </Button>
                      </div>
                      <Space wrap size={[8, 8]} style={{ marginBottom: 8 }}>
                        {batchStoryCreationPromptLayerLabels.map((item) => (
                          <Tag key={item} color="processing">{item}</Tag>
                        ))}
                        <Tag color={isBatchStoryCreationPromptVerbose ? 'gold' : 'blue'}>
                          {batchStoryCreationPromptCharCount + ' chars'}
                        </Tag>
                      </Space>
                      {isBatchStoryCreationPromptVerbose && (
                        <Alert
                          type="warning"
                          showIcon
                          style={{ marginBottom: 8 }}
                          message="Verbose prompt enabled"
                          description="The prompt includes detailed guidance for generation."
                        />
                      )}
                      <TextArea
                        value={resolvedBatchStoryCreationBrief ?? ''}
                        autoSize={{ minRows: 6, maxRows: 12 }}
                        readOnly
                        placeholder="Prompt preview will appear here."
                      />
                    </div>
                    <StoryCreationSnapshotPanel
                      scopeLabel="batch"
                      description="Save prompt snapshots for reuse."
                      emptyText="No snapshots yet."
                      snapshots={batchStoryCreationSnapshots}
                      currentDraft={batchStoryCreationCurrentDraft}
                      canSave={canSaveBatchStoryCreationSnapshot}
                      onSave={() => void saveBatchStoryCreationSnapshot('manual')}
                      onApply={applyBatchStoryCreationSnapshot}
                      onDelete={deleteBatchStoryCreationSnapshot}
                      onCopy={copyStoryCreationPrompt}
                    />
                    <div style={{ padding: '10px 12px', border: '1px solid #f0f0f0', borderRadius: 8 }}>
                      <div style={{ fontWeight: 600, marginBottom: 6 }}>Execution path</div>
                      <Space direction="vertical" size={4} style={{ display: 'flex' }}>
                        {batchStoryCreationControlCard.executionPath.map((item) => (
                          <div key={item} style={{ color: 'var(--color-text-secondary)' }}>- {item}</div>
                        ))}
                      </Space>
                    </div>
                    <div style={{ padding: '10px 12px', border: '1px solid #f0f0f0', borderRadius: 8 }}>
                      <div style={{ fontWeight: 600, marginBottom: 6 }}>Expected outcomes</div>
                      <Space direction="vertical" size={4} style={{ display: 'flex' }}>
                        {batchStoryCreationControlCard.expectedOutcomes.map((item) => (
                          <div key={item} style={{ color: 'var(--color-text-secondary)' }}>- {item}</div>
                        ))}
                      </Space>
                    </div>
                    <div style={{ padding: '10px 12px', border: '1px solid #f0f0f0', borderRadius: 8 }}>
                      <div style={{ fontWeight: 600, marginBottom: 6 }}>Guardrails</div>
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
                extra={<Tag color="gold">Repair focus</Tag>}
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
                    ['Priority target', batchStoryRepairTargetCard.priorityTarget],
                    ['Anti pattern', batchStoryRepairTargetCard.antiPattern],
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
                    <div style={{ fontWeight: 600, marginBottom: 6 }}>Repair targets</div>
                    <Space direction="vertical" size={4} style={{ display: 'flex' }}>
                      {batchStoryRepairTargetCard.repairTargets.map((item) => (
                        <div key={item} style={{ color: 'var(--color-text-secondary)' }}>- {item}</div>
                      ))}
                    </Space>
                  </div>
                  <div style={{ padding: '10px 12px', border: '1px solid #f0f0f0', borderRadius: 8 }}>
                    <div style={{ fontWeight: 600, marginBottom: 6 }}>Preserve strengths</div>
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
              <Card size="small" title="Batch creation blueprint" style={{ marginBottom: 12 }}>
                <div style={{ color: 'var(--color-text-secondary)', marginBottom: 10 }}>
                  {batchCreationBlueprint.summary}
                </div>
                <div style={{ fontWeight: 600, marginBottom: 8 }}>Key beats</div>
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
                    message="Risks"
                    description={batchCreationBlueprint.risks.join(', ')}
                  />
                )}
              </Card>
            )}

            {batchStoryObjectiveCard && (
              <Card size="small" title="Story objective" style={{ marginBottom: 12 }}>
                <div style={{ color: 'var(--color-text-secondary)', marginBottom: 10 }}>
                  {batchStoryObjectiveCard.summary}
                </div>
                <Space direction="vertical" size={8} style={{ display: 'flex' }}>
                  {[
                    ['Objective', batchStoryObjectiveCard.objective],
                    ['Obstacle', batchStoryObjectiveCard.obstacle],
                    ['Turn', batchStoryObjectiveCard.turn],
                    ['Hook', batchStoryObjectiveCard.hook],
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
              <Card size="small" title="Story result" style={{ marginBottom: 12 }}>
                <div style={{ color: 'var(--color-text-secondary)', marginBottom: 10 }}>
                  {batchStoryResultCard.summary}
                </div>
                <Space direction="vertical" size={8} style={{ display: 'flex' }}>
                  {[
                    ['Progress', batchStoryResultCard.progress],
                    ['Reveal', batchStoryResultCard.reveal],
                    ['Relationship', batchStoryResultCard.relationship],
                    ['Fallout', batchStoryResultCard.fallout],
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
              <Card size="small" title="Execution checklist" style={{ marginBottom: 12 }}>
                <div style={{ color: 'var(--color-text-secondary)', marginBottom: 10 }}>
                  {batchStoryExecutionChecklist.summary}
                </div>
                <Space direction="vertical" size={8} style={{ display: 'flex' }}>
                  {[
                    ['Opening', batchStoryExecutionChecklist.opening],
                    ['Pressure', batchStoryExecutionChecklist.pressure],
                    ['Pivot', batchStoryExecutionChecklist.pivot],
                    ['Closing', batchStoryExecutionChecklist.closing],
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
              <Card size="small" title="Repetition risks" style={{ marginBottom: 12 }}>
                <div style={{ color: 'var(--color-text-secondary)', marginBottom: 10 }}>
                  {batchStoryRepetitionRiskCard.summary}
                </div>
                <Space direction="vertical" size={8} style={{ display: 'flex' }}>
                  {[
                    ['Opening risk', batchStoryRepetitionRiskCard.openingRisk],
                    ['Pressure risk', batchStoryRepetitionRiskCard.pressureRisk],
                    ['Pivot risk', batchStoryRepetitionRiskCard.pivotRisk],
                    ['Closing risk', batchStoryRepetitionRiskCard.closingRisk],
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
              <Card size="small" title="Acceptance checklist" style={{ marginBottom: 12 }}>
                <div style={{ color: 'var(--color-text-secondary)', marginBottom: 10 }}>
                  {batchStoryAcceptanceCard.summary}
                </div>
                <Space direction="vertical" size={8} style={{ display: 'flex' }}>
                  {[
                    ['Mission check', batchStoryAcceptanceCard.missionCheck],
                    ['Change check', batchStoryAcceptanceCard.changeCheck],
                    ['Freshness check', batchStoryAcceptanceCard.freshnessCheck],
                    ['Closing check', batchStoryAcceptanceCard.closingCheck],
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
              <Card size="small" title="Character arc" style={{ marginBottom: 12 }}>
                <div style={{ color: 'var(--color-text-secondary)', marginBottom: 10 }}>
                  {batchStoryCharacterArcCard.summary}
                </div>
                <Space direction="vertical" size={8} style={{ display: 'flex' }}>
                  {[
                    ['External line', batchStoryCharacterArcCard.externalLine],
                    ['Internal line', batchStoryCharacterArcCard.internalLine],
                    ['Relationship line', batchStoryCharacterArcCard.relationshipLine],
                    ['Arc landing', batchStoryCharacterArcCard.arcLanding],
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
              <Card size="small" title="Volume pacing plan" style={{ marginBottom: 12 }}>
                <div style={{ color: 'var(--color-text-secondary)', marginBottom: 10 }}>
                  {batchVolumePacingPlan.summary}
                </div>
                <Space direction="vertical" size={8} style={{ display: 'flex' }}>
                  {batchVolumePacingPlan.segments.map((segment) => (
                    <div key={segment.stage + '-' + segment.startChapter}>
                      <strong>{'Segment ' + segment.startChapter + '-' + segment.endChapter + ': ' + segment.label}</strong>
                      <div style={{ color: 'var(--color-text-secondary)', marginTop: 2 }}>
                        {segment.mission}
                      </div>
                    </div>
                  ))}
                </Space>
              </Card>
            )}

            <Form.Item
              label="Creative mode"
              tooltip="Select a creative mode for batch creation."
              style={{ marginBottom: 12 }}
            >
              <Select
                placeholder="Select a creative mode"
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
              label="Story focus"
              tooltip="Choose a focus for batch creation."
              style={{ marginBottom: 12 }}
            >
              <Select
                placeholder="Select a story focus"
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

            {/* 缂備焦顨忛崗娑氱箔娴ｇ儤鍋樼€光偓鐎ｎ剛鐛I濠碘槅鍨埀顒€纾埀?+ 闂佸憡鑹炬姝屻亹閹绢喖绀嗛柛鈩冾焽閳?*/}
            <div style={{ display: 'flex', flexDirection: isMobile ? 'column' : 'row', gap: isMobile ? 0 : 16 }}>
            <Form.Item
              label="AI model"
              tooltip="Choose a model for batch generation."
              style={{ flex: 1, marginBottom: 12 }}
            >
              <Select
                placeholder={batchSelectedModel
                  ? `Selected: ${availableModels.find(m => m.value === batchSelectedModel)?.label || batchSelectedModel}`
                  : 'Select a model'}
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

                label="闂佸憡鑹炬姝屻亹閹绢喖绀嗛柛鈩冾焽閳?"

                name="enableAnalysis"

                tooltip="濠殿喗绻愮徊楣冨几閸愵亖鍋撻悷鐗堟拱闁搞劍宀稿畷銉︽償閳ヨ櫕娅冮梺鍛婅壘妤犳瓕銇愰幘顔肩闁糕剝顭囬埀顑跨矙閺佸秶浠﹂懖鈺冩喒闂傚倸鍟抽褔銆侀敐鍥╁崥闁绘ê鎼灐闂佹眹鍨婚崰鎰板垂?"

                style={{ marginBottom: 12 }}

              >

                <Radio.Group>

                  <Radio value={true}>

                    <span style={{ fontSize: 12, color: '#52c41a' }}>闂?闂佹眹鍨婚崰鎰板垂濮樻墎鍋撻悷鐗堟拱闁搞劍宀稿畷銉︽償閵堝懏顔囬梺鍛婃煟閸斿矂骞冨Δ鍛煑闁哄鐏濋悗濠氭煛</span>

                  </Radio>

                  <Radio value={false}>

                    <span style={{ fontSize: 12, color: '#8c8c8c' }}>婵炲濮撮幊鎰板极閹捐绠ｉ柟閭﹀弾閸斺偓闂佸搫鍊稿ù鍕濞嗘垹鐭欑€广儱鎳忛崐鐢告煙闂堟侗鍎忓┑顔规櫊瀹曟岸宕卞Ο灏栧亾娴犲鏅</span>

                  </Radio>

                </Radio.Group>

              </Form.Item>

            </div>

          </Form>

        ) : (

          <div>

            <Alert

              message="濠电偞鎸撮弲鐘虹亪闂佸湱绮崝妤呭Φ?"

              description={

                <ul style={{ margin: '8px 0 0 0', paddingLeft: 20 }}>

                  <li>闂佸綊娼х紞濠囧闯濞差亝鍋ㄩ柣鏃傤焾閻忓洭姊婚崶锝呬壕闁荤喐娲戝鎺旂博鐎电硶鍋撶憴鍕暡婵＄偛鍊垮缁樻綇閸撗咁槷闂佸憡鐟崹顖涚閹烘绀嗛柛銉ｅ妼鎼村﹪鏌涢幒鎾寸凡闁告瑧鍋撶粋鎺楀冀椤撴壕鍋撴径鎰棃</li>

                  <li>闂佺绻戞繛濠偽涚€涙ǜ浜滈柣銏犳啞濡椼劑鏌涘顒勫弰闁革絾鎮傚顒勬偋閸績鍙洪悗娈垮枓閸嬫捇鏌ㄥ☉妯侯殭缂佸彉鍗抽幊娑㈩敂閸曨倣妤呮煙椤撴粌鐏╂い锕€寮剁粋鎺旀嫚閹绘帩娼抽柡澶嗘櫆缁嬫垹鈧</li>

                  <li>闂佸憡鐟崹顖涚閹烘鈷曢煫鍥ㄦ⒐椤ρ囨煟閹邦喗鍤€闁?闂佸憡鐟﹂悧妤冪矓闁垮顩烽悹鍥ㄥ絻椤?闂佸湱顭堥ˇ鐢稿箰閾忣偆鈻旀い鎾跺櫏閸撻箖鏌ｉ姀銏犳瀾闁</li>

                  {batchProgress?.estimated_time_minutes && batchProgress.completed === 0 && (

                    <li>闂佸啿鐡ㄩ崬鑽ょ箔?婵☆偅婢樼€氼垶顢橀幖浣瑰殌婵°倓鐒﹂ˇ褔鏌ㄥ☉娆愮殤閻?{batchProgress.estimated_time_minutes} 闂佸憡甯掑Λ婵嬪箰</li>

                  )}

                  {batchProgress?.quality_metrics_summary?.avg_overall_score !== undefined && (

                    <li>

                      濡絽鍟幆?濡ょ姷鍋涢崯鍨焽鎼淬劌绀堢憸搴ㄥ磿韫囨洘瀚氶柛鏇ㄥ亜閻庡鏌ㄥ☉娆愮殤闁诡噯缍佸畷?{batchProgress.quality_metrics_summary.avg_overall_score}

                      闂佹寧绋戦悧鍡涘疮鐠恒劎鐜诲〒姘ｅ亾闁?{batchProgress.quality_metrics_summary.avg_conflict_chain_hit_rate}% /

                      闁荤喐鐟ョ€氼剟宕归鐐村闁芥ê顦伴崟?{batchProgress.quality_metrics_summary.avg_rule_grounding_hit_rate}% /

                      閻庢鍠掗崑鎾绘煕閿旇姤銇濋柟鍓插墰閳?{batchProgress.quality_metrics_summary.avg_opening_hook_rate ?? 0}% /

                      闂佺粯鐗滈弲顐﹀磻閿濆鐓?{batchProgress.quality_metrics_summary.avg_payoff_chain_rate ?? 0}% /

                      缂備焦姊绘慨鎾偄椤掑嫭鐓㈤柍杞拌兌閹?{batchProgress.quality_metrics_summary.avg_cliffhanger_rate ?? 0}%闂?

                    </li>

                  )}

                </ul>

              }

              type="info"

              showIcon

              style={{ marginBottom: 16 }}

            />



            {batchProgress?.quality_profile_summary && getQualityProfileDisplayItems(batchProgress.quality_profile_summary).length > 0 && (

              <Card size="small" title="Quality profile summary" style={{ marginBottom: 16 }}>

                <Alert

                  type="success"

                  showIcon

                  style={{ marginBottom: 12 }}

                  message="Quality profile summary"

                  description="Review the guidance below."

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
              <Card size="small" title="Quality metrics summary" style={{ marginBottom: 16 }}>
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

                    title: '缂佺虎鍙庨崰娑㈩敇婵犳艾鐭楅柡宥冨妿鍟?',

                    content: '缂佺虎鍙庨崰鏍偩閸撗勫暫濞达絿顭堢徊鎸庣箾閹存繄澧涘瑙勫浮閺屽矁绠涢弴鐔告闂佺懓鐡ㄩ崝鏇㈠箖瑜旈弫宥夋偄瀹勬澘娈ラ梺姹囧灮閸犳劙宕瑰鑸靛剭闁告洦鍘捐ぐ鍧楁煠閸濆嫬鈧悂鎯冮姀锛勨攳婵犻潧鐗婂▓宀勬煏?',

                    okText: '缂佺虎鍙庨崰鏍偩妤ｅ啫鐭楅柡宥冨妿鍟?',

                    cancelText: '缂傚倷缍€閸涱垱鏆伴梺姹囧灮閸犳劙宕?',

                    okButtonProps: { danger: true },

                    onOk: handleCancelBatchGenerate,

                  });

                }}

              >

                闂佸憡鐟﹂悧妤冪矓闁垮顩烽悹鍥ㄥ絻椤?

              </Button>

            </div>

          </div>

        )}

      </Modal>



      {/* 闂佸憡顨嗗ú婊堟偟閻戣姤鍤嶉柛灞剧矋閺呮悂鏌熺€涙ê濮岀紒缁樕戦幆鏃堟晜閼恒儮鏋栫紓浣插亾?*/}

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



      {/* 闂佸綊娼х紞濠囧闯濞差亝鍋ㄩ柣鏃傤焾閻忓洭寮堕埡鍌溾槈閻庤濞婂浼村礈瑜嬫禒?- 婵炶揪缍€濞夋洟寮妶鍥╃＜闁绘柨澧庨閬嶆煟閵娿儱顏紒缁樕戦幆鏃堟晜閸撗呯厑婵?*/}

      {batchGenerating ? (

        <Suspense fallback={null}>

          <LazySSEProgressModal

            visible={batchGenerating}

            progress={batchProgress ? Math.round((batchProgress.completed / batchProgress.total) * 100) : 0}

            message={

              batchProgress?.current_chapter_number

                ? `濠殿喗绻愮徊钘夛耿椤忓牊鍋ㄩ柣鏃傤焾閻忓洨绱?${batchProgress.current_chapter_number} 缂?.. (${batchProgress.completed}/${batchProgress.total})${

                    batchProgress.latest_quality_metrics?.overall_score !== undefined

                      ? ` 闂佹寧绻冪划蹇涙儊鎼淬劌绀?${batchProgress.latest_quality_metrics.overall_score}`

                      : ''

                  }`

                : `闂佸綊娼х紞濠囧闯濞差亝鍋ㄩ柣鏃傤焾閻忓洭寮堕埡鍌滎灱妞ゃ垺鍨剁粙?.. (${batchProgress?.completed || 0}/${batchProgress?.total || 0})${

                    batchProgress?.latest_quality_metrics?.overall_score !== undefined

                      ? ` 闂佹寧绻冪划蹇涙儊鎼淬劌绀?${batchProgress.latest_quality_metrics.overall_score}`

                      : ''

                  }`

            }

            title="闂佸綊娼х紞濠囧闯濞差亝鍋ㄩ柣鏃傤焾閻忓洨绱掗弮鍌毿┑?"

            onCancel={() => {

              modal.confirm({

                title: '缂佺虎鍙庨崰娑㈩敇婵犳艾鐭楅柡宥冨妿鍟?',

                content: '缂佺虎鍙庨崰鏍偩閸撗勫暫濞达絿顭堢徊鎸庣箾閹存繄澧涘瑙勫浮閺屽矁绠涢弴鐔告闂佺懓鐡ㄩ崝鏇㈠箖瑜旈弫宥夋偄瀹勬澘娈ラ梺姹囧灮閸犳劙宕瑰鑸靛剭闁告洦鍘捐ぐ鍧楁煠閸濆嫬鈧悂鎯冮姀锛勨攳婵犻潧鐗婂▓宀勬煏?',

                okText: '缂佺虎鍙庨崰鏍偩妤ｅ啫鐭楅柡宥冨妿鍟?',

                cancelText: '缂傚倷缍€閸涱垱鏆伴梺姹囧灮閸犳劙宕?',

                okButtonProps: { danger: true },

                centered: true,

                onOk: handleCancelBatchGenerate,

              });

            }}

            cancelButtonText="闂佸憡鐟﹂悧妤冪矓闁垮顩烽悹鍥ㄥ絻椤?"

            blocking={false}

          />

        </Suspense>

      ) : null}



      <FloatButton

        icon={<BookOutlined />}

        type="primary"

        tooltip="缂備焦姊绘慨鐐繆椤撱垺鍎庢い鏃囧亹缁?"

        onClick={() => setIsIndexPanelVisible(true)}

        style={{ right: isMobile ? 24 : 48, bottom: isMobile ? 80 : 48 }}

      />



      <FloatingIndexPanel

        visible={isIndexPanelVisible}

        onClose={() => setIsIndexPanelVisible(false)}

        groupedChapters={groupedChapters}

        onChapterSelect={handleChapterSelect}

      />



      {/* 缂備焦姊绘慨鐐繆椤撱垺鈷撻柛娑㈠亰閸ゃ垽鏌?*/}

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



      {/* 闁诲繒鍋愰崑鎾绘⒑椤斿搫濮傞柛锝嗘倐瀹曟ê鈻庤箛姘⒕缂?*/}

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



      {/* 闁荤喐鐟ョ€氼剟宕瑰┑鍫㈢＝闁哄稁鍓涚敮鍡涙煕?*/}

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


