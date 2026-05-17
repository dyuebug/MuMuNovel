import { Suspense, lazy, memo } from 'react';
import { useChapterGenerationUiStore } from '../store/chapterGenerationUi';
import { formatActiveStoryRepairLabel } from '../utils/activeStoryRepair';

const LazySSEProgressModal = lazy(async () => {
  const module = await import('./SSEProgressModal');
  return { default: module.SSEProgressModal };
});

type ChapterBatchProgressEntryProps = {
  visible: boolean;
  buildCheckpointHint: (checkpoint?: {
    current_chapter_number?: number | null;
    candidate_index?: number | null;
    candidate_count?: number | null;
    word_count?: number | null;
    generation_path?: string | null;
    attempt_kind?: string | null;
    rerank_used?: boolean | null;
    word_budget_repair_used?: boolean | null;
    winner_candidate_index?: number | null;
    pre_compaction_total_length?: number | null;
    context_budget_limit?: number | null;
    compaction_applied?: boolean | null;
    compaction_details?: Record<string, { before?: number | null; after?: number | null }> | null;
  } | null) => string;
  onCancel: () => void;
};

function ChapterBatchProgressEntry({
  visible,
  buildCheckpointHint,
  onCancel,
}: ChapterBatchProgressEntryProps) {
  const batchProgress = useChapterGenerationUiStore((state) => state.batchProgress);

  if (!visible) {
    return null;
  }

  const progress = batchProgress?.progress_percent ?? 0;
  const repairLabel = formatActiveStoryRepairLabel(batchProgress?.active_story_repair_payload);
  const checkpointLabel = buildCheckpointHint(batchProgress?.checkpoint ?? null);
  const message = batchProgress?.current_chapter_number
    ? [
        `正在生成第 ${batchProgress.current_chapter_number} / ${batchProgress.total} 章`,
        batchProgress.latest_quality_metrics?.overall_score !== undefined
          ? `评分 ${batchProgress.latest_quality_metrics.overall_score}`
          : null,
        checkpointLabel,
        repairLabel,
      ].filter(Boolean).join(' | ')
    : [
        '正在准备批量生成',
        batchProgress?.latest_quality_metrics?.overall_score !== undefined
          ? `评分 ${batchProgress.latest_quality_metrics.overall_score}`
          : null,
        checkpointLabel,
        repairLabel,
      ].filter(Boolean).join(' | ');

  return (
    <Suspense fallback={null}>
      <LazySSEProgressModal
        visible={visible}
        progress={progress}
        message={message}
        title={'Batch generation'}
        onCancel={onCancel}
        cancelButtonText={'Close'}
        blocking={false}
      />
    </Suspense>
  );
}

export default memo(ChapterBatchProgressEntry);
