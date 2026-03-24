import { useCallback, useMemo } from 'react';
import { Button, Card, Form, Input, Radio, Select, Space } from 'antd';
import type { FormInstance } from 'antd';

import type { CreativeMode, PlotStage, StoryFocus } from '../types';
import {
  CREATION_PRESETS,
  getCreationPresetById,
  getCreationPresetByModes,
  type CreationPresetId,
} from '../utils/creationPresetsCore';
import {
  buildCreationBlueprint,
  buildStoryExecutionChecklist,
  buildStoryObjectiveCard,
  buildStoryRepetitionRiskCard,
  buildStoryResultCard,
  buildStoryAcceptanceCard,
  buildStoryCharacterArcCard,
  buildVolumePacingPlan,
} from '../utils/creationPresetsStory';

const { TextArea } = Input;

const CREATIVE_MODE_OPTIONS: Array<{ value: CreativeMode; label: string; description: string }> = [
  { value: 'balanced', label: '????', description: '????????????????' },
  { value: 'hook', label: '????', description: '?????????????' },
  { value: 'emotion', label: '????', description: '?????????????' },
  { value: 'suspense', label: '????', description: '?????????????????' },
  { value: 'relationship', label: '????', description: '????????????????' },
  { value: 'payoff', label: '????', description: '????????????????' },
];

const STORY_FOCUS_OPTIONS: Array<{ value: StoryFocus; label: string; description: string }> = [
  { value: 'advance_plot', label: '????', description: '??????????????????' },
  { value: 'deepen_character', label: '????', description: '????????????????????' },
  { value: 'escalate_conflict', label: '????', description: '????????????????' },
  { value: 'reveal_mystery', label: '????', description: '???????????????????' },
  { value: 'relationship_shift', label: '????', description: '???????????????' },
  { value: 'foreshadow_payoff', label: '????', description: '????????????????' },
];

type OutlineGenerateFormValues = {
  mode?: 'auto' | 'new' | 'continue';
  chapter_count?: number;
  narrative_perspective?: string;
  theme?: string;
  story_direction?: string;
  target_words?: number;
  plot_stage?: PlotStage;
  creative_mode?: CreativeMode;
  story_focus?: StoryFocus;
  requirements?: string;
  model?: string;
  keep_existing?: boolean;
};

type OutlineProjectSnapshot = {
  narrative_perspective?: string | null;
  theme?: string | null;
} | null | undefined;

type ModelOption = {
  value: string;
  label: string;
};

type OutlineGenerateModalContentProps = {
  generateForm: FormInstance<OutlineGenerateFormValues>;
  currentProject: OutlineProjectSnapshot;
  outlinesCount: number;
  loadedModels: ModelOption[];
  defaultModel?: string;
  projectDefaultCreativeMode?: CreativeMode;
  projectDefaultStoryFocus?: StoryFocus;
  projectDefaultPlotStage?: PlotStage;
};

