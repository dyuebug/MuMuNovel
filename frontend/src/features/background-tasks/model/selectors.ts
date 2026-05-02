import type { BackgroundTaskRuntimeStatus } from '../../../services/modules/backgroundTaskTypes';
import { isActiveBackgroundTask, type TrackedBackgroundTask } from '../../../store/backgroundTasks';

export type BackgroundTaskStatusPriority = Record<BackgroundTaskRuntimeStatus, number>;

export const selectBackgroundTaskList = (tasksMap: Record<string, TrackedBackgroundTask>): TrackedBackgroundTask[] =>
  Object.values(tasksMap);

export const selectActiveBackgroundTasks = (tasks: TrackedBackgroundTask[]): TrackedBackgroundTask[] =>
  tasks.filter(isActiveBackgroundTask);

export const selectVisibleBackgroundTasks = (
  tasksMap: Record<string, TrackedBackgroundTask>,
  knownProjectIds: Set<string>,
  statusPriority: BackgroundTaskStatusPriority,
): TrackedBackgroundTask[] => {
  const allTasks = selectBackgroundTaskList(tasksMap);
  const filtered = knownProjectIds.size > 0
    ? allTasks.filter((task) => !task.projectId || knownProjectIds.has(task.projectId))
    : allTasks;

  return [...filtered].sort((a, b) => {
    const statusDelta = statusPriority[a.status] - statusPriority[b.status];
    if (statusDelta !== 0) return statusDelta;
    return b.updatedAt - a.updatedAt;
  });
};

export const selectCurrentProjectActiveTaskCount = (
  tasks: TrackedBackgroundTask[],
  focusProjectId: string | null,
): number =>
  focusProjectId
    ? tasks.filter((task) => task.projectId === focusProjectId && isActiveBackgroundTask(task)).length
    : selectActiveBackgroundTasks(tasks).length;

export const selectTerminalBackgroundTaskCount = (tasks: TrackedBackgroundTask[]): number =>
  tasks.filter((task) => task.status === 'completed' || task.status === 'failed' || task.status === 'cancelled').length;

export const selectFailedBackgroundTaskCount = (tasks: TrackedBackgroundTask[]): number =>
  tasks.filter((task) => task.status === 'failed').length;

export const selectRecoverableBackgroundTaskCount = (
  tasks: TrackedBackgroundTask[],
  isTaskResumable: (task: TrackedBackgroundTask) => boolean,
): number => tasks.filter(isTaskResumable).length;

type BackgroundTaskGroupKey =
  | 'chapter'
  | 'outline'
  | 'world'
  | 'character'
  | 'career'
  | 'organization'
  | 'wizard'
  | 'other';

const getBackgroundTaskCategory = (taskType: string): BackgroundTaskGroupKey => {
  if (taskType.startsWith('chapter_') || taskType === 'chapters_batch_generate') return 'chapter';
  if (taskType.startsWith('outline_')) return 'outline';
  if (taskType === 'world_regenerate' || taskType === 'wizard_world_building') return 'world';
  if (taskType === 'character_generate' || taskType === 'wizard_characters') return 'character';
  if (taskType === 'careers_generate_system' || taskType === 'wizard_career_system') return 'career';
  if (taskType === 'organization_generate') return 'organization';
  if (taskType.startsWith('wizard_')) return 'wizard';
  return 'other';
};

const getBackgroundTaskCategoryLabel = (category: BackgroundTaskGroupKey): string => {
  const labels: Record<BackgroundTaskGroupKey, string> = {
    chapter: 'Chapters',
    outline: 'Outlines',
    world: 'World',
    character: 'Characters',
    career: 'Careers',
    organization: 'Organizations',
    wizard: 'Wizard',
    other: 'Other',
  };
  return labels[category] ?? 'Other';
};

export const groupBackgroundTasksByCategory = (
  tasks: TrackedBackgroundTask[],
): Array<{ key: string; title: string; tasks: TrackedBackgroundTask[] }> => {
  const grouped = new Map<BackgroundTaskGroupKey, TrackedBackgroundTask[]>();

  tasks.forEach((task) => {
    const category = getBackgroundTaskCategory(task.taskType);
    const existing = grouped.get(category) ?? [];
    existing.push(task);
    grouped.set(category, existing);
  });

  const order: BackgroundTaskGroupKey[] = ['chapter', 'outline', 'world', 'character', 'career', 'organization', 'wizard', 'other'];

  return order
    .map((key) => ({ key, title: getBackgroundTaskCategoryLabel(key), tasks: grouped.get(key) ?? [] }))
    .filter((group) => group.tasks.length > 0);
};

export type BackgroundTaskSection = {
  key: string;
  title: string;
  description: string;
  tasks: TrackedBackgroundTask[];
  accent?: 'current' | 'global' | 'default';
};

export type TaskFilter = 'overview' | 'active' | 'current' | 'failed';

export const selectBackgroundTaskSections = (
  tasks: TrackedBackgroundTask[],
  focusProjectId: string | null,
  taskFilter: TaskFilter,
): BackgroundTaskSection[] => {
  const activeTasks = selectActiveBackgroundTasks(tasks);
  const recentTasks = tasks.filter((task) => !isActiveBackgroundTask(task));
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

  const sections: BackgroundTaskSection[] = [];

  if (focusProjectId) {
    sections.push({
      key: 'current-active',
      title: 'Current project · Active tasks',
      description: currentActive.length > 0 ? 'Ongoing tasks for the current project.' : 'No active tasks in the current project.',
      tasks: currentActive,
      accent: 'current',
    });
    sections.push({
      key: 'current-recent',
      title: 'Current project · Recent tasks',
      description: currentRecent.length > 0 ? 'Recently finished tasks for the current project.' : 'No recent tasks in the current project.',
      tasks: currentRecent,
      accent: 'current',
    });
  } else {
    sections.push({
      key: 'active',
      title: 'Active tasks',
      description: activeTasks.length > 0 ? 'Tasks currently running across visible projects.' : 'No active tasks.',
      tasks: activeTasks,
    });
    sections.push({
      key: 'recent',
      title: 'Recent tasks',
      description: recentTasks.length > 0 ? 'Recently finished tasks across visible projects.' : 'No recent tasks.',
      tasks: recentTasks,
    });
  }

  if (globalTasks.length > 0) {
    sections.push({
      key: 'global',
      title: 'Global tasks',
      description: 'Tasks not tied to a specific project.',
      tasks: globalTasks,
      accent: 'global',
    });
  }

  if (otherActive.length > 0) {
    sections.push({
      key: 'other-active',
      title: 'Other active tasks',
      description: 'Active tasks from other visible projects.',
      tasks: otherActive,
    });
  }

  if (otherRecent.length > 0) {
    sections.push({
      key: 'other-recent',
      title: 'Other recent tasks',
      description: 'Recent tasks from other visible projects.',
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
      (taskFilter !== 'active' &&
        taskFilter !== 'failed' &&
        (section.key === 'current-active' ||
          section.key === 'current-recent' ||
          section.key === 'active' ||
          section.key === 'recent')),
  );
};
