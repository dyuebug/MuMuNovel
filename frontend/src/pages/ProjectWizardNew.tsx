import { useState, useEffect, useRef, useCallback } from 'react';
import { useBusyNavigationGuard } from '../hooks/useBusyNavigationGuard';
import { useNavigate, useSearchParams } from 'react-router-dom';
import {
  Form, Input, InputNumber, Select, Button, Card,
  Row, Col, Typography, Space, message, Radio, theme, Switch, Alert, Tag
} from 'antd';
import {
  RocketOutlined, ArrowLeftOutlined, CheckCircleOutlined
} from '@ant-design/icons';
import { AIProjectGenerator, type GenerationConfig } from '../components/AIProjectGenerator';
import {
  GenerationExecutionSettingsPanel,
  useGenerationExecutionSettings,
} from '../components/GenerationExecutionSettings';
import type { WizardBasicInfo } from '../types';
import { isProjectWizardCompleted } from '../utils/projectWizardState';
import { syncProjectToStoreById } from '../store/hooks';
import { invalidateAllProjectCollectionFreshness } from '../store/projectCollectionRefresh';
import { invalidateProjectCareers } from '../services/projectCareers';
import { isRequestCancelledError } from '../services/core/httpClient';
import {
  CREATIVE_MODE_OPTIONS,
  PLOT_STAGE_OPTIONS,
  QUALITY_PRESET_OPTIONS,
  STORY_FOCUS_OPTIONS,
} from '../utils/generationPreferenceOptions';
import { designDisplayFont } from '../theme/themeConfig';

const { TextArea } = Input;
const { Title, Paragraph } = Typography;

