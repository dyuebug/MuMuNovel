import { Suspense, lazy, memo } from 'react';
import WorkflowEntryFallback from './WorkflowEntryFallback';

const LazyChapterAnalysis = lazy(() => import('./ChapterAnalysis'));

type ChapterAnalysisEntryProps = {
  chapterId: string | null;
  visible: boolean;
  onClose: () => void;
};

function ChapterAnalysisEntry({
  chapterId,
  visible,
  onClose,
}: ChapterAnalysisEntryProps) {
  if (!visible || !chapterId) {
    return null;
  }

  return (
    <Suspense
      fallback={(
        <WorkflowEntryFallback
          eyebrow="Chapter Analysis"
          title="正在展开章节分析面板"
          message="系统正在准备章节诊断、问题定位与分析详情面板，原有数据装载与关闭链路保持不变。"
          tags={[
            { label: '章节分析', color: 'blue' },
            { label: '诊断视图恢复中', color: 'processing' },
            { label: '交互逻辑保持原样', color: 'green' },
          ]}
        />
      )}
    >
      <LazyChapterAnalysis
        chapterId={chapterId}
        visible={visible}
        onClose={onClose}
      />
    </Suspense>
  );
}

export default memo(ChapterAnalysisEntry);
