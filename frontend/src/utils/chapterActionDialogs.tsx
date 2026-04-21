import { InfoCircleOutlined } from '@ant-design/icons';
import type { FormInstance } from 'antd';
import { Space } from 'antd';
import ChapterExpansionPlanPreviewContent from '../components/ChapterExpansionPlanPreviewContent';
import ChapterNumberConflictConfirmContent from '../components/ChapterNumberConflictConfirmContent';
import ContinueGenerateConfirmContent from '../components/ContinueGenerateConfirmContent';
import ManualChapterCreateFormContent, {
  type ManualChapterCreateFormValues,
  type ManualChapterCreateOutlineOption,
} from '../components/ManualChapterCreateFormContent';
import type { Chapter, ChapterCreate, ExpansionPlanData, Project, WritingStyle } from '../types';
import { CREATION_PLOT_STAGE_OPTIONS } from './creationPresetsCore';
import {
  CREATIVE_MODE_OPTIONS,
  STORY_FOCUS_OPTIONS,
} from './generationPreferenceOptions';

type SelectionOption = {
  value: string;
  label: string;
};

type MessageApiLike = {
  error: (content: string) => void;
  success: (content: string) => void;
};

type ModalConfirmInstance = {
  update: (config: Record<string, unknown>) => void;
  destroy: () => void;
};

type ModalInstanceApiLike = {
  confirm: (config: Record<string, unknown>) => ModalConfirmInstance;
};

type ModalConfirmApiLike = {
  confirm: (config: Record<string, unknown>) => unknown;
};

type ModalInfoApiLike = {
  info: (config: Record<string, unknown>) => unknown;
};

type ContinueGenerateChapterPreview = Pick<Chapter, 'id' | 'chapter_number' | 'title' | 'word_count'>;
type ContinueGenerateChapterContext = Pick<Chapter, 'chapter_number'>;
type WritingStylePreview = Pick<WritingStyle, 'id' | 'name'>;
type ManualCreateChapterRequest = ChapterCreate & { outline_id: string; status: Chapter['status'] };
type ChapterApiLike = {
  createChapter: (data: ManualCreateChapterRequest) => Promise<Chapter>;
};

type ProjectApiLike = {
  getProject: (id: string) => Promise<Project>;
};

type ContinueGenerateDialogParams = {
  modal: ModalInstanceApiLike;
  chapter: ContinueGenerateChapterContext;
  sortedChapters: ContinueGenerateChapterPreview[];
  writingStyles: WritingStylePreview[];
  selectedStyleId?: WritingStyle['id'];
  selectedCreativeMode?: string;
  selectedStoryFocus?: string;
  selectedPlotStage?: string;
  targetWordCount: number;
  handleGenerate: () => Promise<void>;
  message: Pick<MessageApiLike, 'error'>;
};

type ManualCreateDialogParams = {
  modal: ModalConfirmApiLike;
  chapters: Chapter[];
  manualCreateForm: FormInstance<ManualChapterCreateFormValues>;
  sortedOutlines: ManualChapterCreateOutlineOption[];
  currentProject: Project | null;
  chapterApi: ChapterApiLike;
  projectApi: ProjectApiLike;
  refreshChapters: () => Promise<Chapter[]> | void;
  setCurrentProject: (project: Project | null) => void;
  message: MessageApiLike;
  handleDeleteChapter: (chapterId: string) => Promise<void>;
  getStatusText: (status: Chapter['status']) => string;
};

type ExpansionPlanPreviewDialogParams = {
  modal: ModalConfirmApiLike & ModalInfoApiLike;
  chapter: Pick<Chapter, 'chapter_number' | 'title' | 'expansion_plan'>;
  isMobile: boolean;
  message: Pick<MessageApiLike, 'error'>;
};

const resolveOptionLabel = (
  options: SelectionOption[] | undefined,
  value: string | undefined,
  fallback = 'Not selected',
): string => {
  if (!value) {
    return fallback;
  }

  return options?.find((item) => item.value === value)?.label || value;
};

