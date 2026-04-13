const routeOwnedBackgroundTaskTypes = new Set<string>([
  'careers_generate_system',
  'wizard_career_system',
  'character_generate',
  'wizard_characters',
  'organization_generate',
  'world_regenerate',
  'wizard_world_building',
  'outline_generate',
  'outline_expand',
  'outline_batch_expand',
  'wizard_outline',
  'chapters_batch_generate',
  'chapter_single_generate',
  'chapter_analysis',
]);

export type BackgroundTaskCategory =
  | 'chapter'
  | 'outline'
  | 'world'
  | 'character'
  | 'career'
  | 'organization'
  | 'wizard'
  | 'other';

const backgroundTaskCategoryLabels: Record<BackgroundTaskCategory, string> = {
  chapter: '章节相关',
  outline: '大纲相关',
  world: '世界观相关',
  character: '角色相关',
  career: '职业体系',
  organization: '组织势力',
  wizard: '向导流程',
  other: '其他任务',
};

export const isRouteOwnedBackgroundTaskType = (taskType: string): boolean => (
  routeOwnedBackgroundTaskTypes.has(taskType)
);

export const getBackgroundTaskDestination = (
  taskType: string,
  projectId?: string | null,
): string | null => {
  if (!projectId) {
    return taskType.startsWith('wizard_') ? '/wizard' : null;
  }

  switch (taskType) {
    case 'careers_generate_system':
    case 'wizard_career_system':
      return `/project/${projectId}/careers`;
    case 'character_generate':
    case 'wizard_characters':
    case 'organization_generate':
      return `/project/${projectId}/characters`;
    case 'world_regenerate':
    case 'wizard_world_building':
      return `/project/${projectId}/world-setting`;
    case 'outline_generate':
    case 'outline_expand':
    case 'outline_batch_expand':
    case 'wizard_outline':
      return `/project/${projectId}/outline`;
    case 'chapters_batch_generate':
    case 'chapter_single_generate':
    case 'chapter_analysis':
      return `/project/${projectId}/chapters`;
    default:
      return `/project/${projectId}`;
  }
};

export const getBackgroundTaskCategory = (taskType: string): BackgroundTaskCategory => {
  if (taskType.startsWith('chapter_') || taskType === 'chapters_batch_generate') return 'chapter';
  if (taskType.startsWith('outline_') || taskType === 'wizard_outline') return 'outline';
  if (taskType === 'world_regenerate' || taskType === 'wizard_world_building') return 'world';
  if (taskType === 'character_generate' || taskType === 'wizard_characters') return 'character';
  if (taskType === 'careers_generate_system' || taskType === 'wizard_career_system') return 'career';
  if (taskType === 'organization_generate') return 'organization';
  if (taskType.startsWith('wizard_')) return 'wizard';
  return 'other';
};

export const getBackgroundTaskCategoryLabel = (category: BackgroundTaskCategory): string => (
  backgroundTaskCategoryLabels[category] ?? backgroundTaskCategoryLabels.other
);
