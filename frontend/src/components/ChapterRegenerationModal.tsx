import React, { useState, useEffect } from 'react';
import {
  Modal,
  Form,
  Input,
  Button,
  Checkbox,
  InputNumber,
  Space,
  Alert,
  Divider,
  Tag,
  message,
  Collapse,
  Card,
  Radio
} from 'antd';
import {
  ReloadOutlined,
  CheckCircleOutlined,
  CloseCircleOutlined
} from '@ant-design/icons';
import type { StoryRepairGuidance } from '../types';
import { ssePost } from '../utils/sseClient';
import { getRepairGuidanceDisplay } from '../utils/storyCreationQualitySummary';
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
  { value: 'cliffhanger', label: '章尾牵引' },
];

const getFocusAreaLabel = (value: string): string => {
  return FOCUS_AREA_OPTIONS.find((item) => item.value === value)?.label || value;
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
}) => {
  const [form] = Form.useForm();
  const [modal, contextHolder] = Modal.useModal();
  const [loading, setLoading] = useState(false);
  const [progress, setProgress] = useState(0);
  const [status, setStatus] = useState<'idle' | 'generating' | 'success' | 'error'>('idle');
  const [errorMessage, setErrorMessage] = useState('');
  const [wordCount, setWordCount] = useState(0);
  const [selectedSuggestions, setSelectedSuggestions] = useState<number[]>([]);
  const [modificationSource, setModificationSource] = useState<'custom' | 'analysis_suggestions' | 'mixed'>('custom');

  const repairGuidanceDisplay = getRepairGuidanceDisplay(repairGuidance);
  const recommendedFocusAreas = (repairGuidanceDisplay?.focusAreas || []).filter((item) =>
    FOCUS_AREA_OPTIONS.some((option) => option.value === item),
  );
  const recommendedFocusAreasKey = recommendedFocusAreas.join(',');
  const hasRepairGuidance = Boolean(
    repairGuidanceDisplay?.summary
      || repairGuidanceDisplay?.repairTargets.length
      || repairGuidanceDisplay?.preserveStrengths.length,
  );

  useEffect(() => {
    if (visible) {
      // 重置状态
      setStatus('idle');
      setProgress(0);
      setErrorMessage('');
      setWordCount(0);
      setSelectedSuggestions([]);
      
      // 如果有分析建议，默认选择混合模式
      if (hasAnalysis && suggestions.length > 0) {
        setModificationSource('mixed');
      } else {
        setModificationSource('custom');
      }
      
      // 设置默认值
      form.setFieldsValue({
        modification_source: hasAnalysis && suggestions.length > 0 ? 'mixed' : 'custom',
        target_word_count: 3000,
        preserve_structure: false,
        preserve_character_traits: true,
        focus_areas: recommendedFocusAreas,
      });
    }
  }, [visible, hasAnalysis, suggestions.length, form, recommendedFocusAreasKey]);

  const handleSubmit = async () => {
    try {
      const values = await form.validateFields();
      
      // ?????????????
      const hasCustomInstructions = Boolean(values.custom_instructions?.trim());
      const hasFocusAreas = Array.isArray(values.focus_areas) && values.focus_areas.length > 0;

      if (values.modification_source === 'custom' && !hasCustomInstructions && !hasFocusAreas && !hasRepairGuidance) {
        message.error('请输入自定义修改要求，或直接使用自动修复建议 / 重点优化方向');
        return;
      }
      
      if (values.modification_source === 'analysis_suggestions' && selectedSuggestions.length === 0) {
        message.error('请选择至少一条分析建议');
        return;
      }
      
      if (values.modification_source === 'mixed' && 
          selectedSuggestions.length === 0 && 
          !hasCustomInstructions &&
          !hasFocusAreas &&
          !hasRepairGuidance) {
        message.error('请至少选择一条建议、输入自定义要求，或直接使用自动修复建议 / 重点优化方向');
        return;
      }


      setLoading(true);
      setStatus('generating');
      setProgress(0);
      setWordCount(0);

      // 构建请求数据
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
        style_id?: string;
        target_word_count: number;
        focus_areas: string[];
        story_repair_summary?: string;
        story_repair_targets?: string[];
        story_preserve_strengths?: string[];
      }

      const requestData: RegenerationRequest = {
        modification_source: values.modification_source,
        custom_instructions: values.custom_instructions,
        selected_suggestion_indices: selectedSuggestions,
        preserve_elements: {
          preserve_structure: values.preserve_structure,
          preserve_dialogues: values.preserve_dialogues || [],
          preserve_plot_points: values.preserve_plot_points || [],
          preserve_character_traits: values.preserve_character_traits
        },
        style_id: values.style_id,
        target_word_count: values.target_word_count,
        focus_areas: values.focus_areas || [],
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

      // 使用SSE流式生成
      await ssePost(
        `/api/chapters/${chapterId}/regenerate-stream`,
        requestData,
        {
          onProgress: (_msg: string, prog: number, _status: string, wordCount?: number) => {
            // 后端发送的进度消息
            setProgress(prog);
            // 如果后端提供了word_count，使用它；否则使用累积的字数
            if (wordCount !== undefined) {
              setWordCount(wordCount);
              currentWordCount = wordCount;
            }
          },
          onChunk: (content: string) => {
            // 累积内容块
            accumulatedContent += content;
            // 仅作为备用字数统计
            currentWordCount = accumulatedContent.length;
            // 不再自己计算进度，完全依赖后端发送的progress消息
          },
          onResult: (data: { word_count?: number }) => {
            // 生成完成，确保使用最新的累积内容
            setProgress(100);
            setStatus('success');
            const finalWordCount = data.word_count || currentWordCount;
            setWordCount(finalWordCount);
            message.success('重新生成完成！');
            
            // 直接调用onSuccess打开对比界面，传递最终的累积内容
            setTimeout(() => {
              onSuccess(accumulatedContent, finalWordCount);
            }, 500);
          },
          onComplete: () => {
            // SSE完成
          },
          onError: (error: string, code?: number) => {
            console.error('SSE Error:', error, code);
            setStatus('error');
            setErrorMessage(error || '生成失败');
            message.error('重新生成失败: ' + (error || '未知错误'));
          }
        }
      );

    } catch (error: unknown) {
      console.error('提交失败:', error);
      setStatus('error');
      const err = error as Error;
      setErrorMessage(err.message || '提交失败');
      message.error('操作失败: ' + (err.message || '未知错误'));
    } finally {
      setLoading(false);
    }
  };

  const handleSuggestionSelect = (index: number, checked: boolean) => {
    if (checked) {
      setSelectedSuggestions([...selectedSuggestions, index]);
    } else {
      setSelectedSuggestions(selectedSuggestions.filter(i => i !== index));
    }
  };

  const handleCancel = () => {
    if (loading) {
      modal.confirm({
        title: '确认取消',
        content: '生成正在进行中，确定要取消吗？',
        centered: true,
        onOk: () => {
          setLoading(false);
          setStatus('idle');
          onCancel();
        }
      });
    } else {
      onCancel();
    }
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
        status === 'success' ? null : (
          [
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
            </Button>
          ]
        )
      }
    >

      {status === 'success' && (
        <Alert
          message="重新生成成功！"
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
                      ? ` ${repairGuidanceDisplay.weakestMetricValue.toFixed(1)}分`
                      : ''}
                  </span>
                </div>
              )}
              {repairGuidanceDisplay.repairTargets.length > 0 && (
                <div>
                  <strong>下一轮修复：</strong>
                  <div style={{ marginTop: 8, display: 'flex', flexWrap: 'wrap', gap: 8 }}>
                    {repairGuidanceDisplay.repairTargets.map((item) => (
                      <Tag key={item} color="blue">{item}</Tag>
                    ))}
                  </div>
                </div>
              )}
              {repairGuidanceDisplay.preserveStrengths.length > 0 && (
                <div>
                  <strong>保留优势：</strong>
                  <div style={{ marginTop: 8, display: 'flex', flexWrap: 'wrap', gap: 8 }}>
                    {repairGuidanceDisplay.preserveStrengths.map((item) => (
                      <Tag key={item} color="green">{item}</Tag>
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

      <Form
        form={form}
        layout="vertical"
        disabled={loading || status === 'success'}
      >
        {/* 修改来源 */}
        <Form.Item
          name="modification_source"
          label="修改来源"
          rules={[{ required: true, message: '请选择修改来源' }]}
        >
          <Radio.Group onChange={(e) => setModificationSource(e.target.value)}>
            <Radio value="custom">仅自定义修改</Radio>
            {hasAnalysis && suggestions.length > 0 && (
              <>
                <Radio value="analysis_suggestions">仅分析建议</Radio>
                <Radio value="mixed">混合模式</Radio>
              </>
            )}
          </Radio.Group>
        </Form.Item>

        {/* 分析建议选择 */}
        {hasAnalysis && suggestions.length > 0 && 
         (modificationSource === 'analysis_suggestions' || modificationSource === 'mixed') && (
          <Form.Item label={`选择分析建议 (${selectedSuggestions.length}/${suggestions.length})`}>
            <Card size="small" style={{ maxHeight: 300, overflow: 'auto' }}>
              <Space direction="vertical" style={{ width: '100%' }}>
                {suggestions.map((suggestion, index) => (
                  <Checkbox
                    key={index}
                    checked={selectedSuggestions.includes(index)}
                    onChange={(e) => handleSuggestionSelect(index, e.target.checked)}
                  >
                    <Space>
                      <Tag color={
                        suggestion.priority === 'high' ? 'red' :
                        suggestion.priority === 'medium' ? 'orange' : 'blue'
                      }>
                        {suggestion.category}
                      </Tag>
                      <span style={{ fontSize: 13 }}>{suggestion.content}</span>
                    </Space>
                  </Checkbox>
                ))}
              </Space>
            </Card>
          </Form.Item>
        )}

        {/* 自定义修改要求 */}
        {(modificationSource === 'custom' || modificationSource === 'mixed') && (
          <Form.Item
            name="custom_instructions"
            label="自定义修改要求"
            tooltip="描述你希望如何改进这个章节"
            extra={repairGuidanceDisplay ? '如果不填写，将默认结合自动修复建议和重点方向进行重生成。' : undefined}
          >
            <TextArea
              rows={4}
              placeholder="例如：增强情感渲染，让主角的内心戏更加细腻..."
              showCount
              maxLength={1000}
            />
          </Form.Item>
        )}

        {/* 高级选项 */}
        <Collapse ghost>
          <Panel header="高级选项" key="advanced">
            {/* 重点优化方向 */}
            <Form.Item
              name="focus_areas"
              label="重点优化方向"
            >
              <Checkbox.Group>
                <div style={{ display: 'grid', gridTemplateColumns: 'repeat(auto-fit, minmax(160px, 1fr))', gap: 8 }}>
                  {FOCUS_AREA_OPTIONS.map((option) => (
                    <Checkbox key={option.value} value={option.value}>
                      {option.label}
                    </Checkbox>
                  ))}
                </div>
              </Checkbox.Group>
            </Form.Item>

            <Divider />

            {/* 保留元素 */}
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

            {/* 生成参数 */}
            <Form.Item
              name="target_word_count"
              label="目标字数"
              tooltip="生成内容的目标字数，实际字数可能有±20%的浮动"
            >
              <InputNumber min={500} max={10000} step={500} style={{ width: '100%' }} />
            </Form.Item>

          </Panel>
        </Collapse>
      </Form>

      {/* 使用统一的进度显示组件 */}
      <SSEProgressModal
        visible={status === 'generating'}
        progress={progress}
        message={`正在重新生成中... (已生成 ${wordCount} 字)`}
        title="重新生成章节"
        blocking={false}
      />
      </Modal>
    </>
  );
};

export default ChapterRegenerationModal;