export const openContinueGenerateDialog = ({
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
}: ContinueGenerateDialogParams): void => {
  const previousChapters = sortedChapters.filter((item) => item.chapter_number < chapter.chapter_number);
  const selectedStyle = writingStyles.find((item) => item.id === selectedStyleId);
  const creativeModeLabel = resolveOptionLabel(CREATIVE_MODE_OPTIONS, selectedCreativeMode);
  const storyFocusLabel = resolveOptionLabel(STORY_FOCUS_OPTIONS, selectedStoryFocus);
  const plotStageLabel = resolveOptionLabel(CREATION_PLOT_STAGE_OPTIONS, selectedPlotStage);

  const restoreState = {
    okButtonProps: { danger: true, loading: false },
    cancelButtonProps: { disabled: false },
    closable: true,
    maskClosable: true,
    keyboard: true,
  };

  const instance = modal.confirm({
    title: 'Confirm continue generation',
    width: 700,
    centered: true,
    content: (
      <ContinueGenerateConfirmContent
        selectedStyleName={selectedStyle?.name}
        creativeModeLabel={creativeModeLabel}
        storyFocusLabel={storyFocusLabel}
        plotStageLabel={plotStageLabel}
        targetWordCount={targetWordCount}
        previousChapters={previousChapters}
      />
    ),
    okText: 'Continue',
    okButtonProps: { danger: true },
    cancelText: 'Cancel',
    onOk: async () => {
      instance.update({
        okButtonProps: { danger: true, loading: true },
        cancelButtonProps: { disabled: true },
        closable: false,
        maskClosable: false,
        keyboard: false,
      });

      try {
        if (!selectedStyleId) {
          message.error('Please select a writing style first');
          instance.update(restoreState);
          return;
        }

        await handleGenerate();
        instance.destroy();
      } catch {
        instance.update(restoreState);
      }
    },
  });
};

export const openManualCreateChapterDialog = ({
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
}: ManualCreateDialogParams): void => {
  if (!currentProject) {
    return;
  }

  const nextChapterNumber = chapters.length > 0
    ? Math.max(...chapters.map((chapter) => chapter.chapter_number)) + 1
    : 1;

  const createChapter = async (values: ManualChapterCreateFormValues): Promise<void> => {
    try {
      await chapterApi.createChapter({
        project_id: currentProject.id,
        ...values,
      });

      message.success('Chapter created successfully');
      await refreshChapters();
      const updatedProject = await projectApi.getProject(currentProject.id);
      setCurrentProject(updatedProject);
      manualCreateForm.resetFields();
    } catch (error) {
      const err = error as Error;
      message.error(`Failed to create chapter: ${err.message || 'Unknown error'}`);
      throw error;
    }
  };

  modal.confirm({
    title: 'Create chapter manually',
    width: 600,
    centered: true,
    content: (
      <ManualChapterCreateFormContent
        form={manualCreateForm}
        nextChapterNumber={nextChapterNumber}
        sortedOutlines={sortedOutlines}
      />
    ),
    okText: 'Create chapter',
    cancelText: 'Cancel',
    onOk: async () => {
      const values = await manualCreateForm.validateFields();
      const conflictChapter = chapters.find((chapter) => chapter.chapter_number === values.chapter_number);

      if (conflictChapter) {
        modal.confirm({
          title: 'Chapter number conflict',
          icon: <InfoCircleOutlined style={{ color: '#ff4d4f' }} />,
          width: 500,
          centered: true,
          content: (
            <ChapterNumberConflictConfirmContent
              chapterNumber={values.chapter_number}
              conflictChapter={conflictChapter}
              statusText={getStatusText(conflictChapter.status)}
            />
          ),
          okText: 'Delete existing chapter and create',
          okButtonProps: { danger: true },
          cancelText: 'Cancel',
          onOk: async () => {
            await handleDeleteChapter(conflictChapter.id);
            await new Promise((resolve) => window.setTimeout(resolve, 300));
            await createChapter(values);
          },
        });

        return Promise.reject();
      }

      await createChapter(values);
    },
  });
};

export const openExpansionPlanPreviewDialog = ({
  modal,
  chapter,
  isMobile,
  message,
}: ExpansionPlanPreviewDialogParams): void => {
  if (!chapter.expansion_plan) {
    return;
  }

  try {
    const planData: ExpansionPlanData = JSON.parse(chapter.expansion_plan);

    modal.info({
      title: (
        <Space style={{ flexWrap: 'wrap' }}>
          <InfoCircleOutlined style={{ color: 'var(--color-primary)' }} />
          <span style={{ wordBreak: 'break-word' }}>{`Chapter ${chapter.chapter_number} expansion plan`}</span>
        </Space>
      ),
      width: isMobile ? 'calc(100vw - 32px)' : 800,
      centered: true,
      style: isMobile
        ? {
            maxWidth: 'calc(100vw - 32px)',
            margin: '0 auto',
            padding: '0 16px',
          }
        : undefined,
      styles: {
        body: {
          maxHeight: isMobile ? 'calc(100vh - 200px)' : 'calc(80vh - 110px)',
          overflowY: 'auto',
        },
      },
      content: (
        <ChapterExpansionPlanPreviewContent
          chapterTitle={chapter.title}
          isMobile={isMobile}
          planData={planData}
        />
      ),
      okText: 'Close',
    });
  } catch (error) {
    console.error('Failed to load expansion plan:', error);
    message.error('Failed to load expansion plan');
  }
};