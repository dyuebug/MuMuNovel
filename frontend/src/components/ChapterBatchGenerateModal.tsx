/* eslint-disable @typescript-eslint/no-explicit-any */
import { memo, useEffect, useMemo, useRef } from 'react';
import { Button, Card, Form, Input, InputNumber, Modal, Radio, Select, Space, Tag } from 'antd';
import type { FormInstance } from 'antd';
import { RocketOutlined, StopOutlined } from '@ant-design/icons';
import CompactPromptPreviewPanel from './CompactPromptPreviewPanel';
import StoryCreationSnapshotPanel from './StoryCreationSnapshotPanel';
import { renderCompactPresetRecommendationBlock } from './storyCreationPresetUi';
import { renderCompactInsightCardGrid } from './storyCreationInsightUi';
import {
  renderCompactFactCard,
  renderCompactFactGrid,
  renderCompactListCard,
  renderCompactSelectionSummary,
  renderCompactSettingFlow,
  renderCompactSettingHint,
  renderCompactStoryControlHeader,
} from './storyCreationCommonUi';
import {
  getCompactHintToneByAlertType,
  getOverallScoreColor,
  renderCompactMetricGrid,
} from './storyCreationQualityUi';
import {
  getBatchSummaryMetricItems,
  getQualityProfileDisplayItems,
} from '../utils/storyCreationQualitySummary';
import { getCachedWordCount, setCachedWordCount } from '../utils/storyCreationWordCount';
import {
  areStoryBeatPlannerDraftsEqual,
  areStorySceneOutlineDraftsEqual,
  isStoryBeatPlannerDraftEmpty,
  isStorySceneOutlineDraftEmpty,
  STORY_BEAT_PLANNER_FIELDS,
  STORY_SCENE_OUTLINE_FIELDS,
} from '../utils/storyCreationDraft';
import {
  buildBatchCreationPresetRecommendation,
  buildBatchScoreDrivenRecommendationCardFromSummary,
  buildBatchStoryAfterScorecardFromSummary,
  buildBatchStoryCreationControlCardFromSummary,
  buildBatchStoryInsightCards,
  buildBatchStoryRepairTargetCardFromSummary,
} from '../utils/creationPresetsBatch';
import { buildCreationBlueprint, buildVolumePacingPlan } from '../utils/creationPresetsStory';
import { CREATION_PLOT_STAGE_OPTIONS, CREATION_PRESETS, getCreationPresetByModes } from '../utils/creationPresetsCore';

const { TextArea } = Input;

type ChapterBatchGenerateModalProps = {
  batchForm: FormInstance;
  [key: string]: any;
};

type RenderDebugGlobal = typeof globalThis & {
  __NOVEL_RENDER_DEBUG__?: boolean;
  __NOVEL_RENDER_DEBUG_FILTER__?: string[];
};

const noopRenderDiagnostics = (...args: [string, () => Record<string, unknown>]): void => {
  void args;
};

function useActiveRenderDiagnostics(componentName: string, getSnapshot: () => Record<string, unknown>): void {
  const renderCountRef = useRef(0);
  const previousSnapshotRef = useRef<Record<string, unknown> | null>(null);

  useEffect(() => {
    const renderDebugGlobal = globalThis as RenderDebugGlobal;
    if (!renderDebugGlobal.__NOVEL_RENDER_DEBUG__) {
      return;
    }

    const filters = renderDebugGlobal.__NOVEL_RENDER_DEBUG_FILTER__;
    if (Array.isArray(filters) && filters.length > 0 && !filters.includes(componentName)) {
      return;
    }

    renderCountRef.current += 1;
    const nextSnapshot = getSnapshot();
    const previousSnapshot = previousSnapshotRef.current;
    const changedKeys = previousSnapshot
      ? Object.keys(nextSnapshot).filter((key) => !Object.is(previousSnapshot[key], nextSnapshot[key]))
      : Object.keys(nextSnapshot);

    console.debug(`[render-debug] ${componentName} #${renderCountRef.current}`, {
      changedKeys,
      snapshot: nextSnapshot,
    });

    previousSnapshotRef.current = nextSnapshot;
  });
}

