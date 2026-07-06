import { Suspense, lazy, memo } from 'react';
import type { FormInstance } from 'antd';
import WorkflowEntryFallback from './WorkflowEntryFallback';

type ChapterBasicFormValues = {
  title?: string;
  chapter_number?: number | string;
  status?: 'draft' | 'writing' | 'completed';
};

const LazyChapterBasicModal = lazy(() => import('./ChapterBasicModal'));

type ChapterBasicModalEntryProps = {
  open: boolean;
  title: string;
  isMobile: boolean;
  outlineMode: string;
  submitText: string;
  form: FormInstance<ChapterBasicFormValues>;
  onCancel: () => void;
  onFinish: (values: ChapterBasicFormValues) => void | Promise<void>;
};

function ChapterBasicModalEntry({
  open,
  title,
  isMobile,
  outlineMode,
  submitText,
  form,
  onCancel,
  onFinish,
}: ChapterBasicModalEntryProps) {
  if (!open) {
    return null;
  }

  return (
    <Suspense
      fallback={(
        <WorkflowEntryFallback
          eyebrow="Chapter Setup"
          title="正在整理章节基础设置面板"
          message="系统正在恢复章节标题、序号与状态设置面板，原有表单与提交逻辑保持不变。"
          tags={[
            { label: '章节创建 / 编辑', color: 'blue' },
            { label: '表单逻辑保持原样', color: 'green' },
          ]}
        />
      )}
    >
      <LazyChapterBasicModal
        open={open}
        title={title}
        isMobile={isMobile}
        outlineMode={outlineMode}
        submitText={submitText}
        form={form}
        onCancel={onCancel}
        onFinish={onFinish}
      />
    </Suspense>
  );
}

export default memo(ChapterBasicModalEntry);
