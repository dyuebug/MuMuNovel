import { useEffect, useMemo, useRef, useState } from 'react';
import {
  Badge,
  Button,
  Divider,
  Drawer,
  Empty,
  FloatButton,
  Grid,
  List,
  Progress,
  Segmented,
  Space,
  Tag,
  Typography,
  message,
  notification,
} from 'antd';
import {
  CheckCircleOutlined,
  CloseCircleOutlined,
  LoadingOutlined,
  RedoOutlined,
  StopOutlined,
  UnorderedListOutlined,
} from '@ant-design/icons';
import { useLocation, useNavigate } from 'react-router-dom';
import { backgroundTaskApi, chapterApi, chapterBatchTaskApi, chapterSingleTaskApi, getBatchManualReviewInfo } from '../services/modularApi';
import { useStore } from '../store';
import { OPEN_BACKGROUND_TASK_CENTER_EVENT } from '../constants/backgroundTaskEvents';
import {
  getTaskTypeLabel,
  isActiveBackgroundTask,
  useBackgroundTaskStore,
  type TrackedBackgroundTask,
} from '../store/backgroundTasks';
import {
  groupBackgroundTasksByCategory,
  selectActiveBackgroundTasks,
  selectBackgroundTaskSections,
  selectCurrentProjectActiveTaskCount,
  selectFailedBackgroundTaskCount,
  selectRecoverableBackgroundTaskCount,
  selectTerminalBackgroundTaskCount,
  selectVisibleBackgroundTasks,
} from '../store/backgroundTaskSelectors';
import {
  extractFailureReasonTags,
  formatRelativeTime,
  getCompletionNotice,
  getTaskCheckpointSummary,
  getTaskCheckpointTags,
  getTaskDestination,
  getTaskDisplayMessage,
  getTaskStatusMeta,
  isTaskResumable,
  terminalStatuses,
} from './backgroundTaskPresentation';
import { formatActiveStoryRepairLabel } from '../utils/activeStoryRepair';

const { Text } = Typography;
const { useBreakpoint } = Grid;

const statusPriority: Record<TrackedBackgroundTask['status'], number> = {
  running: 0,
  pending: 1,
  failed: 2,
  cancelled: 3,
  completed: 4,
};

const getErrorResponseStatus = (error: unknown): number | null => {
  if (typeof error !== 'object' || error === null || !('response' in error)) {
    return null;
  }

  const response = (error as { response?: unknown }).response;
  if (typeof response !== 'object' || response === null || !('status' in response)) {
    return null;
  }

  const status = (response as { status?: unknown }).status;
  return typeof status === 'number' ? status : null;
};

type TaskSection = {
  key: string;
  title: string;
  description: string;
  tasks: TrackedBackgroundTask[];
  accent?: 'current' | 'global' | 'default';
};

type TaskFilter = 'overview' | 'active' | 'current' | 'failed';

let backgroundTasksApiSupported = true;
let chapterActiveTasksApiSupported = true;
let recoverableTasksSyncPromise: Promise<void> | null = null;