const useLocalRenderDiagnostics = import.meta.env.DEV ? useActiveRenderDiagnostics : noopRenderDiagnostics;

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

const normalizeWritingStyleOptions = (styles: any[]): any[] => {
  const seenStyleIds = new Set<number>();
  return styles.filter((style) => {
    if (typeof style?.id !== 'number' || seenStyleIds.has(style.id)) {
      return false;
    }

    seenStyleIds.add(style.id);
    return true;
  });
};

const normalizeBatchStartChapterOptions = (chapters: any[]): any[] => {
  const seenChapterNumbers = new Set<number>();
  return chapters.filter((chapter) => {
    if (typeof chapter?.chapter_number !== 'number' || seenChapterNumbers.has(chapter.chapter_number)) {
      return false;
    }

    seenChapterNumbers.add(chapter.chapter_number);
    return true;
  });
};

const BATCH_MODAL_IGNORED_PROP_KEYS = new Set([
  'handleBatchGenerate',
  'handleCancelBatchGenerate',
  'selectedModel',
  'selectedStyleId',
  'sortedChapters',
]);

const areBatchGenerateModalPropsEqual = (previousProps: ChapterBatchGenerateModalProps, nextProps: ChapterBatchGenerateModalProps): boolean => {
  const previousKeys = Object.keys(previousProps);
  const nextKeys = Object.keys(nextProps);

  if (previousKeys.length !== nextKeys.length) {
    return false;
  }

  return previousKeys.every((key) => (
    BATCH_MODAL_IGNORED_PROP_KEYS.has(key)
      ? true
      : previousProps[key] === nextProps[key]
  ));
};

