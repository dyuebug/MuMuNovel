import { memo, useEffect, useMemo, useRef, useState } from 'react';
import { Button, Card, Form, Input, InputNumber, Select, Space, Tag, Typography, theme } from 'antd';
import { chapterApi } from '../services/modularApi';
import type { ChapterQualityMetrics, ChapterQualityProfileSummary, CreativeMode, PlotStage, StoryFocus } from '../types';
import CompactPromptPreviewPanel from './CompactPromptPreviewPanel';
import StoryCreationSnapshotPanel from './StoryCreationSnapshotPanel';
import { renderCompactPresetRecommendationBlock } from './storyCreationPresetUi';
import { renderCompactInsightCardGrid } from './storyCreationInsightUi';
import {
  renderCompactFactCard,
  renderCompactFactGrid,
  renderCompactListCard,
  renderCompactSelectionSummary,
  renderCompactSettingHint,
  renderCompactStoryControlHeader,
} from './storyCreationCommonUi';
import {
  getCompactHintToneByAlertType,
  getOverallScoreColor,
  getMetricRateColor,
  renderCompactMetricGrid,
} from './storyCreationQualityUi';
import { buildCreationBlueprint, buildVolumePacingPlan } from '../utils/creationPresetsStory';
import {
  formatRepairWeakestMetricHint,
  getQualityMetricItems,
  getQualityProfileDisplayItems,
  getRepairGuidanceDisplay,
  getWeakestQualityMetric,
  type QualityProfileDisplayItem,
} from '../utils/storyCreationQualitySummary';
import { DEFAULT_WORD_COUNT, setCachedWordCount } from '../utils/storyCreationWordCount';
import {
  areStoryBeatPlannerDraftsEqual,
  areStorySceneOutlineDraftsEqual,
  isStoryBeatPlannerDraftEmpty,
  isStorySceneOutlineDraftEmpty,
  STORY_BEAT_PLANNER_FIELDS,
  STORY_SCENE_OUTLINE_FIELDS,
  type StoryBeatPlannerDraft,
  type StorySceneOutlineDraft,
} from '../utils/storyCreationDraft';
import {
  buildCreationPresetRecommendation,
  buildScoreDrivenRecommendationCard,
  buildStoryAfterScorecard,
} from '../utils/creationPresetsQuality';
import {
  CREATION_PLOT_STAGE_OPTIONS,
  CREATION_PRESETS,
  getCreationPresetByModes,
  type StoryAcceptanceCard,
  type StoryCharacterArcCard,
  type StoryCreationControlCard,
  type StoryExecutionChecklist,
  type StoryObjectiveCard,
  type StoryRepairTargetCard,
  type StoryRepetitionRiskCard,
  type StoryResultCard,
} from '../utils/creationPresetsCore';
import type { PreferenceOption } from '../utils/generationPreferenceOptions';

const { TextArea } = Input;
const { Text } = Typography;

type ModelOption = { value?: unknown; label?: unknown };
type NormalizedModelOption = { value: string; label: string };

const normalizeAvailableModelOptions = (models: ModelOption[]): NormalizedModelOption[] => {
  const seenValues = new Set<string>();
  return models.reduce((options: NormalizedModelOption[], model) => {
    const value = typeof model?.value === 'string' ? model.value.trim() : '';
    if (!value || seenValues.has(value)) {
      return options;
    }

    seenValues.add(value);
    options.push({
      value,
      label: typeof model?.label === 'string' && model.label.trim() ? model.label.trim() : value,
    });
    return options;
  }, []);
};

type ChapterEditorAiSectionSectionProps = {
  currentEditingChapterId: string | null;
  currentEditingChapterNumber: number | null;
  applySingleCreationPreset: (presetId: string) => void;
  projectDefaultCreativeMode?: CreativeMode;
  setSelectedCreativeMode: (value?: CreativeMode) => void;
  projectDefaultStoryFocus?: StoryFocus;
  setSelectedStoryFocus: (value?: StoryFocus) => void;
  selectedPlotStage?: PlotStage;
  setSelectedPlotStage: (value?: PlotStage) => void;
  singleStoryCreationControlCard: StoryCreationControlCard | null;
  isSingleStoryCreationControlCustomized: boolean;
  setSingleStoryCreationBriefDraft: (value: string) => void;
  singleSystemStoryCreationBrief: string;
  singleStoryCreationBriefDraft: string;
  isSingleStoryCreationBriefCustomized: boolean;
  singleStoryBeatPlannerDraft: StoryBeatPlannerDraft;
  setSingleStoryBeatPlannerDraft: (value: StoryBeatPlannerDraft | ((prev: StoryBeatPlannerDraft) => StoryBeatPlannerDraft)) => void;
  singleSystemStoryBeatPlanner: StoryBeatPlannerDraft;
  isSingleStoryBeatPlannerCustomized: boolean;
  isSingleStorySceneOutlineCustomized: boolean;
  setSingleStorySceneOutlineDraft: (value: StorySceneOutlineDraft | ((prev: StorySceneOutlineDraft) => StorySceneOutlineDraft)) => void;
  singleSuggestedStorySceneOutline: StorySceneOutlineDraft;
  singleStorySceneOutlineDraft: StorySceneOutlineDraft;
  resolvedSingleStoryCreationBrief: string;
  singleStoryCreationPromptLayerLabels: string[];
  singleStoryCreationPromptCharCount: number;
  isSingleStoryCreationPromptVerbose: boolean;
  STORY_CREATION_PROMPT_WARN_THRESHOLD: number;
  copyStoryCreationPrompt: (content: string | undefined, scopeLabel: 'single' | 'batch') => Promise<void>;
  singleStoryCreationSnapshots: unknown[];
  singleStoryCreationCurrentDraft: unknown;
  canSaveSingleStoryCreationSnapshot: boolean;
  saveSingleStoryCreationSnapshot: (reason: 'manual' | 'generate') => Promise<void>;
  applySingleStoryCreationSnapshot: (snapshot: unknown) => void;
  deleteSingleStoryCreationSnapshot: (snapshotId: string) => void;
  singleStoryAcceptanceCard: StoryAcceptanceCard | null;
  singleStoryCharacterArcCard: StoryCharacterArcCard | null;
  singleStoryExecutionChecklist: StoryExecutionChecklist | null;
  singleStoryObjectiveCard: StoryObjectiveCard | null;
  singleStoryRepairTargetCard: StoryRepairTargetCard | null;
  singleStoryRepetitionRiskCard: StoryRepetitionRiskCard | null;
  singleStoryResultCard: StoryResultCard | null;
  isMobile: boolean;
  targetWordCount: number;
  CREATIVE_MODE_OPTIONS: PreferenceOption<CreativeMode>[];
  selectedCreativeMode?: CreativeMode;
  STORY_FOCUS_OPTIONS: PreferenceOption<StoryFocus>[];
  selectedStoryFocus?: StoryFocus;
  availableModels: ModelOption[];
  selectedModel?: string;
  setSelectedModel: (value?: string) => void;
  setTargetWordCount: (value: number) => void;
  chapterQualityRefreshToken: number;
  onChapterQualityMetricsChange: (metrics: ChapterQualityMetrics | null) => void;
  knownStructureChapterCount: number;
};

