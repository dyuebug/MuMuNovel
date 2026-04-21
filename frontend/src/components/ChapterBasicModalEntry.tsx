import { Suspense, lazy, memo } from 'react';
import type { FormInstance } from 'antd';

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
    <Suspense fallback={null}>
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