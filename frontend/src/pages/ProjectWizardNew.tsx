import { useState, useEffect } from 'react';
import { useNavigate, useSearchParams } from 'react-router-dom';
import {
  Form, Input, InputNumber, Select, Button, Card,
  Row, Col, Typography, Space, message, Radio, theme, Switch, Alert
} from 'antd';
import {
  RocketOutlined, ArrowLeftOutlined, CheckCircleOutlined
} from '@ant-design/icons';
import { AIProjectGenerator, type GenerationConfig } from '../components/AIProjectGenerator';
import type { WizardBasicInfo } from '../types';
import {
  CREATIVE_MODE_OPTIONS,
  PLOT_STAGE_OPTIONS,
  QUALITY_PRESET_OPTIONS,
  STORY_FOCUS_OPTIONS,
} from '../utils/generationPreferenceOptions';

const { TextArea } = Input;
const { Title, Paragraph } = Typography;

export default function ProjectWizardNew() {
  const navigate = useNavigate();
  const [searchParams] = useSearchParams();
  const [form] = Form.useForm();
  const [isMobile, setIsMobile] = useState(window.innerWidth <= 768);
  const { token } = theme.useToken();

  // 状态管理
  const [currentStep, setCurrentStep] = useState<'form' | 'generating'>('form');
  const [generationConfig, setGenerationConfig] = useState<GenerationConfig | null>(null);
  const [resumeProjectId, setResumeProjectId] = useState<string | null>(null);

  useEffect(() => {
    const handleResize = () => {
      setIsMobile(window.innerWidth <= 768);
    };
    window.addEventListener('resize', handleResize);
    return () => window.removeEventListener('resize', handleResize);
  }, []);

  // 检查URL参数,如果有project_id则恢复生成
  useEffect(() => {
    const projectId = searchParams.get('project_id');
    if (projectId) {
      setResumeProjectId(projectId);
      handleResumeGeneration(projectId);
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [searchParams]);

  // 恢复未完成项目的生成
  const handleResumeGeneration = async (projectId: string) => {
    try {
      const response = await fetch(`/api/projects/${projectId}`, {
        credentials: 'include'
      });
      if (!response.ok) {
        throw new Error('获取项目信息失败');
      }
      const project = await response.json();

      const config: GenerationConfig = {
        title: project.title,
        description: project.description || '',
        theme: project.theme || '',
        genre: project.genre || '',
        narrative_perspective: project.narrative_perspective || '第三人称',
        target_words: project.target_words || 100000,
        chapter_count: 3,
        character_count: project.character_count || 5,
        default_creative_mode: project.default_creative_mode,
        default_story_focus: project.default_story_focus,
        default_plot_stage: project.default_plot_stage,
        default_story_creation_brief: project.default_story_creation_brief || '',
        default_quality_preset: project.default_quality_preset,
        default_quality_notes: project.default_quality_notes || '',
      };

      try {
        const raw = localStorage.getItem('wizard_generation_data');
        if (raw) {
          const saved = JSON.parse(raw);
          if (saved && typeof saved === 'object') {
            config.enable_web_research = saved.enable_web_research;
            config.web_research_query = saved.web_research_query;
            config.world_building_research_query = saved.world_building_research_query;
            config.careers_research_query = saved.careers_research_query;
            config.characters_research_query = saved.characters_research_query;
            config.outline_research_query = saved.outline_research_query;
          }
        }
      } catch {
        // ignore local restore parse failures
      }

      setGenerationConfig(config);
      setCurrentStep('generating');
    } catch (error) {
      console.error('恢复生成失败:', error);
      message.error('恢复生成失败,请重试');
      navigate('/');
    }
  };

  // 开始生成流程
  const handleAutoGenerate = async (values: WizardBasicInfo) => {
    const config: GenerationConfig = {
      title: values.title,
      description: values.description,
      theme: values.theme,
      genre: values.genre,
      narrative_perspective: values.narrative_perspective,
      target_words: values.target_words || 100000,
      chapter_count: 3, // 默认生成3章大纲
      character_count: values.character_count || 5,
      outline_mode: values.outline_mode || 'one-to-many', // 添加大纲模式
      default_creative_mode: values.default_creative_mode,
      default_story_focus: values.default_story_focus,
      default_plot_stage: values.default_plot_stage,
      default_story_creation_brief: values.default_story_creation_brief,
      default_quality_preset: values.default_quality_preset,
      default_quality_notes: values.default_quality_notes,
      enable_web_research: values.enable_web_research,
      web_research_query: values.web_research_query,
      world_building_research_query: values.world_building_research_query,
      careers_research_query: values.careers_research_query,
      characters_research_query: values.characters_research_query,
      outline_research_query: values.outline_research_query,
    };

    setGenerationConfig(config);
    setCurrentStep('generating');
  };

  // 生成完成回调
  const handleComplete = (projectId: string) => {
    console.log('项目创建完成:', projectId);
  };

  // 返回表单页面
  const handleBack = () => {
    setCurrentStep('form');
    setGenerationConfig(null);
  };

  // 渲染表单页面
  const renderForm = () => (
    <Card>
      <Title level={isMobile ? 4 : 3} style={{ marginBottom: 24 }}>
        创建新项目
      </Title>
      <Paragraph type="secondary" style={{ marginBottom: 32 }}>
        填写基本信息后，AI将自动生成世界观、角色和开局大纲。建议简介写清“目标→阻力→代价”，主题写成“价值冲突”。
      </Paragraph>

      <Form
        form={form}
        layout="vertical"
        onFinish={handleAutoGenerate}
        initialValues={{
          genre: ['玄幻'],
          chapter_count: 30,
          narrative_perspective: '第三人称',
          character_count: 5,
          target_words: 100000,
          outline_mode: 'one-to-one', // 默认为传统模式（1-1）
          default_plot_stage: 'development',
          enable_web_research: false,
        }}
      >
        <Form.Item
          label="书名"
          name="title"
          rules={[{ required: true, message: '请输入书名' }]}
        >
          <Input placeholder="例如：离婚当天，我继承了仇家的公司" size="large" />
        </Form.Item>

        <Form.Item
          label="小说简介"
          name="description"
          rules={[{ required: true, message: '请输入小说简介' }]}
        >
          <TextArea
            rows={3}
            placeholder="建议2-4句：主角要做什么、眼前卡在哪里、失败会失去什么"
            showCount
            maxLength={300}
          />
        </Form.Item>

        <Form.Item
          label="主题"
          name="theme"
          rules={[{ required: true, message: '请输入主题' }]}
        >
          <TextArea
            rows={4}
            placeholder="例如：真相 vs 体面、生存 vs 尊严（写出核心价值冲突）"
            showCount
            maxLength={500}
          />
        </Form.Item>

        <Form.Item
          label="类型"
          name="genre"
          rules={[{ required: true, message: '请选择小说类型' }]}
        >
          <Select
            mode="tags"
            placeholder="选择主赛道+气质标签（如：都市、规则怪谈、权谋）"
            size="large"
            tokenSeparators={[',']}
            maxTagCount={5}
          >
            <Select.Option value="玄幻">玄幻</Select.Option>
            <Select.Option value="都市">都市</Select.Option>
            <Select.Option value="历史">历史</Select.Option>
            <Select.Option value="科幻">科幻</Select.Option>
            <Select.Option value="武侠">武侠</Select.Option>
            <Select.Option value="仙侠">仙侠</Select.Option>
            <Select.Option value="奇幻">奇幻</Select.Option>
            <Select.Option value="悬疑">悬疑</Select.Option>
            <Select.Option value="言情">言情</Select.Option>
            <Select.Option value="修仙">修仙</Select.Option>
          </Select>
        </Form.Item>

        <Form.Item
          label="大纲章节模式"
          name="outline_mode"
          rules={[{ required: true, message: '请选择大纲章节模式' }]}
          tooltip="创建后不可更改，请根据创作习惯选择"
        >
          <Radio.Group size="large">
            <Row gutter={16}>
              <Col xs={24} sm={12}>
                <Card
                  hoverable
                  style={{
                    // borderColor: form.getFieldValue('outline_mode') === 'one-to-one' ? token.colorPrimary : token.colorBorder,
                    borderWidth: 2,
                    height: '100%',
                  }}
                  onClick={() => form.setFieldValue('outline_mode', 'one-to-one')}
                >
                  <Radio value="one-to-one" style={{ width: '100%' }}>
                    <Space direction="vertical" size={4} style={{ width: '100%' }}>
                      <div style={{ fontSize: 16, fontWeight: 'bold' }}>
                        <CheckCircleOutlined style={{ marginRight: 8, color: token.colorSuccess }} />
                        传统模式 (1→1)
                      </div>
                      <div style={{ fontSize: 12, color: token.colorTextSecondary }}>
                        一个大纲对应一个章节，简单直接
                      </div>
                      <div style={{ fontSize: 11, color: token.colorTextTertiary }}>
                        💡 适合：简单剧情、快速创作、短篇小说
                      </div>
                    </Space>
                  </Radio>
                </Card>
              </Col>

              <Col xs={24} sm={12}>
                <Card
                  hoverable
                  style={{
                    // borderColor: form.getFieldValue('outline_mode') === 'one-to-many' ? token.colorPrimary : token.colorBorder,
                    borderWidth: 2,
                    height: '100%',
                  }}
                  onClick={() => form.setFieldValue('outline_mode', 'one-to-many')}
                >
                  <Radio value="one-to-many" style={{ width: '100%' }}>
                    <Space direction="vertical" size={4} style={{ width: '100%' }}>
                      <div style={{ fontSize: 16, fontWeight: 'bold' }}>
                        <CheckCircleOutlined style={{ marginRight: 8, color: token.colorSuccess }} />
                        细化模式 (1→N) 推荐
                      </div>
                      <div style={{ fontSize: 12, color: token.colorTextSecondary }}>
                        一个大纲可展开为多个章节，灵活控制
                      </div>
                      <div style={{ fontSize: 11, color: token.colorTextTertiary }}>
                        💡 适合：复杂剧情、长篇创作、需要细化控制
                      </div>
                    </Space>
                  </Radio>
                </Card>
              </Col>
            </Row>
          </Radio.Group>
        </Form.Item>

        <Row gutter={16}>
          <Col xs={24} sm={12}>
            <Form.Item
              label="叙事视角"
              name="narrative_perspective"
              rules={[{ required: true, message: '请选择叙事视角' }]}
            >
              <Select size="large" placeholder="选择小说的叙事视角">
                <Select.Option value="第一人称">第一人称</Select.Option>
                <Select.Option value="第三人称">第三人称</Select.Option>
                <Select.Option value="全知视角">全知视角</Select.Option>
              </Select>
            </Form.Item>
          </Col>
          <Col xs={24} sm={12}>
            <Form.Item
              label="角色数量"
              name="character_count"
              rules={[{ required: true, message: '请输入角色数量' }]}
            >
              <InputNumber
                min={3}
                max={20}
                style={{ width: '100%' }}
                size="large"
                addonAfter="个"
                placeholder="AI生成的角色数量"
              />
            </Form.Item>
          </Col>
        </Row>

        <Form.Item
          label="目标字数"
          name="target_words"
          rules={[{ required: true, message: '请输入目标字数' }]}
        >
          <InputNumber
            min={10000}
            style={{ width: '100%' }}
            size="large"
            addonAfter="字"
            placeholder="整部小说的目标字数"
          />
        </Form.Item>

        <Card size="small" title="默认创作偏好" style={{ marginBottom: 24 }}>
          <Alert
            type="info"
            showIcon
            style={{ marginBottom: 16 }}
            message="这些偏好会写入项目默认值，并自动作用于首次大纲与后续章节生成；创建后仍可在世界设定中继续调整。"
          />

          <Row gutter={16}>
            <Col xs={24} md={12}>
              <Form.Item label="默认创作模式" name="default_creative_mode" tooltip="控制整体更偏钩子、情绪、悬念、关系或爽点回收">
                <Select allowClear placeholder="不额外偏置，保持均衡" optionLabelProp="label">
                  {CREATIVE_MODE_OPTIONS.map((option) => (
                    <Select.Option key={option.value} value={option.value} label={option.label}>
                      <div>{option.label}</div>
                      <div style={{ fontSize: 12, color: token.colorTextTertiary }}>{option.description}</div>
                    </Select.Option>
                  ))}
                </Select>
              </Form.Item>
            </Col>
            <Col xs={24} md={12}>
              <Form.Item label="默认结构侧重点" name="default_story_focus" tooltip="控制整体更偏主线推进、人物塑形、冲突升级等叙事任务">
                <Select allowClear placeholder="不额外偏置，保持均衡" optionLabelProp="label">
                  {STORY_FOCUS_OPTIONS.map((option) => (
                    <Select.Option key={option.value} value={option.value} label={option.label}>
                      <div>{option.label}</div>
                      <div style={{ fontSize: 12, color: token.colorTextTertiary }}>{option.description}</div>
                    </Select.Option>
                  ))}
                </Select>
              </Form.Item>
            </Col>
          </Row>

          <Row gutter={16}>
            <Col xs={24} md={12}>
              <Form.Item label="默认情节阶段" name="default_plot_stage" tooltip="帮助系统判断当前项目默认处于发展、高潮还是收束阶段">
                <Select allowClear placeholder="留空时按具体场景判断" optionLabelProp="label">
                  {PLOT_STAGE_OPTIONS.map((option) => (
                    <Select.Option key={option.value} value={option.value} label={option.label}>
                      <div>{option.label}</div>
                      <div style={{ fontSize: 12, color: token.colorTextTertiary }}>{option.description}</div>
                    </Select.Option>
                  ))}
                </Select>
              </Form.Item>
            </Col>
            <Col xs={24} md={12}>
              <Form.Item label="默认质量预设" name="default_quality_preset" tooltip="为大纲与章节生成施加统一的质量偏好">
                <Select allowClear placeholder="默认不额外施压" optionLabelProp="label">
                  {QUALITY_PRESET_OPTIONS.map((option) => (
                    <Select.Option key={option.value} value={option.value} label={option.label}>
                      <div>{option.label}</div>
                      <div style={{ fontSize: 12, color: token.colorTextTertiary }}>{option.description}</div>
                    </Select.Option>
                  ))}
                </Select>
              </Form.Item>
            </Col>
          </Row>

          <Form.Item
            label="默认创作总控"
            name="default_story_creation_brief"
            tooltip="用几句话定义这个项目长期遵循的创作重心、推进节奏或核心约束"
          >
            <TextArea
              rows={3}
              placeholder="例如：始终围绕主角的目标、阻力与代价推进，优先保证钩子和回报闭环。"
              showCount
              maxLength={600}
            />
          </Form.Item>

          <Form.Item
            label="默认额外质量要求"
            name="default_quality_notes"
            tooltip="补充你长期想保留或压制的写作倾向，例如减少说明句、加强动作反馈等"
          >
            <TextArea
              rows={3}
              placeholder="例如：减少解释性旁白，优先用动作和对话推进信息；章尾必须保留牵引。"
              showCount
              maxLength={600}
            />
          </Form.Item>
        </Card>

        <Card size="small" title="生成前网络检索" style={{ marginBottom: 24 }}>
          <Alert
            type="info"
            showIcon
            style={{ marginBottom: 16 }}
            message="可选。开启后会在世界观、职业体系、角色和大纲生成前先做网络检索，并把资料归档到项目记忆。"
          />

          <Form.Item
            label="启用网络检索"
            name="enable_web_research"
            valuePropName="checked"
          >
            <Switch checkedChildren="开启" unCheckedChildren="关闭" />
          </Form.Item>

          <Form.Item
            label="自定义检索 Query"
            name="web_research_query"
            tooltip="可选。留空时系统会按书名、简介、主题、类型自动生成检索词。"
          >
            <TextArea
              rows={3}
              placeholder="例如：现代都市权谋、资本运作、公关舆论、年轻高管说话风格、企业组织架构"
              showCount
              maxLength={400}
            />
          </Form.Item>

          <Row gutter={16}>
            <Col xs={24} md={12}>
              <Form.Item label="世界观检索 Query" name="world_building_research_query">
                <TextArea rows={2} placeholder="可选，单独覆盖世界观生成的检索词" maxLength={300} showCount />
              </Form.Item>
            </Col>
            <Col xs={24} md={12}>
              <Form.Item label="职业体系检索 Query" name="careers_research_query">
                <TextArea rows={2} placeholder="可选，单独覆盖职业体系生成的检索词" maxLength={300} showCount />
              </Form.Item>
            </Col>
          </Row>

          <Row gutter={16}>
            <Col xs={24} md={12}>
              <Form.Item label="角色检索 Query" name="characters_research_query">
                <TextArea rows={2} placeholder="可选，单独覆盖角色生成的检索词" maxLength={300} showCount />
              </Form.Item>
            </Col>
            <Col xs={24} md={12}>
              <Form.Item label="大纲检索 Query" name="outline_research_query">
                <TextArea rows={2} placeholder="可选，单独覆盖大纲生成的检索词" maxLength={300} showCount />
              </Form.Item>
            </Col>
          </Row>
        </Card>

        <Form.Item>
          <Space direction="vertical" style={{ width: '100%' }} size={12}>
            <Button
              type="primary"
              htmlType="submit"
              size="large"
              block
              icon={<RocketOutlined />}
            >
              开始创建项目
            </Button>
            <Button
              size="large"
              block
              onClick={() => navigate('/')}
            >
              返回首页
            </Button>
          </Space>
        </Form.Item>
      </Form>
    </Card>
  );

  return (
    <div style={{
      minHeight: '100dvh',
      background: token.colorBgBase,
    }}>
      {/* 顶部标题栏 - 固定不滚动 */}
      <div style={{
        position: 'sticky',
        top: 0,
        zIndex: 100,
        background: token.colorPrimary,
        boxShadow: `0 6px 20px color-mix(in srgb, ${token.colorPrimary} 30%, transparent)`,
      }}>
        <div style={{
          maxWidth: 1200,
          margin: '0 auto',
          display: 'flex',
          alignItems: 'center',
          justifyContent: 'space-between',
          padding: isMobile ? '12px 16px' : '16px 24px',
        }}>
          <Button
            icon={<ArrowLeftOutlined />}
            onClick={() => navigate('/')}
            size={isMobile ? 'middle' : 'large'}
            disabled={currentStep === 'generating'}
            style={{
              background: `color-mix(in srgb, ${token.colorWhite} 20%, transparent)`,
              borderColor: `color-mix(in srgb, ${token.colorWhite} 30%, transparent)`,
              color: token.colorWhite,
            }}
          >
            {isMobile ? '返回' : '返回首页'}
          </Button>

          <Title level={isMobile ? 4 : 2} style={{
            margin: 0,
            color: token.colorWhite,
            textShadow: '0 2px 4px color-mix(in srgb, var(--ant-color-black) 18%, transparent)',
          }}>
            <RocketOutlined style={{ marginRight: 8 }} />
            项目创建向导
          </Title>

          <div style={{ width: isMobile ? 60 : 120 }}></div>
        </div>
      </div>

      {/* 内容区域 */}
      <div style={{
        maxWidth: 800,
        margin: '0 auto',
        padding: isMobile ? '16px 12px' : '24px 24px',
      }}>
        {currentStep === 'form' && renderForm()}
        {currentStep === 'generating' && generationConfig && (
          <AIProjectGenerator
            config={generationConfig}
            storagePrefix="wizard"
            onComplete={handleComplete}
            onBack={handleBack}
            isMobile={isMobile}
            resumeProjectId={resumeProjectId || undefined}
          />
        )}
      </div>
    </div>
  );
}
