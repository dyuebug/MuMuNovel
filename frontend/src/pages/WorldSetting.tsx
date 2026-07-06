import { Card, Descriptions, Empty, Typography, Button, Modal, Form, Input, message, Flex, InputNumber, Select, Row, Col, Alert, theme } from 'antd';
import { GlobalOutlined, EditOutlined, SyncOutlined, FormOutlined } from '@ant-design/icons';
import { useEffect, useRef, useState } from 'react';
import { useStore } from '../store';
import { isActiveBackgroundTask, useBackgroundTaskStore } from '../store/backgroundTasks';
import { backgroundTaskApi, projectApi } from '../services/modularApi';
import { formatBackgroundTaskError } from '../utils/taskPolling';
import { useRestorableBackgroundTaskPolling } from '../hooks/useRestorableBackgroundTaskPolling';
import { isRequestCancelledError } from '../services/core/httpClient';
import { SSELoadingOverlay } from '../components/SSELoadingOverlay';
import { designDisplayFont } from '../theme/themeConfig';
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
const WORLD_TASK_REPLAY_KEY_PREFIX = 'background-task-replay:world:';
const WORLD_TASK_OPEN_REQUEST_KEY_PREFIX = 'background-task-open:world:';

const hasWorldTaskReplayBeenHandled = (taskId: string): boolean => {
  try {
    return sessionStorage.getItem(`${WORLD_TASK_REPLAY_KEY_PREFIX}${taskId}`) === '1';
  } catch {
    return false;
  }
};

const markWorldTaskReplayHandled = (taskId: string) => {
  try {
    sessionStorage.setItem(`${WORLD_TASK_REPLAY_KEY_PREFIX}${taskId}`, '1');
  } catch {
    // ignore sessionStorage failures
  }
};

const getRequestedWorldTaskId = (projectId: string): string | null => {
  try {
    return sessionStorage.getItem(`${WORLD_TASK_OPEN_REQUEST_KEY_PREFIX}${projectId}`);
  } catch {
    return null;
  }
};

const clearRequestedWorldTaskId = (projectId: string) => {
  try {
    sessionStorage.removeItem(`${WORLD_TASK_OPEN_REQUEST_KEY_PREFIX}${projectId}`);
  } catch {
    // ignore sessionStorage failures
  }
};

const selectActiveWorldTask = (
  tasks: Record<string, import('../store/backgroundTasks').TrackedBackgroundTask>,
  projectId?: string | null,
) => {
  if (!projectId) {
    return null;
  }

  return Object.values(tasks)
    .filter(
      (task) => task.projectId === projectId
        && task.taskType === 'world_regenerate'
        && isActiveBackgroundTask(task)
    )
    .sort((left, right) => right.updatedAt - left.updatedAt)[0] ?? null;
};

const selectWorldReplayTaskSignature = (
  tasks: Record<string, import('../store/backgroundTasks').TrackedBackgroundTask>,
  projectId?: string | null,
): string => {
  if (!projectId) {
    return '';
  }

  const requestedTaskId = getRequestedWorldTaskId(projectId);
  const completedTasks = Object.values(tasks).filter(
    (task) => task.projectId === projectId
      && task.taskType === 'world_regenerate'
      && task.status === 'completed'
      && task.result
  );
  const completedTask = requestedTaskId
    ? completedTasks.find((task) => task.taskId === requestedTaskId)
    : completedTasks
      .filter((task) => !hasWorldTaskReplayBeenHandled(task.taskId))
      .sort((left, right) => (right.completedAt ?? right.updatedAt) - (left.completedAt ?? left.updatedAt))[0];

  if (!completedTask) {
    return '';
  }

  return `${completedTask.taskId}:${requestedTaskId ? 'requested' : 'latest'}:${completedTask.completedAt ?? completedTask.updatedAt}`;
};

