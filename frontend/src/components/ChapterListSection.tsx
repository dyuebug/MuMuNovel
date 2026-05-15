import { memo } from 'react';
import { Collapse, Empty, List, Tag } from 'antd';
import { CaretRightOutlined } from '@ant-design/icons';
import type { AnalysisTask, Chapter } from '../types';
import ChapterListItem from './ChapterListItem';

type GroupedChapterViewModel = {
  key: string;
  outlineId: string | null;
  outlineTitle: string;
  outlineOrder: number;
  chapters: Chapter[];
  totalWordCount: number;
};

type ChapterGenerationStateMap = Record<string, {
  canGenerate?: boolean;
  disabledReason?: string;
} | undefined>;

type ChapterListSectionProps = {
  chapters: Chapter[];
  sortedChapters: Chapter[];
  outlineMode?: string | null;
  groupedChapters: GroupedChapterViewModel[];
  expandedChapterGroupKeys: string[];
  isMobile: boolean;
  analysisTasksMap: Record<string, AnalysisTask>;
  chapterGenerationStateById: ChapterGenerationStateMap;
  onOpenReader: (chapter: Chapter) => void;
  onOpenEditor: (chapterId: string) => void;
  onShowAnalysis: (chapterId: string) => void;
  onOpenSettings: (chapterId: string) => void;
  onDeleteChapter: (chapterId: string) => void;
  onShowExpansionPlan: (chapter: Chapter) => void;
  onOpenPlanEditor: (chapter: Chapter) => void;
};

const areStringArraysEqual = (left: string[], right: string[]) => (
  left.length === right.length && left.every((value, index) => value === right[index])
);

const areVisibleAnalysisTasksEqual = (
  leftMap: Record<string, AnalysisTask>,
  rightMap: Record<string, AnalysisTask>,
  chapterIds: string[],
) => chapterIds.every((chapterId) => leftMap[chapterId] === rightMap[chapterId]);

const areVisibleGenerationStatesEqual = (
  leftMap: ChapterGenerationStateMap,
  rightMap: ChapterGenerationStateMap,
  chapterIds: string[],
) => chapterIds.every((chapterId) => leftMap[chapterId] === rightMap[chapterId]);

const collectVisibleChapterIds = (
  outlineMode: string | null | undefined,
  sortedChapters: Chapter[],
  groupedChapters: GroupedChapterViewModel[],
) => {
  if (outlineMode === 'one-to-one') {
    return sortedChapters.map((chapter) => chapter.id);
  }

  return groupedChapters.flatMap((group) => group.chapters.map((chapter) => chapter.id));
};

