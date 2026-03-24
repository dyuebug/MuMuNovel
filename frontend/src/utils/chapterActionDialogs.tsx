/* eslint-disable @typescript-eslint/no-explicit-any */
import { InfoCircleOutlined } from '@ant-design/icons';
import { Space } from 'antd';
import ChapterExpansionPlanPreviewContent from '../components/ChapterExpansionPlanPreviewContent';
import ChapterNumberConflictConfirmContent from '../components/ChapterNumberConflictConfirmContent';
import ContinueGenerateConfirmContent from '../components/ContinueGenerateConfirmContent';
import ManualChapterCreateFormContent from '../components/ManualChapterCreateFormContent';
import { CREATION_PLOT_STAGE_OPTIONS } from '../utils/creationPresetsCore';
import type { ExpansionPlanData } from '../types';

const resolveOptionLabel = (options: any[], value: string | undefined, fallback = '???') => {
  if (!value) return fallback;
  return options.find((item) => item.value === value)?.label || value;
};

export const openContinueGenerateDialog = ({
  modal,
  chapter,
  sortedChapters,
  writingStyles,
  selectedStyleId,
  selectedCreativeMode,
  CREATIVE_MODE_OPTIONS,
  selectedStoryFocus,
  STORY_FOCUS_OPTIONS,
  selectedPlotStage,
  targetWordCount,
  handleGenerate,
  message,
}: any) => {
  const previousChapters = sortedChapters.filter((item: any) => item.chapter_number < chapter.chapter_number);
  const selectedStyle = writingStyles.find((item: any) => item.id === selectedStyleId);
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
    title: '??????',
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
    okText: '????',
    okButtonProps: { danger: true },
    cancelText: '??',
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
          message.error('?????????');
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
}: any) => {
  if (!currentProject) return;

  const nextChapterNumber = chapters.length > 0
    ? Math.max(...chapters.map((chapter: any) => chapter.chapter_number)) + 1
    : 1;

  const createChapter = async (values: any) => {
    try {
      await chapterApi.createChapter({
        project_id: currentProject.id,
        ...values,
      });

      message.success('???????');
      await refreshChapters();
      const updatedProject = await projectApi.getProject(currentProject.id);
      setCurrentProject(updatedProject);
      manualCreateForm.resetFields();
    } catch (error: any) {
      message.error(`???????${error?.message || '????'}`);
      throw error;
    }
  };

  modal.confirm({
    title: '??????',
    width: 600,
    centered: true,
    content: (
      <ManualChapterCreateFormContent
        form={manualCreateForm}
        nextChapterNumber={nextChapterNumber}
        sortedOutlines={sortedOutlines}
      />
    ),
    okText: '????',
    cancelText: '??',
    onOk: async () => {
      const values = await manualCreateForm.validateFields();
      const conflictChapter = chapters.find((chapter: any) => chapter.chapter_number === values.chapter_number);

      if (conflictChapter) {
        modal.confirm({
          title: '??????',
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
          okText: '????????',
          okButtonProps: { danger: true },
          cancelText: '??',
          onOk: async () => {
            await handleDeleteChapter(conflictChapter.id);
            await new Promise((resolve) => setTimeout(resolve, 300));
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
}: any) => {
  if (!chapter.expansion_plan) return;

  try {
    const planData: ExpansionPlanData = JSON.parse(chapter.expansion_plan);

    modal.info({
      title: (
        <Space style={{ flexWrap: 'wrap' }}>
          <InfoCircleOutlined style={{ color: 'var(--color-primary)' }} />
          <span style={{ wordBreak: 'break-word' }}>{`? ${chapter.chapter_number} ?????`}</span>
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
      okText: '???',
    });
  } catch (error) {
    console.error('Failed to load expansion plan:', error);
    message.error('?????????');
  }
};