type ChapterEditorAiSectionProps = {
  sectionProps: ChapterEditorAiSectionSectionProps;
};

function ChapterEditorAiSection({ sectionProps }: ChapterEditorAiSectionProps) {
    const { token } = theme.useToken();
    const alphaColor = (color: string, alpha: number) => `color-mix(in srgb, ${color} ${(alpha * 100).toFixed(0)}%, transparent)`;
    const {
      currentEditingChapterId,
      currentEditingChapterNumber,
      applySingleCreationPreset,
      projectDefaultCreativeMode,
      setSelectedCreativeMode,
      projectDefaultStoryFocus,
      setSelectedStoryFocus,
      selectedPlotStage,
      setSelectedPlotStage,
      singleStoryCreationControlCard,
      isSingleStoryCreationControlCustomized,
      setSingleStoryCreationBriefDraft,
      singleSystemStoryCreationBrief,
      singleStoryCreationBriefDraft,
      isSingleStoryCreationBriefCustomized,
      singleStoryBeatPlannerDraft,
      setSingleStoryBeatPlannerDraft,
      singleSystemStoryBeatPlanner,
      isSingleStoryBeatPlannerCustomized,
      isSingleStorySceneOutlineCustomized,
      setSingleStorySceneOutlineDraft,
      singleSuggestedStorySceneOutline,
      singleStorySceneOutlineDraft,
      resolvedSingleStoryCreationBrief,
      singleStoryCreationPromptLayerLabels,
      singleStoryCreationPromptCharCount,
      isSingleStoryCreationPromptVerbose,
      STORY_CREATION_PROMPT_WARN_THRESHOLD,
      copyStoryCreationPrompt,
      singleStoryCreationSnapshots,
      singleStoryCreationCurrentDraft,
      canSaveSingleStoryCreationSnapshot,
      saveSingleStoryCreationSnapshot,
      applySingleStoryCreationSnapshot,
      deleteSingleStoryCreationSnapshot,
      singleStoryAcceptanceCard,
      singleStoryCharacterArcCard,
      singleStoryExecutionChecklist,
      singleStoryObjectiveCard,
      singleStoryRepairTargetCard,
      singleStoryRepetitionRiskCard,
      singleStoryResultCard,
      isMobile,
      targetWordCount,
      CREATIVE_MODE_OPTIONS,
      selectedCreativeMode,
      STORY_FOCUS_OPTIONS,
      selectedStoryFocus,
      availableModels,
      selectedModel,
      setSelectedModel,
      setTargetWordCount,
      chapterQualityRefreshToken,
      onChapterQualityMetricsChange,
      knownStructureChapterCount,
    } = sectionProps;

    const normalizedAvailableModels = useMemo(
      () => normalizeAvailableModelOptions(availableModels),
      [availableModels],
    );

    const [chapterQualityLoading, setChapterQualityLoading] = useState(false);
    const [chapterQualityMetrics, setChapterQualityMetrics] = useState<ChapterQualityMetrics | null>(null);
    const [chapterQualityProfileSummary, setChapterQualityProfileSummary] = useState<ChapterQualityProfileSummary | null>(null);
    const [chapterQualityGeneratedAt, setChapterQualityGeneratedAt] = useState<string | null>(null);
    const mountedRef = useRef(false);
    const chapterQualityRequestIdRef = useRef(0);

    const beginChapterQualityRequest = () => {
      const nextRequestId = chapterQualityRequestIdRef.current + 1;
      chapterQualityRequestIdRef.current = nextRequestId;
      return nextRequestId;
    };

    const isChapterQualityRequestActive = (requestId: number) => (
      mountedRef.current && chapterQualityRequestIdRef.current === requestId
    );

    useEffect(() => {
      mountedRef.current = true;

      return () => {
        mountedRef.current = false;
        chapterQualityRequestIdRef.current += 1;
      };
    }, []);

    useEffect(() => {
      if (!currentEditingChapterId) {
        chapterQualityRequestIdRef.current += 1;
        setChapterQualityLoading(false);
        setChapterQualityMetrics(null);
        setChapterQualityProfileSummary(null);
        setChapterQualityGeneratedAt(null);
        onChapterQualityMetricsChange(null);
        return undefined;
      }

      const loadChapterQualityMetrics = async () => {
        const requestId = beginChapterQualityRequest();
        setChapterQualityLoading(true);

        try {
          const result = await chapterApi.getChapterQualityMetrics(currentEditingChapterId);
          if (!isChapterQualityRequestActive(requestId)) {
            return;
          }

          const nextMetrics = result.has_metrics && result.latest_metrics ? result.latest_metrics : null;
          setChapterQualityMetrics(nextMetrics);
          setChapterQualityProfileSummary(result.quality_profile_summary ?? null);
          setChapterQualityGeneratedAt(nextMetrics ? result.generated_at : null);
          onChapterQualityMetricsChange(nextMetrics);
        } catch (error) {
          if (!isChapterQualityRequestActive(requestId)) {
            return;
          }

          console.error('Failed to load chapter quality metrics.', error);
          setChapterQualityMetrics(null);
          setChapterQualityProfileSummary(null);
          setChapterQualityGeneratedAt(null);
          onChapterQualityMetricsChange(null);
        } finally {
          if (isChapterQualityRequestActive(requestId)) {
            setChapterQualityLoading(false);
          }
        }
      };

      void loadChapterQualityMetrics();

      return () => {
        chapterQualityRequestIdRef.current += 1;
      };
    }, [currentEditingChapterId, chapterQualityRefreshToken, onChapterQualityMetricsChange]);

    const activeSingleCreationPreset = useMemo(
      () => getCreationPresetByModes(selectedCreativeMode, selectedStoryFocus),
      [selectedCreativeMode, selectedStoryFocus],
    );

    const recommendedCreationPresets = useMemo(
      () => buildCreationPresetRecommendation(chapterQualityMetrics),
      [chapterQualityMetrics],
    );

    const chapterQualityProfileItems = useMemo(
      () => getQualityProfileDisplayItems(chapterQualityProfileSummary),
      [chapterQualityProfileSummary],
    );

    const chapterQualityMetricItems = useMemo(
      () => (chapterQualityMetrics ? getQualityMetricItems(chapterQualityMetrics) : []),
      [chapterQualityMetrics],
    );

    const weakestQualityMetric = useMemo(
      () => (chapterQualityMetrics ? getWeakestQualityMetric(chapterQualityMetrics) : null),
      [chapterQualityMetrics],
    );

    const chapterRepairGuidance = useMemo(
      () => getRepairGuidanceDisplay(chapterQualityMetrics?.repair_guidance),
      [chapterQualityMetrics],
    );

    const chapterRepairWeakestMetricHint = useMemo(
      () => formatRepairWeakestMetricHint(chapterRepairGuidance),
      [chapterRepairGuidance],
    );

    const singleScoreDrivenRecommendationCard = useMemo(
      () => buildScoreDrivenRecommendationCard(chapterQualityMetrics, selectedCreativeMode, selectedStoryFocus, {
        plotStage: selectedPlotStage,
        chapterNumber: currentEditingChapterNumber,
        totalChapters: knownStructureChapterCount,
        activePresetId: activeSingleCreationPreset?.id,
      }),
      [
        activeSingleCreationPreset?.id,
        chapterQualityMetrics,
        currentEditingChapterNumber,
        knownStructureChapterCount,
        selectedCreativeMode,
        selectedPlotStage,
        selectedStoryFocus,
      ],
    );

    const singleCreationBlueprint = useMemo(
      () => buildCreationBlueprint(selectedCreativeMode, selectedStoryFocus, {
        scene: 'chapter',
        plotStage: selectedPlotStage,
      }),
      [selectedCreativeMode, selectedPlotStage, selectedStoryFocus],
    );

    const singleStoryInsightCards = useMemo(() => ([
      singleStoryObjectiveCard
        ? {
            key: 'single-objective',
            title: '故事目标',
            summary: singleStoryObjectiveCard.summary,
            items: [
              ['目标', singleStoryObjectiveCard.objective],
              ['阻碍', singleStoryObjectiveCard.obstacle],
              ['转折', singleStoryObjectiveCard.turn],
              ['钩子', singleStoryObjectiveCard.hook],
            ],
          }
        : null,
      singleStoryResultCard
        ? {
            key: 'single-result',
            title: '故事结果',
            summary: singleStoryResultCard.summary,
            items: [
              ['推进结果', singleStoryResultCard.progress],
              ['揭示信息', singleStoryResultCard.reveal],
              ['关系变化', singleStoryResultCard.relationship],
              ['后续影响', singleStoryResultCard.fallout],
            ],
          }
        : null,
      singleStoryExecutionChecklist
        ? {
            key: 'single-execution',
            title: '执行清单',
            summary: singleStoryExecutionChecklist.summary,
            items: [
              ['开篇', singleStoryExecutionChecklist.opening],
              ['压力', singleStoryExecutionChecklist.pressure],
              ['转折', singleStoryExecutionChecklist.pivot],
              ['收束', singleStoryExecutionChecklist.closing],
            ],
          }
        : null,
      singleStoryRepetitionRiskCard
        ? {
            key: 'single-repetition',
            title: '重复风险',
            summary: singleStoryRepetitionRiskCard.summary,
            items: [
              ['开篇风险', singleStoryRepetitionRiskCard.openingRisk],
              ['压力风险', singleStoryRepetitionRiskCard.pressureRisk],
              ['转折风险', singleStoryRepetitionRiskCard.pivotRisk],
              ['收束风险', singleStoryRepetitionRiskCard.closingRisk],
            ],
          }
        : null,
      singleStoryAcceptanceCard
        ? {
            key: 'single-acceptance',
            title: '验收检查',
            summary: singleStoryAcceptanceCard.summary,
            items: [
              ['目标达成检查', singleStoryAcceptanceCard.missionCheck],
              ['变化检查', singleStoryAcceptanceCard.changeCheck],
              ['新鲜度检查', singleStoryAcceptanceCard.freshnessCheck],
              ['收束检查', singleStoryAcceptanceCard.closingCheck],
            ],
          }
        : null,
      singleStoryCharacterArcCard
        ? {
            key: 'single-character-arc',
            title: '人物弧光',
            summary: singleStoryCharacterArcCard.summary,
            items: [
              ['外在线', singleStoryCharacterArcCard.externalLine],
              ['内在线', singleStoryCharacterArcCard.internalLine],
              ['关系线', singleStoryCharacterArcCard.relationshipLine],
              ['弧光落点', singleStoryCharacterArcCard.arcLanding],
            ],
          }
        : null,
    ]).filter(Boolean), [
      singleStoryAcceptanceCard,
      singleStoryCharacterArcCard,
      singleStoryExecutionChecklist,
      singleStoryObjectiveCard,
      singleStoryRepetitionRiskCard,
      singleStoryResultCard,
    ]);

    const singleVolumePacingPlan = useMemo(
      () => buildVolumePacingPlan(knownStructureChapterCount, {
        preferredStage: selectedPlotStage,
        currentChapterNumber: currentEditingChapterNumber,
      }),
      [currentEditingChapterNumber, knownStructureChapterCount, selectedPlotStage],
    );

    const selectedCreativeModeLabel = selectedCreativeMode
      ? (CREATIVE_MODE_OPTIONS.find((item) => item.value === selectedCreativeMode)?.label || selectedCreativeMode)
      : '默认推荐';
    const selectedStoryFocusLabel = selectedStoryFocus
      ? (STORY_FOCUS_OPTIONS.find((item) => item.value === selectedStoryFocus)?.label || selectedStoryFocus)
      : '默认推荐';
    const selectedPlotStageLabel = selectedPlotStage
      ? (CREATION_PLOT_STAGE_OPTIONS.find((item) => item.value === selectedPlotStage)?.label || selectedPlotStage)
      : '自动推断';
    const selectedModelLabel = selectedModel
      ? (normalizedAvailableModels.find((item) => item.value === selectedModel)?.label || selectedModel)
      : '项目默认';

    const sectionCardStyle = {
      marginBottom: 12,
      borderRadius: 22,
      border: `1px solid ${alphaColor(token.colorBorderSecondary, 0.92)}`,
      background: `linear-gradient(180deg, ${alphaColor(token.colorBgContainer, 0.99)} 0%, ${alphaColor(token.colorFillAlter, 0.42)} 100%)`,
      boxShadow: `0 18px 40px ${alphaColor(token.colorTextBase, 0.04)}`,
    };

    const accentCardStyle = {
      ...sectionCardStyle,
      background: `linear-gradient(135deg, ${alphaColor(token.colorPrimaryBg, 0.92)} 0%, ${alphaColor(token.colorBgContainer, 0.99)} 52%, ${alphaColor(token.colorInfoBg, 0.68)} 100%)`,
      border: `1px solid ${alphaColor(token.colorPrimary, 0.12)}`,
    };

    const sectionBodyStyle = { padding: isMobile ? 14 : 18 };
    const panelStyle = {
      padding: isMobile ? '12px 12px' : '14px 14px',
      border: `1px solid ${alphaColor(token.colorBorderSecondary, 0.85)}`,
      borderRadius: 16,
      background: `linear-gradient(180deg, ${alphaColor(token.colorBgElevated, 0.98)} 0%, ${alphaColor(token.colorFillQuaternary, 0.42)} 100%)`,
    };
    const summaryCardStyle = {
      height: '100%',
      borderRadius: 18,
      border: `1px solid ${alphaColor(token.colorBorderSecondary, 0.82)}`,
      background: alphaColor(token.colorBgContainer, 0.98),
    };
    const sectionLabelStyle = {
      display: 'block',
      fontSize: 11,
      letterSpacing: '0.08em',
      textTransform: 'uppercase' as const,
      color: token.colorTextTertiary,
      marginBottom: 6,
    };
    const sectionTitleStyle = { display: 'block', fontSize: 17, marginBottom: 6 };
    const sectionDescriptionStyle = {
      display: 'block',
      lineHeight: 1.7,
      color: token.colorTextSecondary,
      marginBottom: 14,
    };

    const singleAfterScorecard = useMemo(
      () => buildStoryAfterScorecard(chapterQualityMetrics, selectedCreativeMode, selectedStoryFocus, {
        plotStage: selectedPlotStage,
      }),
      [chapterQualityMetrics, selectedCreativeMode, selectedPlotStage, selectedStoryFocus],
    );

    return (
      <>
        <Card
          size="small"
          style={accentCardStyle}
          styles={{ body: sectionBodyStyle }}
        >
          <Text style={sectionLabelStyle}>
            Chapter AI Studio
          </Text>
          <Text strong style={sectionTitleStyle}>
            快速预设
          </Text>
          <Text style={sectionDescriptionStyle}>
            先用预设锁定这一章的创作气质，再决定是否手动微调。默认优先保持工作流连续性，避免每次都从零配置。
          </Text>
          {renderCompactSelectionSummary(
            [
              { label: '模式', value: selectedCreativeModeLabel, color: 'blue' },
              { label: '聚焦', value: selectedStoryFocusLabel, color: 'purple' },
              { label: '阶段', value: selectedPlotStageLabel, color: 'gold' },
            ],
            { style: { marginBottom: 14 } },
          )}
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

          {activeSingleCreationPreset && renderCompactSettingHint(
            `已选预设：${activeSingleCreationPreset.label}`,
            activeSingleCreationPreset.description,
            { style: { marginTop: 12 }, tone: 'success' },
          )}

          {renderCompactPresetRecommendationBlock(recommendedCreationPresets, {
            activePresetId: activeSingleCreationPreset?.id,
            applyPreset: applySingleCreationPreset,
                })}

          {singleScoreDrivenRecommendationCard && (
            <Card
              size="small"
              title={singleScoreDrivenRecommendationCard.title}
              style={{ ...summaryCardStyle, marginTop: 14 }}
              styles={{ body: { padding: 14 } }}
            >
              <Space direction="vertical" size={10} style={{ display: 'flex' }}>
                {renderCompactSettingHint(
                  singleScoreDrivenRecommendationCard.summary,
                  singleScoreDrivenRecommendationCard.applyHint,
                )}

                {singleScoreDrivenRecommendationCard.recommendedPresetLabel && renderCompactStoryControlHeader(
                  '推荐预设',
                  singleScoreDrivenRecommendationCard.recommendedPresetReason || '优先用这个预设起步。',
                  {
                    tagText: singleScoreDrivenRecommendationCard.recommendedPresetLabel,
                    tagColor: singleScoreDrivenRecommendationCard.recommendedPresetId === activeSingleCreationPreset?.id ? 'blue' : 'processing',
                  },
                )}

                {renderCompactStoryControlHeader(
                  '推荐阶段',
                  singleScoreDrivenRecommendationCard.stageReason,
                  {
                    tagText: singleScoreDrivenRecommendationCard.recommendedStageLabel,
                    tagColor: singleScoreDrivenRecommendationCard.recommendedStage === selectedPlotStage ? 'blue' : 'purple',
                  },
                )}

                {singleScoreDrivenRecommendationCard.alternatives.length > 0 && (
                  renderCompactListCard(
                    '备选方案',
                    singleScoreDrivenRecommendationCard.alternatives.map((item) => (
                      item.reason ? `${item.label}：${item.reason}` : item.label
                    )),
                    { tagText: `${singleScoreDrivenRecommendationCard.alternatives.length}项` },
                  )
                )}

                <Space wrap>
                  {singleScoreDrivenRecommendationCard.recommendedPresetId && (
                    <Button size="small" onClick={() => applySingleCreationPreset(singleScoreDrivenRecommendationCard.recommendedPresetId!)}>
                      应用预设
                    </Button>
                  )}
                  {singleScoreDrivenRecommendationCard.recommendedStage && (
                    <Button size="small" onClick={() => setSelectedPlotStage(singleScoreDrivenRecommendationCard.recommendedStage)}>
                      应用阶段
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
                      一键应用
                    </Button>
                  )}
                </Space>
              </Space>
            </Card>
          )}
        </Card>

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
              style={{ ...sectionCardStyle, marginTop: 12 }}
              styles={{ body: sectionBodyStyle }}
            >
              <Text style={sectionLabelStyle}>
                Story Briefing
              </Text>
              <Text type="secondary" style={{ ...sectionDescriptionStyle, marginBottom: 12 }}>
                这里集中管理本章的创作意图、节拍、场景链路和提示词快照，是进入正文续写前的主控工作台。
              </Text>
              {renderCompactSettingHint(
                singleStoryCreationControlCard.summary,
                singleStoryCreationControlCard.directive,
              )}
              <Space direction="vertical" size={8} style={{ display: 'flex' }}>
                <div style={panelStyle}>
                  {renderCompactStoryControlHeader(
                    '故事简介',
                    '一句话说明本轮方向。',
                    {
                      tagText: isSingleStoryCreationBriefCustomized ? '自定义' : '系统建议',
                      tagColor: isSingleStoryCreationBriefCustomized ? 'purple' : 'blue',
                    },
                  )}
                  <TextArea
                    value={singleStoryCreationBriefDraft}
                    onChange={(event) => setSingleStoryCreationBriefDraft(event.target.value)}
                    autoSize={{ minRows: 4, maxRows: 8 }}
                    maxLength={600}
                    showCount
                    placeholder="请简要描述故事..."
                  />
                </div>
                <div style={panelStyle}>
                  {renderCompactStoryControlHeader(
                    '故事节拍',
                    '按五拍锁住节奏。',
                    {
                      tagText: isSingleStoryBeatPlannerCustomized ? '自定义' : '系统建议',
                      tagColor: isSingleStoryBeatPlannerCustomized ? 'purple' : 'blue',
                      action: (
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
                      ),
                    },
                  )}
                  <Space direction="vertical" size={8} style={{ display: 'flex' }}>
                    {STORY_BEAT_PLANNER_FIELDS.map((field) => (
                      <div key={field.key}>
                        <div style={{ fontSize: 12, fontWeight: 600, marginBottom: 6 }}>{field.label}</div>
                        <Input
                          value={singleStoryBeatPlannerDraft[field.key]}
                          onChange={(event) => setSingleStoryBeatPlannerDraft((prev: StoryBeatPlannerDraft) => ({
                            ...prev,
                            [field.key]: event.target.value,
                          }))}
                          placeholder={field.placeholder}
                          maxLength={120}
                        />
                      </div>
                    ))}
                  </Space>
                </div>
                <div style={panelStyle}>
                  {renderCompactStoryControlHeader(
                    '场景提纲',
                    '列出场景链路。',
                    {
                      tagText: isSingleStorySceneOutlineCustomized ? '自定义' : '系统建议',
                      tagColor: isSingleStorySceneOutlineCustomized ? 'purple' : 'blue',
                      action: (
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
                      ),
                    },
                  )}
                  <Space direction="vertical" size={8} style={{ display: 'flex' }}>
                    {STORY_SCENE_OUTLINE_FIELDS.map((field) => (
                      <div key={field.key}>
                        <div style={{ fontSize: 12, fontWeight: 600, marginBottom: 6 }}>{field.label}</div>
                        <TextArea
                          value={singleStorySceneOutlineDraft[field.key]}
                          onChange={(event) => setSingleStorySceneOutlineDraft((prev: StorySceneOutlineDraft) => ({
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
                </div>
                <div style={panelStyle}>
                  <Text style={sectionLabelStyle}>
                    Prompt Preview
                  </Text>
                  <Text type="secondary" style={{ ...sectionDescriptionStyle, marginBottom: 12 }}>
                    这里用于检查最终送给模型的提示词层，避免带着模糊意图直接续写。
                  </Text>
                  <CompactPromptPreviewPanel
                    prompt={resolvedSingleStoryCreationBrief}
                    promptLayerLabels={singleStoryCreationPromptLayerLabels}
                    promptCharCount={singleStoryCreationPromptCharCount}
                    isVerbose={isSingleStoryCreationPromptVerbose}
                    onCopy={() => void copyStoryCreationPrompt(resolvedSingleStoryCreationBrief, 'single')}
                    placeholder="提示词将显示在此"
                  />
                </div>
                <div style={panelStyle}>
                  <Text style={sectionLabelStyle}>
                    Snapshot Memory
                  </Text>
                  <Text type="secondary" style={{ ...sectionDescriptionStyle, marginBottom: 12 }}>
                    保存或回放当前创作配置，适合在多轮生成之间快速对比不同方案。
                  </Text>
                  <StoryCreationSnapshotPanel
                    scopeLabel="single"
                    emptyText="还没有快照。"
                    snapshots={singleStoryCreationSnapshots}
                    currentDraft={singleStoryCreationCurrentDraft}
                    canSave={canSaveSingleStoryCreationSnapshot}
                    onSave={() => void saveSingleStoryCreationSnapshot('manual')}
                    onApply={applySingleStoryCreationSnapshot}
                    onDelete={deleteSingleStoryCreationSnapshot}
                    onCopy={copyStoryCreationPrompt}
                    includeNarrativePerspective
                    promptWarnThreshold={STORY_CREATION_PROMPT_WARN_THRESHOLD}
                  />
                </div>
                <div
                  style={{
                    display: 'grid',
                    gridTemplateColumns: isMobile ? '1fr' : 'repeat(3, minmax(0, 1fr))',
                    gap: 8,
                  }}
                >
                  {renderCompactListCard('执行路径', singleStoryCreationControlCard.executionPath, { numbered: true })}
                  {renderCompactListCard('预期结果', singleStoryCreationControlCard.expectedOutcomes, { numbered: true })}
                  {renderCompactListCard('约束规则', singleStoryCreationControlCard.guardrails)}
                </div>
              </Space>
            </Card>
          )}

          {(singleStoryRepairTargetCard || singleCreationBlueprint) && (
          <div
            style={{
              display: 'grid',
              gridTemplateColumns: isMobile ? '1fr' : 'repeat(2, minmax(0, 1fr))',
              gap: 12,
              marginBottom: 12,
            }}
          >
            {singleStoryRepairTargetCard && (
              <Card
                size="small"
                title={singleStoryRepairTargetCard.title}
                extra={<Tag color="gold">修复重点</Tag>}
                style={summaryCardStyle}
                styles={{ body: { padding: 14 } }}
              >
                {renderCompactSettingHint(
                  singleStoryRepairTargetCard.repairSummary,
                  singleStoryRepairTargetCard.applyHint,
                  { tone: 'warning' },
                )}
                <div
                  style={{
                    display: 'grid',
                    gridTemplateColumns: isMobile ? '1fr' : 'repeat(2, minmax(0, 1fr))',
                    gap: 8,
                  }}
                >
                  {renderCompactFactCard('优先修复项', singleStoryRepairTargetCard.priorityTarget)}
                  {chapterRepairWeakestMetricHint && renderCompactFactCard('当前最弱项', chapterRepairWeakestMetricHint)}
                  {renderCompactFactCard('反模式', singleStoryRepairTargetCard.antiPattern)}
                  {renderCompactListCard('修复目标', singleStoryRepairTargetCard.repairTargets, { tagColor: 'gold' })}
                  {renderCompactListCard('保留优势', singleStoryRepairTargetCard.preserveStrengths, { tagColor: 'green' })}
                </div>
              </Card>
            )}

            {singleCreationBlueprint && (
              <Card size="small" title="创作蓝图" style={summaryCardStyle} styles={{ body: { padding: 14 } }}>
                <div style={{ color: token.colorTextSecondary, marginBottom: 10, lineHeight: 1.7 }}>
                  {singleCreationBlueprint.summary}
                </div>
                {renderCompactListCard(
                  '推荐节拍',
                  singleCreationBlueprint.beats,
                  { numbered: true, tagText: `${singleCreationBlueprint.beats.length}拍` },
                )}
                {singleCreationBlueprint.risks.length > 0 && (
                  renderCompactSettingHint(
                    '风险提示',
                    singleCreationBlueprint.risks.join(', '),
                    { tone: 'warning', style: { marginTop: 12, marginBottom: 0 } },
                  )
                )}
              </Card>
            )}
          </div>
        )}

        {renderCompactInsightCardGrid(singleStoryInsightCards, isMobile, { style: { marginBottom: 12 } })}


        {singleVolumePacingPlan && (
          <Card size="small" title="篇幅节奏规划" style={sectionCardStyle} styles={{ body: sectionBodyStyle }}>
            <Text style={sectionLabelStyle}>
              Pacing Map
            </Text>
            <Text type="secondary" style={{ ...sectionDescriptionStyle, marginBottom: 12 }}>
              用章节段落视角看这一章在整卷中的位置，避免单章情绪和总节奏脱节。
            </Text>
            {renderCompactSettingHint(
              `当前阶段：${selectedPlotStageLabel}`,
              singleVolumePacingPlan.summary,
              { style: { marginBottom: 10 } },
            )}
            {renderCompactListCard(
              "章节分段",
              singleVolumePacingPlan.segments.map(
                (segment) => `第${segment.startChapter}-${segment.endChapter}章 · ${segment.label}：${segment.mission}`,
              ),
              { tagText: `${singleVolumePacingPlan.segments.length}段` },
            )}
          </Card>
        )}

        <Card size="small" title="补充微调（可选）" style={sectionCardStyle} styles={{ body: sectionBodyStyle }}>
          <Text style={sectionLabelStyle}>
            Optional Fine Tuning
          </Text>
          <Text type="secondary" style={{ ...sectionDescriptionStyle, marginBottom: 12 }}>
            只有当你明确想改变这一章的生成倾向时，再手动覆盖推荐项；否则优先保持默认组合。
          </Text>
          {renderCompactSettingHint(
            "不改则沿用上方推荐；只在你明确想改变生成偏向时再手动调整。",
            "单章通常优先调整模式与聚焦，模型与字数保持默认即可。",
            { style: { marginBottom: 10 } },
          )}
          {renderCompactSelectionSummary(
            [
              { label: "模式", value: selectedCreativeModeLabel, color: "blue" },
              { label: "聚焦", value: selectedStoryFocusLabel, color: "purple" },
              { label: "字数", value: `${targetWordCount}字`, color: "green" },
              { label: "模型", value: selectedModelLabel },
            ],
          )}
          <div
            style={{
              display: "grid",
              gridTemplateColumns: `repeat(auto-fit, minmax(${isMobile ? 220 : 260}px, 1fr))`,
              gap: 12,
            }}
          >
            <Form.Item
              label="创作模式"
              tooltip="控制这一章的主要写法偏向"
              style={{ marginBottom: 0 }}
            >
              <Select
                placeholder="留空=默认推荐"
                value={selectedCreativeMode}
                onChange={setSelectedCreativeMode}
                allowClear
                optionLabelProp="label"
              >
                {CREATIVE_MODE_OPTIONS.map((option) => (
                  <Select.Option key={option.value} value={option.value} label={option.label}>
                    <div>{option.label}</div>
                    <div style={{ fontSize: 12, color: "var(--color-text-tertiary)" }}>{option.description}</div>
                  </Select.Option>
                ))}
              </Select>
            </Form.Item>
            <Form.Item
              label="故事聚焦"
              tooltip="控制这一章的主要发力点"
              style={{ marginBottom: 0 }}
            >
              <Select
                placeholder="留空=默认推荐"
                value={selectedStoryFocus}
                onChange={setSelectedStoryFocus}
                allowClear
                optionLabelProp="label"
              >
                {STORY_FOCUS_OPTIONS.map((option) => (
                  <Select.Option key={option.value} value={option.value} label={option.label}>
                    <div>{option.label}</div>
                    <div style={{ fontSize: 12, color: "var(--color-text-tertiary)" }}>{option.description}</div>
                  </Select.Option>
                ))}
              </Select>
            </Form.Item>
          </div>
          <div
            style={{
              display: "grid",
              gridTemplateColumns: `repeat(auto-fit, minmax(${isMobile ? 220 : 260}px, 1fr))`,
              gap: 12,
              marginTop: 12,
            }}
          >
            <Form.Item
              label="目标字数"
              tooltip="留空则沿用默认字数"
              style={{ marginBottom: 0 }}
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
                style={{ width: "100%" }}
                formatter={(value) => (value ? String(value) + " 字" : "")}
                parser={(value) => parseInt((value || "").replace(" 字", ""), 10)}
              />
            </Form.Item>
            <Form.Item
              label="AI 模型"
              tooltip="留空则沿用项目默认模型"
              style={{ marginBottom: 0 }}
            >
              <Select
                placeholder={selectedModel ? `已选择：${selectedModelLabel}` : "留空=项目默认"}
                value={selectedModel ?? undefined}
                onChange={setSelectedModel}
                allowClear
                showSearch
                optionFilterProp="label"
              >
                {normalizedAvailableModels.map((model) => (
                  <Select.Option key={model.value} value={model.value} label={model.label}>
                    {model.label}
                  </Select.Option>
                ))}
              </Select>
            </Form.Item>
          </div>
        </Card>




        <Card
          size="small"
          style={sectionCardStyle}
          styles={{ body: sectionBodyStyle }}
        >
          <Text style={sectionLabelStyle}>
            Quality Portrait
          </Text>
          <Text strong style={sectionTitleStyle}>
            质量画像
          </Text>
          <Text type="secondary" style={{ ...sectionDescriptionStyle, marginBottom: 12 }}>
            这里偏向“诊断式阅读”，帮你快速判断当前章节的风格特征、维度分布和优先优化方向。
          </Text>
          {chapterQualityProfileItems.length > 0 ? (
            <>
              {renderCompactSettingHint(
                "质量画像汇总了风格、维度与主要优化方向。",
                "优先关注与当前章节目标不一致的条目。",
                { tone: "success", style: { marginBottom: 10 } },
              )}
              {renderCompactFactGrid(
                chapterQualityProfileItems.map((item: QualityProfileDisplayItem) => [item.label, item.description] as [string, string]),
              )}
            </>
          ) : (
            renderCompactSettingHint(
              "暂无质量画像",
              "运行分析后可生成质量画像。",
              { style: { marginBottom: 0 } },
            )
          )}
        </Card>



        <Card
          size="small"
          style={sectionCardStyle}
          loading={chapterQualityLoading}
          styles={{ body: sectionBodyStyle }}
        >
          <Text style={sectionLabelStyle}>
            Quality Metrics
          </Text>
          <Text strong style={sectionTitleStyle}>
            质量指标
          </Text>
          <Text type="secondary" style={{ ...sectionDescriptionStyle, marginBottom: 12 }}>
            从分数、弱项和修复建议三个层面看这一章的可迭代空间，适合决定下一轮该补什么。
          </Text>
          {chapterQualityMetrics ? (
            <>
              {singleAfterScorecard && (
                <>
                  {renderCompactSettingHint(
                    singleAfterScorecard.verdict,
                    `${singleAfterScorecard.summary} ${singleAfterScorecard.nextAction}`,
                    {
                      tone: getCompactHintToneByAlertType(singleAfterScorecard.verdictColor as "success" | "info" | "warning" | "error"),
                      style: { marginBottom: 10 },
                    },
                  )}
                  <div
                    style={{
                      display: "grid",
                      gridTemplateColumns: isMobile ? "1fr" : "repeat(2, minmax(0, 1fr))",
                      gap: 8,
                      marginBottom: 10,
                    }}
                  >
                    <div style={{ minWidth: 0 }}>
                      {renderCompactListCard(
                        "优势",
                        singleAfterScorecard.strengths,
                        { tagText: `${singleAfterScorecard.strengths.length}项`, tagColor: "green", style: { height: "100%" } },
                      )}
                    </div>
                    <div style={{ minWidth: 0 }}>
                      {renderCompactListCard(
                        "缺口",
                        singleAfterScorecard.gaps,
                        { tagText: `${singleAfterScorecard.gaps.length}项`, tagColor: "gold", style: { height: "100%" } },
                      )}
                    </div>
                  </div>
                </>
              )}
              {renderCompactSelectionSummary(
                [
                  { label: "综合得分", value: `${chapterQualityMetrics.overall_score}`, color: getOverallScoreColor(chapterQualityMetrics.overall_score) },
                  ...(weakestQualityMetric
                    ? [{
                        label: "最弱项",
                        value: `${weakestQualityMetric.label} ${weakestQualityMetric.value}%`,
                        color: getMetricRateColor(weakestQualityMetric.value),
                      }]
                    : []),
                  {
                    label: "生成时间",
                    value: chapterQualityGeneratedAt ? new Date(chapterQualityGeneratedAt).toLocaleString() : "尚未生成",
                  },
                ],
                { style: { marginBottom: 10 } },
              )}
              {chapterRepairGuidance && (
                <>
                  {chapterRepairGuidance.summary && renderCompactSettingHint(
                    "自动修复建议",
                    chapterRepairGuidance.summary,
                    { style: { marginBottom: 10 } },
                  )}
                  {chapterRepairWeakestMetricHint && renderCompactFactCard("当前最弱项", chapterRepairWeakestMetricHint)}
                  {(chapterRepairGuidance.repairTargets.length > 0 || chapterRepairGuidance.preserveStrengths.length > 0 || chapterRepairGuidance.focusAreas.length > 0) && (
                    <div
                      style={{
                        display: "grid",
                        gridTemplateColumns: "repeat(auto-fit, minmax(220px, 1fr))",
                        gap: 8,
                        marginBottom: 10,
                      }}
                    >
                      <div style={{ minWidth: 0 }}>
                        {chapterRepairGuidance.repairTargets.length > 0 && renderCompactListCard(
                          "下一轮修复",
                          chapterRepairGuidance.repairTargets,
                          { tagText: `${chapterRepairGuidance.repairTargets.length}项`, tagColor: "gold", style: { height: "100%" } },
                        )}
                      </div>
                      <div style={{ minWidth: 0 }}>
                        {chapterRepairGuidance.preserveStrengths.length > 0 && renderCompactListCard(
                          "保留优势",
                          chapterRepairGuidance.preserveStrengths,
                          { tagText: `${chapterRepairGuidance.preserveStrengths.length}项`, tagColor: "green", style: { height: "100%" } },
                        )}
                      </div>
                      <div style={{ minWidth: 0 }}>
                        {chapterRepairGuidance.focusAreas.length > 0 && renderCompactListCard(
                          "关注重点",
                          chapterRepairGuidance.focusAreas,
                          { tagText: `${chapterRepairGuidance.focusAreas.length}项`, tagColor: "blue", style: { height: "100%" } },
                        )}
                      </div>
                    </div>
                  )}
                </>
              )}
              {renderCompactMetricGrid(chapterQualityMetricItems)}
            </>
          ) : (
            renderCompactSettingHint(
              "暂无质量指标",
              "运行分析后可生成质量指标。",
              { style: { marginBottom: 0 } },
            )
          )}
        </Card>

      </>
    );
  }

export default memo(ChapterEditorAiSection);
