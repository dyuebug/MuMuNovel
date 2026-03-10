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
import { backgroundTaskApi, chapterApi, chapterBatchTaskApi, chapterSingleTaskApi } from '../services/api';
import { useStore } from '../store';
import { OPEN_BACKGROUND_TASK_CENTER_EVENT } from '../constants/backgroundTaskEvents';
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

const statusPriority: Record<TrackedBackgroundTask['status'], number> = {
  running: 0,
  pending: 1,
  failed: 2,
  cancelled: 3,
  completed: 4,
};

type TaskSection = {
  key: string;
  title: string;
  description: string;
  tasks: TrackedBackgroundTask[];
  accent?: 'current' | 'global' | 'default';
};

type TaskFilter = 'overview' | 'active' | 'current' | 'failed';

type TaskGroup = {
  key: string;
  title: string;
  tasks: TrackedBackgroundTask[];
};

type FailureReasonTag = {
  label: string;
  color: string;
};

let backgroundTasksApiSupported = true;
let chapterActiveTasksApiSupported = true;
let recoverableTasksSyncPromise: Promise<void> | null = null;

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

const getTaskDisplayMessage = (task: TrackedBackgroundTask): string => {
  if (task.taskType !== 'chapter_analysis') {
    return task.message || '任务执行中...';
  }

  if (task.status === 'completed') return '章节分析已完成';
  if (task.status === 'failed') return task.error || '章节分析失败';
  if (task.status === 'cancelled') return '章节分析已取消';
  return `章节分析进行中 (${task.progress}%)`;
};

const getTaskCategory = (taskType: string): TaskGroup['key'] => {
  if (taskType.startsWith('chapter_') || taskType === 'chapters_batch_generate') return 'chapter';
  if (taskType.startsWith('outline_')) return 'outline';
  if (taskType === 'world_regenerate' || taskType === 'wizard_world_building') return 'world';
  if (taskType === 'character_generate' || taskType === 'wizard_characters') return 'character';
  if (taskType === 'careers_generate_system' || taskType === 'wizard_career_system') return 'career';
  if (taskType === 'organization_generate') return 'organization';
  if (taskType.startsWith('wizard_')) return 'wizard';
  return 'other';
};

const getTaskCategoryLabel = (category: TaskGroup['key']): string => {
  const labels: Record<TaskGroup['key'], string> = {
    chapter: '章节相关',
    outline: '大纲相关',
    world: '世界观相关',
    character: '角色相关',
    career: '职业体系',
    organization: '组织势力',
    wizard: '向导流程',
    other: '其他任务',
  };
  return labels[category] ?? '其他任务';
};

const groupTasksByCategory = (tasks: TrackedBackgroundTask[]): TaskGroup[] => {
  const grouped = new Map<TaskGroup['key'], TrackedBackgroundTask[]>();

  tasks.forEach((task) => {
    const category = getTaskCategory(task.taskType);
    const existing = grouped.get(category) ?? [];
    existing.push(task);
    grouped.set(category, existing);
  });

  const order: TaskGroup['key'][] = ['chapter', 'outline', 'world', 'character', 'career', 'organization', 'wizard', 'other'];

  return order
    .map((key) => ({ key, title: getTaskCategoryLabel(key), tasks: grouped.get(key) ?? [] }))
    .filter((group) => group.tasks.length > 0);
};