function ChapterListSection({
  chapters,
  sortedChapters,
  outlineMode,
  groupedChapters,
  expandedChapterGroupKeys,
  isMobile,
  analysisTasksMap,
  chapterGenerationStateById,
  onOpenReader,
  onOpenEditor,
  onShowAnalysis,
  onOpenSettings,
  onDeleteChapter,
  onShowExpansionPlan,
  onOpenPlanEditor,
}: ChapterListSectionProps) {
  if (chapters.length === 0) {
    return <Empty description="暂无章节" />;
  }

  if (outlineMode === 'one-to-one') {
    return (
      <List
        rowKey="id"
        dataSource={sortedChapters}
        renderItem={(item) => (
          <ChapterListItem
            chapter={item}
            variant="flat"
            isMobile={isMobile}
            showOutlineActions={false}
            analysisTask={analysisTasksMap[item.id]}
            canGenerate={chapterGenerationStateById[item.id]?.canGenerate ?? false}
            generateDisabledReason={chapterGenerationStateById[item.id]?.disabledReason ?? ''}
            onOpenReader={onOpenReader}
            onOpenEditor={onOpenEditor}
            onShowAnalysis={onShowAnalysis}
            onOpenSettings={onOpenSettings}
            onDeleteChapter={onDeleteChapter}
            onShowExpansionPlan={onShowExpansionPlan}
            onOpenPlanEditor={onOpenPlanEditor}
          />
        )}
      />
    );
  }

  return (
    <Collapse
      bordered={false}
      defaultActiveKey={expandedChapterGroupKeys}
      expandIcon={({ isActive }) => <CaretRightOutlined rotate={isActive ? 90 : 0} />}
      style={{ background: 'transparent' }}
    >
      {groupedChapters.map((group) => (
        <Collapse.Panel
          key={group.key}
          header={
            <div style={{ display: 'flex', alignItems: 'center', gap: 12, flexWrap: 'wrap' }}>
              <Tag color={group.outlineId ? 'blue' : 'default'} style={{ margin: 0, flexShrink: 0 }}>
                {group.outlineId
                  ? `大纲 ${group.outlineOrder ?? '-'}`
                  : '手动章节'}
              </Tag>
              <span
                style={{
                  fontWeight: 600,
                  fontSize: 16,
                  wordBreak: 'break-word',
                  lineHeight: 1.5,
                }}
              >
                {group.outlineTitle || '未命名大纲'}
              </span>
              <Tag color="green" style={{ margin: 0 }}>
                章节数 {group.chapters.length}
              </Tag>
              <Tag color="blue" style={{ margin: 0 }}>
                总字数 {group.totalWordCount}
              </Tag>
            </div>
          }
          style={{
            marginBottom: 16,
            background: '#fff',
            borderRadius: 8,
            border: '1px solid #f0f0f0',
          }}
        >
          <List
            rowKey="id"
            dataSource={group.chapters}
            renderItem={(item) => (
              <ChapterListItem
                chapter={item}
                variant="grouped"
                isMobile={isMobile}
                showOutlineActions={outlineMode === 'one-to-many'}
                analysisTask={analysisTasksMap[item.id]}
                canGenerate={chapterGenerationStateById[item.id]?.canGenerate ?? false}
                generateDisabledReason={chapterGenerationStateById[item.id]?.disabledReason ?? ''}
                onOpenReader={onOpenReader}
                onOpenEditor={onOpenEditor}
                onShowAnalysis={onShowAnalysis}
                onOpenSettings={onOpenSettings}
                onDeleteChapter={onDeleteChapter}
                onShowExpansionPlan={onShowExpansionPlan}
                onOpenPlanEditor={onOpenPlanEditor}
              />
            )}
          />
        </Collapse.Panel>
      ))}
    </Collapse>
  );
}

export default memo(ChapterListSection, (prevProps, nextProps) => {
  if (prevProps.chapters !== nextProps.chapters) return false;
  if (prevProps.sortedChapters !== nextProps.sortedChapters) return false;
  if (prevProps.outlineMode !== nextProps.outlineMode) return false;
  if (prevProps.groupedChapters !== nextProps.groupedChapters) return false;
  if (!areStringArraysEqual(prevProps.expandedChapterGroupKeys, nextProps.expandedChapterGroupKeys)) return false;
  if (prevProps.isMobile !== nextProps.isMobile) return false;
  if (prevProps.onOpenReader !== nextProps.onOpenReader) return false;
  if (prevProps.onOpenEditor !== nextProps.onOpenEditor) return false;
  if (prevProps.onShowAnalysis !== nextProps.onShowAnalysis) return false;
  if (prevProps.onOpenSettings !== nextProps.onOpenSettings) return false;
  if (prevProps.onDeleteChapter !== nextProps.onDeleteChapter) return false;
  if (prevProps.onShowExpansionPlan !== nextProps.onShowExpansionPlan) return false;
  if (prevProps.onOpenPlanEditor !== nextProps.onOpenPlanEditor) return false;

  const visibleChapterIds = collectVisibleChapterIds(
    nextProps.outlineMode,
    nextProps.sortedChapters,
    nextProps.groupedChapters,
  );

  if (!areVisibleAnalysisTasksEqual(prevProps.analysisTasksMap, nextProps.analysisTasksMap, visibleChapterIds)) {
    return false;
  }

  if (!areVisibleGenerationStatesEqual(prevProps.chapterGenerationStateById, nextProps.chapterGenerationStateById, visibleChapterIds)) {
    return false;
  }

  return true;
});
