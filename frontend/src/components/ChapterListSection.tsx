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

export default memo(ChapterListSection);