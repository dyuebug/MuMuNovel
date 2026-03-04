import { useEffect, useMemo, useRef, useState } from 'react';
import { Badge, Button, Drawer, FloatButton, Grid, List, Progress, Space, Tag, Typography, message, notification } from 'antd';
import {
  CheckCircleOutlined,
  CloseCircleOutlined,
  LoadingOutlined,
  RedoOutlined,
  StopOutlined,
  UnorderedListOutlined,
} from '@ant-design/icons';
import { useLocation, useNavigate } from 'react-router-dom';
import { backgroundTaskApi, chapterApi, chapterBatchTaskApi, chapterSingleTaskApi } from '../services/api';
import {
  getTaskTypeLabel,
  isActiveBackgroundTask,
  useBackgroundTaskStore,
  type TrackedBackgroundTask,
} from '../store/backgroundTasks';

const { Text } = Typography;
const { useBreakpoint } = Grid;

const statusMeta: Record<TrackedBackgroundTask['status'], { color: string; label: string }> = {
  pending: { color: 'default', label: '排队中' },
  running: { color: 'processing', label: '执行中' },
  completed: { color: 'success', label: '已完成' },
  failed: { color: 'error', label: '失败' },
  cancelled: { color: 'warning', label: '已取消' },
};

const terminalStatuses = new Set<TrackedBackgroundTask['status']>(['completed', 'failed', 'cancelled']);

const getTaskDestination = (task: TrackedBackgroundTask): string | null => {
  if (!task.projectId) {
    if (task.taskType.startsWith('wizard_')) return '/wizard';
    return null;
  }

  switch (task.taskType) {
    case 'careers_generate_system':
    case 'wizard_career_system':
      return `/project/${task.projectId}/careers`;
    case 'character_generate':
    case 'wizard_characters':
      return `/project/${task.projectId}/characters`;
    case 'organization_generate':
      return `/project/${task.projectId}/organizations`;
    case 'world_regenerate':
    case 'wizard_world_building':
      return `/project/${task.projectId}/world-setting`;
    case 'outline_generate':
    case 'outline_expand':
    case 'outline_batch_expand':
    case 'wizard_outline':
      return `/project/${task.projectId}/outline`;
    case 'chapters_batch_generate':
    case 'chapter_single_generate':
    case 'chapter_analysis':
      return `/project/${task.projectId}/chapters`;
    default:
      return `/project/${task.projectId}`;
  }
};

const getCompletionNotice = (task: TrackedBackgroundTask): { title: string; description: string } => {
  const taskLabel = getTaskTypeLabel(task.taskType);
  if (task.status === 'completed') {
    return {
      title: `${taskLabel}已完成`,
      description: task.message || '后台任务执行完成',
    };
  }
  if (task.status === 'failed') {
    return {
      title: `${taskLabel}执行失败`,
      description: task.error || task.message || '后台任务执行失败',
    };
  }
  return {
    title: `${taskLabel}已取消`,
    description: task.message || '后台任务已取消',
  };
};