export default function BackgroundTaskCenter() {
  const location = useLocation();
  const navigate = useNavigate();
  const screens = useBreakpoint();
  const isMobile = !screens.md;
  const [open, setOpen] = useState(false);
  const [taskFilter, setTaskFilter] = useState<TaskFilter>('overview');
  const [cancellingTaskIds, setCancellingTaskIds] = useState<Record<string, boolean>>({});
  const [resumingTaskIds, setResumingTaskIds] = useState<Record<string, boolean>>({});

  const currentProject = useStore((state) => state.currentProject);
  const projects = useStore((state) => state.projects);
  const hiddenByRoute = location.pathname === '/login' || location.pathname.startsWith('/auth/callback');
  const knownProjectIds = useMemo(() => new Set(projects.map((project) => project.id)), [projects]);
  const routeProjectId = useMemo(() => {
    const matched = location.pathname.match(/^\/project\/([^/]+)/);
    return matched?.[1] ?? null;
  }, [location.pathname]);
  const rawFocusProjectId = routeProjectId ?? currentProject?.id ?? null;
  const focusProjectId =
    rawFocusProjectId && (knownProjectIds.size === 0 || knownProjectIds.has(rawFocusProjectId))
      ? rawFocusProjectId
      : null;

  const tasksMap = useBackgroundTaskStore((state) => state.tasks);
  const removeTask = useBackgroundTaskStore((state) => state.removeTask);
  const clearTerminalTasks = useBackgroundTaskStore((state) => state.clearTerminalTasks);

  const tasks = useMemo(
    () => selectVisibleBackgroundTasks(tasksMap, knownProjectIds, statusPriority),
    [tasksMap, knownProjectIds]
  );

  const activeTasks = useMemo(() => selectActiveBackgroundTasks(tasks), [tasks]);
  const activeTaskPollKey = useMemo(
    () => activeTasks.map((task) => `${task.taskType}:${task.taskId}`).join('|'),
    [activeTasks]
  );

  const filterOptions = useMemo(
    () => [
      { label: '总览', value: 'overview' },
      { label: '进行中', value: 'active' },
      { label: '失败', value: 'failed' },
      ...(focusProjectId ? [{ label: '当前项目', value: 'current' }] : []),
    ],
    [focusProjectId]
  );

  useEffect(() => {
    if (!focusProjectId && taskFilter === 'current') {
      setTaskFilter('overview');
    }
  }, [focusProjectId, taskFilter]);

  const taskSections = useMemo<TaskSection[]>(
    () => selectBackgroundTaskSections(tasks, focusProjectId, taskFilter),
    [tasks, focusProjectId, taskFilter]
  );

  const summary = useMemo(() => {
    const currentProjectActiveCount = selectCurrentProjectActiveTaskCount(tasks, focusProjectId);
    const terminalTaskCount = selectTerminalBackgroundTaskCount(tasks);
    const otherActiveCount = activeTasks.length - currentProjectActiveCount;
    const failedTaskCount = selectFailedBackgroundTaskCount(tasks);
    const recoverableTaskCount = selectRecoverableBackgroundTaskCount(tasks, isTaskResumable);

    return {
      currentProjectActiveCount,
      terminalTaskCount,
      failedTaskCount,
      recoverableTaskCount,
      otherActiveCount: Math.max(0, otherActiveCount),
    };
  }, [tasks, activeTasks, focusProjectId]);

  const statusSnapshotRef = useRef<Record<string, TrackedBackgroundTask['status']>>({});
  const statusSnapshotReadyRef = useRef(false);
  const recoverableTasksInitializedRef = useRef(false);

  useEffect(() => {
    const handleOpenTaskCenter = () => setOpen(true);
    window.addEventListener(OPEN_BACKGROUND_TASK_CENTER_EVENT, handleOpenTaskCenter);

    return () => {
      window.removeEventListener(OPEN_BACKGROUND_TASK_CENTER_EVENT, handleOpenTaskCenter);
    };
  }, []);

  useEffect(() => {
    if (hiddenByRoute) return;

    let stopped = false;

    const syncRecoverableTasks = async () => {
      if (stopped) return;
      if (recoverableTasksSyncPromise) {
        await recoverableTasksSyncPromise;
        return;
      }

      const backgroundRequest = backgroundTasksApiSupported
        ? backgroundTaskApi.listTasks({ active_only: true, limit: 100 })
          .then((response) => ({ ok: true, items: response.items || [] }))
          .catch((error: unknown) => {
            if (getErrorResponseStatus(error) === 404) {
              backgroundTasksApiSupported = false;
            }
            return { ok: false, items: [] as Array<{ task_id: string }> };
          })
        : Promise.resolve({ ok: false, items: [] as Array<{ task_id: string }> });

      const chapterRequest = chapterActiveTasksApiSupported
        ? chapterBatchTaskApi.listActiveTasks(100)
          .then((response) => ({ ok: true, items: response.items || [] }))
          .catch((error: unknown) => {
            if (getErrorResponseStatus(error) === 404) {
              chapterActiveTasksApiSupported = false;
            }
            return { ok: false, items: [] as Array<{ batch_id: string }> };
          })
        : Promise.resolve({ ok: false, items: [] as Array<{ batch_id: string }> });

      recoverableTasksSyncPromise = (async () => {
        const [backgroundResult, chapterResult] = await Promise.all([backgroundRequest, chapterRequest]);
        if (stopped) return;

        if (backgroundResult.ok || chapterResult.ok) {
          const activeIds = [
            ...backgroundResult.items.map((item) => item.task_id),
            ...chapterResult.items.map((item) => item.batch_id),
          ];
          useBackgroundTaskStore.getState().pruneMissingActiveTasks(activeIds);
        }
      })();

      try {
        await recoverableTasksSyncPromise;
      } finally {
        recoverableTasksSyncPromise = null;
      }
    };

    let initialSyncTimer: number | null = null;

    if (!recoverableTasksInitializedRef.current || open) {
      recoverableTasksInitializedRef.current = true;

      if (!open && activeTasks.length === 0) {
        initialSyncTimer = window.setTimeout(() => {
          if (!stopped) {
            void syncRecoverableTasks();
          }
        }, 2500);
      } else {
        void syncRecoverableTasks();
      }
    }

    if (!open) {
      return () => {
        stopped = true;
        if (initialSyncTimer !== null) {
          window.clearTimeout(initialSyncTimer);
        }
      };
    }

    const timer = window.setInterval(() => {
      void syncRecoverableTasks();
    }, 8000);

    return () => {
      stopped = true;
      window.clearInterval(timer);
    };
  }, [activeTasks.length, hiddenByRoute, open]);

  useEffect(() => {
    if (hiddenByRoute) return;
    if (activeTasks.length === 0) return;

    let stopped = false;
    const handleMissingTask = (taskId: string, error: unknown) => {
      if (getErrorResponseStatus(error) !== 404) return;
      removeTask(taskId);
    };
    const poll = async () => {
      if (stopped) return;
      await Promise.allSettled(
        activeTasks.map((task) => {
          if (task.taskType === 'chapters_batch_generate') {
            return chapterBatchTaskApi
              .getBatchGenerateStatus(task.taskId, task.projectId)
              .catch((error: unknown) => handleMissingTask(task.taskId, error));
          }
          if (task.taskType === 'chapter_single_generate') {
            return chapterSingleTaskApi
              .getSingleGenerateTaskStatus(task.taskId, task.projectId)
              .catch((error: unknown) => handleMissingTask(task.taskId, error));
          }
          if (task.taskType === 'chapter_analysis') {
            const chapterId = typeof task.checkpoint?.chapter_id === 'string'
              ? task.checkpoint.chapter_id
              : undefined;
            if (!chapterId) return Promise.resolve(null);
            return chapterApi
              .getChapterAnalysisStatus(chapterId, task.projectId)
              .catch((error: unknown) => handleMissingTask(task.taskId, error));
          }
          return backgroundTaskApi
            .getTaskStatus(task.taskId)
            .catch((error: unknown) => handleMissingTask(task.taskId, error));
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
  }, [activeTaskPollKey, activeTasks, hiddenByRoute, removeTask]);

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

  const canResumeTask = (task: TrackedBackgroundTask) => isTaskResumable(task);

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

  const resumeAllRecoverableTasks = async () => {
    const recoverableTasks = tasks.filter(canResumeTask).filter((task) => !resumingTaskIds[task.taskId]);
    if (recoverableTasks.length === 0) {
      message.info('暂无可继续的失败任务');
      return;
    }

    setResumingTaskIds((prev) => {
      const next = { ...prev };
      recoverableTasks.forEach((task) => {
        next[task.taskId] = true;
      });
      return next;
    });

    const results = await Promise.allSettled(
      recoverableTasks.map((task) =>
        task.taskType === 'chapters_batch_generate'
          ? chapterBatchTaskApi.resumeBatchGenerateTask(task.taskId, task.projectId)
          : chapterSingleTaskApi.resumeSingleGenerateTask(task.taskId, task.projectId)
      )
    );

    const successCount = results.filter((result) => result.status === 'fulfilled').length;
    const failedCount = results.length - successCount;

    setResumingTaskIds((prev) => {
      const next = { ...prev };
      recoverableTasks.forEach((task) => {
        delete next[task.taskId];
      });
      return next;
    });

    if (successCount > 0) {
      message.success(`已重新排队 ${successCount} 个任务`);
    }
    if (failedCount > 0) {
      message.warning(`${failedCount} 个任务继续失败，请逐个检查`);
    }
  };

  const renderTaskItem = (task: TrackedBackgroundTask, accent: TaskSection['accent']) => {
    const active = isActiveBackgroundTask(task);
    const status = getTaskStatusMeta(task);
    const manualReviewInfo = task.status === 'failed' && (task.taskType === 'chapters_batch_generate' || task.taskType === 'chapter_single_generate')
      ? getBatchManualReviewInfo(
        task.failedChapters,
        task.error,
        task.terminalReason,
        task.terminalLabel,
        task.reviewRequired,
      )
      : null;
    const hasError = task.status === 'failed' && Boolean(task.error || manualReviewInfo?.message);
    const failureReasonTags = task.status === 'failed' ? extractFailureReasonTags(task) : [];
    const checkpointSummary = getTaskCheckpointSummary(task);
    const checkpointTags = getTaskCheckpointTags(task);
    const targetRoute = getTaskDestination(task);

    return (
      <List.Item
        key={task.taskId}
        style={{
          marginBottom: 12,
          border: accent === 'current'
            ? '1px solid rgba(22, 119, 255, 0.25)'
            : accent === 'global'
              ? '1px solid rgba(114, 46, 209, 0.18)'
              : '1px solid var(--color-border-secondary)',
          background: accent === 'current'
            ? 'rgba(22, 119, 255, 0.03)'
            : accent === 'global'
              ? 'rgba(114, 46, 209, 0.03)'
              : '#fff',
          borderRadius: 8,
          padding: 12,
          display: 'block',
        }}
      >
        <Space direction="vertical" size={8} style={{ width: '100%' }}>
          <Space style={{ width: '100%', justifyContent: 'space-between', alignItems: 'flex-start' }}>
            <Space direction="vertical" size={2} style={{ maxWidth: '60%' }}>
              <Text strong>{getTaskTypeLabel(task.taskType)}</Text>
              <Text type="secondary" style={{ fontSize: 12 }}>
                {task.projectId ? `项目任务 · ${formatRelativeTime(task.updatedAt)}` : `全局任务 · ${formatRelativeTime(task.updatedAt)}`}
              </Text>
            </Space>
            <Space size={6} wrap>
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
                ? (manualReviewInfo ? 'normal' : 'exception')
                : task.status === 'completed'
                  ? 'success'
                  : 'active'
            }
          />

          <Text type="secondary" style={{ fontSize: 12 }}>
            {getTaskDisplayMessage(task)}
          </Text>

          {task.workflowScope ? (
            <Text type="secondary" style={{ fontSize: 12 }}>
              范围：{task.workflowScope}
            </Text>
          ) : null}

          {checkpointSummary ? (
            <Text type="secondary" style={{ fontSize: 12 }}>
              {checkpointSummary}
            </Text>
          ) : null}

          {checkpointTags.length > 0 ? (
            <Space size={[6, 6]} wrap>
              {checkpointTags.map((tag) => (
                <Tag key={`${task.taskId}-${tag.label}`} color={tag.color}>
                  {tag.label}
                </Tag>
              ))}
            </Space>
          ) : null}

          {formatActiveStoryRepairLabel(task.activeStoryRepairPayload) ? (
            <Text type="secondary" style={{ fontSize: 12 }}>
              {formatActiveStoryRepairLabel(task.activeStoryRepairPayload)}
            </Text>
          ) : null}

          {hasError ? (
            <Space direction="vertical" size={6} style={{ width: '100%' }}>
              {failureReasonTags.length > 0 ? (
                <Space size={[6, 6]} wrap>
                  {failureReasonTags.map((tag) => (
                    <Tag key={`${task.taskId}-${tag.label}`} color={tag.color}>
                      {tag.label}
                    </Tag>
                  ))}
                </Space>
              ) : null}
              <Text type={manualReviewInfo ? 'warning' : 'danger'} style={{ fontSize: 12 }}>
                {manualReviewInfo?.message ?? task.error}
              </Text>
            </Space>
          ) : null}

          <Space size={8} wrap>
            {targetRoute ? (
              <Button size="small" onClick={() => navigate(targetRoute)}>
                前往
              </Button>
            ) : null}

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
  };

  return (
    <>
      <Badge count={activeTasks.length} size="small" offset={[-2, 8]}>
        <FloatButton
          icon={<UnorderedListOutlined />}
          type={summary.currentProjectActiveCount > 0 ? 'primary' : activeTasks.length > 0 ? 'default' : 'default'}
          tooltip={
            summary.currentProjectActiveCount > 0
              ? `当前项目后台任务 (${summary.currentProjectActiveCount})`
              : activeTasks.length > 0
                ? `后台任务进行中 (${activeTasks.length})`
                : '后台任务'
          }
          onClick={() => setOpen(true)}
          style={{
            right: 24,
            bottom: 24,
            zIndex: 10001,
          }}
        />
      </Badge>

      <Drawer
        title={focusProjectId ? `后台任务 · 当前项目优先 (${tasks.length})` : `后台任务 (${tasks.length})`}
        placement="right"
        open={open}
        onClose={() => setOpen(false)}
        width={isMobile ? '100vw' : 440}
        extra={
          <Space size={8}>
            <Button
              size="small"
              type="primary"
              onClick={() => void resumeAllRecoverableTasks()}
              disabled={summary.recoverableTaskCount === 0}
            >
              重试可恢复任务
            </Button>
            <Button size="small" onClick={clearTerminalTasks} disabled={activeTasks.length === tasks.length}>
              清理已结束
            </Button>
          </Space>
        }
      >
        <Space direction="vertical" size={12} style={{ width: '100%', marginBottom: 16 }}>
          <Space wrap>
            <Tag color="processing">进行中 {activeTasks.length}</Tag>
            <Tag color="blue">当前项目 {summary.currentProjectActiveCount}</Tag>
            {summary.failedTaskCount > 0 ? <Tag color="error">失败 {summary.failedTaskCount}</Tag> : null}
            {summary.otherActiveCount > 0 ? <Tag>其他项目 {summary.otherActiveCount}</Tag> : null}
            {summary.terminalTaskCount > 0 ? <Tag color="default">已结束 {summary.terminalTaskCount}</Tag> : null}
          </Space>
          <Segmented
            block
            size="small"
            value={taskFilter}
            onChange={(value) => setTaskFilter(value as TaskFilter)}
            options={filterOptions}
          />
          {focusProjectId ? (
            <Text type="secondary" style={{ fontSize: 12 }}>
              {taskFilter === 'current'
                ? '仅展示当前项目任务，方便专注处理本项目。'
                : taskFilter === 'active'
                  ? '仅展示仍在排队或执行中的任务。'
                  : taskFilter === 'failed'
                    ? '仅展示失败任务，便于集中排查和恢复。'
                  : '当前项目任务会优先显示，避免在多项目并行时被其他任务淹没。'}
            </Text>
          ) : (
            <Text type="secondary" style={{ fontSize: 12 }}>
              {taskFilter === 'active'
                ? '当前视图仅保留进行中的后台任务。'
                : taskFilter === 'failed'
                  ? '当前视图仅保留失败任务。'
                : '这里汇总所有后台任务；进入项目页后会自动优先展示当前项目任务。'}
            </Text>
          )}
        </Space>

        {taskSections.length === 0 ? (
          <Empty description="暂无后台任务" />
        ) : (
          taskSections.map((section, index) => (
            <div key={section.key} style={{ marginBottom: 8 }}>
              {index > 0 ? <Divider style={{ margin: '12px 0' }} /> : null}
              <Space direction="vertical" size={4} style={{ width: '100%', marginBottom: 8 }}>
                <Space style={{ width: '100%', justifyContent: 'space-between' }}>
                  <Text strong>{section.title}</Text>
                  <Tag>{section.tasks.length}</Tag>
                </Space>
                <Text type="secondary" style={{ fontSize: 12 }}>
                  {section.description}
                </Text>
              </Space>

              {section.tasks.length === 0 ? (
                <Empty image={Empty.PRESENTED_IMAGE_SIMPLE} description="暂无任务" />
              ) : (
                (() => {
                  const groups = groupBackgroundTasksByCategory(section.tasks);
                  return groups.map((group) => (
                    <div key={`${section.key}-${group.key}`} style={{ marginBottom: 12 }}>
                      {groups.length > 1 ? (
                        <div style={{ marginBottom: 8 }}>
                          <Text type="secondary" style={{ fontSize: 12, fontWeight: 600 }}>
                            {group.title}
                          </Text>
                        </div>
                      ) : null}
                      <List
                        dataSource={group.tasks}
                        rowKey={(task) => task.taskId}
                        split={false}
                        renderItem={(task) => renderTaskItem(task, section.accent)}
                      />
                    </div>
                  ));
                })()
              )}
            </div>
          ))
        )}
      </Drawer>
    </>
  );
}