const extractFailureReasonTags = (task: TrackedBackgroundTask): FailureReasonTag[] => {
  const source = `${task.error ?? ''} ${task.message ?? ''}`.toLowerCase();
  const tags: FailureReasonTag[] = [];

  const pushTag = (label: string, color: string) => {
    if (!tags.some((tag) => tag.label === label)) {
      tags.push({ label, color });
    }
  };

  if (!source.trim()) {
    return [{ label: '未知原因', color: 'default' }];
  }

  if (
    source.includes('timeout') ||
    source.includes('time out') ||
    source.includes('timed out') ||
    source.includes('超时') ||
    source.includes('deadline exceeded')
  ) {
    pushTag('超时', 'gold');
  }

  if (
    source.includes('401') ||
    source.includes('403') ||
    source.includes('unauthorized') ||
    source.includes('forbidden') ||
    source.includes('permission') ||
    source.includes('权限') ||
    source.includes('认证') ||
    source.includes('token') ||
    source.includes('apikey') ||
    source.includes('api key')
  ) {
    pushTag('权限错误', 'red');
  }

  if (
    source.includes('429') ||
    source.includes('rate limit') ||
    source.includes('quota') ||
    source.includes('配额') ||
    source.includes('限流') ||
    source.includes('余额不足') ||
    source.includes('too many requests') ||
    source.includes('insufficient_quota')
  ) {
    pushTag('限流/配额', 'volcano');
  }

  if (
    source.includes('network') ||
    source.includes('socket') ||
    source.includes('connection') ||
    source.includes('connect') ||
    source.includes('econn') ||
    source.includes('dns') ||
    source.includes('网络') ||
    source.includes('连接')
  ) {
    pushTag('网络异常', 'cyan');
  }

  if (
    source.includes('model') ||
    source.includes('模型') ||
    source.includes('provider') ||
    source.includes('completion') ||
    source.includes('llm')
  ) {
    pushTag('模型错误', 'purple');
  }

  if (
    source.includes('context length') ||
    source.includes('maximum context') ||
    source.includes('too long') ||
    source.includes('length') ||
    source.includes('上下文') ||
    source.includes('长度超限') ||
    source.includes('token limit')
  ) {
    pushTag('上下文过长', 'magenta');
  }

  if (
    source.includes('invalid') ||
    source.includes('validation') ||
    source.includes('missing') ||
    source.includes('required') ||
    source.includes('参数') ||
    source.includes('格式') ||
    source.includes('校验')
  ) {
    pushTag('参数问题', 'orange');
  }

  if (tags.length === 0) {
    pushTag('未知原因', 'default');
  }

  return tags.slice(0, 2);
};

