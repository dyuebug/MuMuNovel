import { memo, useEffect, useMemo, useState } from 'react';
/* eslint-disable @typescript-eslint/no-explicit-any */
import { Button, Card, Form, Input, InputNumber, Select, Space, Tag } from 'antd';
import { chapterApi } from '../services/api';
import type { ChapterQualityMetrics, ChapterQualityProfileSummary } from '../types';
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
  getQualityMetricItems,
  getQualityProfileDisplayItems,
  getRepairGuidanceDisplay,
  getWeakestQualityMetric,
} from '../utils/storyCreationQualitySummary';
import { DEFAULT_WORD_COUNT, setCachedWordCount } from '../utils/storyCreationWordCount';
import {
  areStoryBeatPlannerDraftsEqual,
  areStorySceneOutlineDraftsEqual,
  isStoryBeatPlannerDraftEmpty,
  isStorySceneOutlineDraftEmpty,
  STORY_BEAT_PLANNER_FIELDS,
  STORY_SCENE_OUTLINE_FIELDS,
} from '../utils/storyCreationDraft';
import {
  buildCreationPresetRecommendation,
  buildScoreDrivenRecommendationCard,
  buildStoryAfterScorecard,
} from '../utils/creationPresetsQuality';
import { CREATION_PLOT_STAGE_OPTIONS, CREATION_PRESETS, getCreationPresetByModes } from '../utils/creationPresetsCore';

const { TextArea } = Input;