export default function OutlineGenerateModalContent({
  generateForm,
  currentProject,
  outlinesCount,
  loadedModels,
  defaultModel,
  projectDefaultCreativeMode,
  projectDefaultStoryFocus,
  projectDefaultPlotStage,
}: OutlineGenerateModalContentProps) {
  const hasOutlines = outlinesCount > 0;
  const initialMode = hasOutlines ? 'continue' : 'new';

  const selectedOutlineChapterCount = Form.useWatch('chapter_count', generateForm) as number | undefined;
  const selectedOutlineCreativeMode = Form.useWatch('creative_mode', generateForm) as CreativeMode | undefined;
  const selectedOutlineStoryFocus = Form.useWatch('story_focus', generateForm) as StoryFocus | undefined;
  const selectedOutlinePlotStage = Form.useWatch('plot_stage', generateForm) as PlotStage | undefined;

  const activeOutlineCreationPreset = useMemo(
    () => getCreationPresetByModes(selectedOutlineCreativeMode, selectedOutlineStoryFocus),
    [selectedOutlineCreativeMode, selectedOutlineStoryFocus],
  );

  const outlineCreationBlueprint = useMemo(
    () => buildCreationBlueprint(selectedOutlineCreativeMode, selectedOutlineStoryFocus, {
      scene: 'outline',
      plotStage: selectedOutlinePlotStage,
    }),
    [selectedOutlineCreativeMode, selectedOutlineStoryFocus, selectedOutlinePlotStage],
  );

  const outlineStoryObjectiveCard = useMemo(
    () => buildStoryObjectiveCard(selectedOutlineCreativeMode, selectedOutlineStoryFocus, {
      scene: 'outline',
      plotStage: selectedOutlinePlotStage,
    }),
    [selectedOutlineCreativeMode, selectedOutlineStoryFocus, selectedOutlinePlotStage],
  );

  const outlineStoryResultCard = useMemo(
    () => buildStoryResultCard(selectedOutlineCreativeMode, selectedOutlineStoryFocus, {
      scene: 'outline',
      plotStage: selectedOutlinePlotStage,
    }),
    [selectedOutlineCreativeMode, selectedOutlineStoryFocus, selectedOutlinePlotStage],
  );

  const outlineStoryExecutionChecklist = useMemo(
    () => buildStoryExecutionChecklist(selectedOutlineCreativeMode, selectedOutlineStoryFocus, {
      scene: 'outline',
      plotStage: selectedOutlinePlotStage,
    }),
    [selectedOutlineCreativeMode, selectedOutlineStoryFocus, selectedOutlinePlotStage],
  );

  const outlineStoryRepetitionRiskCard = useMemo(
    () => buildStoryRepetitionRiskCard(selectedOutlineCreativeMode, selectedOutlineStoryFocus, {
      scene: 'outline',
      plotStage: selectedOutlinePlotStage,
    }),
    [selectedOutlineCreativeMode, selectedOutlineStoryFocus, selectedOutlinePlotStage],
  );

  const outlineStoryAcceptanceCard = useMemo(
    () => buildStoryAcceptanceCard(selectedOutlineCreativeMode, selectedOutlineStoryFocus, {
      scene: 'outline',
      plotStage: selectedOutlinePlotStage,
    }),
    [selectedOutlineCreativeMode, selectedOutlineStoryFocus, selectedOutlinePlotStage],
  );

  const outlineStoryCharacterArcCard = useMemo(
    () => buildStoryCharacterArcCard(selectedOutlineCreativeMode, selectedOutlineStoryFocus, {
      scene: 'outline',
      plotStage: selectedOutlinePlotStage,
    }),
    [selectedOutlineCreativeMode, selectedOutlineStoryFocus, selectedOutlinePlotStage],
  );

  const outlineVolumePacingPlan = useMemo(
    () => buildVolumePacingPlan(selectedOutlineChapterCount, {
      preferredStage: selectedOutlinePlotStage,
    }),
    [selectedOutlineChapterCount, selectedOutlinePlotStage],
  );

  const applyOutlineCreationPreset = useCallback((presetId: CreationPresetId) => {
    const preset = getCreationPresetById(presetId);
    if (!preset) return;
    generateForm.setFieldsValue({
      creative_mode: preset.creativeMode,
      story_focus: preset.storyFocus,
    });
  }, [generateForm]);

  return (
        <Form


          form={generateForm}


          layout="vertical"


          style={{ marginTop: 16 }}


          initialValues={{


            mode: initialMode,


            chapter_count: 5,


            narrative_perspective: currentProject?.narrative_perspective || '????',


            creative_mode: projectDefaultCreativeMode,


            story_focus: projectDefaultStoryFocus,


            plot_stage: projectDefaultPlotStage || 'development',


            keep_existing: true,


            theme: currentProject?.theme || '',


            model: defaultModel,


          }}


        >


          {hasOutlines && (


            <Form.Item


              label="生成模式"


              name="mode"


              tooltip="自动判断：根据是否有大纲自动选择；全新生成：删除旧大纲重新生成；续写模式：基于已有大纲继续创作"


            >


              <Radio.Group buttonStyle="solid">


                <Radio.Button value="auto">自动判断</Radio.Button>


                <Radio.Button value="new">全新生成</Radio.Button>


                <Radio.Button value="continue">续写模式</Radio.Button>


              </Radio.Group>


            </Form.Item>


          )}


          <Form.Item


            noStyle


            shouldUpdate={(prevValues, currentValues) => prevValues.mode !== currentValues.mode}


          >


            {({ getFieldValue }) => {


              const mode = getFieldValue('mode');


              const isContinue = mode === 'continue' || (mode === 'auto' && hasOutlines);


              // 续写模式不显示主题输入，使用项目原有主题


              if (isContinue) {


                return null;


              }


              // 全新生成模式需要输入主题


              return (


                <Form.Item


                  label="故事主题"


                  name="theme"


                  rules={[{ required: true, message: '请输入故事主题' }]}


                >


                  <TextArea rows={3} placeholder="描述你的故事主题、核心设定和主要情节..." />


                </Form.Item>


              );


            }}


          </Form.Item>


          <Form.Item


            noStyle


            shouldUpdate={(prevValues, currentValues) => prevValues.mode !== currentValues.mode}


          >


            {({ getFieldValue }) => {


              const mode = getFieldValue('mode');


              const isContinue = mode === 'continue' || (mode === 'auto' && hasOutlines);


              return (


                <>


                  {isContinue && (


                    <>


                      <Form.Item


                        label="故事发展方向"


                        name="story_direction"


                        tooltip="告诉系统你希望故事接下来如何发展"


                      >


                        <TextArea


                          rows={3}


                          placeholder="例如：主角遇到新的挑战、引入新角色、揭示关键秘密等..."


                        />


                      </Form.Item>


                      <Form.Item


                        label="情节阶段"


                        name="plot_stage"


                        tooltip="帮助系统理解当前故事所处的阶段"


                      >


                        <Select>


                          <Select.Option value="development">发展阶段 - 继续展开情节</Select.Option>


                          <Select.Option value="climax">高潮阶段 - 矛盾激化</Select.Option>


                          <Select.Option value="ending">结局阶段 - 收束伏笔</Select.Option>


                        </Select>


                      </Form.Item>


                    </>


                  )}


                  <Form.Item


                    label={isContinue ? "续写章节数" : "章节数量"}


                    name="chapter_count"


                    rules={[{ required: true, message: '请输入章节数量' }]}


                  >


                    <Input


                      type="number"


                      min={1}


                      max={50}


                      placeholder={isContinue ? "建议5-10章" : "如：30"}


                    />


                  </Form.Item>


                  <Form.Item


                    label="叙事视角"


                    name="narrative_perspective"


                    rules={[{ required: true, message: '请选择叙事视角' }]}


                  >


                    <Select>


                      <Select.Option value="第一人称">第一人称</Select.Option>


                      <Select.Option value="第三人称">第三人称</Select.Option>


                      <Select.Option value="全知视角">全知视角</Select.Option>


                    </Select>


                  </Form.Item>


                  <Card size="small" title="创作预设" style={{ marginBottom: 12 }}>
                    <Space wrap>
                      {CREATION_PRESETS.map((preset) => (
                        <Button
                          key={preset.id}
                          type={activeOutlineCreationPreset?.id === preset.id ? 'primary' : 'default'}
                          onClick={() => applyOutlineCreationPreset(preset.id)}
                        >
                          {preset.label}
                        </Button>
                      ))}
                      <Button
                        onClick={() => generateForm.setFieldsValue({
                          creative_mode: projectDefaultCreativeMode,
                          story_focus: projectDefaultStoryFocus,
                        })}
                      >
                        清空预设
                      </Button>
                    </Space>

                    {activeOutlineCreationPreset && (
                      <div style={{ marginTop: 12, color: 'var(--color-text-secondary)' }}>
                        当前预设：<strong>{activeOutlineCreationPreset.label}</strong>，{activeOutlineCreationPreset.description}
                      </div>
                    )}
                  </Card>

                  {outlineCreationBlueprint && (
                    <Card size="small" title="结构蓝图预览" style={{ marginBottom: 12 }}>
                      <div style={{ color: 'var(--color-text-secondary)', marginBottom: 10 }}>
                        {outlineCreationBlueprint.summary}
                      </div>
                      <div style={{ fontWeight: 600, marginBottom: 8 }}>本轮大纲节拍</div>
                      <Space direction="vertical" size={6} style={{ display: 'flex' }}>
                        {outlineCreationBlueprint.beats.map((beat, index) => (
                          <div key={beat}>{index + 1}. {beat}</div>
                        ))}
                      </Space>
                      {outlineCreationBlueprint.risks.length > 0 && (
                        <div style={{ marginTop: 12, color: 'var(--color-warning)' }}>
                          结构提醒：{outlineCreationBlueprint.risks.join('；')}
                        </div>
                      )}
                    </Card>
                  )}

                  {outlineStoryObjectiveCard && (
                    <Card size="small" title="目标卡预览" style={{ marginBottom: 12 }}>
                      <div style={{ color: 'var(--color-text-secondary)', marginBottom: 10 }}>
                        {outlineStoryObjectiveCard.summary}
                      </div>
                      <Space direction="vertical" size={8} style={{ display: 'flex' }}>
                        {[
                          ['目标', outlineStoryObjectiveCard.objective],
                          ['阻力', outlineStoryObjectiveCard.obstacle],
                          ['转折', outlineStoryObjectiveCard.turn],
                          ['钩子', outlineStoryObjectiveCard.hook],
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

                  {outlineStoryResultCard && (
                    <Card size="small" title="结果卡预览" style={{ marginBottom: 12 }}>
                      <div style={{ color: 'var(--color-text-secondary)', marginBottom: 10 }}>
                        {outlineStoryResultCard.summary}
                      </div>
                      <Space direction="vertical" size={8} style={{ display: 'flex' }}>
                        {[
                          ['推进', outlineStoryResultCard.progress],
                          ['揭示', outlineStoryResultCard.reveal],
                          ['关系', outlineStoryResultCard.relationship],
                          ['余波', outlineStoryResultCard.fallout],
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

                  {outlineStoryExecutionChecklist && (
                    <Card size="small" title="执行清单预览" style={{ marginBottom: 12 }}>
                      <div style={{ color: 'var(--color-text-secondary)', marginBottom: 10 }}>
                        {outlineStoryExecutionChecklist.summary}
                      </div>
                      <Space direction="vertical" size={8} style={{ display: 'flex' }}>
                        {[
                          ['开场', outlineStoryExecutionChecklist.opening],
                          ['加压', outlineStoryExecutionChecklist.pressure],
                          ['转折', outlineStoryExecutionChecklist.pivot],
                          ['收束', outlineStoryExecutionChecklist.closing],
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

                  {outlineStoryRepetitionRiskCard && (
                    <Card size="small" title="重复风险预警" style={{ marginBottom: 12 }}>
                      <div style={{ color: 'var(--color-text-secondary)', marginBottom: 10 }}>
                        {outlineStoryRepetitionRiskCard.summary}
                      </div>
                      <Space direction="vertical" size={8} style={{ display: 'flex' }}>
                        {[
                          ['开场风险', outlineStoryRepetitionRiskCard.openingRisk],
                          ['加压风险', outlineStoryRepetitionRiskCard.pressureRisk],
                          ['转折风险', outlineStoryRepetitionRiskCard.pivotRisk],
                          ['收尾风险', outlineStoryRepetitionRiskCard.closingRisk],
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

                  {outlineStoryAcceptanceCard && (
                    <Card size="small" title="验收卡预览" style={{ marginBottom: 12 }}>
                      <div style={{ color: 'var(--color-text-secondary)', marginBottom: 10 }}>
                        {outlineStoryAcceptanceCard.summary}
                      </div>
                      <Space direction="vertical" size={8} style={{ display: 'flex' }}>
                        {[
                          ['任务命中', outlineStoryAcceptanceCard.missionCheck],
                          ['变化落地', outlineStoryAcceptanceCard.changeCheck],
                          ['新鲜度', outlineStoryAcceptanceCard.freshnessCheck],
                          ['收束质量', outlineStoryAcceptanceCard.closingCheck],
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

                  {outlineStoryCharacterArcCard && (
                    <Card size="small" title="角色弧光预览" style={{ marginBottom: 12 }}>
                      <div style={{ color: 'var(--color-text-secondary)', marginBottom: 10 }}>
                        {outlineStoryCharacterArcCard.summary}
                      </div>
                      <Space direction="vertical" size={8} style={{ display: 'flex' }}>
                        {[
                          ['外在线', outlineStoryCharacterArcCard.externalLine],
                          ['内在线', outlineStoryCharacterArcCard.internalLine],
                          ['关系线', outlineStoryCharacterArcCard.relationshipLine],
                          ['落点', outlineStoryCharacterArcCard.arcLanding],
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

                  {outlineVolumePacingPlan && (
                    <Card size="small" title="卷级节奏预览" style={{ marginBottom: 12 }}>
                      <div style={{ color: 'var(--color-text-secondary)', marginBottom: 10 }}>
                        {outlineVolumePacingPlan.summary}
                      </div>
                      <Space direction="vertical" size={8} style={{ display: 'flex' }}>
                        {outlineVolumePacingPlan.segments.map((segment) => (
                          <div key={`${segment.stage}-${segment.startChapter}`}>
                            <strong>第{segment.startChapter}-{segment.endChapter}章 · {segment.label}</strong>
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


                    name="creative_mode"


                    tooltip="可选：让本轮大纲更偏向钩子、情绪、悬念、关系或爽点回收"


                  >


                    <Select allowClear placeholder="默认均衡，不额外强调单一方向" optionLabelProp="label">


                      {CREATIVE_MODE_OPTIONS.map((option) => (


                        <Select.Option key={option.value} value={option.value} label={option.label}>


                          <div>{option.label}</div>


                          <div style={{ fontSize: 12, color: 'var(--color-text-tertiary)' }}>{option.description}</div>


                        </Select.Option>


                      ))}


                    </Select>


                  </Form.Item>


                  <Form.Item


                    label="结构侧重点"


                    name="story_focus"


                    tooltip="可选：指定本轮大纲更偏向主线推进、人物塑形、冲突升级等叙事任务"


                  >


                    <Select allowClear placeholder="默认均衡承担多种功能" optionLabelProp="label">


                      {STORY_FOCUS_OPTIONS.map((option) => (


                        <Select.Option key={option.value} value={option.value} label={option.label}>


                          <div>{option.label}</div>


                          <div style={{ fontSize: 12, color: 'var(--color-text-tertiary)' }}>{option.description}</div>


                        </Select.Option>


                      ))}


                    </Select>


                  </Form.Item>


                  <Form.Item label="其他要求" name="requirements">


                    <TextArea rows={2} placeholder="其他特殊要求（可选）" />


                  </Form.Item>


                </>


              );


            }}


          </Form.Item>


          {/* 自定义模型选择 - 移到外层，所有模式都显示 */}


          {loadedModels.length > 0 && (


            <Form.Item


              label="AI模型"


              name="model"


              tooltip="选择用于生成的AI模型，不选则使用系统默认模型"


            >


              <Select


                placeholder={defaultModel ? `默认: ${loadedModels.find(m => m.value === defaultModel)?.label || defaultModel}` : "使用默认模型"}


                allowClear


                showSearch


                optionFilterProp="label"


                options={loadedModels}


                onChange={(value) => {


                  console.log('用户在下拉框中选择了模型:', value);


                  // 手动同步到Form


                  generateForm.setFieldsValue({ model: value });


                  console.log('已同步到Form，当前Form值:', generateForm.getFieldsValue());


                }}


              />


              <div style={{ color: 'var(--color-text-tertiary)', fontSize: 12, marginTop: 4 }}>


                {defaultModel ? `当前默认模型: ${loadedModels.find(m => m.value === defaultModel)?.label || defaultModel}` : '未配置默认模型'}


              </div>


            </Form.Item>


          )}


        </Form>
  );
}
