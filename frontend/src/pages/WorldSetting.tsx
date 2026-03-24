import { Card, Descriptions, Empty, Typography, Button, Modal, Form, Input, message, Flex, InputNumber, Select } from 'antd';
import { GlobalOutlined, EditOutlined, SyncOutlined, FormOutlined } from '@ant-design/icons';
import { useEffect, useRef, useState } from 'react';
import { useStore } from '../store';
import { cardStyles } from '../components/CardStyles';
import { backgroundTaskApi, projectApi } from '../services/api';
import { SSELoadingOverlay } from '../components/SSELoadingOverlay';
import type { QualityPreset } from '../types';
import {
  CREATIVE_MODE_OPTIONS,
  PLOT_STAGE_OPTIONS,
  QUALITY_PRESET_OPTIONS,
  STORY_FOCUS_OPTIONS,
  resolveOptionDescription,
  resolveOptionLabel,
} from '../utils/generationPreferenceOptions';

const { Title, Paragraph } = Typography;
const { TextArea } = Input;
export default function WorldSetting() {
  const { currentProject, setCurrentProject } = useStore();
  const [isEditModalVisible, setIsEditModalVisible] = useState(false);
  const [editForm] = Form.useForm();
  const [isSaving, setIsSaving] = useState(false);
  const [isEditProjectModalVisible, setIsEditProjectModalVisible] = useState(false);
  const [editProjectForm] = Form.useForm();
  const [isSavingProject, setIsSavingProject] = useState(false);
  const selectedDefaultQualityPreset = Form.useWatch('default_quality_preset', editProjectForm) as QualityPreset | undefined;
  const selectedDefaultQualityPresetOption = QUALITY_PRESET_OPTIONS.find(
    (item) => item.value === selectedDefaultQualityPreset,
  );
  const [isRegenerating, setIsRegenerating] = useState(false);
  const [isCancellingTask, setIsCancellingTask] = useState(false);
  const [regenerateProgress, setRegenerateProgress] = useState(0);
  const [regenerateMessage, setRegenerateMessage] = useState('');
  const [isPreviewModalVisible, setIsPreviewModalVisible] = useState(false);
  const [newWorldData, setNewWorldData] = useState<{
    time_period: string;
    location: string;
    atmosphere: string;
    rules: string;
  } | null>(null);
  const [isSavingPreview, setIsSavingPreview] = useState(false);
  const [modal, contextHolder] = Modal.useModal();
  const taskPollTimerRef = useRef<number | null>(null);
  const currentTaskIdRef = useRef<string | null>(null);

  const stopTaskPolling = () => {
    if (taskPollTimerRef.current) {
      window.clearInterval(taskPollTimerRef.current);
      taskPollTimerRef.current = null;
    }
  };

  useEffect(() => {
    return () => {
      stopTaskPolling();
      currentTaskIdRef.current = null;
    };
  }, []);

  const startTaskPolling = (taskId: string) => {
    stopTaskPolling();
    currentTaskIdRef.current = taskId;
    setIsCancellingTask(false);

    const poll = async () => {
      try {
        const task = await backgroundTaskApi.getTaskStatus(taskId);
        setRegenerateProgress(task.progress || 0);
        setRegenerateMessage(task.message || '');

        if (task.status === 'completed') {
          stopTaskPolling();
          currentTaskIdRef.current = null;
          setIsCancellingTask(false);
          setIsRegenerating(false);
          setRegenerateProgress(0);
          setRegenerateMessage('');

          const result = task.result as Record<string, unknown> | null;
          if (result) {
            setNewWorldData({
              time_period: String(result.time_period || ''),
              location: String(result.location || ''),
              atmosphere: String(result.atmosphere || ''),
              rules: String(result.rules || ''),
            });
          }
          setIsPreviewModalVisible(true);
          return;
        }

        if (task.status === 'failed') {
          stopTaskPolling();
          currentTaskIdRef.current = null;
          setIsCancellingTask(false);
          setIsRegenerating(false);
          setRegenerateProgress(0);
          setRegenerateMessage('');
          message.error(task.error || task.message || '重新生成失败，请重试');
          return;
        }

        if (task.status === 'cancelled') {
          stopTaskPolling();
          currentTaskIdRef.current = null;
          setIsCancellingTask(false);
          setIsRegenerating(false);
          setRegenerateProgress(0);
          setRegenerateMessage('');
          message.info(task.message || '后台任务已取消');
        }
      } catch (error) {
        console.error('轮询世界观任务状态失败:', error);
      }
    };

    void poll();
    taskPollTimerRef.current = window.setInterval(() => {
      void poll();
    }, 1500);
  };

  const handleRegenerateBackground = async () => {
    if (!currentProject) return;
    if (isRegenerating) {
      message.info('后台世界观任务正在运行，请稍后查看结果');
      return;
    }

    modal.confirm({
      title: '确认重新生成',
      content: '确定要使用智能重新生成世界观设定吗？这将替换当前的世界观内容。',
      centered: true,
      okText: '确认重新生成',
      cancelText: '取消',
      onOk: async () => {
        setIsRegenerating(true);
        setIsCancellingTask(false);
        setRegenerateProgress(0);
        setRegenerateMessage('正在创建后台任务...');

        try {
          const task = await backgroundTaskApi.createTask({
            task_type: 'world_regenerate',
            project_id: currentProject.id,
            payload: {},
          });
          message.success('后台世界观生成任务已创建，可继续进行其他操作');
          currentTaskIdRef.current = task.task_id;
          startTaskPolling(task.task_id);
        } catch (error) {
          console.error('创建后台任务失败:', error);
          currentTaskIdRef.current = null;
          setIsCancellingTask(false);
          setIsRegenerating(false);
          setRegenerateProgress(0);
          setRegenerateMessage('');
          message.error('重新生成失败，请重试');
        }
      }
    });
  };

  const handleCancelRegenerateTask = async () => {
    const taskId = currentTaskIdRef.current;
    if (!taskId || isCancellingTask) {
      return;
    }

    setIsCancellingTask(true);
    try {
      await backgroundTaskApi.cancelTask(taskId);
      message.info('正在取消后台任务...');
      stopTaskPolling();
      currentTaskIdRef.current = null;
      setIsRegenerating(false);
      setRegenerateProgress(0);
      setRegenerateMessage('');
    } catch (error) {
      console.error('取消世界观重生成任务失败:', error);
      message.error('取消任务失败，请重试');
    } finally {
      setIsCancellingTask(false);
    }
  };

  // 确认保存重新生成的内容
  const handleConfirmSave = async () => {
    if (!currentProject || !newWorldData) return;

    setIsSavingPreview(true);
    try {
      const updatedProject = await projectApi.updateProject(currentProject.id, {
        world_time_period: newWorldData.time_period,
        world_location: newWorldData.location,
        world_atmosphere: newWorldData.atmosphere,
        world_rules: newWorldData.rules,
      });

      setCurrentProject(updatedProject);
      message.success('世界观已更新！');
      setIsPreviewModalVisible(false);
      setNewWorldData(null);
    } catch (error) {
      console.error('保存失败:', error);
      message.error('保存失败，请重试');
    } finally {
      setIsSavingPreview(false);
    }
  };

  // 取消保存，关闭预览
  const handleCancelSave = () => {
    setIsPreviewModalVisible(false);
    setNewWorldData(null);
    message.info('已取消，保持原有内容');
  };

  if (!currentProject) return null;

  // 检查是否有世界设定信息
  const hasWorldSetting = currentProject.world_time_period ||
    currentProject.world_location ||
    currentProject.world_atmosphere ||
    currentProject.world_rules;

  if (!hasWorldSetting) {
    return (
      <div style={{ display: 'flex', flexDirection: 'column', height: '100%' }}>
        {/* 固定头部 */}
        <div style={{
          position: 'sticky',
          top: 0,
          zIndex: 10,
          backgroundColor: '#fff',
          padding: '16px 0',
          marginBottom: 16,
          borderBottom: '1px solid var(--color-border-secondary)',
          display: 'flex',
          alignItems: 'center'
        }}>
          <GlobalOutlined style={{ fontSize: 24, marginRight: 12, color: 'var(--color-primary)' }} />
          <h2 style={{ margin: 0 }}>世界设定</h2>
        </div>

        {/* 可滚动内容区域 */}
        <div style={{ flex: 1, overflowY: 'auto' }}>
          <Empty
            description="暂无世界设定信息"
            style={{ marginTop: 60 }}
          >
            <Paragraph type="secondary">
              世界设定信息在创建项目向导中生成，用于构建小说的世界观背景。
            </Paragraph>
          </Empty>
        </div>
      </div>
    );
  }

  return (
    <div style={{ display: 'flex', flexDirection: 'column', height: '100%' }}>
      {contextHolder}
      {/* 固定头部 */}
      <div style={{
        position: 'sticky',
        top: 0,
        zIndex: 10,
        backgroundColor: '#fff',
        padding: '16px 0',
        marginBottom: 24,
        borderBottom: '1px solid #f0f0f0'
      }}>
        <Flex
          justify="space-between"
          align="flex-start"
          gap={12}
          wrap="wrap"
        >
          <div style={{ display: 'flex', alignItems: 'center', minWidth: 'fit-content' }}>
            <GlobalOutlined style={{ fontSize: 24, marginRight: 12, color: 'var(--color-primary)' }} />
            <h2 style={{ margin: 0, whiteSpace: 'nowrap' }}>世界设定</h2>
          </div>
          <Flex gap={8} wrap="wrap" style={{ flex: '0 1 auto' }}>
            <Button
              icon={<SyncOutlined />}
              onClick={handleRegenerateBackground}
              disabled={isRegenerating}
              style={{
                minWidth: 'fit-content',
                flex: '1 1 auto'
              }}
            >
              <span className="button-text-mobile">智能重新生成</span>
            </Button>
            <Button
              type="primary"
              icon={<FormOutlined />}
              onClick={() => {
                editProjectForm.setFieldsValue({
                  title: currentProject.title || '',
                  description: currentProject.description || '',
                  theme: currentProject.theme || '',
                  genre: currentProject.genre || '',
                  narrative_perspective: currentProject.narrative_perspective || '',
                  target_words: currentProject.target_words || 0,
                  default_creative_mode: currentProject.default_creative_mode,
                  default_story_focus: currentProject.default_story_focus,
                  default_plot_stage: currentProject.default_plot_stage,
                  default_story_creation_brief: currentProject.default_story_creation_brief || '',
                  default_quality_preset: currentProject.default_quality_preset,
                  default_quality_notes: currentProject.default_quality_notes || '',
                });
                setIsEditProjectModalVisible(true);
              }}
              style={{
                minWidth: 'fit-content',
                flex: '1 1 auto'
              }}
            >
              <span className="button-text-mobile">编辑基础信息</span>
            </Button>
            <Button
              type="primary"
              icon={<EditOutlined />}
              onClick={() => {
                editForm.setFieldsValue({
                  world_time_period: currentProject.world_time_period || '',
                  world_location: currentProject.world_location || '',
                  world_atmosphere: currentProject.world_atmosphere || '',
                  world_rules: currentProject.world_rules || '',
                });
                setIsEditModalVisible(true);
              }}
              style={{
                minWidth: 'fit-content',
                flex: '1 1 auto'
              }}
            >
              <span className="button-text-mobile">编辑世界观</span>
            </Button>
          </Flex>
        </Flex>
      </div>

      {/* 可滚动内容区域 */}
      <div style={{ flex: 1, overflowY: 'auto' }}>
        <Card
          style={{
            ...cardStyles.base,
            marginBottom: 16
          }}
          title={
            <span style={{ fontSize: 18, fontWeight: 500 }}>
              基础信息
            </span>
          }
        >
          <Descriptions bordered column={1} styles={{ label: { width: 120, fontWeight: 500 } }}>
            <Descriptions.Item label="小说名称">{currentProject.title}</Descriptions.Item>
            {currentProject.description && (
              <Descriptions.Item label="小说简介">{currentProject.description}</Descriptions.Item>
            )}
            <Descriptions.Item label="小说主题">{currentProject.theme || '未设定'}</Descriptions.Item>
            <Descriptions.Item label="小说类型">{currentProject.genre || '未设定'}</Descriptions.Item>
            <Descriptions.Item label="叙事视角">{currentProject.narrative_perspective || '未设定'}</Descriptions.Item>
            <Descriptions.Item label="目标字数">
              {currentProject.target_words ? `${currentProject.target_words.toLocaleString()} 字` : '未设定'}
            </Descriptions.Item>
            <Descriptions.Item label="默认创作模式">
              {resolveOptionLabel(CREATIVE_MODE_OPTIONS, currentProject.default_creative_mode)}
            </Descriptions.Item>
            <Descriptions.Item label="默认结构侧重点">
              {resolveOptionLabel(STORY_FOCUS_OPTIONS, currentProject.default_story_focus)}
            </Descriptions.Item>
            <Descriptions.Item label="默认剧情阶段">
              {resolveOptionLabel(PLOT_STAGE_OPTIONS, currentProject.default_plot_stage)}
            </Descriptions.Item>
            <Descriptions.Item label="??????">
              <Flex vertical gap={2}>
                <span>{resolveOptionLabel(QUALITY_PRESET_OPTIONS, currentProject.default_quality_preset)}</span>
                {resolveOptionDescription(QUALITY_PRESET_OPTIONS, currentProject.default_quality_preset) && (
                  <Typography.Text type="secondary" style={{ fontSize: 12 }}>
                    {resolveOptionDescription(QUALITY_PRESET_OPTIONS, currentProject.default_quality_preset)}
                  </Typography.Text>
                )}
              </Flex>
            </Descriptions.Item>
            <Descriptions.Item label="默认创作总控摘要">
              <Paragraph style={{ whiteSpace: 'pre-wrap', marginBottom: 0 }}>
                {currentProject.default_story_creation_brief?.trim() || '未设定'}
              </Paragraph>
            </Descriptions.Item>
            <Descriptions.Item label="默认质量补充偏好">
              <Paragraph style={{ whiteSpace: 'pre-wrap', marginBottom: 0 }}>
                {currentProject.default_quality_notes?.trim() || '未设定'}
              </Paragraph>
            </Descriptions.Item>
          </Descriptions>
        </Card>

        <Card
          style={{
            ...cardStyles.base,
            marginBottom: 16
          }}
          title={
            <span style={{ fontSize: 18, fontWeight: 500 }}>
              <GlobalOutlined style={{ marginRight: 8 }} />
              小说世界观
            </span>
          }
        >
          <div style={{ padding: '16px 0' }}>
            {currentProject.world_time_period && (
              <div style={{ marginBottom: 24 }}>
                <Title level={5} style={{ color: 'var(--color-primary)', marginBottom: 12 }}>
                  时间设定
                </Title>
                <Paragraph style={{
                  fontSize: 15,
                  lineHeight: 1.8,
                  padding: 16,
                  background: 'var(--color-bg-layout)',
                  borderRadius: 8,
                  borderLeft: '4px solid var(--color-primary)'
                }}>
                  {currentProject.world_time_period}
                </Paragraph>
              </div>
            )}

            {currentProject.world_location && (
              <div style={{ marginBottom: 24 }}>
                <Title level={5} style={{ color: 'var(--color-success)', marginBottom: 12 }}>
                  地点设定
                </Title>
                <Paragraph style={{
                  fontSize: 15,
                  lineHeight: 1.8,
                  padding: 16,
                  background: 'var(--color-bg-layout)',
                  borderRadius: 8,
                  borderLeft: '4px solid var(--color-success)'
                }}>
                  {currentProject.world_location}
                </Paragraph>
              </div>
            )}

            {currentProject.world_atmosphere && (
              <div style={{ marginBottom: 24 }}>
                <Title level={5} style={{ color: 'var(--color-warning)', marginBottom: 12 }}>
                  氛围设定
                </Title>
                <Paragraph style={{
                  fontSize: 15,
                  lineHeight: 1.8,
                  padding: 16,
                  background: 'var(--color-bg-layout)',
                  borderRadius: 8,
                  borderLeft: '4px solid var(--color-warning)'
                }}>
                  {currentProject.world_atmosphere}
                </Paragraph>
              </div>
            )}

            {currentProject.world_rules && (
              <div style={{ marginBottom: 0 }}>
                <Title level={5} style={{ color: 'var(--color-error)', marginBottom: 12 }}>
                  规则设定
                </Title>
                <Paragraph style={{
                  fontSize: 15,
                  lineHeight: 1.8,
                  padding: 16,
                  background: 'var(--color-bg-layout)',
                  borderRadius: 8,
                  borderLeft: '4px solid var(--color-error)'
                }}>
                  {currentProject.world_rules}
                </Paragraph>
              </div>
            )}
          </div>
        </Card>
      </div>

      {/* 编辑世界观模态框 */}
      <Modal
        title="编辑世界观"
        open={isEditModalVisible}
        centered
        onCancel={() => {
          setIsEditModalVisible(false);
          editForm.resetFields();
        }}
        onOk={async () => {
          try {
            const values = await editForm.validateFields();
            setIsSaving(true);

            const updatedProject = await projectApi.updateProject(currentProject.id, {
              world_time_period: values.world_time_period,
              world_location: values.world_location,
              world_atmosphere: values.world_atmosphere,
              world_rules: values.world_rules,
            });

            setCurrentProject(updatedProject);
            message.success('世界观更新成功');
            setIsEditModalVisible(false);
            editForm.resetFields();
          } catch (error) {
            console.error('更新世界观失败:', error);
            message.error('更新失败，请重试');
          } finally {
            setIsSaving(false);
          }
        }}
        confirmLoading={isSaving}
        width={800}
        okText="保存"
        cancelText="取消"
      >
        <Form
          form={editForm}
          layout="vertical"
          style={{ marginTop: 16 }}
        >
          <Form.Item
            label="时间设定"
            name="world_time_period"
            rules={[{ required: true, message: '请输入时间设定' }]}
          >
            <TextArea
              rows={4}
              placeholder="描述故事发生的时代背景..."
              showCount
              maxLength={1000}
            />
          </Form.Item>

          <Form.Item
            label="地点设定"
            name="world_location"
            rules={[{ required: true, message: '请输入地点设定' }]}
          >
            <TextArea
              rows={4}
              placeholder="描述故事发生的地理位置和环境..."
              showCount
              maxLength={1000}
            />
          </Form.Item>

          <Form.Item
            label="氛围设定"
            name="world_atmosphere"
            rules={[{ required: true, message: '请输入氛围设定' }]}
          >
            <TextArea
              rows={4}
              placeholder="描述故事的整体氛围和基调..."
              showCount
              maxLength={1000}
            />
          </Form.Item>

          <Form.Item
            label="规则设定"
            name="world_rules"
            rules={[{ required: true, message: '请输入规则设定' }]}
          >
            <TextArea
              rows={4}
              placeholder="描述这个世界的特殊规则和设定..."
              showCount
              maxLength={1000}
            />
          </Form.Item>
        </Form>
      </Modal>

      {/* 编辑项目基础信息模态框 */}
      <Modal
        title="编辑项目基础信息"
        open={isEditProjectModalVisible}
        centered
        onCancel={() => {
          setIsEditProjectModalVisible(false);
          editProjectForm.resetFields();
        }}
        onOk={async () => {
          try {
            const values = await editProjectForm.validateFields();
            setIsSavingProject(true);

            const updatedProject = await projectApi.updateProject(currentProject.id, {
              title: values.title,
              description: values.description,
              theme: values.theme,
              genre: values.genre,
              narrative_perspective: values.narrative_perspective,
              target_words: values.target_words,
              default_creative_mode: values.default_creative_mode,
              default_story_focus: values.default_story_focus,
              default_plot_stage: values.default_plot_stage,
              default_story_creation_brief: values.default_story_creation_brief,
              default_quality_preset: values.default_quality_preset,
              default_quality_notes: values.default_quality_notes,
            });

            setCurrentProject(updatedProject);
            message.success('项目基础信息更新成功');
            setIsEditProjectModalVisible(false);
            editProjectForm.resetFields();
          } catch (error) {
            console.error('更新项目基础信息失败:', error);
            message.error('更新失败，请重试');
          } finally {
            setIsSavingProject(false);
          }
        }}
        confirmLoading={isSavingProject}
        width={800}
        okText="保存"
        cancelText="取消"
      >
        <Form
          form={editProjectForm}
          layout="vertical"
          style={{ marginTop: 16 }}
        >
          <Form.Item
            label="小说名称"
            name="title"
            rules={[
              { required: true, message: '请输入小说名称' },
              { max: 200, message: '名称不能超过200字' }
            ]}
          >
            <Input
              placeholder="请输入小说名称"
              showCount
              maxLength={200}
            />
          </Form.Item>

          <Form.Item
            label="小说简介"
            name="description"
            rules={[
              { max: 1000, message: '简介不能超过1000字' }
            ]}
          >
            <TextArea
              rows={4}
              placeholder="请输入小说简介（选填）"
              showCount
              maxLength={1000}
            />
          </Form.Item>

          <Form.Item
            label="小说主题"
            name="theme"
            rules={[
              { max: 500, message: '主题不能超过500字' }
            ]}
          >
            <TextArea
              rows={3}
              placeholder="请输入小说主题（选填）"
              showCount
              maxLength={500}
            />
          </Form.Item>

          <Form.Item
            label="小说类型"
            name="genre"
            rules={[
              { max: 100, message: '类型不能超过100字' }
            ]}
          >
            <Input
              placeholder="请输入小说类型，如：玄幻、都市、科幻等（选填）"
              showCount
              maxLength={100}
            />
          </Form.Item>

          <Form.Item
            label="叙事视角"
            name="narrative_perspective"
          >
            <Select
              placeholder="请选择叙事视角（选填）"
              allowClear
              options={[
                { label: '第一人称', value: '第一人称' },
                { label: '第三人称', value: '第三人称' },
                { label: '全知视角', value: '全知视角' }
              ]}
            />
          </Form.Item>

          <Form.Item
            label="目标字数"
            name="target_words"
            rules={[
              { type: 'number', min: 0, message: '目标字数不能为负数' },
              { type: 'number', max: 2147483647, message: '目标字数超出范围' }
            ]}
          >
            <InputNumber
              style={{ width: '100%' }}
              placeholder="请输入目标字数（选填，最大21亿字）"
              min={0}
              max={2147483647}
              step={1000}
              addonAfter="字"
            />
          </Form.Item>

          <Card
            size="small"
            title="??????"
            style={{ marginBottom: 0, background: 'var(--color-fill-quaternary)' }}
          >
            <Form.Item
              label="??????"
              name="default_creative_mode"
              extra="???????????????????????????????"
            >
              <Select
                placeholder="?????????????"
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
              label="???????"
              name="default_story_focus"
            >
              <Select
                placeholder="?????????????????"
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

            <Form.Item
              label="??????"
              name="default_plot_stage"
            >
              <Select
                placeholder="?????????????"
                allowClear
                optionLabelProp="label"
              >
                {PLOT_STAGE_OPTIONS.map((option) => (
                  <Select.Option key={option.value} value={option.value} label={option.label}>
                    <div>{option.label}</div>
                    <div style={{ fontSize: 12, color: 'var(--color-text-tertiary)' }}>{option.description}</div>
                  </Select.Option>
                ))}
              </Select>
            </Form.Item>

            <Form.Item
              label="????????"
              name="default_story_creation_brief"
              extra="?????????????????????????"
            >
              <TextArea
                rows={4}
                placeholder="?????????????????????????????????????"
                showCount
                maxLength={1200}
              />
            </Form.Item>

            <Form.Item
              label="??????"
              extra="?????????????????????????????"
            >
              <Form.Item name="default_quality_preset" hidden>
                <Input />
              </Form.Item>

              <div
                style={{
                  display: 'grid',
                  gridTemplateColumns: 'repeat(auto-fit, minmax(180px, 1fr))',
                  gap: 12,
                }}
              >
                <Card
                  hoverable
                  onClick={() => editProjectForm.setFieldValue('default_quality_preset', undefined)}
                  style={{
                    cursor: 'pointer',
                    borderStyle: 'dashed',
                    borderColor: !selectedDefaultQualityPreset ? 'var(--color-primary)' : 'var(--color-border)',
                    background: !selectedDefaultQualityPreset ? 'var(--color-primary-bg)' : 'var(--color-bg-container)',
                    boxShadow: !selectedDefaultQualityPreset ? '0 0 0 1px rgba(24, 144, 255, 0.12)' : 'none',
                  }}
                >
                  <Flex vertical gap={8}>
                    <Typography.Text strong>?????</Typography.Text>
                    <Typography.Text type="secondary" style={{ fontSize: 12 }}>
                      ????????????????????????????
                    </Typography.Text>
                  </Flex>
                </Card>

                {QUALITY_PRESET_OPTIONS.map((option) => {
                  const isActive = selectedDefaultQualityPreset === option.value;
                  return (
                    <Card
                      key={option.value}
                      hoverable
                      onClick={() => editProjectForm.setFieldValue('default_quality_preset', isActive ? undefined : option.value)}
                      style={{
                        cursor: 'pointer',
                        borderColor: isActive ? 'var(--color-primary)' : 'var(--color-border)',
                        background: isActive ? 'var(--color-primary-bg)' : 'var(--color-bg-container)',
                        boxShadow: isActive ? '0 0 0 1px rgba(24, 144, 255, 0.12)' : 'none',
                      }}
                    >
                      <Flex vertical gap={8}>
                        <div>
                          <Typography.Text strong>{option.label}</Typography.Text>
                          <div style={{ marginTop: 6, fontSize: 12, color: 'var(--color-text-secondary)' }}>
                            {option.description}
                          </div>
                        </div>
                        <div style={{ fontSize: 12, color: 'var(--color-text-tertiary)' }}>
                          ???{option.bestFor}
                        </div>
                        <div style={{ fontSize: 12, color: 'var(--color-text-tertiary)' }}>
                          ???{option.caution}
                        </div>
                      </Flex>
                    </Card>
                  );
                })}
              </div>

              <Card
                size="small"
                style={{
                  marginTop: 12,
                  borderColor: 'var(--color-border-secondary)',
                  background: 'var(--color-fill-quaternary)',
                }}
              >
                <Flex vertical gap={6}>
                  <Typography.Text strong>
                    ?????{selectedDefaultQualityPresetOption?.label || '?????'}
                  </Typography.Text>
                  <Typography.Text type="secondary" style={{ fontSize: 12 }}>
                    {selectedDefaultQualityPresetOption?.description || '?????????????????????????????????'}
                  </Typography.Text>
                  {selectedDefaultQualityPresetOption && (
                    <>
                      <Typography.Text type="secondary" style={{ fontSize: 12 }}>
                        ???{selectedDefaultQualityPresetOption.bestFor}
                      </Typography.Text>
                      <Typography.Text type="secondary" style={{ fontSize: 12 }}>
                        ???{selectedDefaultQualityPresetOption.caution}
                      </Typography.Text>
                    </>
                  )}
                </Flex>
              </Card>
            </Form.Item>

            <Form.Item
              label="????????"
              name="default_quality_notes"
              extra="??????????????????????????????????????"
            >
              <TextArea
                rows={3}
                placeholder="??????????????????????????????????????"
                showCount
                maxLength={600}
              />
            </Form.Item>
          </Card>
        </Form>
      </Modal>

      {/* AI重新生成加载遮罩 */}
      <SSELoadingOverlay
        loading={isRegenerating}
        progress={regenerateProgress}
        message={regenerateMessage}
        blocking={false}
        onCancel={handleCancelRegenerateTask}
        cancelButtonLoading={isCancellingTask}
        cancelButtonDisabled={isCancellingTask || !currentTaskIdRef.current}
      />

      {/* 预览重新生成的内容模态框 */}
      <Modal
        title="预览重新生成的世界观"
        open={isPreviewModalVisible}
        centered
        width={900}
        onOk={handleConfirmSave}
        onCancel={handleCancelSave}
        confirmLoading={isSavingPreview}
        okText="确认替换"
        cancelText="取消"
        okButtonProps={{ danger: true }}
      >
        {newWorldData && (
          <div style={{ maxHeight: '60vh', overflowY: 'auto' }}>
            <div style={{ marginBottom: 24, padding: 16, background: 'var(--color-warning-bg)', border: '1px solid var(--color-warning-border)', borderRadius: 8 }}>
              <Typography.Text type="warning" strong>
                ⚠️ 注意：点击"确认替换"将会用新内容替换当前的世界观设定
              </Typography.Text>
            </div>

            <div style={{ marginBottom: 24 }}>
              <Title level={5} style={{ color: 'var(--color-primary)', marginBottom: 12 }}>
                时间设定
              </Title>
              <Paragraph style={{
                fontSize: 15,
                lineHeight: 1.8,
                padding: 16,
                background: '#f5f5f5',
                borderRadius: 8,
                borderLeft: '4px solid #1890ff'
              }}>
                {newWorldData.time_period}
              </Paragraph>
            </div>

            <div style={{ marginBottom: 24 }}>
              <Title level={5} style={{ color: '#52c41a', marginBottom: 12 }}>
                地点设定
              </Title>
              <Paragraph style={{
                fontSize: 15,
                lineHeight: 1.8,
                padding: 16,
                background: '#f5f5f5',
                borderRadius: 8,
                borderLeft: '4px solid #52c41a'
              }}>
                {newWorldData.location}
              </Paragraph>
            </div>

            <div style={{ marginBottom: 24 }}>
              <Title level={5} style={{ color: '#faad14', marginBottom: 12 }}>
                氛围设定
              </Title>
              <Paragraph style={{
                fontSize: 15,
                lineHeight: 1.8,
                padding: 16,
                background: '#f5f5f5',
                borderRadius: 8,
                borderLeft: '4px solid #faad14'
              }}>
                {newWorldData.atmosphere}
              </Paragraph>
            </div>

            <div style={{ marginBottom: 0 }}>
              <Title level={5} style={{ color: '#f5222d', marginBottom: 12 }}>
                规则设定
              </Title>
              <Paragraph style={{
                fontSize: 15,
                lineHeight: 1.8,
                padding: 16,
                background: '#f5f5f5',
                borderRadius: 8,
                borderLeft: '4px solid #f5222d'
              }}>
                {newWorldData.rules}
              </Paragraph>
            </div>
          </div>
        )}
      </Modal>
    </div>
  );
}
