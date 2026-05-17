import { memo } from 'react';
import { Collapse, Empty, List, Tag } from 'antd';
import { CaretRightOutlined } from '@ant-design/icons';
import type { Chapter } from '../types';
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
  sortedChapters: Chapter[];
  outlineMode?: string | null;
  groupedChapters: GroupedChapterViewModel[];
  expandedChapterGroupKeys: string[];
  isMobile: boolean;
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

const areVisibleGenerationStatesEqualForChapters = (
  leftMap: ChapterGenerationStateMap,
  rightMap: ChapterGenerationStateMap,
  chapters: Chapter[],
) => chapters.every((chapter) => leftMap[chapter.id] === rightMap[chapter.id]);

const areVisibleGenerationStatesEqualForGroups = (
  leftMap: ChapterGenerationStateMap,
  rightMap: ChapterGenerationStateMap,
  groups: GroupedChapterViewModel[],
) => groups.every((group) => (
  group.chapters.every((chapter) => leftMap[chapter.id] === rightMap[chapter.id])
));

const areChapterReferenceArraysEqual = (left: Chapter[], right: Chapter[]) => (
  left.length === right.length
  && left.every((chapter, index) => chapter === right[index])
);

type ChapterListItemActionProps = Pick<
  ChapterListSectionProps,
  | 'isMobile'
  | 'onOpenReader'
  | 'onOpenEditor'
  | 'onShowAnalysis'
  | 'onOpenSettings'
  | 'onDeleteChapter'
  | 'onShowExpansionPlan'
  | 'onOpenPlanEditor'
>;

type FlatChapterListProps = ChapterListItemActionProps & {
  sortedChapters: Chapter[];
  chapterGenerationStateById: ChapterGenerationStateMap;
};

const FlatChapterList = memo(function FlatChapterList({
  sortedChapters,
  isMobile,
  chapterGenerationStateById,
  onOpenReader,
  onOpenEditor,
  onShowAnalysis,
  onOpenSettings,
  onDeleteChapter,
  onShowExpansionPlan,
  onOpenPlanEditor,
}: FlatChapterListProps) {
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
}, (prevProps, nextProps) => {
  if (!areChapterReferenceArraysEqual(prevProps.sortedChapters, nextProps.sortedChapters)) return false;
  if (prevProps.isMobile !== nextProps.isMobile) return false;
  if (prevProps.onOpenReader !== nextProps.onOpenReader) return false;
  if (prevProps.onOpenEditor !== nextProps.onOpenEditor) return false;
  if (prevProps.onShowAnalysis !== nextProps.onShowAnalysis) return false;
  if (prevProps.onOpenSettings !== nextProps.onOpenSettings) return false;
  if (prevProps.onDeleteChapter !== nextProps.onDeleteChapter) return false;
  if (prevProps.onShowExpansionPlan !== nextProps.onShowExpansionPlan) return false;
  if (prevProps.onOpenPlanEditor !== nextProps.onOpenPlanEditor) return false;
  return areVisibleGenerationStatesEqualForChapters(
    prevProps.chapterGenerationStateById,
    nextProps.chapterGenerationStateById,
    nextProps.sortedChapters,
  );
});

type GroupedChapterPanelProps = ChapterListItemActionProps & {
  group: GroupedChapterViewModel;
  showOutlineActions: boolean;
  chapterGenerationStateById: ChapterGenerationStateMap;
};

const GroupedChapterPanel = memo(function GroupedChapterPanel({
  group,
  isMobile,
  showOutlineActions,
  chapterGenerationStateById,
  onOpenReader,
  onOpenEditor,
  onShowAnalysis,
  onOpenSettings,
  onDeleteChapter,
  onShowExpansionPlan,
  onOpenPlanEditor,
}: GroupedChapterPanelProps) {
  return (
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
            showOutlineActions={showOutlineActions}
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
  );
}, (prevProps, nextProps) => {
  if (prevProps.group.key !== nextProps.group.key) return false;
  if (prevProps.group.outlineId !== nextProps.group.outlineId) return false;
  if (prevProps.group.outlineTitle !== nextProps.group.outlineTitle) return false;
  if (prevProps.group.outlineOrder !== nextProps.group.outlineOrder) return false;
  if (prevProps.group.totalWordCount !== nextProps.group.totalWordCount) return false;
  if (!areChapterReferenceArraysEqual(prevProps.group.chapters, nextProps.group.chapters)) return false;
  if (prevProps.isMobile !== nextProps.isMobile) return false;
  if (prevProps.showOutlineActions !== nextProps.showOutlineActions) return false;
  if (prevProps.onOpenReader !== nextProps.onOpenReader) return false;
  if (prevProps.onOpenEditor !== nextProps.onOpenEditor) return false;
  if (prevProps.onShowAnalysis !== nextProps.onShowAnalysis) return false;
  if (prevProps.onOpenSettings !== nextProps.onOpenSettings) return false;
  if (prevProps.onDeleteChapter !== nextProps.onDeleteChapter) return false;
  if (prevProps.onShowExpansionPlan !== nextProps.onShowExpansionPlan) return false;
  if (prevProps.onOpenPlanEditor !== nextProps.onOpenPlanEditor) return false;
  return areVisibleGenerationStatesEqualForChapters(
    prevProps.chapterGenerationStateById,
    nextProps.chapterGenerationStateById,
    nextProps.group.chapters,
  );
});

function ChapterListSection({
  sortedChapters,
  outlineMode,
  groupedChapters,
  expandedChapterGroupKeys,
  isMobile,
  chapterGenerationStateById,
  onOpenReader,
  onOpenEditor,
  onShowAnalysis,
  onOpenSettings,
  onDeleteChapter,
  onShowExpansionPlan,
  onOpenPlanEditor,
}: ChapterListSectionProps) {
  if (sortedChapters.length === 0) {
    return <Empty description="暂无章节" />;
  }

  if (outlineMode === 'one-to-one') {
    return (
      <FlatChapterList
        sortedChapters={sortedChapters}
        isMobile={isMobile}
        chapterGenerationStateById={chapterGenerationStateById}
        onOpenReader={onOpenReader}
        onOpenEditor={onOpenEditor}
        onShowAnalysis={onShowAnalysis}
        onOpenSettings={onOpenSettings}
        onDeleteChapter={onDeleteChapter}
        onShowExpansionPlan={onShowExpansionPlan}
        onOpenPlanEditor={onOpenPlanEditor}
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
        <GroupedChapterPanel
          key={group.key}
          group={group}
          isMobile={isMobile}
          showOutlineActions={outlineMode === 'one-to-many'}
          chapterGenerationStateById={chapterGenerationStateById}
          onOpenReader={onOpenReader}
          onOpenEditor={onOpenEditor}
          onShowAnalysis={onShowAnalysis}
          onOpenSettings={onOpenSettings}
          onDeleteChapter={onDeleteChapter}
          onShowExpansionPlan={onShowExpansionPlan}
          onOpenPlanEditor={onOpenPlanEditor}
        />
      ))}
    </Collapse>
  );
}

export default memo(ChapterListSection, (prevProps, nextProps) => {
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
  const generationStatesEqual = nextProps.outlineMode === 'one-to-one'
    ? areVisibleGenerationStatesEqualForChapters(
        prevProps.chapterGenerationStateById,
        nextProps.chapterGenerationStateById,
        nextProps.sortedChapters,
      )
    : areVisibleGenerationStatesEqualForGroups(
        prevProps.chapterGenerationStateById,
        nextProps.chapterGenerationStateById,
        nextProps.groupedChapters,
      );

  if (!generationStatesEqual) {
    return false;
  }

  return true;
});
