import { memo } from 'react';
import { Collapse, Empty, List, Tag, Typography, theme } from 'antd';
import { CaretRightOutlined } from '@ant-design/icons';
import type { Chapter } from '../types';
import ChapterListItem from './ChapterListItem';
import { designDisplayFont } from '../theme/themeConfig';

const { Paragraph, Text, Title } = Typography;

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
  const { token } = theme.useToken();
  const alphaColor = (color: string, alpha: number) =>
    `color-mix(in srgb, ${color} ${(alpha * 100).toFixed(0)}%, transparent)`;
  const quietPanelBackground = `linear-gradient(180deg, color-mix(in srgb, ${token.colorBgContainer} 95%, ${token.colorFillAlter} 5%) 0%, color-mix(in srgb, ${token.colorBgContainer} 88%, ${token.colorFillAlter} 12%) 100%)`;
  const heroBackground = group.outlineId
    ? `linear-gradient(135deg, color-mix(in srgb, ${token.colorInfo} 14%, ${token.colorBgContainer} 86%) 0%, color-mix(in srgb, ${token.colorPrimary} 8%, ${token.colorBgContainer} 92%) 100%)`
    : `linear-gradient(135deg, color-mix(in srgb, ${token.colorWarning} 10%, ${token.colorBgContainer} 90%) 0%, color-mix(in srgb, ${token.colorPrimary} 6%, ${token.colorBgContainer} 94%) 100%)`;
  const groupLabel = group.outlineId ? `大纲 ${group.outlineOrder ?? '-'}` : '手动章节';
  const groupSummary = group.outlineId
    ? '这一组章节按当前大纲顺序组织，适合先看当前批次覆盖范围，再进入单章阅读、编辑或分析。'
    : '这一组章节来自手动创建链路，当前保留独立创作入口与后续规划动作。';

  return (
    <Collapse.Panel
      key={group.key}
      header={
        <div
          style={{
            display: 'grid',
            gap: 12,
            width: '100%',
          }}
        >
          <div
            style={{
              padding: isMobile ? '12px 12px 10px' : '14px 14px 12px',
              borderRadius: 18,
              background: heroBackground,
              border: `1px solid ${alphaColor(token.colorPrimary, 0.08)}`,
            }}
          >
            <Text
              style={{
                display: 'block',
                fontSize: 11,
                letterSpacing: '0.12em',
                textTransform: 'uppercase',
                color: token.colorTextTertiary,
              }}
            >
              Outline Workspace
            </Text>
            <Title
              level={isMobile ? 5 : 4}
              style={{
                margin: '6px 0 6px',
                fontFamily: designDisplayFont,
                fontSize: isMobile ? 16 : 18,
                lineHeight: 1.35,
                wordBreak: 'break-word',
              }}
            >
              {group.outlineTitle || '未命名大纲'}
            </Title>
            <Paragraph style={{ margin: 0, color: token.colorTextSecondary, lineHeight: 1.7 }}>
              {groupSummary}
            </Paragraph>
          </div>

          <div style={{ display: 'flex', alignItems: 'center', gap: 8, flexWrap: 'wrap' }}>
            <Tag color={group.outlineId ? 'blue' : 'default'} style={{ margin: 0, borderRadius: 999, flexShrink: 0 }}>
              {groupLabel}
            </Tag>
            <Tag color="green" style={{ margin: 0, borderRadius: 999 }}>
              章节数 {group.chapters.length}
            </Tag>
            <Tag color="blue" style={{ margin: 0, borderRadius: 999 }}>
              总字数 {group.totalWordCount}
            </Tag>
          </div>
        </div>
      }
      style={{
        marginBottom: 16,
        background: quietPanelBackground,
        borderRadius: 20,
        border: `1px solid ${alphaColor(token.colorPrimary, 0.08)}`,
        boxShadow: `0 20px 38px ${alphaColor(token.colorText, 0.06)}`,
        overflow: 'hidden',
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
  const { token } = theme.useToken();
  const alphaColor = (color: string, alpha: number) =>
    `color-mix(in srgb, ${color} ${(alpha * 100).toFixed(0)}%, transparent)`;
  const quietPanelBackground = `linear-gradient(180deg, color-mix(in srgb, ${token.colorBgContainer} 95%, ${token.colorFillAlter} 5%) 0%, color-mix(in srgb, ${token.colorBgContainer} 88%, ${token.colorFillAlter} 12%) 100%)`;

  if (sortedChapters.length === 0) {
    return (
      <div
        style={{
          borderRadius: 22,
          padding: '28px 20px',
          background: quietPanelBackground,
          border: `1px solid ${alphaColor(token.colorPrimary, 0.08)}`,
          boxShadow: `0 20px 38px ${alphaColor(token.colorText, 0.06)}`,
        }}
      >
        <Empty description="暂无章节" />
      </div>
    );
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