const normalizeAvailableModelOptions = (models: any[]): Array<{ value: string; label: string }> => {
  const seenValues = new Set<string>();
  return models.reduce((options: Array<{ value: string; label: string }>, model: any) => {
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

type ChapterEditorAiSectionProps = {
  sectionProps: any;
};

function ChapterEditorAiSection({ sectionProps }: ChapterEditorAiSectionProps) {
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

    useEffect(() => {
      let cancelled = false;

      if (!currentEditingChapterId) {
        setChapterQualityLoading(false);
        setChapterQualityMetrics(null);
        setChapterQualityProfileSummary(null);
        setChapterQualityGeneratedAt(null);
        onChapterQualityMetricsChange(null);
        return () => {
          cancelled = true;
        };
      }

      const loadChapterQualityMetrics = async () => {
        setChapterQualityLoading(true);

        try {
          const result = await chapterApi.getChapterQualityMetrics(currentEditingChapterId);
          if (cancelled) {
            return;
          }

          const nextMetrics = result.has_metrics && result.latest_metrics ? result.latest_metrics : null;
          setChapterQualityMetrics(nextMetrics);
          setChapterQualityProfileSummary(result.quality_profile_summary ?? null);
          setChapterQualityGeneratedAt(nextMetrics ? result.generated_at : null);
          onChapterQualityMetricsChange(nextMetrics);
        } catch (error) {
          if (cancelled) {
            return;
          }

          console.error('Failed to load chapter quality metrics.', error);
          setChapterQualityMetrics(null);
          setChapterQualityProfileSummary(null);
          setChapterQualityGeneratedAt(null);
          onChapterQualityMetricsChange(null);
        } finally {
          if (!cancelled) {
            setChapterQualityLoading(false);
          }
        }
      };

      void loadChapterQualityMetrics();

      return () => {
        cancelled = true;
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
      ? (CREATIVE_MODE_OPTIONS.find((item: any) => item.value === selectedCreativeMode)?.label || selectedCreativeMode)
      : '默认推荐';
    const selectedStoryFocusLabel = selectedStoryFocus
      ? (STORY_FOCUS_OPTIONS.find((item: any) => item.value === selectedStoryFocus)?.label || selectedStoryFocus)
      : '默认推荐';
    const selectedPlotStageLabel = selectedPlotStage
      ? (CREATION_PLOT_STAGE_OPTIONS.find((item: any) => item.value === selectedPlotStage)?.label || selectedPlotStage)
      : '自动推断';
    const selectedModelLabel = selectedModel
      ? (normalizedAvailableModels.find((item) => item.value === selectedModel)?.label || selectedModel)
      : '项目默认';

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
          title="快速预设"
          style={{ marginBottom: 12 }}
        >
          <Space wrap>
            {CREATION_PRESETS.map((preset: any) => (
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
            <Card size="small" title={singleScoreDrivenRecommendationCard.title} style={{ marginTop: 12 }}>
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
                    singleScoreDrivenRecommendationCard.alternatives.map((item: any) => (
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
              style={{ marginTop: 12 }}
            >

              {renderCompactSettingHint(
                singleStoryCreationControlCard.summary,
                singleStoryCreationControlCard.directive,
              )}
              <Space direction="vertical" size={8} style={{ display: 'flex' }}>
                <div style={{ padding: '10px 12px', border: '1px solid #f0f0f0', borderRadius: 8 }}>
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
                <div style={{ padding: '10px 12px', border: '1px solid #f0f0f0', borderRadius: 8 }}>
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
                    {STORY_BEAT_PLANNER_FIELDS.map((field: any) => (
                      <div key={field.key}>
                        <div style={{ fontSize: 12, fontWeight: 600, marginBottom: 6 }}>{field.label}</div>
                        <Input
                          value={singleStoryBeatPlannerDraft[field.key]}
                          onChange={(event) => setSingleStoryBeatPlannerDraft((prev: any) => ({
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
                <div style={{ padding: '10px 12px', border: '1px solid #f0f0f0', borderRadius: 8 }}>
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
                    {STORY_SCENE_OUTLINE_FIELDS.map((field: any) => (
                      <div key={field.key}>
                        <div style={{ fontSize: 12, fontWeight: 600, marginBottom: 6 }}>{field.label}</div>
                        <TextArea
                          value={singleStorySceneOutlineDraft[field.key]}
                          onChange={(event) => setSingleStorySceneOutlineDraft((prev: any) => ({
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


<CompactPromptPreviewPanel
  prompt={resolvedSingleStoryCreationBrief}
  promptLayerLabels={singleStoryCreationPromptLayerLabels}
  promptCharCount={singleStoryCreationPromptCharCount}
  isVerbose={isSingleStoryCreationPromptVerbose}
  onCopy={() => void copyStoryCreationPrompt(resolvedSingleStoryCreationBrief, 'single')}
  placeholder="提示词将显示在此"
/>
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
                style={{ height: '100%' }}
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
                  {renderCompactFactCard('反模式', singleStoryRepairTargetCard.antiPattern)}
                  {renderCompactListCard('修复目标', singleStoryRepairTargetCard.repairTargets, { tagColor: 'gold' })}
                  {renderCompactListCard('保留优势', singleStoryRepairTargetCard.preserveStrengths, { tagColor: 'green' })}
                </div>
              </Card>
            )}

            {singleCreationBlueprint && (
              <Card size="small" title="创作蓝图" style={{ height: '100%' }}>
                <div style={{ color: 'var(--color-text-secondary)', marginBottom: 10 }}>
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
          <Card size="small" title="篇幅节奏规划" style={{ marginBottom: 12 }}>
            {renderCompactSettingHint(
              `当前阶段：${selectedPlotStageLabel}`,
              singleVolumePacingPlan.summary,
              { style: { marginBottom: 10 } },
            )}
            {renderCompactListCard(
              "章节分段",
              singleVolumePacingPlan.segments.map(
                (segment: any) => `第${segment.startChapter}-${segment.endChapter}章 · ${segment.label}：${segment.mission}`,
              ),
              { tagText: `${singleVolumePacingPlan.segments.length}段` },
            )}
          </Card>
        )}

        <Card size="small" title="补充微调（可选）" style={{ marginBottom: 12 }}>
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
                {CREATIVE_MODE_OPTIONS.map((option: any) => (
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
                {STORY_FOCUS_OPTIONS.map((option: any) => (
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
          title="质量画像"
          style={{ marginBottom: 12 }}
        >
          {chapterQualityProfileItems.length > 0 ? (
            <>
              {renderCompactSettingHint(
                "质量画像汇总了风格、维度与主要优化方向。",
                "优先关注与当前章节目标不一致的条目。",
                { tone: "success", style: { marginBottom: 10 } },
              )}
              {renderCompactFactGrid(
                chapterQualityProfileItems.map((item: any) => [item.label, item.description] as [string, string]),
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
          title="质量指标"
          loading={chapterQualityLoading}
          style={{ marginBottom: 12 }}
        >
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
                  {(chapterRepairGuidance.repairTargets.length > 0 || chapterRepairGuidance.preserveStrengths.length > 0) && (
                    <div
                      style={{
                        display: "grid",
                        gridTemplateColumns: isMobile ? "1fr" : "repeat(2, minmax(0, 1fr))",
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
