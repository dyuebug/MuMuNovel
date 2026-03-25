import React, { useEffect, useState } from 'react';
import {
  Alert,
  Button,
  Card,
  Checkbox,
  Collapse,
  Divider,
  Form,
  Input,
  InputNumber,
  message,
  Modal,
  Radio,
  Select,
  Space,
  Tag,
} from 'antd';
import {
  CheckCircleOutlined,
  CloseCircleOutlined,
  ReloadOutlined,
} from '@ant-design/icons';

import type {
  ChapterQualityMetricsSummary,
  CreativeMode,
  PlotStage,
  QualityPreset,
  StoryFocus,
  StoryQualityGateDecision,
  StoryRepairGuidance,
} from '../types';
import { ssePost } from '../utils/sseClient';
import {
  getQualityGateRecommendedDefaults,
  getQualityTrendLabel,
  getRepairGuidanceDisplay,
} from '../utils/storyCreationQualitySummary';
import {
  CREATIVE_MODE_OPTIONS,
  PLOT_STAGE_OPTIONS,
  QUALITY_PRESET_OPTIONS,
  STORY_FOCUS_OPTIONS,
} from '../utils/generationPreferenceOptions';
import { SSEProgressModal } from './SSEProgressModal';

const { TextArea } = Input;
const { Panel } = Collapse;

const FOCUS_AREA_OPTIONS: Array<{ value: string; label: string }> = [
  { value: 'pacing', label: '节奏把控' },
  { value: 'emotion', label: '情感渲染' },
  { value: 'description', label: '场景描写' },
  { value: 'dialogue', label: '对话质量' },
  { value: 'conflict', label: '冲突强度' },
  { value: 'outline', label: '大纲贴合' },
  { value: 'rule_grounding', label: '规则落地' },
  { value: 'opening', label: '开场钩子' },
  { value: 'payoff', label: '回报兑现' },
  { value: 'cliffhanger', label: '章末牵引' },
];

const EXTRA_FOCUS_AREA_LABELS: Record<string, string> = {
  relationship_continuity: "关系连续性",
  foreshadow_continuity: "伏笔连续性",
  character_continuity: "角色连续性",
  organization_continuity: "组织连续性",
  career_continuity: "职业成长连续性",
};

const getFocusAreaLabel = (value: string): string => {
  return FOCUS_AREA_OPTIONS.find((item) => item.value === value)?.label || EXTRA_FOCUS_AREA_LABELS[value] || value;
};

interface Suggestion {
  category: string;
  content: string;
  priority: string;
}

interface ChapterRegenerationModalProps {
  visible: boolean;
  onCancel: () => void;
  onSuccess: (newContent: string, wordCount: number) => void;
  chapterId: string;
  chapterTitle: string;
  chapterNumber: number;
  suggestions?: Suggestion[];
  hasAnalysis: boolean;
  repairGuidance?: StoryRepairGuidance | null;
  qualityMetricsSummary?: ChapterQualityMetricsSummary | null;
  qualityGate?: StoryQualityGateDecision | null;
}

type ModificationSource = 'custom' | 'analysis_suggestions' | 'mixed';

interface RegenerationFormValues {
  modification_source: ModificationSource;
  custom_instructions?: string;
  preserve_structure?: boolean;
  preserve_character_traits?: boolean;
  preserve_dialogues?: string[];
  preserve_plot_points?: string[];
  style_id?: string | number;
  target_word_count: number;
  focus_areas?: string[];
  creative_mode?: CreativeMode;
  story_focus?: StoryFocus;
  plot_stage?: PlotStage;
  story_creation_brief?: string;
  quality_preset?: QualityPreset;
  quality_notes?: string;
}

interface RegenerationRequest {
  modification_source: string;
  custom_instructions?: string;
  selected_suggestion_indices: number[];
  preserve_elements: {
    preserve_structure: boolean;
    preserve_dialogues: string[];
    preserve_plot_points: string[];
    preserve_character_traits: boolean;
  };
  style_id?: string | number;
  target_word_count: number;
  focus_areas: string[];
  creative_mode?: CreativeMode;
  story_focus?: StoryFocus;
  plot_stage?: PlotStage;
  story_creation_brief?: string;
  quality_preset?: QualityPreset;
  quality_notes?: string;
  story_repair_summary?: string;
  story_repair_targets?: string[];
  story_preserve_strengths?: string[];
}