const formatRelativeTime = (timestamp: number): string => {
  const diff = Date.now() - timestamp;
  if (diff < 60_000) return '刚刚更新';
  if (diff < 3_600_000) return `${Math.max(1, Math.floor(diff / 60_000))} 分钟前更新`;
  if (diff < 86_400_000) return `${Math.floor(diff / 3_600_000)} 小时前更新`;
  return `${Math.floor(diff / 86_400_000)} 天前更新`;
};

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
  const hiddenByRoute = location.pathname === '/login' || location.pathname.startsWith('/auth/callback');
  const routeProjectId = useMemo(() => {
    const matched = location.pathname.match(/^\/project\/([^/]+)/);
    return matched?.[1] ?? null;
  }, [location.pathname]);
  const focusProjectId = routeProjectId ?? currentProject?.id ?? null;

  const tasksMap = useBackgroundTaskStore((state) => state.tasks);
  const removeTask = useBackgroundTaskStore((state) => state.removeTask);
  const clearTerminalTasks = useBackgroundTaskStore((state) => state.clearTerminalTasks);

  const tasks = useMemo(
    () =>
      Object.values(tasksMap).sort((a, b) => {
        const statusDelta = statusPriority[a.status] - statusPriority[b.status];
        if (statusDelta !== 0) return statusDelta;
        return b.updatedAt - a.updatedAt;
      }),
    [tasksMap]
  );

  const activeTasks = useMemo(() => tasks.filter(isActiveBackgroundTask), [tasks]);
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

  const taskSections = useMemo<TaskSection[]>(() => {
    const currentActive: TrackedBackgroundTask[] = [];
    const currentRecent: TrackedBackgroundTask[] = [];
    const globalTasks: TrackedBackgroundTask[] = [];
    const otherActive: TrackedBackgroundTask[] = [];
    const otherRecent: TrackedBackgroundTask[] = [];

    tasks.forEach((task) => {
      const active = isActiveBackgroundTask(task);

      if (focusProjectId && task.projectId === focusProjectId) {
        if (active) {
          currentActive.push(task);
        } else {
          currentRecent.push(task);
        }
        return;
      }

      if (!task.projectId) {
        globalTasks.push(task);
        return;
      }

      if (active) {
        otherActive.push(task);
      } else {
        otherRecent.push(task);
      }
    });

    const sections: TaskSection[] = [];

    if (focusProjectId) {
      sections.push({
        key: 'current-active',
        title: '当前项目进行中',
        description: currentActive.length > 0 ? '优先展示与你当前页面相关的任务' : '当前项目暂无进行中的后台任务',
        tasks: currentActive,
        accent: 'current',
      });
      sections.push({
        key: 'current-recent',
        title: '当前项目近期任务',
        description: currentRecent.length > 0 ? '方便直接回看刚完成或失败的任务' : '当前项目暂无近期任务',
        tasks: currentRecent,
        accent: 'current',
      });
    } else {
      sections.push({
        key: 'active',
        title: '进行中的任务',
        description: activeTasks.length > 0 ? '所有仍在排队或执行中的任务' : '暂无进行中的后台任务',
        tasks: activeTasks,
      });
      sections.push({
        key: 'recent',
        title: '近期任务',
        description: tasks.filter((task) => !isActiveBackgroundTask(task)).length > 0 ? '最近结束的后台任务记录' : '暂无近期任务',
        tasks: tasks.filter((task) => !isActiveBackgroundTask(task)),
      });
    }

    if (globalTasks.length > 0) {
      sections.push({
        key: 'global',
        title: '全局任务',
        description: '未绑定到具体项目的向导或系统任务',
        tasks: globalTasks,
        accent: 'global',
      });
    }

    if (otherActive.length > 0) {
      sections.push({
        key: 'other-active',
        title: '其他项目进行中',
        description: '这些任务不属于当前项目，但仍在运行',
        tasks: otherActive,
      });
    }

    if (otherRecent.length > 0) {
      sections.push({
        key: 'other-recent',
        title: '其他项目近期任务',
        description: '保留最近结束的其他项目任务，便于追踪',
        tasks: otherRecent,
      });
    }

    let filteredSections = sections;

    if (taskFilter === 'active') {
      filteredSections = sections
        .map((section) => ({
          ...section,
          tasks: section.tasks.filter(isActiveBackgroundTask),
        }))
        .filter((section) => section.tasks.length > 0);
    } else if (taskFilter === 'failed') {
      filteredSections = sections
        .map((section) => ({
          ...section,
          tasks: section.tasks.filter((task) => task.status === 'failed'),
        }))
        .filter((section) => section.tasks.length > 0);
    } else if (taskFilter === 'current' && focusProjectId) {
      filteredSections = sections.filter((section) => section.key.startsWith('current-'));
    }

    return filteredSections.filter(
      (section) =>
        section.tasks.length > 0 ||
        (taskFilter !== 'active' && taskFilter !== 'failed' && (section.key === 'current-active' || section.key === 'current-recent' || section.key === 'active' || section.key === 'recent'))
    );
  }, [tasks, focusProjectId, activeTasks, taskFilter]);

  const summary = useMemo(() => {
    const currentProjectActiveCount = focusProjectId
      ? tasks.filter((task) => task.projectId === focusProjectId && isActiveBackgroundTask(task)).length
      : activeTasks.length;
    const terminalTaskCount = tasks.filter((task) => terminalStatuses.has(task.status)).length;
    const otherActiveCount = activeTasks.length - currentProjectActiveCount;
    const failedTaskCount = tasks.filter((task) => task.status === 'failed').length;
    const recoverableTaskCount = tasks.filter(
      (task) =>
        (task.taskType === 'chapters_batch_generate' || task.taskType === 'chapter_single_generate') &&
        (task.status === 'failed' || task.status === 'cancelled')
    ).length;

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

      const requests: Promise<unknown>[] = [];

      if (backgroundTasksApiSupported) {
        requests.push(
          backgroundTaskApi.listTasks({ active_only: true, limit: 50 }).catch((error: any) => {
            if (error?.response?.status === 404) {
              backgroundTasksApiSupported = false;
            }
            return null;
          })
        );
      }

      if (chapterActiveTasksApiSupported) {
        requests.push(
          chapterBatchTaskApi.listActiveTasks(50).catch((error: any) => {
            if (error?.response?.status === 404) {
              chapterActiveTasksApiSupported = false;
            }
            return null;
          })
        );
      }

      if (requests.length === 0) {
        return;
      }

      recoverableTasksSyncPromise = (async () => {
        await Promise.allSettled(requests);
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
    const status = statusMeta[task.status];
    const hasError = task.status === 'failed' && task.error;
    const failureReasonTags = task.status === 'failed' ? extractFailureReasonTags(task) : [];
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
                ? 'exception'
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

          {typeof task.checkpoint?.current_chapter_number === 'number' ? (
            <Text type="secondary" style={{ fontSize: 12 }}>
              检查点：当前第 {task.checkpoint.current_chapter_number} 章
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
              <Text type="danger" style={{ fontSize: 12 }}>
                {task.error}
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
                  const groups = groupTasksByCategory(section.tasks);
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