export default function ProjectWizardNew() {
  const navigate = useNavigate();
  const [searchParams] = useSearchParams();
  const [form] = Form.useForm();
  const watchedEnableMcp = Boolean(Form.useWatch('enable_mcp', form));
  const watchedModel = Form.useWatch('model', form);
  const watchedOutlineMode = Form.useWatch('outline_mode', form);
  const [isMobile, setIsMobile] = useState(window.innerWidth <= 768);
  const { token } = theme.useToken();
  const alphaColor = (color: string, alpha: number) => `color-mix(in srgb, ${color} ${(alpha * 100).toFixed(0)}%, transparent)`;

  // 状态管理
  const [currentStep, setCurrentStep] = useState<'form' | 'generating'>('form');
  const [generationConfig, setGenerationConfig] = useState<GenerationConfig | null>(null);
  const [resumeProjectId, setResumeProjectId] = useState<string | null>(null);
  const mountedRef = useRef(true);
  const runtimeDefaultsRequestIdRef = useRef(0);
  const resumeRequestIdRef = useRef(0);

  useEffect(() => {
    return () => {
      mountedRef.current = false;
      runtimeDefaultsRequestIdRef.current += 1;
      resumeRequestIdRef.current += 1;
    };
  }, []);

  const beginRuntimeDefaultsRequest = useCallback(() => {
    runtimeDefaultsRequestIdRef.current += 1;
    return runtimeDefaultsRequestIdRef.current;
  }, []);

  const beginResumeRequest = useCallback(() => {
    resumeRequestIdRef.current += 1;
    return resumeRequestIdRef.current;
  }, []);

  const isRuntimeDefaultsRequestActive = useCallback((requestId: number) => {
    return mountedRef.current && runtimeDefaultsRequestIdRef.current === requestId;
  }, []);

  const isResumeRequestActive = useCallback((requestId: number) => {
    return mountedRef.current && resumeRequestIdRef.current === requestId;
  }, []);

  const clearWizardResumeStorage = () => {
    localStorage.removeItem('wizard_project_id');
    localStorage.removeItem('wizard_generation_data');
    localStorage.removeItem('wizard_current_step');
  };

  const {
    availableModels,
    fetchingModels,
    runtimeProvider,
    currentSettingsModel,
    loadDefaults,
  } = useGenerationExecutionSettings();

  const {
    setBusy: setIsGenerationBusy,
    releaseBusy: releaseGenerationBusy,
    shouldDisableNavigation,
  } = useBusyNavigationGuard();
  useEffect(() => {
    const handleResize = () => {
      setIsMobile(window.innerWidth <= 768);
    };
    window.addEventListener('resize', handleResize);
    return () => window.removeEventListener('resize', handleResize);
  }, []);

  useEffect(() => {
    let cancelled = false;
    const requestId = beginRuntimeDefaultsRequest();

    const loadRuntimeDefaults = async () => {
      try {
        const { model } = await loadDefaults();
        if (cancelled || !isRuntimeDefaultsRequestActive(requestId)) {
          return;
        }

        const currentEnableMcp = form.getFieldValue('enable_mcp');
        const currentModel = form.getFieldValue('model');
        form.setFieldsValue({
          enable_mcp: typeof currentEnableMcp === 'boolean' ? currentEnableMcp : true,
          model: currentModel || model,
        });
      } catch (error) {
        if (!cancelled && isRuntimeDefaultsRequestActive(requestId)) {
          console.warn('加载向导执行设置失败:', error);
        }
      }
    };

    void loadRuntimeDefaults();

    return () => {
      cancelled = true;
    };
  }, [beginRuntimeDefaultsRequest, form, isRuntimeDefaultsRequestActive, loadDefaults]);
  // 检查URL参数,如果有project_id则恢复生成
  useEffect(() => {
    const projectId = searchParams.get('project_id');
    if (!projectId) {
      return;
    }

    const abortController = new AbortController();
    setResumeProjectId(projectId);
    void handleResumeGeneration(projectId, abortController.signal);

    return () => {
      abortController.abort();
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [searchParams]);

  // Resume unfinished wizard generation
  const handleResumeGeneration = async (projectId: string, signal?: AbortSignal) => {
    const requestId = beginResumeRequest();
    try {
      const response = await fetch(`/api/projects/${projectId}`, {
        credentials: 'include',
        signal,
      });
      if (!isResumeRequestActive(requestId)) {
        return;
      }
      if (!response.ok) {
        throw new Error('获取项目信息失败');
      }
      const project = await response.json();
      if (!isResumeRequestActive(requestId)) {
        return;
      }

      if (isProjectWizardCompleted(project)) {
        clearWizardResumeStorage();
        setResumeProjectId(null);
        setGenerationConfig(null);
        setCurrentStep('form');
        navigate(`/project/${projectId}`, { replace: true });
        return;
      }

      const config: GenerationConfig = {
        title: project.title,
        description: project.description || '',
        theme: project.theme || '',
        genre: project.genre || '',
        narrative_perspective: project.narrative_perspective || '第三人称',
        target_words: project.target_words || 100000,
        chapter_count: project.chapter_count || 3,
        character_count: project.character_count || 5,
        outline_mode: project.outline_mode || 'one-to-many',
        default_creative_mode: project.default_creative_mode,
        default_story_focus: project.default_story_focus,
        default_plot_stage: project.default_plot_stage,
        default_story_creation_brief: project.default_story_creation_brief || '',
        default_quality_preset: project.default_quality_preset,
        default_quality_notes: project.default_quality_notes || '',
        provider: runtimeProvider,
        model: currentSettingsModel,
        enable_mcp: true,
      };

      try {
        const savedProjectId = localStorage.getItem('wizard_project_id');
        const raw = localStorage.getItem('wizard_generation_data');
        if (raw && savedProjectId === projectId) {
          const saved = JSON.parse(raw) as Partial<GenerationConfig> | null;
          if (saved && typeof saved === 'object') {
            config.chapter_count = saved.chapter_count || config.chapter_count;
            config.outline_mode = saved.outline_mode || config.outline_mode;
            config.enable_web_research = saved.enable_web_research;
            config.web_research_query = saved.web_research_query;
            config.provider = saved.provider || config.provider;
            config.model = saved.model || config.model;
            config.enable_mcp = typeof saved.enable_mcp === 'boolean' ? saved.enable_mcp : config.enable_mcp;
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
      if (isRequestCancelledError(error) || !isResumeRequestActive(requestId)) {
        return;
      }
      console.error('恢复生成失败:', error);
      message.error('恢复生成失败，请重试');
      navigate('/');
    }
  };

  // Start generation flow
  const handleAutoGenerate = async (values: WizardBasicInfo) => {
    const config: GenerationConfig = {
      title: values.title,
      description: values.description,
      theme: values.theme,
      genre: values.genre,
      narrative_perspective: values.narrative_perspective,
      target_words: values.target_words || 100000,
      chapter_count: values.chapter_count || 3,
      character_count: values.character_count || 5,
      outline_mode: values.outline_mode || 'one-to-many',
      default_creative_mode: values.default_creative_mode,
      default_story_focus: values.default_story_focus,
      default_plot_stage: values.default_plot_stage,
      default_story_creation_brief: values.default_story_creation_brief,
      default_quality_preset: values.default_quality_preset,
      default_quality_notes: values.default_quality_notes,
      provider: runtimeProvider,
      model: values.model,
      enable_mcp: values.enable_mcp,
      enable_web_research: values.enable_web_research,
      web_research_query: values.web_research_query,
      world_building_research_query: values.world_building_research_query,
      careers_research_query: values.careers_research_query,
      characters_research_query: values.characters_research_query,
      outline_research_query: values.outline_research_query,
    };

    setResumeProjectId(null);
    setGenerationConfig(config);
    setCurrentStep('generating');
  };

  const syncCompletedProject = async (projectId: string) => {
    try {
      invalidateAllProjectCollectionFreshness(projectId);
      invalidateProjectCareers(projectId);
      await syncProjectToStoreById(projectId);
    } catch (error) {
      console.error('同步完成项目到 store 失败:', error);
    }
  };

  // Completion callback
  const handleComplete = async (projectId: string) => {
    console.log('项目创建完成:', projectId);
    clearWizardResumeStorage();
    setResumeProjectId(null);
    await syncCompletedProject(projectId);
    releaseGenerationBusy();
  };

  // Back to form page
  const handleBack = () => {
    setCurrentStep('form');
    setGenerationConfig(null);
    setResumeProjectId(null);
    clearWizardResumeStorage();
    releaseGenerationBusy();
    navigate('/wizard', { replace: true });
  };
  // 渲染表单页面
  const renderForm = () => (
    <Card
      variant="borderless"
      style={{
        borderRadius: 22,
        border: `1px solid ${alphaColor(token.colorBorderSecondary, 0.9)}`,
        background: `linear-gradient(180deg, ${alphaColor(token.colorBgContainer, 0.98)} 0%, ${alphaColor(token.colorFillQuaternary, 0.44)} 100%)`,
        boxShadow: `0 18px 40px ${alphaColor(token.colorTextBase, 0.05)}`,
      }}
      styles={{ body: { padding: isMobile ? 16 : 18 } }}
    >
      <Typography.Text
        style={{
          display: 'block',
          marginBottom: 6,
          fontSize: 11,
          letterSpacing: '0.08em',
          textTransform: 'uppercase',
          color: token.colorTextTertiary,
        }}
      >
        Project Brief
      </Typography.Text>
      <Title level={isMobile ? 4 : 3} style={{ marginBottom: 12, fontFamily: designDisplayFont, letterSpacing: '-0.02em' }}>
        创建新项目
      </Title>
      <Paragraph type="secondary" style={{ marginBottom: 18, lineHeight: 1.8 }}>
        填写基本信息后，AI将自动生成世界观、角色和开局大纲。建议简介写清“目标→阻力→代价”，主题写成“价值冲突”。
      </Paragraph>
      <Space wrap size={[8, 8]} style={{ marginBottom: 24 }}>
        <Tag color="blue" style={{ borderRadius: 999, paddingInline: 12 }}>
          先定定位
        </Tag>
        <Tag color="purple" style={{ borderRadius: 999, paddingInline: 12 }}>
          再配默认偏好
        </Tag>
        <Tag color="gold" style={{ borderRadius: 999, paddingInline: 12 }}>
          最后确认执行策略
        </Tag>
      </Space>

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
          enable_mcp: true,
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
          <Typography.Text type="secondary" style={{ display: 'block', marginBottom: 12, lineHeight: 1.7 }}>
            这是项目创建后最重要的结构决策之一。你可以把它理解成“每一条大纲与章节之间的映射粒度”。
          </Typography.Text>
          <Radio.Group size="large">
            <Row gutter={16}>
              <Col xs={24} sm={12}>
                <Card
                  hoverable
                  style={{
                    borderColor: watchedOutlineMode === 'one-to-one' ? alphaColor(token.colorPrimary, 0.36) : alphaColor(token.colorBorderSecondary, 0.88),
                    borderWidth: 2,
                    height: '100%',
                    borderRadius: 18,
                    background: watchedOutlineMode === 'one-to-one'
                      ? `linear-gradient(180deg, ${alphaColor(token.colorPrimaryBg, 0.92)} 0%, ${alphaColor(token.colorBgContainer, 0.98)} 100%)`
                      : `linear-gradient(180deg, ${alphaColor(token.colorBgElevated, 0.98)} 0%, ${alphaColor(token.colorFillQuaternary, 0.42)} 100%)`,
                    boxShadow: watchedOutlineMode === 'one-to-one'
                      ? `0 18px 36px ${alphaColor(token.colorPrimary, 0.12)}`
                      : 'none',
                  }}
                  styles={{ body: { padding: 16 } }}
                  onClick={() => form.setFieldValue('outline_mode', 'one-to-one')}
                >
                  <Radio value="one-to-one" style={{ width: '100%' }}>
                    <Space direction="vertical" size={4} style={{ width: '100%' }}>
                      <Tag color={watchedOutlineMode === 'one-to-one' ? 'blue' : 'default'} style={{ width: 'fit-content', borderRadius: 999 }}>
                        简洁直推
                      </Tag>
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
                    borderColor: watchedOutlineMode === 'one-to-many' ? alphaColor(token.colorPrimary, 0.36) : alphaColor(token.colorBorderSecondary, 0.88),
                    borderWidth: 2,
                    height: '100%',
                    borderRadius: 18,
                    background: watchedOutlineMode === 'one-to-many'
                      ? `linear-gradient(180deg, ${alphaColor(token.colorPrimaryBg, 0.92)} 0%, ${alphaColor(token.colorBgContainer, 0.98)} 100%)`
                      : `linear-gradient(180deg, ${alphaColor(token.colorBgElevated, 0.98)} 0%, ${alphaColor(token.colorFillQuaternary, 0.42)} 100%)`,
                    boxShadow: watchedOutlineMode === 'one-to-many'
                      ? `0 18px 36px ${alphaColor(token.colorPrimary, 0.12)}`
                      : 'none',
                  }}
                  styles={{ body: { padding: 16 } }}
                  onClick={() => form.setFieldValue('outline_mode', 'one-to-many')}
                >
                  <Radio value="one-to-many" style={{ width: '100%' }}>
                    <Space direction="vertical" size={4} style={{ width: '100%' }}>
                      <Tag color={watchedOutlineMode === 'one-to-many' ? 'gold' : 'default'} style={{ width: 'fit-content', borderRadius: 999 }}>
                        长篇友好
                      </Tag>
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
          <Typography.Text type="secondary" style={{ display: 'block', marginTop: 12, lineHeight: 1.7 }}>
            当前选择：{watchedOutlineMode === 'one-to-many' ? '细化模式，适合复杂长篇与多章节展开。' : '传统模式，适合节奏直接、结构简单的项目。'}
          </Typography.Text>
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

        <Card
          size="small"
          style={{
            marginBottom: 24,
            borderRadius: 20,
            border: `1px solid ${alphaColor(token.colorBorderSecondary, 0.85)}`,
            background: `linear-gradient(180deg, ${alphaColor(token.colorBgElevated, 0.98)} 0%, ${alphaColor(token.colorFillAlter, 0.52)} 100%)`,
          }}
          styles={{ body: { padding: 16 } }}
        >
          <Typography.Text style={{ display: 'block', marginBottom: 6, fontSize: 11, letterSpacing: '0.08em', textTransform: 'uppercase', color: token.colorTextTertiary }}>
            Default Creative Profile
          </Typography.Text>
          <Typography.Text strong style={{ display: 'block', marginBottom: 8, fontSize: 16 }}>
            默认创作偏好
          </Typography.Text>
          <Typography.Text type="secondary" style={{ display: 'block', marginBottom: 14, lineHeight: 1.7 }}>
            这些偏好会成为项目的长期默认值，影响首次大纲和后续章节生成，适合在启动前先定好整体气质。
          </Typography.Text>
          <Alert
            type="info"
            showIcon
            style={{
              marginBottom: 16,
              borderRadius: 14,
              border: `1px solid ${alphaColor(token.colorInfo, 0.12)}`,
              background: `linear-gradient(135deg, ${alphaColor(token.colorInfoBg, 0.88)} 0%, ${alphaColor(token.colorBgContainer, 0.98)} 100%)`,
            }}
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

        <Card
          size="small"
          style={{
            marginBottom: 24,
            borderRadius: 20,
            border: `1px solid ${alphaColor(token.colorBorderSecondary, 0.85)}`,
            background: `linear-gradient(180deg, ${alphaColor(token.colorBgElevated, 0.98)} 0%, ${alphaColor(token.colorFillAlter, 0.52)} 100%)`,
          }}
          styles={{ body: { padding: 16 } }}
        >
          <Typography.Text style={{ display: 'block', marginBottom: 6, fontSize: 11, letterSpacing: '0.08em', textTransform: 'uppercase', color: token.colorTextTertiary }}>
            Research Before Generate
          </Typography.Text>
          <Typography.Text strong style={{ display: 'block', marginBottom: 8, fontSize: 16 }}>
            生成前网络检索
          </Typography.Text>
          <Typography.Text type="secondary" style={{ display: 'block', marginBottom: 14, lineHeight: 1.7 }}>
            如果你希望模型先借外部资料补齐行业细节、世界设定或说话风格，可以在这里为不同环节设定检索入口。
          </Typography.Text>
          <Alert
            type="info"
            showIcon
            style={{
              marginBottom: 16,
              borderRadius: 14,
              border: `1px solid ${alphaColor(token.colorInfo, 0.12)}`,
              background: `linear-gradient(135deg, ${alphaColor(token.colorInfoBg, 0.88)} 0%, ${alphaColor(token.colorBgContainer, 0.98)} 100%)`,
            }}
            message="可选。开启后会在世界观、职业体系、角色和大纲生成前先做联网检索，并把资料归档到项目记忆。"
          />

          <Form.Item
            label="联网检索"
            name="enable_web_research"
            valuePropName="checked"
          >
            <Switch checkedChildren="开启" unCheckedChildren="关闭" />
          </Form.Item>

          <Form.Item
            label="联网检索查询词"
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
              <Form.Item label="世界观检索查询词" name="world_building_research_query">
                <TextArea rows={2} placeholder="可选，单独覆盖世界观生成的检索词" maxLength={300} showCount />
              </Form.Item>
            </Col>
            <Col xs={24} md={12}>
              <Form.Item label="职业体系检索查询词" name="careers_research_query">
                <TextArea rows={2} placeholder="可选，单独覆盖职业体系生成的检索词" maxLength={300} showCount />
              </Form.Item>
            </Col>
          </Row>

          <Row gutter={16}>
            <Col xs={24} md={12}>
              <Form.Item label="角色检索查询词" name="characters_research_query">
                <TextArea rows={2} placeholder="可选，单独覆盖角色生成的检索词" maxLength={300} showCount />
              </Form.Item>
            </Col>
            <Col xs={24} md={12}>
              <Form.Item label="大纲检索查询词" name="outline_research_query">
                <TextArea rows={2} placeholder="可选，单独覆盖大纲生成的检索词" maxLength={300} showCount />
              </Form.Item>
            </Col>
          </Row>
        </Card>


        <GenerationExecutionSettingsPanel
          enableMcp={watchedEnableMcp}
          onEnableMcpChange={(value) => form.setFieldValue('enable_mcp', value)}
          model={watchedModel}
          onModelChange={(value) => form.setFieldValue('model', value)}
          fetchingModels={fetchingModels}
          availableModels={availableModels}
          runtimeProvider={runtimeProvider}
          currentSettingsModel={currentSettingsModel}
        />

        <Form.Item name="enable_mcp" hidden>
          <Input type="hidden" />
        </Form.Item>
        <Form.Item name="model" hidden>
          <Input type="hidden" />
        </Form.Item>
        <Form.Item>
          <div
            style={{
              padding: isMobile ? '14px 14px' : '16px 18px',
              borderRadius: 20,
              border: `1px solid ${alphaColor(token.colorBorderSecondary, 0.88)}`,
              background: `linear-gradient(180deg, ${alphaColor(token.colorBgContainer, 0.98)} 0%, ${alphaColor(token.colorFillQuaternary, 0.52)} 100%)`,
            }}
          >
            <Typography.Text style={{ display: 'block', marginBottom: 6, fontSize: 11, letterSpacing: '0.08em', textTransform: 'uppercase', color: token.colorTextTertiary }}>
              Launch Confirmation
            </Typography.Text>
            <Typography.Text strong style={{ display: 'block', marginBottom: 8, fontSize: 16 }}>
              开始创建项目
            </Typography.Text>
            <Typography.Text type="secondary" style={{ display: 'block', marginBottom: 14, lineHeight: 1.7 }}>
              这里是发起生成前的最后确认区。确认结构模式、执行设置和联网策略后，就可以开始让系统搭建项目骨架。
            </Typography.Text>
            <Space wrap size={[8, 8]} style={{ marginBottom: 14 }}>
              <Tag color="blue" style={{ borderRadius: 999 }}>模式：{watchedOutlineMode === 'one-to-many' ? '细化模式' : '传统模式'}</Tag>
              <Tag color={watchedEnableMcp ? 'green' : 'default'} style={{ borderRadius: 999 }}>MCP：{watchedEnableMcp ? '开启' : '关闭'}</Tag>
              <Tag color="purple" style={{ borderRadius: 999 }}>模型：{watchedModel || '系统默认'}</Tag>
            </Space>
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
          </div>
        </Form.Item>
      </Form>
    </Card>
  );

  return (
    <div style={{
      minHeight: '100dvh',
      background: `linear-gradient(180deg, ${token.colorBgLayout} 0%, ${token.colorFillSecondary} 100%)`,
      padding: isMobile ? '16px 12px 32px' : '24px',
    }}>
      <div style={{ maxWidth: 1120, margin: '0 auto' }}>
        <div style={{
          position: 'sticky',
          top: 0,
          zIndex: 100,
          marginBottom: isMobile ? 16 : 20,
        }}>
          <Card
            variant="borderless"
            style={{
              background: `linear-gradient(135deg,
                color-mix(in srgb, ${token.colorPrimary} 78%, #6f4537 22%) 0%,
                color-mix(in srgb, ${token.colorInfo} 28%, #162129 72%) 100%)`,
              borderRadius: 28,
              border: `1px solid color-mix(in srgb, ${token.colorBgContainer} 12%, transparent)`,
              boxShadow: `0 26px 52px color-mix(in srgb, ${token.colorText} 20%, transparent)`,
              overflow: 'hidden',
              position: 'relative',
            }}
            styles={{ body: { padding: isMobile ? 20 : 24 } }}
          >
            <div style={{ position: 'absolute', top: -56, right: -30, width: 176, height: 176, borderRadius: '50%', background: 'rgba(255,255,255,0.08)', pointerEvents: 'none' }} />
            <div style={{ position: 'absolute', bottom: -30, left: isMobile ? '58%' : '28%', width: 120, height: 120, borderRadius: '50%', background: 'rgba(255,255,255,0.05)', pointerEvents: 'none' }} />
            <Row gutter={[24, 18]} align="middle" style={{ position: 'relative', zIndex: 1 }}>
              <Col xs={24} lg={14}>
                <Space direction="vertical" size={8} style={{ width: '100%' }}>
                  <Typography.Text style={{ color: 'rgba(255,255,255,0.72)', fontSize: 11, letterSpacing: '0.18em', textTransform: 'uppercase' }}>
                    Launchpad
                  </Typography.Text>
                  <Title level={isMobile ? 3 : 2} style={{
                    margin: 0,
                    color: token.colorWhite,
                    fontFamily: designDisplayFont,
                    letterSpacing: '-0.03em',
                    textShadow: '0 2px 4px color-mix(in srgb, var(--ant-color-black) 18%, transparent)',
                  }}>
                    <RocketOutlined style={{ marginRight: 8 }} />
                    项目创建向导
                  </Title>
                  <Paragraph style={{ margin: 0, color: 'rgba(255,255,255,0.82)', fontSize: 15, lineHeight: 1.8 }}>
                    把新项目的定位、默认创作偏好和联网检索策略在一页里设置清楚。这里应该像创作启动台，而不是一张普通表单。
                  </Paragraph>
                  <Space wrap size={[10, 10]}>
                    <Tag style={{ borderRadius: 999, paddingInline: 12, border: '1px solid rgba(255,255,255,0.12)', background: 'rgba(255,255,255,0.08)', color: token.colorWhite }}>
                      当前阶段：{currentStep === 'form' ? '填写向导' : 'AI 生成中'}
                    </Tag>
                    <Tag style={{ borderRadius: 999, paddingInline: 12, border: '1px solid rgba(255,255,255,0.12)', background: 'rgba(255,255,255,0.08)', color: token.colorWhite }}>
                      MCP：{watchedEnableMcp ? '已启用' : '未启用'}
                    </Tag>
                    {resumeProjectId ? (
                      <Tag style={{ borderRadius: 999, paddingInline: 12, border: '1px solid rgba(255,255,255,0.12)', background: 'rgba(255,255,255,0.08)', color: token.colorWhite }}>
                        支持恢复未完成项目
                      </Tag>
                    ) : null}
                  </Space>
                </Space>
              </Col>
              <Col xs={24} lg={10}>
                <Row gutter={[12, 12]}>
                  {[
                    { label: '章节目标', value: form.getFieldValue('chapter_count') || 30 },
                    { label: '角色数量', value: form.getFieldValue('character_count') || 5 },
                    { label: '目标字数', value: form.getFieldValue('target_words') || 100000 },
                    { label: '生成模型', value: watchedModel || '自动', compact: true },
                  ].map((item) => (
                    <Col xs={12} key={item.label}>
                      <div
                        style={{
                          minHeight: 92,
                          borderRadius: 18,
                          padding: '12px 14px',
                          background: 'rgba(255,255,255,0.08)',
                          border: '1px solid rgba(255,255,255,0.1)',
                          backdropFilter: 'blur(10px)',
                          display: 'flex',
                          flexDirection: 'column',
                          justifyContent: 'space-between',
                        }}
                      >
                        <Typography.Text style={{ color: 'rgba(255,255,255,0.72)', fontSize: 12, display: 'block' }}>
                          {item.label}
                        </Typography.Text>
                        <Typography.Text style={{ color: token.colorWhite, fontWeight: 700, fontSize: item.compact ? 15 : 24, lineHeight: 1.2, wordBreak: 'break-word' }}>
                          {item.value}
                        </Typography.Text>
                      </div>
                    </Col>
                  ))}
                </Row>
              </Col>
            </Row>
            <Space wrap size={[10, 10]} style={{ marginTop: 20, position: 'relative', zIndex: 1 }}>
              <Button
                icon={<ArrowLeftOutlined />}
                onClick={() => navigate('/')}
                size={isMobile ? 'middle' : 'large'}
                disabled={shouldDisableNavigation(currentStep === 'generating')}
                style={{
                  borderRadius: 999,
                  background: `color-mix(in srgb, ${token.colorWhite} 14%, transparent)`,
                  borderColor: `color-mix(in srgb, ${token.colorWhite} 20%, transparent)`,
                  color: token.colorWhite,
                }}
              >
                {isMobile ? '返回' : '返回首页'}
              </Button>
            </Space>
          </Card>
        </div>

      <div style={{
        maxWidth: currentStep === 'form' ? 920 : 1120,
        margin: '0 auto',
      }}>
        <Card
          variant="borderless"
          style={{
            background: `linear-gradient(180deg, ${token.colorBgContainer} 0%, ${token.colorFillAlter} 100%)`,
            borderRadius: 24,
            border: `1px solid ${token.colorBorderSecondary}`,
            boxShadow: `0 18px 36px color-mix(in srgb, ${token.colorText} 8%, transparent)`,
          }}
          styles={{ body: { padding: isMobile ? 16 : 20 } }}
        >
          {currentStep === 'form' && renderForm()}
          {currentStep === 'generating' && generationConfig && (
            <AIProjectGenerator
              config={generationConfig}
              storagePrefix="wizard"
              onComplete={handleComplete}
              onBack={handleBack}
              onBusyChange={setIsGenerationBusy}
              backButtonText="返回向导首页"
              isMobile={isMobile}
              resumeProjectId={resumeProjectId ?? undefined}
            />
          )}
        </Card>
      </div>
      </div>
    </div>
  );
}