export default function BackgroundTaskCenter() {
  const location = useLocation();
  const navigate = useNavigate();
  const screens = useBreakpoint();
  const isMobile = !screens.md;
  const [open, setOpen] = useState(false);
  const [cancellingTaskIds, setCancellingTaskIds] = useState<Record<string, boolean>>({});
  const [resumingTaskIds, setResumingTaskIds] = useState<Record<string, boolean>>({});

  const hiddenByRoute = location.pathname === '/login' || location.pathname.startsWith('/auth/callback');

  const tasksMap = useBackgroundTaskStore((state) => state.tasks);
  const removeTask = useBackgroundTaskStore((state) => state.removeTask);
  const clearTerminalTasks = useBackgroundTaskStore((state) => state.clearTerminalTasks);

  const tasks = useMemo(
    () =>
      Object.values(tasksMap).sort((a, b) => {
        const aActive = isActiveBackgroundTask(a) ? 1 : 0;
        const bActive = isActiveBackgroundTask(b) ? 1 : 0;
        if (aActive !== bActive) return bActive - aActive;
        return b.updatedAt - a.updatedAt;
      }),
    [tasksMap]
  );

  const activeTasks = useMemo(() => tasks.filter(isActiveBackgroundTask), [tasks]);
  const activeTaskPollKey = useMemo(
    () => activeTasks.map((task) => `${task.taskType}:${task.taskId}`).join('|'),
    [activeTasks]
  );

  const statusSnapshotRef = useRef<Record<string, TrackedBackgroundTask['status']>>({});
  const statusSnapshotReadyRef = useRef(false);

  useEffect(() => {
    if (hiddenByRoute) return;

    let stopped = false;

    const syncRecoverableTasks = async () => {
      if (stopped) return;
      await Promise.allSettled([
        backgroundTaskApi.listTasks({ active_only: true, limit: 50 }),
        chapterBatchTaskApi.listActiveTasks(50),
      ]);
    };

    void syncRecoverableTasks();
    const timer = window.setInterval(() => {
      void syncRecoverableTasks();
    }, 8000);

    return () => {
      stopped = true;
      window.clearInterval(timer);
    };
  }, [hiddenByRoute]);

  useEffect(() => {
    if (hiddenByRoute) return;
    if (activeTasks.length === 0) return;

    let stopped = false;
    const poll = async () => {
      if (stopped) return;
      await Promise.allSettled(
        activeTasks.map((task) => {
          if (task.taskType === 'chapters_batch_generate') {
            return chapterBatchTaskApi.getBatchGenerateStatus(task.taskId, task.projectId);
          }
          if (task.taskType === 'chapter_single_generate') {
            return chapterSingleTaskApi.getSingleGenerateTaskStatus(task.taskId, task.projectId);
          }
          if (task.taskType === 'chapter_analysis') {
            const chapterId = typeof task.checkpoint?.chapter_id === 'string'
              ? task.checkpoint.chapter_id
              : undefined;
            if (!chapterId) return Promise.resolve(null);
            return chapterApi.getChapterAnalysisStatus(chapterId, task.projectId);
          }
          return backgroundTaskApi.getTaskStatus(task.taskId);
        })
      );
    };

    void poll();
    const timer = window.setInterval(() => {
      void poll();
    }, 2000);

    return () => {
      stopped = true;
      window.clearInterval(timer);
    };
  }, [activeTaskPollKey, activeTasks, hiddenByRoute]);

  useEffect(() => {
    const currentSnapshot = Object.fromEntries(tasks.map((task) => [task.taskId, task.status]));

    if (!statusSnapshotReadyRef.current) {
      statusSnapshotRef.current = currentSnapshot;
      statusSnapshotReadyRef.current = true;
      return;
    }

    for (const task of tasks) {
      const previousStatus = statusSnapshotRef.current[task.taskId];
      if (!previousStatus || previousStatus === task.status || !terminalStatuses.has(task.status)) {
        continue;
      }

      const notice = getCompletionNotice(task);
      const targetRoute = getTaskDestination(task);
      const notificationKey = `task-result-${task.taskId}-${task.status}`;

      notification.open({
        key: notificationKey,
        message: notice.title,
        description: notice.description,
        duration: 6,
        btn: targetRoute ? (
          <Button
            type="link"
            size="small"
            onClick={() => {
              notification.destroy(notificationKey);
              navigate(targetRoute);
            }}
          >
            查看详情
          </Button>
        ) : undefined,
      });
    }

    statusSnapshotRef.current = currentSnapshot;
  }, [tasks, navigate]);

  if (hiddenByRoute || tasks.length === 0) {
    return null;
  }

  const cancelTask = async (task: TrackedBackgroundTask) => {
    const taskId = task.taskId;
    if (cancellingTaskIds[taskId]) return;

    setCancellingTaskIds((prev) => ({ ...prev, [taskId]: true }));
    try {
      if (task.taskType === 'chapters_batch_generate') {
        await chapterBatchTaskApi.cancelBatchGenerateTask(taskId, task.projectId);
      } else if (task.taskType === 'chapter_single_generate') {
        await chapterSingleTaskApi.cancelSingleGenerateTask(taskId, task.projectId);
      } else {
        await backgroundTaskApi.cancelTask(taskId);
      }
      message.info('正在取消后台任务...');
    } catch (error) {
      const err = error as Error;
      message.error(err.message || '取消后台任务失败');
    } finally {
      setCancellingTaskIds((prev) => {
        const next = { ...prev };
        delete next[taskId];
        return next;
      });
    }
  };

  const canResumeTask = (task: TrackedBackgroundTask) =>
    (task.taskType === 'chapters_batch_generate' || task.taskType === 'chapter_single_generate') &&
    (task.status === 'failed' || task.status === 'cancelled');

  const canCancelTask = (task: TrackedBackgroundTask) =>
    task.taskType !== 'chapter_analysis';

  const resumeTask = async (task: TrackedBackgroundTask) => {
    const taskId = task.taskId;
    if (resumingTaskIds[taskId]) return;
    if (!canResumeTask(task)) return;

    setResumingTaskIds((prev) => ({ ...prev, [taskId]: true }));
    try {
      if (task.taskType === 'chapters_batch_generate') {
        await chapterBatchTaskApi.resumeBatchGenerateTask(taskId, task.projectId);
      } else {
        await chapterSingleTaskApi.resumeSingleGenerateTask(taskId, task.projectId);
      }
      message.success('已创建继续任务，正在排队执行');
    } catch (error) {
      const err = error as Error;
      message.error(err.message || '继续任务失败');
    } finally {
      setResumingTaskIds((prev) => {
        const next = { ...prev };
        delete next[taskId];
        return next;
      });
    }
  };

  return (
    <>
      <Badge count={activeTasks.length} size="small" offset={[-2, 8]}>
        <FloatButton
          icon={<UnorderedListOutlined />}
          type={activeTasks.length > 0 ? 'primary' : 'default'}
          tooltip={activeTasks.length > 0 ? `后台任务进行中 (${activeTasks.length})` : '后台任务'}
          onClick={() => setOpen(true)}
          style={{
            right: 24,
            bottom: 24,
            zIndex: 10001,
          }}
        />
      </Badge>

      <Drawer
        title={`后台任务 (${tasks.length})`}
        placement="right"
        open={open}
        onClose={() => setOpen(false)}
        width={isMobile ? '100vw' : 420}
        extra={
          <Button size="small" onClick={clearTerminalTasks} disabled={activeTasks.length === tasks.length}>
            清理已结束
          </Button>
        }
      >
        <List
          dataSource={tasks}
          rowKey={(task) => task.taskId}
          split={false}
          renderItem={(task) => {
            const active = isActiveBackgroundTask(task);
            const status = statusMeta[task.status];
            const hasError = task.status === 'failed' && task.error;

            return (
              <List.Item
                style={{
                  marginBottom: 12,
                  border: '1px solid var(--color-border-secondary)',
                  borderRadius: 8,
                  padding: 12,
                  display: 'block',
                }}
              >
                <Space direction="vertical" size={8} style={{ width: '100%' }}>
                  <Space style={{ width: '100%', justifyContent: 'space-between' }}>
                    <Text strong>{getTaskTypeLabel(task.taskType)}</Text>
                    <Space size={6}>
                      {task.executionMode === 'auto' ? <Tag color="geekblue">全自动</Tag> : <Tag>交互</Tag>}
                      {task.stageCode ? <Tag color="purple">{task.stageCode}</Tag> : null}
                      <Tag color={status.color}>{status.label}</Tag>
                    </Space>
                  </Space>

                  <Progress
                    percent={task.progress}
                    size="small"
                    status={
                      task.status === 'failed'
                        ? 'exception'
                        : task.status === 'completed'
                          ? 'success'
                          : 'active'
                    }
                  />

                  <Text type="secondary" style={{ fontSize: 12 }}>
                    {task.message || '任务执行中...'}
                  </Text>

                  {task.workflowScope ? (
                    <Text type="secondary" style={{ fontSize: 12 }}>
                      范围：{task.workflowScope}
                    </Text>
                  ) : null}

                  {typeof task.checkpoint?.current_chapter_number === 'number' ? (
                    <Text type="secondary" style={{ fontSize: 12 }}>
                      检查点：当前第 {task.checkpoint.current_chapter_number} 章
                    </Text>
                  ) : null}

                  {hasError ? (
                    <Text type="danger" style={{ fontSize: 12 }}>
                      {task.error}
                    </Text>
                  ) : null}

                  <Space size={8}>
                    {active ? (
                      canCancelTask(task) ? (
                        <Button
                          size="small"
                          danger
                          icon={cancellingTaskIds[task.taskId] ? <LoadingOutlined /> : <StopOutlined />}
                          loading={Boolean(cancellingTaskIds[task.taskId])}
                          onClick={() => void cancelTask(task)}
                        >
                          取消
                        </Button>
                      ) : (
                        <Button
                          size="small"
                          icon={<CloseCircleOutlined />}
                          onClick={() => removeTask(task.taskId)}
                        >
                          移除
                        </Button>
                      )
                    ) : (
                      <>
                        {canResumeTask(task) ? (
                          <Button
                            size="small"
                            type="primary"
                            icon={resumingTaskIds[task.taskId] ? <LoadingOutlined /> : <RedoOutlined />}
                            loading={Boolean(resumingTaskIds[task.taskId])}
                            onClick={() => void resumeTask(task)}
                          >
                            继续
                          </Button>
                        ) : null}
                        <Button
                          size="small"
                          icon={task.status === 'completed' ? <CheckCircleOutlined /> : <CloseCircleOutlined />}
                          onClick={() => removeTask(task.taskId)}
                        >
                          移除
                        </Button>
                      </>
                    )}
                  </Space>
                </Space>
              </List.Item>
            );
          }}
        />
      </Drawer>
    </>
  );
}