function ChapterBatchGenerateModal(props: ChapterBatchGenerateModalProps) {
  const {
    applyBatchCreationPreset,
    applyBatchStoryCreationSnapshot,
    applyInferredBatchPlotStage,
    availableModels,
    batchEnableAnalysis,
    batchForm,
    batchGenerateVisible,
    batchGenerating,
    batchProgress,
    batchSelectedCreativeMode,
    batchSelectedModel,
    batchSelectedPlotStage,
    batchSelectedStoryFocus,
    batchStartChapterOptions,
    batchStoryBeatPlannerDraft,
    batchStoryCreationBriefDraft,
    batchStoryCreationCurrentDraft,
    batchStoryCreationSnapshots,
    batchStorySceneOutlineDraft,
    batchSuggestedStorySceneOutline,
    batchSystemStoryBeatPlanner,
    canSaveBatchStoryCreationSnapshot,
    copyStoryCreationPrompt,
    CREATIVE_MODE_OPTIONS,
    deleteBatchStoryCreationSnapshot,
    handleBatchGenerate,
    handleCancelBatchGenerate,
    isBatchStoryBeatPlannerCustomized,
    isBatchStoryCreationBriefCustomized,
    isBatchStoryCreationControlCustomized,
    isBatchStorySceneOutlineCustomized,
    isMobile,
    modal,
    knownStructureChapterCount,
    projectDefaultCreativeMode,
    projectDefaultStoryFocus,
    resolvedBatchStoryCreationBrief,
    batchStoryCreationPromptLayerLabels,
    batchStoryCreationPromptCharCount,
    isBatchStoryCreationPromptVerbose,
    STORY_CREATION_PROMPT_WARN_THRESHOLD,
    saveBatchStoryCreationSnapshot,
    selectedModel,
    selectedStyleId,
    setBatchGenerateVisible,
    setBatchSelectedCreativeMode,
    setBatchSelectedModel,
    setBatchSelectedPlotStage,
    setBatchSelectedStoryFocus,
    setBatchStoryBeatPlannerDraft,
    setBatchStoryCreationBriefDraft,
    setBatchStorySceneOutlineDraft,
    sortedChapters,
    STORY_FOCUS_OPTIONS,
    writingStyles
  } = props;


const batchStartChapterNumber = Form.useWatch('startChapterNumber', batchForm) as number | undefined;
useLocalRenderDiagnostics('ChapterBatchGenerateModal', () => ({
  visible: batchGenerateVisible,
  generating: batchGenerating,
  enableAnalysis: batchEnableAnalysis,
  creativeMode: batchSelectedCreativeMode,
  storyFocus: batchSelectedStoryFocus,
  plotStage: batchSelectedPlotStage,
  model: batchSelectedModel,
  startChapterNumber: batchStartChapterNumber,
  progressPercent: batchProgress?.progress_percent,
}));
const normalizedAvailableModels = useMemo(
  () => normalizeAvailableModelOptions(availableModels),
  [availableModels],
);
const normalizedWritingStyles = useMemo(
  () => normalizeWritingStyleOptions(writingStyles),
  [writingStyles],
);
const normalizedBatchStartChapterOptions = useMemo(
  () => normalizeBatchStartChapterOptions(batchStartChapterOptions),
  [batchStartChapterOptions],
);
const activeBatchCreationPreset = useMemo(
  () => getCreationPresetByModes(batchSelectedCreativeMode, batchSelectedStoryFocus),
  [batchSelectedCreativeMode, batchSelectedStoryFocus],
);
const batchQualityMetricsSummary = batchProgress?.quality_metrics_summary ?? null;
const batchRecommendedCreationPresets = useMemo(
  () => buildBatchCreationPresetRecommendation(batchQualityMetricsSummary),
  [batchQualityMetricsSummary],
);

const batchAfterScorecard = useMemo(
  () => buildBatchStoryAfterScorecardFromSummary(batchQualityMetricsSummary, batchSelectedCreativeMode, batchSelectedStoryFocus, {
    plotStage: batchSelectedPlotStage,
  }),
  [batchQualityMetricsSummary, batchSelectedCreativeMode, batchSelectedPlotStage, batchSelectedStoryFocus],
);

const batchScoreDrivenRecommendationCard = useMemo(
  () => buildBatchScoreDrivenRecommendationCardFromSummary(
    batchQualityMetricsSummary,
    batchSelectedCreativeMode,
    batchSelectedStoryFocus,
    {
      plotStage: batchSelectedPlotStage,
      chapterNumber: batchStartChapterNumber,
      totalChapters: knownStructureChapterCount,
      activePresetId: activeBatchCreationPreset?.id,
    },
  ),
  [
    activeBatchCreationPreset?.id,
    batchQualityMetricsSummary,
    batchSelectedCreativeMode,
    batchSelectedPlotStage,
    batchSelectedStoryFocus,
    batchStartChapterNumber,
    knownStructureChapterCount,
  ],
);

const batchStoryRepairTargetCard = useMemo(
  () => buildBatchStoryRepairTargetCardFromSummary(batchQualityMetricsSummary, batchSelectedCreativeMode, batchSelectedStoryFocus, {
    plotStage: batchSelectedPlotStage,
    chapterNumber: batchStartChapterNumber,
    totalChapters: knownStructureChapterCount,
    activePresetId: activeBatchCreationPreset?.id,
  }),
  [
    activeBatchCreationPreset?.id,
    batchQualityMetricsSummary,
    batchSelectedCreativeMode,
    batchSelectedPlotStage,
    batchSelectedStoryFocus,
    batchStartChapterNumber,
    knownStructureChapterCount,
  ],
);

const batchStoryCreationControlCard = useMemo(
  () => buildBatchStoryCreationControlCardFromSummary(batchQualityMetricsSummary, batchSelectedCreativeMode, batchSelectedStoryFocus, {
    plotStage: batchSelectedPlotStage,
    chapterNumber: batchStartChapterNumber,
    totalChapters: knownStructureChapterCount,
    activePresetId: activeBatchCreationPreset?.id,
  }),
  [
    activeBatchCreationPreset?.id,
    batchQualityMetricsSummary,
    batchSelectedCreativeMode,
    batchSelectedPlotStage,
    batchSelectedStoryFocus,
    batchStartChapterNumber,
    knownStructureChapterCount,
  ],
);

const batchSystemStoryCreationBrief = batchStoryCreationControlCard?.promptBrief ?? '';

const batchCreationBlueprint = useMemo(
  () => buildCreationBlueprint(batchSelectedCreativeMode, batchSelectedStoryFocus, {
    scene: 'chapter',
    plotStage: batchSelectedPlotStage,
  }),
  [batchSelectedCreativeMode, batchSelectedPlotStage, batchSelectedStoryFocus],
);

const batchSelectedCreativeModeLabel = batchSelectedCreativeMode
  ? (CREATIVE_MODE_OPTIONS.find((item: any) => item.value === batchSelectedCreativeMode)?.label || batchSelectedCreativeMode)
  : '????';
const batchSelectedStoryFocusLabel = batchSelectedStoryFocus
  ? (STORY_FOCUS_OPTIONS.find((item: any) => item.value === batchSelectedStoryFocus)?.label || batchSelectedStoryFocus)
  : '????';
const batchSelectedPlotStageLabel = batchSelectedPlotStage
  ? (CREATION_PLOT_STAGE_OPTIONS.find((item: any) => item.value === batchSelectedPlotStage)?.label || batchSelectedPlotStage)
  : '????';
const batchSelectedModelLabel = batchSelectedModel
  ? (normalizedAvailableModels.find((item) => item.value === batchSelectedModel)?.label || batchSelectedModel)
  : '????';
const batchQualityAnalysisLabel = batchEnableAnalysis === false ? '???' : '????';

const batchQualityProfileItems = useMemo(
  () => getQualityProfileDisplayItems(batchProgress?.quality_profile_summary),
  [batchProgress?.quality_profile_summary],
);

const batchSummaryMetricItems = useMemo(
  () => getBatchSummaryMetricItems(batchQualityMetricsSummary),
  [batchQualityMetricsSummary],
);

const batchVolumePacingPlan = useMemo(
  () => buildVolumePacingPlan(knownStructureChapterCount, {
    preferredStage: batchSelectedPlotStage,
    currentChapterNumber: batchStartChapterNumber,
  }),
  [batchSelectedPlotStage, batchStartChapterNumber, knownStructureChapterCount],
);

const batchStoryInsightCards = useMemo(
  () => buildBatchStoryInsightCards(batchSelectedCreativeMode, batchSelectedStoryFocus, {
    plotStage: batchSelectedPlotStage,
  }),
  [batchSelectedCreativeMode, batchSelectedPlotStage, batchSelectedStoryFocus],
);

  return (
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

              startChapterNumber: sortedChapters.find((ch: any) => !ch.content || ch.content.trim() === '')?.chapter_number || 1,

              count: 5,

              enableAnalysis: true,

              styleId: selectedStyleId,

              targetWordCount: getCachedWordCount(),

              model: selectedModel,

            }}

          >

            {renderCompactSettingFlow(
              "批量生成会基于当前设置连续生成多个章节。",
              "先选起始章和数量，再决定是否附带分析；适合补稿或集中铺量。",
              ["选择起始章", "设定生成数量", "可选附带分析"],
              { style: { marginBottom: 16 } },
            )}




            <div style={{ display: 'flex', flexDirection: isMobile ? 'column' : 'row', gap: isMobile ? 0 : 16 }}>

              <Form.Item

                label="起始章节"

                name="startChapterNumber"

                rules={[{ required: true, message: '请选择起始章节' }]}

                style={{ flex: 1, marginBottom: 12 }}

              >

                <Select placeholder="请选择章节">

                  {normalizedBatchStartChapterOptions.map((ch: any) => (

                    <Select.Option key={ch.id ?? `chapter-${ch.chapter_number}`} value={ch.chapter_number}>

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




            {renderCompactSettingFlow(
              '批量任务先锁统一方向，再决定要不要展开后两步。',
              '想省心时，完成基础约束和快速预设即可；故事总控与微调用于统一多章节奏。',
              [
                '基础约束',
                '快速预设',
                '故事总控',
                '补充微调',
              ],
            )}

            <Card
              size="small"
              title="基础约束"
              style={{ marginBottom: 12 }}
            >
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

                  {normalizedWritingStyles.map((style: any) => (

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
                {CREATION_PLOT_STAGE_OPTIONS.map((option: any) => (
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
                    已选择： {CREATION_PLOT_STAGE_OPTIONS.find((item: any) => item.value === batchSelectedPlotStage)?.label || batchSelectedPlotStage}
                  </span>
                )}
              </Space>
            </Form.Item>
            </Card>


            <Card
              size="small"
              title="快速预设"
              style={{ marginBottom: 12 }}
            >
              <Space wrap>
                {CREATION_PRESETS.map((preset: any) => (
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

              {activeBatchCreationPreset && renderCompactSettingHint(
                `已选预设：${activeBatchCreationPreset.label}`,
                activeBatchCreationPreset.description,
                { style: { marginTop: 12 }, tone: 'success' },
              )}

              {renderCompactPresetRecommendationBlock(batchRecommendedCreationPresets, {
                activePresetId: activeBatchCreationPreset?.id,
                applyPreset: applyBatchCreationPreset,
                          })}

              {batchScoreDrivenRecommendationCard && (
                <Card size="small" title={batchScoreDrivenRecommendationCard.title} style={{ marginTop: 12 }}>
                  <Space direction="vertical" size={10} style={{ display: 'flex' }}>
                    {renderCompactSettingHint(
                      batchScoreDrivenRecommendationCard.summary,
                      batchScoreDrivenRecommendationCard.applyHint,
                    )}

                    {batchScoreDrivenRecommendationCard.recommendedPresetLabel && renderCompactStoryControlHeader(
                      '推荐预设',
                      batchScoreDrivenRecommendationCard.recommendedPresetReason || '优先用这个预设起步。',
                      {
                        tagText: batchScoreDrivenRecommendationCard.recommendedPresetLabel,
                        tagColor: batchScoreDrivenRecommendationCard.recommendedPresetId === activeBatchCreationPreset?.id ? 'blue' : 'processing',
                      },
                    )}

                    {renderCompactStoryControlHeader(
                      '推荐阶段',
                      batchScoreDrivenRecommendationCard.stageReason,
                      {
                        tagText: batchScoreDrivenRecommendationCard.recommendedStageLabel,
                        tagColor: batchScoreDrivenRecommendationCard.recommendedStage === batchSelectedPlotStage ? 'blue' : 'purple',
                      },
                    )}

                    {batchScoreDrivenRecommendationCard.alternatives.length > 0 && (
                      renderCompactListCard(
                        '备选方案',
                        batchScoreDrivenRecommendationCard.alternatives.map((item: any) => (
                          item.reason ? `${item.label}：${item.reason}` : item.label
                        )),
                        { tagText: `${batchScoreDrivenRecommendationCard.alternatives.length}项` },
                      )
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
                          {"一键应用"}
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

                  {renderCompactSettingHint(
                    batchStoryCreationControlCard.summary,
                    batchStoryCreationControlCard.directive,
                  )}
                  <Space direction="vertical" size={8} style={{ display: 'flex' }}>
                    <div style={{ padding: '10px 12px', border: '1px solid #f0f0f0', borderRadius: 8 }}>
                      {renderCompactStoryControlHeader(
                        '故事简介',
                        '一句话说明本轮方向。',
                        {
                          tagText: isBatchStoryCreationBriefCustomized ? '自定义' : '系统建议',
                          tagColor: isBatchStoryCreationBriefCustomized ? 'purple' : 'blue',
                        },
                      )}
                      <TextArea
                        value={batchStoryCreationBriefDraft}
                        onChange={(event) => setBatchStoryCreationBriefDraft(event.target.value)}
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
                          tagText: isBatchStoryBeatPlannerCustomized ? '自定义' : '系统建议',
                          tagColor: isBatchStoryBeatPlannerCustomized ? 'purple' : 'blue',
                          action: (
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
                          ),
                        },
                      )}
                      <Space direction="vertical" size={8} style={{ display: 'flex' }}>
                        {STORY_BEAT_PLANNER_FIELDS.map((field: any) => (
                          <div key={field.key}>
                            <div style={{ fontSize: 12, fontWeight: 600, marginBottom: 6 }}>{field.label}</div>
                            <Input
                              value={batchStoryBeatPlannerDraft[field.key]}
                              onChange={(event) => setBatchStoryBeatPlannerDraft((prev: any) => ({
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
                          tagText: isBatchStorySceneOutlineCustomized ? '自定义' : '系统建议',
                          tagColor: isBatchStorySceneOutlineCustomized ? 'purple' : 'blue',
                          action: (
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
                          ),
                        },
                      )}
                      <Space direction="vertical" size={8} style={{ display: 'flex' }}>
                        {STORY_SCENE_OUTLINE_FIELDS.map((field: any) => (
                          <div key={field.key}>
                            <div style={{ fontSize: 12, fontWeight: 600, marginBottom: 6 }}>{field.label}</div>
                            <TextArea
                              value={batchStorySceneOutlineDraft[field.key]}
                              onChange={(event) => setBatchStorySceneOutlineDraft((prev: any) => ({
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
  prompt={resolvedBatchStoryCreationBrief}
  promptLayerLabels={batchStoryCreationPromptLayerLabels}
  promptCharCount={batchStoryCreationPromptCharCount}
  isVerbose={isBatchStoryCreationPromptVerbose}
  onCopy={() => void copyStoryCreationPrompt(resolvedBatchStoryCreationBrief, 'batch')}
  placeholder="???????????"
/>
                    <StoryCreationSnapshotPanel
                      scopeLabel="batch"
                      emptyText="还没有快照。"
                      snapshots={batchStoryCreationSnapshots}
                      currentDraft={batchStoryCreationCurrentDraft}
                      canSave={canSaveBatchStoryCreationSnapshot}
                      onSave={() => void saveBatchStoryCreationSnapshot('manual')}
                      onApply={applyBatchStoryCreationSnapshot}
                      onDelete={deleteBatchStoryCreationSnapshot}
                      onCopy={copyStoryCreationPrompt}
                      promptWarnThreshold={STORY_CREATION_PROMPT_WARN_THRESHOLD}
                    />
                    <div
                      style={{
                        display: 'grid',
                        gridTemplateColumns: isMobile ? '1fr' : 'repeat(3, minmax(0, 1fr))',
                        gap: 8,
                      }}
                    >
                      {renderCompactListCard('执行路径', batchStoryCreationControlCard.executionPath, { numbered: true })}
                      {renderCompactListCard('预期结果', batchStoryCreationControlCard.expectedOutcomes, { numbered: true })}
                      {renderCompactListCard('约束规则', batchStoryCreationControlCard.guardrails)}
                    </div>
                  </Space>
                </Card>
              )}

            {(batchStoryRepairTargetCard || batchCreationBlueprint) && (
              <div
                style={{
                  display: 'grid',
                  gridTemplateColumns: isMobile ? '1fr' : 'repeat(2, minmax(0, 1fr))',
                  gap: 12,
                  marginBottom: 12,
                }}
              >
                {batchStoryRepairTargetCard && (
                  <Card
                    size="small"
                    title={batchStoryRepairTargetCard.title}
                    extra={<Tag color="gold">修复重点</Tag>}
                    style={{ height: '100%' }}
                  >
                    {renderCompactSettingHint(
                      batchStoryRepairTargetCard.repairSummary,
                      batchStoryRepairTargetCard.applyHint,
                      { tone: 'warning' },
                    )}
                    <div
                      style={{
                        display: 'grid',
                        gridTemplateColumns: isMobile ? '1fr' : 'repeat(2, minmax(0, 1fr))',
                        gap: 8,
                      }}
                    >
                      {renderCompactFactCard('优先修复项', batchStoryRepairTargetCard.priorityTarget)}
                      {renderCompactFactCard('反模式', batchStoryRepairTargetCard.antiPattern)}
                      {renderCompactListCard('修复目标', batchStoryRepairTargetCard.repairTargets, { tagColor: 'gold' })}
                      {renderCompactListCard('保留优势', batchStoryRepairTargetCard.preserveStrengths, { tagColor: 'green' })}
                    </div>
                  </Card>
                )}

                {batchCreationBlueprint && (
                  <Card size="small" title="批量创作蓝图" style={{ height: '100%' }}>
                    <div style={{ color: 'var(--color-text-secondary)', marginBottom: 10 }}>
                      {batchCreationBlueprint.summary}
                    </div>
                    {renderCompactListCard(
                      '关键节拍',
                      batchCreationBlueprint.beats,
                      { numbered: true, tagText: `${batchCreationBlueprint.beats.length}拍` },
                    )}
                    {batchCreationBlueprint.risks.length > 0 && (
                      renderCompactSettingHint(
                        '风险提示',
                        batchCreationBlueprint.risks.join(', '),
                        { tone: 'warning', style: { marginTop: 12, marginBottom: 0 } },
                      )
                    )}
                  </Card>
                )}
              </div>
            )}

            {renderCompactInsightCardGrid(batchStoryInsightCards, isMobile, { style: { marginBottom: 12 } })}


            {batchVolumePacingPlan && (
              <Card size="small" title="篇幅节奏规划" style={{ marginBottom: 12 }}>
                {renderCompactSettingHint(
                  `当前阶段：${batchSelectedPlotStageLabel}`,
                  batchVolumePacingPlan.summary,
                  { style: { marginBottom: 10 } },
                )}
                {renderCompactListCard(
                  "章节分段",
                  batchVolumePacingPlan.segments.map(
                    (segment: any) => `?${segment.startChapter}-${segment.endChapter}? ? ${segment.label}?${segment.mission}`,
                  ),
                  { tagText: `${batchVolumePacingPlan.segments.length}段` },
                )}
              </Card>
            )}

            <Card size="small" title="补充微调（可选）" style={{ marginBottom: 12 }}>
              {renderCompactSettingHint(
                "批量通常只建议调整模式、聚焦和模型；质量分析按需开启即可。",
                "不改则沿用默认推荐，先稳定生成，再根据结果回头细调。",
                { style: { marginBottom: 10 } },
              )}
              {renderCompactSelectionSummary(
                [
                  { label: "模式", value: batchSelectedCreativeModeLabel, color: "blue" },
                  { label: "聚焦", value: batchSelectedStoryFocusLabel, color: "purple" },
                  { label: "模型", value: batchSelectedModelLabel },
                  { label: "分析", value: batchQualityAnalysisLabel, color: batchEnableAnalysis === false ? "default" : "green" },
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
                  tooltip="控制这一批章节的主要写法偏向"
                  style={{ marginBottom: 0 }}
                >
                  <Select
                    placeholder="留空=默认推荐"
                    value={batchSelectedCreativeMode}
                    onChange={setBatchSelectedCreativeMode}
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
                  tooltip="控制这一批章节的主要发力点"
                  style={{ marginBottom: 0 }}
                >
                  <Select
                    placeholder="留空=默认推荐"
                    value={batchSelectedStoryFocus}
                    onChange={setBatchSelectedStoryFocus}
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
                  label="AI 模型"
                  tooltip="留空则沿用项目默认模型"
                  style={{ marginBottom: 0 }}
                >
                  <Select
                    placeholder={batchSelectedModel ? `已选择：${batchSelectedModelLabel}` : "留空=项目默认"}
                    value={batchSelectedModel ?? undefined}
                    onChange={setBatchSelectedModel}
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
                <Form.Item
                  label="开启质量分析"
                  name="enableAnalysis"
                  tooltip="决定批量生成后是否自动附带质量建议"
                  style={{ marginBottom: 0 }}
                >
                  <div>
                    <Radio.Group optionType="button" buttonStyle="solid">
                      <Radio.Button value={true}>开启分析</Radio.Button>
                      <Radio.Button value={false}>仅生成</Radio.Button>
                    </Radio.Group>
                    <div style={{ fontSize: 12, color: "var(--color-text-secondary)", marginTop: 6 }}>
                      关闭后只生成正文，不自动附带质量分析。
                    </div>
                  </div>
                </Form.Item>
              </div>
            </Card>


          </Form>

        ) : (

          <div>

            {renderCompactSettingHint(
              `批量生成进行中：${batchProgress?.completed || 0}/${batchProgress?.total || 0} 章`,
              "可关闭窗口后台继续；需要停止时在下方取消生成。",
              { style: { marginBottom: 10 } },
            )}
            {renderCompactSelectionSummary(
              [
                ...(batchProgress?.current_chapter_number
                  ? [{ label: "当前", value: `第${batchProgress.current_chapter_number}章`, color: "blue" }]
                  : []),
                { label: "进度", value: `${batchProgress?.completed || 0}/${batchProgress?.total || 0}`, color: "blue" },
                ...(batchProgress?.estimated_time_minutes && batchProgress.completed === 0
                  ? [{ label: "预计", value: `${batchProgress.estimated_time_minutes}分钟` }]
                  : []),
                ...(batchProgress?.quality_metrics_summary?.avg_overall_score !== undefined
                  ? [{
                      label: "均分",
                      value: `${batchProgress.quality_metrics_summary.avg_overall_score}`,
                      color: getOverallScoreColor(batchProgress.quality_metrics_summary.avg_overall_score),
                    }]
                  : []),
              ],
              { style: { marginBottom: 16 } },
            )}



            {batchProgress?.quality_profile_summary && batchQualityProfileItems.length > 0 && (

              <Card size="small" title="质量画像摘要" style={{ marginBottom: 16 }}>
                {renderCompactSettingHint(
                  "质量画像用于概括当前批次的风格、维度与优化方向。",
                  "优先关注不稳定或偏离目标的条目。",
                  { tone: "success", style: { marginBottom: 10 } },
                )}
                {renderCompactFactGrid(
                  batchQualityProfileItems.map((item: any) => [item.label, item.description] as [string, string]),
                )}
              </Card>

            )}



            {batchProgress?.quality_metrics_summary?.avg_overall_score !== undefined && (
              <Card size="small" title="质量指标摘要" style={{ marginBottom: 16 }}>
                {batchAfterScorecard && (
                  renderCompactSettingHint(
                    batchAfterScorecard.verdict,
                    `${batchAfterScorecard.summary} ${batchAfterScorecard.nextAction}`,
                    {
                      tone: getCompactHintToneByAlertType(batchAfterScorecard.verdictColor as "success" | "info" | "warning" | "error"),
                      style: { marginBottom: 10 },
                    },
                  )
                )}
                {renderCompactSelectionSummary(
                  [
                    {
                      label: "平均得分",
                      value: `${batchProgress?.quality_metrics_summary?.avg_overall_score ?? 0}`,
                      color: getOverallScoreColor(batchProgress?.quality_metrics_summary?.avg_overall_score),
                    },
                    { label: "已完成", value: `${batchProgress?.completed || 0}/${batchProgress?.total || 0}`, color: "blue" },
                  ],
                  { style: { marginBottom: 10 } },
                )}
                {renderCompactMetricGrid(batchSummaryMetricItems)}
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
  );
}

export default memo(ChapterBatchGenerateModal, areBatchGenerateModalPropsEqual);
