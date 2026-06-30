const routeOwnedBackgroundTaskTypes = new Set<string>([
  'book_import_apply',
  'book_import_retry_failed_steps',
  'polish_text',
  'polish_batch',
  'inspiration_generate_options',
  'inspiration_refine_options',
  'inspiration_quick_generate',
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
  'chapter_regenerate',
  'chapter_partial_regenerate',
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
  taskId?: string | null,
): string | null => {
  if (!projectId) {
    if (taskType.startsWith('wizard_')) return '/wizard';
    if (taskType.startsWith('inspiration_')) {
      return taskId ? `/inspiration?task_id=${encodeURIComponent(taskId)}` : '/inspiration';
    }
    if (taskType.startsWith('book_import_')) return '/projects?view=book-import';
    if (taskType.startsWith('polish_')) return '/projects';
    return null;
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
    case 'chapter_regenerate':
    case 'chapter_partial_regenerate':
      return `/project/${projectId}/chapters`;
    case 'book_import_apply':
    case 'book_import_retry_failed_steps':
      return `/project/${projectId}/chapters`;
    case 'polish_text':
    case 'polish_batch':
      return `/project/${projectId}`;
    default:
      return `/project/${projectId}`;
  }
};

export const getBackgroundTaskCategory = (taskType: string): BackgroundTaskCategory => {
  if (taskType.startsWith('polish_')) return 'other';
  if (taskType.startsWith('book_import_')) return 'other';
  if (taskType.startsWith('inspiration_')) return 'other';
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