export default function WorldSetting() {
  const { token } = theme.useToken();
  const { currentProject, setCurrentProject } = useStore();
  const activeProjectIdRef = useRef<string | null>(currentProject?.id ?? null);
  const [isMobile, setIsMobile] = useState(() => window.innerWidth <= 768);
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
  const activeTrackedWorldTask = useBackgroundTaskStore(
    (state) => selectActiveWorldTask(state.tasks, currentProject?.id)
  );
  const worldReplayTaskSignature = useBackgroundTaskStore(
    (state) => selectWorldReplayTaskSignature(state.tasks, currentProject?.id)
  );

  useEffect(() => {
    activeProjectIdRef.current = currentProject?.id ?? null;
  }, [currentProject?.id]);

  useEffect(() => {
    const handleResize = () => {
      setIsMobile(window.innerWidth <= 768);
    };

    window.addEventListener('resize', handleResize);
    return () => {
      window.removeEventListener('resize', handleResize);
    };
  }, []);

  const { currentTaskIdRef, startTaskPolling, stopTaskPolling } = useRestorableBackgroundTaskPolling({
    projectId: currentProject?.id,
    activeTrackedTask: activeTrackedWorldTask,
    isMatchingTask: (task) => task.task_type === 'world_regenerate' && (task.status === 'pending' || task.status === 'running'),
    onRestoreTask: ({ progress, message: taskMessage }) => {
      if (!activeProjectIdRef.current) {
        return;
      }
      setIsRegenerating(true);
      setIsCancellingTask(false);
      setRegenerateProgress(progress || 0);
      setRegenerateMessage(taskMessage || '正在恢复世界观重建任务...');
    },
    createPollingOptions: () => ({
      pollTask: (currentPollingTaskId) => backgroundTaskApi.getTaskStatus(currentPollingTaskId),
      onTask: (task) => {
        if (!activeProjectIdRef.current || task.project_id !== activeProjectIdRef.current) {
          return;
        }
        setRegenerateProgress(task.progress || 0);
        setRegenerateMessage(task.message || '');
      },
      onCompleted: (task) => {
        if (!activeProjectIdRef.current || task.project_id !== activeProjectIdRef.current) {
          stopTaskPolling();
          currentTaskIdRef.current = null;
          return;
        }
        stopTaskPolling();
        currentTaskIdRef.current = null;
        setIsCancellingTask(false);
        setIsRegenerating(false);
        setRegenerateProgress(0);
        setRegenerateMessage('');

        const result = task.result as Record<string, unknown> | null;
        if (result) {
          markWorldTaskReplayHandled(task.task_id);
          setNewWorldData({
            time_period: String(result.time_period || ''),
            location: String(result.location || ''),
            atmosphere: String(result.atmosphere || ''),
            rules: String(result.rules || ''),
          });
        }
        setIsPreviewModalVisible(true);
      },
      onFailed: (task) => {
        if (!activeProjectIdRef.current || task.project_id !== activeProjectIdRef.current) {
          stopTaskPolling();
          currentTaskIdRef.current = null;
          return;
        }
        stopTaskPolling();
        currentTaskIdRef.current = null;
        setIsCancellingTask(false);
        setIsRegenerating(false);
        setRegenerateProgress(0);
        setRegenerateMessage('');
        message.error(formatBackgroundTaskError(task.error, task.message, '世界设定重生失败'));
      },
      onCancelled: (task) => {
        if (!activeProjectIdRef.current || task.project_id !== activeProjectIdRef.current) {
          stopTaskPolling();
          currentTaskIdRef.current = null;
          return;
        }
        stopTaskPolling();
        currentTaskIdRef.current = null;
        setIsCancellingTask(false);
        setIsRegenerating(false);
        setRegenerateProgress(0);
        setRegenerateMessage('');
        message.info(task.message || '已取消重生');
      },
      onPollingError: (error) => {
        if (isRequestCancelledError(error)) {
          return;
        }
        console.error('世界设定轮询失败:', error);
        stopTaskPolling();
        currentTaskIdRef.current = null;
        setIsCancellingTask(false);
        setIsRegenerating(false);
        setRegenerateProgress(0);
        setRegenerateMessage('世界设定重生状态轮询失败，请稍后刷新重试');
        if (currentProject?.id) {
          const targetProjectId = currentProject.id;
          void projectApi.getProject(targetProjectId).then((project) => {
            if (activeProjectIdRef.current === targetProjectId) {
              setCurrentProject(project);
            }
          }).catch(() => undefined);
        }
        message.error('世界设定重生状态轮询失败，请稍后重试');
      },
    }),
  });



  useEffect(() => {
    if (!currentProject?.id || currentTaskIdRef.current || isPreviewModalVisible || newWorldData) {
      return;
    }

    if (!worldReplayTaskSignature) {
      return;
    }

    const [taskId, replayMode] = worldReplayTaskSignature.split(':');
    if (!taskId) {
      return;
    }

    const tasks = useBackgroundTaskStore.getState().tasks;
    const completedTask = tasks[taskId];

    if (!completedTask?.result) {
      return;
    }

    if (completedTask.projectId && completedTask.projectId !== currentProject.id) {
      return;
    }

    if (replayMode === 'requested') {
      clearRequestedWorldTaskId(currentProject.id);
    }
    markWorldTaskReplayHandled(completedTask.taskId);
    setNewWorldData({
      time_period: String(completedTask.result.time_period || ''),
      location: String(completedTask.result.location || ''),
      atmosphere: String(completedTask.result.atmosphere || ''),
      rules: String(completedTask.result.rules || ''),
    });
    setIsPreviewModalVisible(true);
  }, [currentProject?.id, currentTaskIdRef, isPreviewModalVisible, newWorldData, worldReplayTaskSignature]);

  const handleRegenerateBackground = async () => {
    if (!currentProject) return;
    if (isRegenerating || activeTrackedWorldTask) {
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

  const editorialInk = token.colorText;
  const heroBackground = `linear-gradient(135deg, #171411 0%, color-mix(in srgb, #171411 68%, ${token.colorPrimary} 32%) 100%)`;
  const panelBackground = `linear-gradient(180deg, color-mix(in srgb, ${token.colorBgContainer} 96%, ${token.colorPrimary} 4%) 0%, color-mix(in srgb, ${token.colorBgContainer} 92%, ${token.colorWarning} 8%) 100%)`;
  const quietPanelBackground = `linear-gradient(180deg, color-mix(in srgb, ${token.colorBgContainer} 98%, ${token.colorBgLayout} 2%) 0%, color-mix(in srgb, ${token.colorBgContainer} 92%, ${token.colorBgLayout} 8%) 100%)`;
  const panelBorder = `1px solid color-mix(in srgb, ${token.colorPrimary} 12%, ${token.colorBorder} 88%)`;
  const outlineButtonStyle = {
    borderRadius: 999,
    background: 'color-mix(in srgb, var(--ant-color-bg-container) 14%, transparent)',
    border: '1px solid color-mix(in srgb, var(--ant-color-bg-container) 20%, transparent)',
    color: editorialInk,
    boxShadow: `0 10px 18px color-mix(in srgb, ${token.colorText} 18%, transparent)`,
    backdropFilter: 'blur(8px)',
  } as const;
  const worldSections = [
    {
      key: 'time',
      title: '时间设定',
      color: token.colorPrimary,
      content: currentProject.world_time_period,
      summary: '定义时代背景、文明阶段与历史感。',
    },
    {
      key: 'location',
      title: '地点设定',
      color: token.colorSuccess,
      content: currentProject.world_location,
      summary: '承载地理空间、区域差异与场景分层。',
    },
    {
      key: 'atmosphere',
      title: '氛围设定',
      color: token.colorWarning,
      content: currentProject.world_atmosphere,
      summary: '决定作品整体情绪、质地和阅读温度。',
    },
    {
      key: 'rules',
      title: '规则设定',
      color: token.colorError,
      content: currentProject.world_rules,
      summary: '约束力量体系、社会规则与冲突边界。',
    },
  ].filter((item) => item.content);

  const projectSummaryItems = [
    {
      label: '小说类型',
      value: currentProject.genre || '未设定',
    },
    {
      label: '叙事视角',
      value: currentProject.narrative_perspective || '未设定',
    },
    {
      label: '目标字数',
      value: currentProject.target_words ? `${currentProject.target_words.toLocaleString()} 字` : '未设定',
    },
  ];

  const preferenceSummaryItems = [
    {
      label: '默认创作模式',
      value: resolveOptionLabel(CREATIVE_MODE_OPTIONS, currentProject.default_creative_mode),
    },
    {
      label: '结构侧重点',
      value: resolveOptionLabel(STORY_FOCUS_OPTIONS, currentProject.default_story_focus),
    },
    {
      label: '剧情阶段',
      value: resolveOptionLabel(PLOT_STAGE_OPTIONS, currentProject.default_plot_stage),
    },
    {
      label: '质量预设',
      value: resolveOptionLabel(QUALITY_PRESET_OPTIONS, currentProject.default_quality_preset),
    },
  ];
  const worldGuideItems = [
    {
      label: '阅读顺序',
      value: '先看项目基础信息，再确认创作偏好，最后回到世界四大维度统一校对。',
    },
    {
      label: '当前用途',
      value: '这里维护的是角色、组织、章节与生成链路共享的世界观母本。',
    },
  ];
  const worldCoverageItems = [
    {
      title: '世界框架',
      description: '时间、地点、氛围、规则四块一起看，才更容易发现设定冲突与信息缺口。',
    },
    {
      title: '生成入口',
      description: '智能重建适合快速出第一稿，手动编辑更适合后续做精修和一致性治理。',
    },
  ];
  const modalSurfaceStyles = {
    content: {
      borderRadius: 24,
      border: `1px solid ${token.colorBorderSecondary}`,
      background: `linear-gradient(180deg, ${token.colorBgContainer} 0%, color-mix(in srgb, ${token.colorFillQuaternary} 52%, ${token.colorBgContainer} 48%) 100%)`,
      boxShadow: `0 28px 56px color-mix(in srgb, ${token.colorText} 12%, transparent)`,
    },
    header: {
      background: 'transparent',
      borderBottom: 'none',
      paddingBottom: 0,
    },
    body: {
      paddingTop: 16,
    },
  } as const;

  return (
    <div style={{ display: 'flex', flexDirection: 'column', gap: 18, paddingBottom: 24 }}>
      {contextHolder}
      <Card
        variant="borderless"
        style={{
          background: heroBackground,
          borderRadius: 28,
          border: `1px solid color-mix(in srgb, ${token.colorBgContainer} 12%, transparent)`,
          boxShadow: `0 26px 52px color-mix(in srgb, ${token.colorText} 20%, transparent)`,
          overflow: 'hidden',
          position: 'relative',
        }}
        styles={{ body: { padding: 24 } }}
      >
        <div style={{ position: 'absolute', top: -54, right: -44, width: 180, height: 180, borderRadius: '50%', background: 'rgba(255,255,255,0.08)', pointerEvents: 'none' }} />
        <div style={{ position: 'absolute', bottom: -28, left: '28%', width: 110, height: 110, borderRadius: '50%', background: 'rgba(255,255,255,0.05)', pointerEvents: 'none' }} />
        <Row gutter={[24, 18]} align="middle" style={{ position: 'relative', zIndex: 1 }}>
          <Col xs={24} lg={15}>
            <Flex vertical gap={10}>
              <Typography.Text style={{ color: 'rgba(255,255,255,0.72)', fontSize: 11, letterSpacing: '0.18em', textTransform: 'uppercase' }}>
                Story Bible
              </Typography.Text>
              <Title level={2} style={{ margin: 0, color: editorialInk, fontFamily: designDisplayFont, letterSpacing: '-0.03em' }}>
                <GlobalOutlined style={{ marginRight: 10, color: 'rgba(255,255,255,0.9)' }} />
                世界设定
              </Title>
              <Paragraph style={{ margin: 0, color: 'rgba(255,255,255,0.82)', fontSize: 15, lineHeight: 1.8 }}>
                这里不再只是字段展示，而是一份能被持续打磨的“项目世界观手册”。用更清晰的阅读节奏，统一项目设定、创作偏好与背景生成结果，让后续创作入口都能共享同一套底稿。
              </Paragraph>
            </Flex>
          </Col>
          <Col xs={24} lg={9}>
            <Flex vertical gap={12}>
              {projectSummaryItems.map((item) => (
                <div
                  key={item.label}
                  style={{
                    display: 'flex',
                    justifyContent: 'space-between',
                    gap: 12,
                    padding: '12px 14px',
                    borderRadius: 18,
                    background: 'rgba(255,255,255,0.08)',
                    border: '1px solid rgba(255,255,255,0.1)',
                    backdropFilter: 'blur(10px)',
                  }}
                >
                  <Typography.Text style={{ color: 'rgba(255,255,255,0.72)', fontSize: 12 }}>{item.label}</Typography.Text>
                  <Typography.Text style={{ color: editorialInk, fontWeight: 600 }}>{item.value}</Typography.Text>
                </div>
              ))}
            </Flex>
          </Col>
        </Row>
        <Flex wrap gap={10} style={{ marginTop: 20, position: 'relative', zIndex: 1 }}>
          <Button
            icon={<SyncOutlined />}
            onClick={handleRegenerateBackground}
            disabled={Boolean(isRegenerating || activeTrackedWorldTask)}
            style={outlineButtonStyle}
          >
            智能重新生成
          </Button>
          <Button
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
            style={outlineButtonStyle}
          >
            编辑基础信息
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
            style={{ borderRadius: 999, paddingInline: 18 }}
          >
            编辑世界观
          </Button>
        </Flex>
      </Card>

      <div
        style={{
          display: 'grid',
          gridTemplateColumns: isMobile ? '1fr' : 'minmax(0, 1.15fr) minmax(320px, 0.95fr)',
          gap: 16,
        }}
      >
        <Card
          variant="borderless"
          style={{
            borderRadius: 22,
            background: quietPanelBackground,
            border: panelBorder,
            boxShadow: `0 18px 36px color-mix(in srgb, ${token.colorText} 8%, transparent)`,
          }}
          styles={{ body: { padding: 18 } }}
        >
          <Typography.Text style={{ fontSize: 11, letterSpacing: '0.18em', textTransform: 'uppercase', color: token.colorTextTertiary }}>
            World Manual
          </Typography.Text>
          <Title level={4} style={{ margin: '8px 0 10px', fontFamily: designDisplayFont, letterSpacing: '-0.03em' }}>
            世界观阅读导引
          </Title>
          <Paragraph type="secondary" style={{ marginBottom: 14, lineHeight: 1.8 }}>
            这页更像项目的世界设定手册。先把基础资料和默认偏好看成“创作前提”，再用四大世界维度去校正设定本身是否完整、统一、可落地。
          </Paragraph>
          <div style={{ display: 'grid', gridTemplateColumns: 'repeat(auto-fit, minmax(220px, 1fr))', gap: 10 }}>
            {worldGuideItems.map((item) => (
              <div
                key={item.label}
                style={{
                  borderRadius: 16,
                  padding: '12px 14px',
                  border: `1px solid ${token.colorBorderSecondary}`,
                  background: token.colorBgContainer,
                }}
              >
                <Typography.Text style={{ display: 'block', marginBottom: 4, fontSize: 12, color: token.colorTextTertiary }}>
                  {item.label}
                </Typography.Text>
                <Typography.Text strong style={{ lineHeight: 1.7 }}>
                  {item.value}
                </Typography.Text>
              </div>
            ))}
          </div>
        </Card>

        <Card
          variant="borderless"
          style={{
            borderRadius: 22,
            background: quietPanelBackground,
            border: panelBorder,
            boxShadow: `0 18px 36px color-mix(in srgb, ${token.colorText} 8%, transparent)`,
          }}
          styles={{ body: { padding: 18 } }}
        >
          <Typography.Text style={{ fontSize: 11, letterSpacing: '0.18em', textTransform: 'uppercase', color: token.colorTextTertiary }}>
            Coverage Map
          </Typography.Text>
          <Title level={4} style={{ margin: '8px 0 10px', fontFamily: designDisplayFont, letterSpacing: '-0.03em' }}>
            当前维护重点
          </Title>
          <Flex vertical gap={10}>
            {worldCoverageItems.map((item) => (
              <div
                key={item.title}
                style={{
                  borderRadius: 16,
                  padding: '12px 14px',
                  border: `1px solid ${token.colorBorderSecondary}`,
                  background: token.colorBgContainer,
                }}
              >
                <Typography.Text strong style={{ display: 'block', marginBottom: 4 }}>
                  {item.title}
                </Typography.Text>
                <Typography.Text type="secondary" style={{ lineHeight: 1.7 }}>
                  {item.description}
                </Typography.Text>
              </div>
            ))}
          </Flex>
        </Card>
      </div>

      <Row gutter={[18, 18]}>
        <Col xs={24} xl={9}>
          <Flex vertical gap={18}>
            <Card
              variant="borderless"
              style={{
                borderRadius: 24,
                background: panelBackground,
                border: panelBorder,
                boxShadow: `0 18px 36px color-mix(in srgb, ${token.colorText} 8%, transparent)`,
              }}
              styles={{ body: { padding: 20 } }}
            >
              <Flex vertical gap={16}>
                <div>
                  <Typography.Text style={{ fontSize: 11, letterSpacing: '0.18em', textTransform: 'uppercase', color: token.colorTextTertiary }}>
                    Project Profile
                  </Typography.Text>
                  <Title level={4} style={{ margin: '8px 0 0', fontFamily: designDisplayFont, letterSpacing: '-0.03em' }}>
                    基础信息
                  </Title>
                </div>
                <Descriptions bordered column={1} styles={{ label: { width: 124, fontWeight: 600 } }}>
                  <Descriptions.Item label="小说名称">{currentProject.title}</Descriptions.Item>
                  {currentProject.description && (
                    <Descriptions.Item label="小说简介">{currentProject.description}</Descriptions.Item>
                  )}
                  <Descriptions.Item label="小说主题">{currentProject.theme || '未设定'}</Descriptions.Item>
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
              </Flex>
            </Card>

            <Card
              variant="borderless"
              style={{
                borderRadius: 24,
                background: quietPanelBackground,
                border: panelBorder,
                boxShadow: `0 18px 36px color-mix(in srgb, ${token.colorText} 8%, transparent)`,
              }}
              styles={{ body: { padding: 20 } }}
            >
              <Flex vertical gap={16}>
                <div>
                  <Typography.Text style={{ fontSize: 11, letterSpacing: '0.18em', textTransform: 'uppercase', color: token.colorTextTertiary }}>
                    Writing Defaults
                  </Typography.Text>
                  <Title level={4} style={{ margin: '8px 0 0', fontFamily: designDisplayFont, letterSpacing: '-0.03em' }}>
                    创作偏好速览
                  </Title>
                </div>
                <Flex vertical gap={12}>
                  {preferenceSummaryItems.map((item) => (
                    <div
                      key={item.label}
                      style={{
                        display: 'flex',
                        justifyContent: 'space-between',
                        gap: 12,
                        padding: '12px 14px',
                        borderRadius: 16,
                        background: 'color-mix(in srgb, var(--ant-color-bg-container) 74%, var(--ant-color-bg-layout) 26%)',
                        border: `1px solid ${token.colorBorderSecondary}`,
                      }}
                    >
                      <Typography.Text type="secondary" style={{ fontSize: 12 }}>{item.label}</Typography.Text>
                      <Typography.Text style={{ fontWeight: 600, textAlign: 'right' }}>{item.value}</Typography.Text>
                    </div>
                  ))}
                </Flex>
                <Typography.Text type="secondary" style={{ fontSize: 12, lineHeight: 1.7 }}>
                  当前质量预设说明：{resolveOptionDescription(QUALITY_PRESET_OPTIONS, currentProject.default_quality_preset) || '未设置额外质量说明。'}
                </Typography.Text>
              </Flex>
            </Card>
          </Flex>
        </Col>

        <Col xs={24} xl={15}>
          <Card
            variant="borderless"
            style={{
              borderRadius: 24,
              background: quietPanelBackground,
              border: panelBorder,
              boxShadow: `0 18px 36px color-mix(in srgb, ${token.colorText} 8%, transparent)`,
            }}
            styles={{ body: { padding: 20 } }}
          >
            <Flex vertical gap={18}>
              <div>
                <Typography.Text style={{ fontSize: 11, letterSpacing: '0.18em', textTransform: 'uppercase', color: token.colorTextTertiary }}>
                  World Notes
                </Typography.Text>
                <Title level={3} style={{ margin: '8px 0 0', fontFamily: designDisplayFont, letterSpacing: '-0.03em' }}>
                  小说世界观
                </Title>
              </div>
              {worldSections.length === 0 ? (
                <Empty description="暂无世界设定信息" style={{ padding: '48px 0 40px' }}>
                  <Paragraph type="secondary" style={{ maxWidth: 520, margin: '8px auto 20px', lineHeight: 1.8 }}>
                    世界设定信息会成为角色、组织与章节生成的共同背景。建议先补齐项目基础信息，然后使用智能重建或手动编辑，建立第一版可迭代的世界观母本。
                  </Paragraph>
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
                  >
                    立即创建世界观
                  </Button>
                </Empty>
              ) : worldSections.map((section) => (
                <Card
                  key={section.key}
                  variant="borderless"
                  style={{
                    borderRadius: 20,
                    background: 'color-mix(in srgb, var(--ant-color-bg-container) 86%, var(--ant-color-bg-layout) 14%)',
                    border: `1px solid color-mix(in srgb, ${section.color} 18%, ${token.colorBorder} 82%)`,
                    boxShadow: `0 12px 28px color-mix(in srgb, ${token.colorText} 5%, transparent)`,
                  }}
                  styles={{ body: { padding: 18 } }}
                >
                  <Flex vertical gap={10}>
                    <div>
                      <Title level={5} style={{ margin: 0, color: section.color }}>
                        {section.title}
                      </Title>
                      <Typography.Text type="secondary" style={{ fontSize: 12 }}>
                        {section.summary}
                      </Typography.Text>
                    </div>
                    <Paragraph style={{ marginBottom: 0, whiteSpace: 'pre-wrap', lineHeight: 1.85, fontSize: 15 }}>
                      {section.content}
                    </Paragraph>
                  </Flex>
                </Card>
              ))}
            </Flex>
          </Card>
        </Col>
      </Row>

      {/* 编辑世界观模态框 */}
      <Modal
        title={(
          <div>
            <Typography.Text style={{ display: 'block', marginBottom: 4, fontSize: 11, letterSpacing: '0.08em', textTransform: 'uppercase', color: token.colorTextTertiary }}>
              World Editor
            </Typography.Text>
            <Typography.Text strong style={{ display: 'block', fontSize: 18 }}>
              编辑世界观
            </Typography.Text>
            <Typography.Text type="secondary" style={{ display: 'block', marginTop: 4, lineHeight: 1.7 }}>
              逐项校对四大维度，把世界观写成一份能长期复用的项目底稿。
            </Typography.Text>
          </div>
        )}
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
        styles={modalSurfaceStyles}
      >
        <Alert
          type="info"
          showIcon
          style={{ borderRadius: 14, marginBottom: 16 }}
          message="编辑建议"
          description="建议用“背景 -> 作用 -> 限制”的方式写每一项，这样后续角色和章节生成更容易复用。"
        />
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
        title={(
          <div>
            <Typography.Text style={{ display: 'block', marginBottom: 4, fontSize: 11, letterSpacing: '0.08em', textTransform: 'uppercase', color: token.colorTextTertiary }}>
              Project Profile
            </Typography.Text>
            <Typography.Text strong style={{ display: 'block', fontSize: 18 }}>
              编辑项目基础信息
            </Typography.Text>
            <Typography.Text type="secondary" style={{ display: 'block', marginTop: 4, lineHeight: 1.7 }}>
              这里维护的是世界观手册的前提信息，包括题材、主题、叙事视角与默认创作偏好。
            </Typography.Text>
          </div>
        )}
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
        styles={modalSurfaceStyles}
      >
        <Alert
          type="info"
          showIcon
          style={{ borderRadius: 14, marginBottom: 16 }}
          message="维护建议"
          description="优先保证简介、主题和默认偏好相互一致，这些信息会持续影响后续的大纲与章节生成。"
        />
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
            title="默认创作偏好"
            style={{ marginBottom: 0, background: 'var(--color-fill-quaternary)' }}
          >
            <Form.Item
              label="默认创作模式"
              name="default_creative_mode"
              extra="控制整体更偏钩子、情绪、悬念、关系或爽点回收"
            >
              <Select
                placeholder="不额外偏置，保持均衡"
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
              label="默认结构侧重点"
              name="default_story_focus"
            >
              <Select
                placeholder="不额外偏置，保持均衡"
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
              label="默认剧情阶段"
              name="default_plot_stage"
            >
              <Select
                placeholder="留空时按具体场景判断"
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
              label="默认创作总控"
              name="default_story_creation_brief"
              extra="用几句话定义这个项目长期遵循的创作重心、推进节奏或核心约束"
            >
              <TextArea
                rows={4}
                placeholder="例如：始终围绕主角的目标、阻力与代价推进，优先保证钩子和回报闭环。"
                showCount
                maxLength={1200}
              />
            </Form.Item>

            <Form.Item
              label="默认质量预设"
              extra="为大纲与章节生成施加统一的质量偏好"
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
                    <Typography.Text strong>不额外施压</Typography.Text>
                    <Typography.Text type="secondary" style={{ fontSize: 12 }}>
                      保持模型默认质量策略，不额外施加统一偏好。
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
                          适合：{option.bestFor}
                        </div>
                        <div style={{ fontSize: 12, color: 'var(--color-text-tertiary)' }}>
                          注意：{option.caution}
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
                    当前预设：{selectedDefaultQualityPresetOption?.label || '不额外施压'}
                  </Typography.Text>
                  <Typography.Text type="secondary" style={{ fontSize: 12 }}>
                    {selectedDefaultQualityPresetOption?.description || '未选择时将沿用模型默认质量策略，不额外施加统一偏好。'}
                  </Typography.Text>
                  {selectedDefaultQualityPresetOption && (
                    <>
                      <Typography.Text type="secondary" style={{ fontSize: 12 }}>
                        适合：{selectedDefaultQualityPresetOption.bestFor}
                      </Typography.Text>
                      <Typography.Text type="secondary" style={{ fontSize: 12 }}>
                        注意：{selectedDefaultQualityPresetOption.caution}
                      </Typography.Text>
                    </>
                  )}
                </Flex>
              </Card>
            </Form.Item>

            <Form.Item
              label="默认额外质量要求"
              name="default_quality_notes"
              extra="补充你长期想保留或压制的写作倾向，例如减少说明句、加强动作反馈等"
            >
              <TextArea
                rows={3}
                placeholder="例如：减少解释性旁白，优先用动作和对话推进信息；章尾必须保留牵引。"
                showCount
                maxLength={600}
              />
            </Form.Item>
          </Card>
        </Form>
      </Modal>

      {/* AI重新生成加载遮罩 */}
      <SSELoadingOverlay
        loading={Boolean(isRegenerating || activeTrackedWorldTask)}
        progress={regenerateProgress}
        message={regenerateMessage}
        blocking={false}
        onCancel={handleCancelRegenerateTask}
        cancelButtonLoading={isCancellingTask}
        cancelButtonDisabled={isCancellingTask || !currentTaskIdRef.current}
      />

      {/* 预览重新生成的内容模态框 */}
      <Modal
        title={(
          <div>
            <Typography.Text style={{ display: 'block', marginBottom: 4, fontSize: 11, letterSpacing: '0.08em', textTransform: 'uppercase', color: token.colorTextTertiary }}>
              Regenerated Draft
            </Typography.Text>
            <Typography.Text strong style={{ display: 'block', fontSize: 18 }}>
              预览重新生成的世界观
            </Typography.Text>
          </div>
        )}
        open={isPreviewModalVisible}
        centered
        width={900}
        onOk={handleConfirmSave}
        onCancel={handleCancelSave}
        confirmLoading={isSavingPreview}
        okText="确认替换"
        cancelText="取消"
        okButtonProps={{ danger: true }}
        styles={modalSurfaceStyles}
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
