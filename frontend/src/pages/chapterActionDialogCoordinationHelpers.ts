import type { FormInstance } from 'antd';
import type { Chapter, Project, WritingStyle } from '../types';
import type {
  ManualChapterCreateFormValues,
  ManualChapterCreateOutlineOption,
} from '../components/ManualChapterCreateFormContent';

type MessageLike = {
  warning: (content: string) => void;
  success: (content: string) => void;
  error: (content: string) => void;
};

type ModalLike = {
  confirm: (config: Record<string, unknown>) => unknown;
  info?: (config: Record<string, unknown>) => unknown;
};

type ModalDialogInstanceLike = {
  update: (config: Record<string, unknown>) => void;
  destroy: () => void;
};

type ContinueGenerateModalLike = ModalLike & {
  confirm: (config: Record<string, unknown>) => ModalDialogInstanceLike;
};

export function confirmChapterExportWorkflow({
  currentProject,
  chapterCount,
  modal,
  message,
}: {
  currentProject: Project | null;
  chapterCount: number;
  modal: ModalLike;
  message: MessageLike;
}): void {
  if (!currentProject) {
    return;
  }

  if (chapterCount === 0) {
    message.warning('No chapters to export.');
    return;
  }

  modal.confirm({
    title: 'Export project data',
    content: 'Export the current project data as a backup file?',
    centered: true,
    okText: 'Export',
    cancelText: 'Cancel',
    onOk: async () => {
      try {
        const { projectApi } = await import('../services/modularApi');
        projectApi.exportProject(currentProject.id);
        message.success('Export started.');
      } catch {
        message.error('Export failed.');
      }
    },
  });
}


export async function openSingleChapterGenerateWorkflow({
  modal,
  chapter,
  sortedChapters,
  writingStyles,
  selectedStyleId,
  selectedCreativeMode,
  selectedStoryFocus,
  selectedPlotStage,
  targetWordCount,
  handleGenerate,
  message,
}: {
  modal: ModalLike;
  chapter: Pick<Chapter, 'chapter_number'>;
  sortedChapters: Array<Pick<Chapter, 'id' | 'chapter_number' | 'title' | 'word_count'>>;
  writingStyles: Array<Pick<WritingStyle, 'id' | 'name'>>;
  selectedStyleId?: WritingStyle['id'];
  selectedCreativeMode?: string;
  selectedStoryFocus?: string;
  selectedPlotStage?: string;
  targetWordCount: number;
  handleGenerate: () => Promise<void>;
  message: Pick<MessageLike, 'error'>;
}): Promise<void> {
  const { openContinueGenerateDialog } = await import('../utils/chapterActionDialogs');

  openContinueGenerateDialog({
    modal: modal as ContinueGenerateModalLike,
    chapter,
    sortedChapters,
    writingStyles,
    selectedStyleId,
    selectedCreativeMode,
    selectedStoryFocus,
    selectedPlotStage,
    targetWordCount,
    handleGenerate,
    message,
  });
}

export async function openManualCreateChapterWorkflow({
  modal,
  chapters,
  manualCreateForm,
  sortedOutlines,
  currentProject,
  refreshChapters,
  setCurrentProject,
  message,
  handleDeleteChapter,
  getStatusText,
}: {
  modal: ModalLike;
  chapters: Chapter[];
  manualCreateForm: FormInstance<ManualChapterCreateFormValues>;
  sortedOutlines: ManualChapterCreateOutlineOption[];
  currentProject: Project | null;
  refreshChapters: () => Promise<Chapter[]> | void;
  setCurrentProject: (project: Project | null) => void;
  message: MessageLike;
  handleDeleteChapter: (chapterId: string) => Promise<void>;
  getStatusText: (status: Chapter['status']) => string;
}): Promise<void> {
  const [{ openManualCreateChapterDialog }, { chapterApi, projectApi }] = await Promise.all([
    import('../utils/chapterActionDialogs'),
    import('../services/modularApi'),
  ]);

  openManualCreateChapterDialog({
    modal,
    chapters,
    manualCreateForm,
    sortedOutlines,
    currentProject,
    chapterApi,
    projectApi,
    refreshChapters,
    setCurrentProject,
    message,
    handleDeleteChapter,
    getStatusText,
  });
}

export async function openExpansionPlanPreviewWorkflow({
  modal,
  chapter,
  isMobile,
  message,
}: {
  modal: ModalLike;
  chapter: Chapter;
  isMobile: boolean;
  message: Pick<MessageLike, 'error'>;
}): Promise<void> {
  const { openExpansionPlanPreviewDialog } = await import('../utils/chapterActionDialogs');

  openExpansionPlanPreviewDialog({
    modal: modal as ModalLike & { info: (config: Record<string, unknown>) => unknown },
    chapter,
    isMobile,
    message,
  });
}