const ChapterRegenerationModal: React.FC<ChapterRegenerationModalProps> = ({
  visible,
  onCancel,
  onSuccess,
  chapterId,
  chapterTitle,
  chapterNumber,
  suggestions = [],
  hasAnalysis,
  repairGuidance = null,
  qualityMetricsSummary = null,
  qualityGate = null,
}) => {
  const [form] = Form.useForm<RegenerationFormValues>();
  const [modal, contextHolder] = Modal.useModal();
  const [loading, setLoading] = useState(false);
  const [progress, setProgress] = useState(0);
  const [status, setStatus] = useState<'idle' | 'generating' | 'success' | 'error'>('idle');
  const [errorMessage, setErrorMessage] = useState('');
  const [wordCount, setWordCount] = useState(0);
  const [selectedSuggestions, setSelectedSuggestions] = useState<number[]>([]);
  const [modificationSource, setModificationSource] = useState<ModificationSource>('custom');

  const repairGuidanceDisplay = getRepairGuidanceDisplay(repairGuidance);
  const effectiveQualityGate = qualityGate ?? qualityMetricsSummary?.quality_gate ?? null;
  const recommendedFocusAreas = (repairGuidanceDisplay?.rawFocusAreas || []).filter((item) =>
    FOCUS_AREA_OPTIONS.some((option) => option.value === item),
  );
  const recommendedRepairDefaults = getQualityGateRecommendedDefaults(effectiveQualityGate);
  const recommendedFocusAreasKey = recommendedFocusAreas.join(',');
  const recommendedRepairDefaultsKey = [
    recommendedRepairDefaults.creative_mode || '',
    recommendedRepairDefaults.story_focus || '',
    recommendedRepairDefaults.quality_preset || '',
  ].join(',');
  const hasRepairGuidance = Boolean(
    repairGuidanceDisplay?.summary
      || repairGuidanceDisplay?.repairTargets.length
      || repairGuidanceDisplay?.preserveStrengths.length,
  );
  const recentFocusAreas = (qualityMetricsSummary?.recent_focus_areas || []).filter(Boolean);
  const recentFailedMetricCounts = (qualityMetricsSummary?.recent_failed_metric_counts || []).filter((item) => Boolean(item?.label || item?.key));
  const qualityGateCounts = qualityMetricsSummary?.quality_gate_counts || null;
  const qualityGateSignalCount = (qualityGateCounts?.pass ?? 0) + (qualityGateCounts?.repairable ?? 0) + (qualityGateCounts?.blocked ?? 0);
  const continuityWarningCount = effectiveQualityGate?.continuity_warning_count
    ?? qualityMetricsSummary?.continuity_preflight?.warning_count
    ?? 0;
  const trendDirection = qualityMetricsSummary?.overall_score_trend;
  const trendDelta = qualityMetricsSummary?.overall_score_delta;
  const trendLabel = getQualityTrendLabel(trendDirection);
  const trendColor = trendDirection === 'rising' ? 'green' : trendDirection === 'falling' ? 'red' : 'blue';
  const hasQualitySignals = Boolean(
    effectiveQualityGate?.summary
      || effectiveQualityGate?.recommended_action_label
      || recentFocusAreas.length
      || recentFailedMetricCounts.length
      || continuityWarningCount
      || qualityGateSignalCount
      || trendLabel,
  );

  useEffect(() => {
    if (!visible) {
      return;
    }

    const defaultSource: ModificationSource = hasAnalysis && suggestions.length > 0 ? 'mixed' : 'custom';

    setStatus('idle');
    setProgress(0);
    setErrorMessage('');
    setWordCount(0);
    setSelectedSuggestions([]);
    setModificationSource(defaultSource);

    form.resetFields();
    form.setFieldsValue({
      modification_source: defaultSource,
      target_word_count: 3000,
      preserve_structure: false,
      preserve_character_traits: true,
      focus_areas: recommendedFocusAreas,
      ...recommendedRepairDefaults,
    });
  }, [
    visible,
    hasAnalysis,
    suggestions.length,
    form,
    recommendedFocusAreasKey,
    recommendedRepairDefaultsKey,
  ]);

  const handleSubmit = async () => {
    try {
      const values = await form.validateFields();
      const hasCustomInstructions = Boolean(values.custom_instructions?.trim());
      const hasFocusAreas = Array.isArray(values.focus_areas) && values.focus_areas.length > 0;

      if (values.modification_source === 'custom' && !hasCustomInstructions && !hasFocusAreas && !hasRepairGuidance) {
        message.error('请输入自定义修改要求，或直接使用自动修复建议 / 重点优化方向。');
        return;
      }

      if (values.modification_source === 'analysis_suggestions' && selectedSuggestions.length === 0) {
        message.error('请选择至少一条分析建议。');
        return;
      }

      if (
        values.modification_source === 'mixed'
        && selectedSuggestions.length === 0
        && !hasCustomInstructions
        && !hasFocusAreas
        && !hasRepairGuidance
      ) {
        message.error('请至少选择一条建议、输入自定义要求，或直接使用自动修复建议 / 重点优化方向。');
        return;
      }

      setLoading(true);
      setStatus('generating');
      setProgress(0);
      setWordCount(0);

      const requestData: RegenerationRequest = {
        modification_source: values.modification_source,
        custom_instructions: values.custom_instructions,
        selected_suggestion_indices: selectedSuggestions,
        preserve_elements: {
          preserve_structure: Boolean(values.preserve_structure),
          preserve_dialogues: values.preserve_dialogues || [],
          preserve_plot_points: values.preserve_plot_points || [],
          preserve_character_traits: values.preserve_character_traits !== false,
        },
        style_id: values.style_id,
        target_word_count: values.target_word_count,
        focus_areas: values.focus_areas || [],
        creative_mode: values.creative_mode,
        story_focus: values.story_focus,
        plot_stage: values.plot_stage,
        story_creation_brief: values.story_creation_brief,
        quality_preset: values.quality_preset,
        quality_notes: values.quality_notes,
        story_repair_summary: repairGuidanceDisplay?.summary || undefined,
        story_repair_targets: repairGuidanceDisplay?.repairTargets.length
          ? repairGuidanceDisplay.repairTargets.slice(0, 3)
          : undefined,
        story_preserve_strengths: repairGuidanceDisplay?.preserveStrengths.length
          ? repairGuidanceDisplay.preserveStrengths.slice(0, 2)
          : undefined,
      };

      let accumulatedContent = '';
      let currentWordCount = 0;

      await ssePost(`/api/chapters/${chapterId}/regenerate-stream`, requestData, {
        onProgress: (_msg: string, nextProgress: number, _status: string, nextWordCount?: number) => {
          setProgress(nextProgress);
          if (nextWordCount !== undefined) {
            setWordCount(nextWordCount);
            currentWordCount = nextWordCount;
          }
        },
        onChunk: (content: string) => {
          accumulatedContent += content;
          currentWordCount = accumulatedContent.length;
        },
        onResult: (data: { word_count?: number }) => {
          setProgress(100);
          setStatus('success');
          const finalWordCount = data.word_count || currentWordCount;
          setWordCount(finalWordCount);
          message.success('重新生成完成。');

          setTimeout(() => {
            onSuccess(accumulatedContent, finalWordCount);
          }, 500);
        },
        onComplete: () => undefined,
        onError: (error: string, code?: number) => {
          console.error('SSE Error:', error, code);
          setStatus('error');
          setErrorMessage(error || '生成失败');
          message.error(`生成失败: ${error || '未知错误'}`);
        },
      });
    } catch (error: unknown) {
      console.error('提交失败:', error);
      setStatus('error');
      const err = error as Error;
      setErrorMessage(err.message || '提交失败');
      message.error(`提交失败: ${err.message || '未知错误'}`);
    } finally {
      setLoading(false);
    }
  };

  const handleSuggestionSelect = (index: number, checked: boolean) => {
    if (checked) {
      setSelectedSuggestions((current) => [...current, index]);
      return;
    }
    setSelectedSuggestions((current) => current.filter((item) => item !== index));
  };

  const handleCancel = () => {
    if (!loading) {
      onCancel();
      return;
    }

    modal.confirm({
      title: '确认取消',
      content: '生成正在进行中，确定要取消吗？',
      centered: true,
      onOk: () => {
        setLoading(false);
        setStatus('idle');
        onCancel();
      },
    });
  };

  return (
    <>
      {contextHolder}
      <Modal
        title={`重新生成章节 - 第${chapterNumber}章：${chapterTitle}`}
        open={visible}
        onCancel={handleCancel}
        width={800}
        centered
        footer={
          status === 'success'
            ? null
            : [
                <Button key="cancel" onClick={handleCancel} disabled={loading}>
                  取消
                </Button>,
                <Button
                  key="submit"
                  type="primary"
                  onClick={handleSubmit}
                  loading={loading}
                  icon={<ReloadOutlined />}
                >
                  开始重新生成
                </Button>,
              ]
        }
      >
        {status === 'success' && (
          <Alert
            message="重新生成成功"
            description={`共生成 ${wordCount} 字`}
            type="success"
            showIcon
            icon={<CheckCircleOutlined />}
            style={{ marginBottom: 16 }}
          />
        )}

        {status === 'error' && (
          <Alert
            message="生成失败"
            description={errorMessage}
            type="error"
            showIcon
            icon={<CloseCircleOutlined />}
            style={{ marginBottom: 16 }}
          />
        )}

        {repairGuidanceDisplay && (
          <Alert
            type="info"
            showIcon
            style={{ marginBottom: 16 }}
            message="已加载自动修复建议"
            description={(
              <Space direction="vertical" size={8} style={{ width: '100%' }}>
                {repairGuidanceDisplay.summary && <span>{repairGuidanceDisplay.summary}</span>}

                {repairGuidanceDisplay.weakestMetricLabel && (
                  <div>
                    <Tag color="orange">当前短板</Tag>
                    <span>
                      {repairGuidanceDisplay.weakestMetricLabel}
                      {typeof repairGuidanceDisplay.weakestMetricValue === 'number'
                        ? ` ${repairGuidanceDisplay.weakestMetricValue.toFixed(1)}`
                        : ''}
                    </span>
                  </div>
                )}

                {repairGuidanceDisplay.repairTargets.length > 0 && (
                  <div>
                    <strong>下一轮修复：</strong>
                    <div style={{ marginTop: 8, display: 'flex', flexWrap: 'wrap', gap: 8 }}>
                      {repairGuidanceDisplay.repairTargets.map((item) => (
                        <Tag key={item} color="blue">
                          {item}
                        </Tag>
                      ))}
                    </div>
                  </div>
                )}

                {repairGuidanceDisplay.preserveStrengths.length > 0 && (
                  <div>
                    <strong>保留优势：</strong>
                    <div style={{ marginTop: 8, display: 'flex', flexWrap: 'wrap', gap: 8 }}>
                      {repairGuidanceDisplay.preserveStrengths.map((item) => (
                        <Tag key={item} color="green">
                          {item}
                        </Tag>
                      ))}
                    </div>
                  </div>
                )}

                {recommendedFocusAreas.length > 0 && (
                  <div>
                    <strong>已预选重点方向：</strong>
                    <div style={{ marginTop: 8, display: 'flex', flexWrap: 'wrap', gap: 8 }}>
                      {recommendedFocusAreas.map((area) => (
                        <Tag key={area}>{getFocusAreaLabel(area)}</Tag>
                      ))}
                    </div>
                  </div>
                )}
              </Space>
            )}
          />
        )}

        {hasQualitySignals && (
          <Alert
            type={effectiveQualityGate?.requires_manual_review ? 'warning' : 'info'}
            showIcon
            style={{ marginBottom: 16 }}
            message="最近质量信号"
            description={(
              <Space direction="vertical" size={8} style={{ width: '100%' }}>
                {effectiveQualityGate?.summary && <span>{effectiveQualityGate.summary}</span>}

                <div style={{ display: 'flex', flexWrap: 'wrap', gap: 8 }}>
                  {trendLabel && (
                    <Tag color={trendColor}>
                      {trendLabel}
                      {typeof trendDelta === 'number' ? ` ${trendDelta > 0 ? '+' : ''}${trendDelta.toFixed(1)}` : ''}
                    </Tag>
                  )}
                  {(qualityGateCounts?.blocked ?? 0) > 0 && <Tag color="red">人工复核 {(qualityGateCounts?.blocked ?? 0)}</Tag>}
                  {(qualityGateCounts?.repairable ?? 0) > 0 && <Tag color="orange">自动修复 {(qualityGateCounts?.repairable ?? 0)}</Tag>}
                  {(qualityGateCounts?.pass ?? 0) > 0 && <Tag color="green">?? {(qualityGateCounts?.pass ?? 0)}</Tag>}
                  {continuityWarningCount > 0 && <Tag color="gold">连续性预警 {continuityWarningCount}</Tag>}
                </div>

                {effectiveQualityGate?.recommended_action_label && (
                  <div>
                    <strong>建议动作：</strong>
                    <Tag color="magenta" style={{ marginLeft: 8 }}>
                      {effectiveQualityGate.recommended_action_label}
                    </Tag>
                  </div>
                )}

                {recentFailedMetricCounts.length > 0 && (
                  <div>
                    <strong>最近反复失分：</strong>
                    <div style={{ marginTop: 8, display: 'flex', flexWrap: 'wrap', gap: 8 }}>
                      {recentFailedMetricCounts.map((item) => (
                        <Tag key={`${item.key || item.label}-${item.count || 0}`} color="volcano">
                          {item.label || item.key}
                          {typeof item.count === 'number' ? ` ?${item.count}` : ''}
                        </Tag>
                      ))}
                    </div>
                  </div>
                )}

                {recentFocusAreas.length > 0 && (
                  <div>
                    <strong>建议动作：</strong>
                    <div style={{ marginTop: 8, display: 'flex', flexWrap: 'wrap', gap: 8 }}>
                      {recentFocusAreas.map((area) => (
                        <Tag key={area}>{getFocusAreaLabel(area)}</Tag>
                      ))}
                    </div>
                  </div>
                )}
              </Space>
            )}
          />
        )}

        <Form form={form} layout="vertical" disabled={loading || status === 'success'}>
          <Form.Item
            name="modification_source"
            label="修改来源"
            rules={[{ required: true, message: '请选择修改来源。' }]}
          >
            <Radio.Group onChange={(event) => setModificationSource(event.target.value)}>
              <Radio value="custom">仅自定义修改</Radio>
              {hasAnalysis && suggestions.length > 0 && (
                <>
                  <Radio value="analysis_suggestions">仅分析建议</Radio>
                  <Radio value="mixed">混合模式</Radio>
                </>
              )}
            </Radio.Group>
          </Form.Item>

          {hasAnalysis && suggestions.length > 0 && (
            (modificationSource === 'analysis_suggestions' || modificationSource === 'mixed') && (
              <Form.Item label={`选择分析建议 (${selectedSuggestions.length}/${suggestions.length})`}>
                <Card size="small" style={{ maxHeight: 300, overflow: 'auto' }}>
                  <Space direction="vertical" style={{ width: '100%' }}>
                    {suggestions.map((suggestion, index) => (
                      <Checkbox
                        key={index}
                        checked={selectedSuggestions.includes(index)}
                        onChange={(event) => handleSuggestionSelect(index, event.target.checked)}
                      >
                        <Space>
                          <Tag
                            color={
                              suggestion.priority === 'high'
                                ? 'red'
                                : suggestion.priority === 'medium'
                                  ? 'orange'
                                  : 'blue'
                            }
                          >
                            {suggestion.category}
                          </Tag>
                          <span style={{ fontSize: 13 }}>{suggestion.content}</span>
                        </Space>
                      </Checkbox>
                    ))}
                  </Space>
                </Card>
              </Form.Item>
            )
          )}

          {(modificationSource === 'custom' || modificationSource === 'mixed') && (
            <Form.Item
              name="custom_instructions"
              label="自定义修改要求"
              tooltip="描述你希望如何改进这一章。"
              extra={repairGuidanceDisplay ? '若留空，系统会结合自动修复建议与重点方向进行重生成。' : undefined}
            >
              <TextArea
                rows={4}
                placeholder="例如：增强情绪张力，保持主角行为逻辑一致，并在章末留下更强的牵引。"
                showCount
                maxLength={1000}
              />
            </Form.Item>
          )}

          <Collapse ghost>
            <Panel header="高级选项" key="advanced">
              <Form.Item name="focus_areas" label="重点优化方向">
                <Checkbox.Group>
                  <div
                    style={{
                      display: 'grid',
                      gridTemplateColumns: 'repeat(auto-fit, minmax(160px, 1fr))',
                      gap: 8,
                    }}
                  >
                    {FOCUS_AREA_OPTIONS.map((option) => (
                      <Checkbox key={option.value} value={option.value}>
                        {option.label}
                      </Checkbox>
                    ))}
                  </div>
                </Checkbox.Group>
              </Form.Item>

              <Divider />

              <Form.Item
                name="creative_mode"
                label="创作模式"
                tooltip="显式指定这一轮重生成更偏向的创作模式。"
              >
                <Select allowClear placeholder="默认沿用项目偏好" optionLabelProp="label">
                  {CREATIVE_MODE_OPTIONS.map((option) => (
                    <Select.Option key={option.value} value={option.value} label={option.label}>
                      <div>{option.label}</div>
                      <div style={{ fontSize: 12, color: 'var(--color-text-tertiary)' }}>{option.description}</div>
                    </Select.Option>
                  ))}
                </Select>
              </Form.Item>

              <Form.Item
                name="story_focus"
                label="结构侧重点"
                tooltip="指定本轮重生成优先承担的叙事任务。"
              >
                <Select allowClear placeholder="默认沿用项目偏好" optionLabelProp="label">
                  {STORY_FOCUS_OPTIONS.map((option) => (
                    <Select.Option key={option.value} value={option.value} label={option.label}>
                      <div>{option.label}</div>
                      <div style={{ fontSize: 12, color: 'var(--color-text-tertiary)' }}>{option.description}</div>
                    </Select.Option>
                  ))}
                </Select>
              </Form.Item>

              <Form.Item
                name="plot_stage"
                label="剧情阶段"
                tooltip="帮助系统理解当前章节更像铺陈、高潮还是收束回收。"
              >
                <Select allowClear placeholder="默认沿用项目偏好" optionLabelProp="label">
                  {PLOT_STAGE_OPTIONS.map((option) => (
                    <Select.Option key={option.value} value={option.value} label={option.label}>
                      <div>{option.label}</div>
                      <div style={{ fontSize: 12, color: 'var(--color-text-tertiary)' }}>{option.description}</div>
                    </Select.Option>
                  ))}
                </Select>
              </Form.Item>

              <Form.Item
                name="story_creation_brief"
                label="创作总控摘要"
                tooltip="用几句话告诉系统这一轮重生成必须守住的目标、节奏和约束。"
              >
                <TextArea
                  rows={3}
                  placeholder="例如：优先补强这一章的情绪拉扯和结尾牵引，减少解释性旁白，让冲突与选择自己落地。"
                  showCount
                  maxLength={600}
                />
              </Form.Item>

              <Form.Item
                name="quality_preset"
                label="质量预设"
                tooltip="为这一轮重生成施加统一质量偏好，控制更偏推进、氛围、情绪或干净表达。"
              >
                <Select allowClear placeholder="默认沿用项目偏好" optionLabelProp="label">
                  {QUALITY_PRESET_OPTIONS.map((option) => (
                    <Select.Option key={option.value} value={option.value} label={option.label}>
                      <div>{option.label}</div>
                      <div style={{ fontSize: 12, color: 'var(--color-text-secondary)' }}>{option.description}</div>
                      <div style={{ fontSize: 12, color: 'var(--color-text-tertiary)' }}>适合：{option.bestFor}</div>
                    </Select.Option>
                  ))}
                </Select>
              </Form.Item>

              <Form.Item
                name="quality_notes"
                label="额外质量要求"
                tooltip="补充这一轮重生成需要特别强化或压制的表达习惯。"
              >
                <TextArea
                  rows={3}
                  placeholder="例如：减少解释性总结，优先通过动作、对话和场景细节传递情绪变化。"
                  showCount
                  maxLength={600}
                />
              </Form.Item>

              <Divider />

              <Form.Item label="保留元素">
                <Space direction="vertical" style={{ width: '100%' }}>
                  <Form.Item name="preserve_structure" valuePropName="checked" noStyle>
                    <Checkbox>保留整体结构和情节框架</Checkbox>
                  </Form.Item>
                  <Form.Item name="preserve_character_traits" valuePropName="checked" noStyle>
                    <Checkbox>保持角色性格一致</Checkbox>
                  </Form.Item>
                </Space>
              </Form.Item>

              <Divider />

              <Form.Item
                name="target_word_count"
                label="目标字数"
                tooltip="生成内容的目标字数，实际结果可能存在一定浮动。"
              >
                <InputNumber min={500} max={10000} step={500} style={{ width: '100%' }} />
              </Form.Item>
            </Panel>
          </Collapse>
        </Form>

        <SSEProgressModal
          visible={status === 'generating'}
          progress={progress}
          message={`正在重新生成中...（已生成 ${wordCount} 字）`}
          title="重新生成章节"
          blocking={false}
        />
      </Modal>
    </>
  );
};

export default ChapterRegenerationModal;
