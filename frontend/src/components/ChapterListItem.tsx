import { memo } from 'react';
import type { CSSProperties } from 'react';
import { Badge, Button, List, Popconfirm, Space, Tag, Typography, theme } from 'antd';
import {
  CheckCircleOutlined,
  CloseCircleOutlined,
  DeleteOutlined,
  EditOutlined,
  FileTextOutlined,
  FormOutlined,
  FundOutlined,
  InfoCircleOutlined,
  LockOutlined,
  ReadOutlined,
  SettingOutlined,
  SyncOutlined,
} from '@ant-design/icons';
import type { Chapter, AnalysisTask } from '../types';
import { useChapterAnalysisUiStore } from '../store/chapterAnalysisUi';
import { designDisplayFont } from '../theme/themeConfig';

type ChapterListItemVariant = 'flat' | 'grouped';

type ChapterListItemProps = {
  chapter: Chapter;
  variant: ChapterListItemVariant;
  isMobile: boolean;
  showOutlineActions: boolean;
  canGenerate: boolean;
  generateDisabledReason: string;
  onOpenReader: (chapter: Chapter) => void;
  onOpenEditor: (chapterId: string) => void;
  onShowAnalysis: (chapterId: string) => void;
  onOpenSettings: (chapterId: string) => void;
  onDeleteChapter: (chapterId: string) => void;
  onShowExpansionPlan: (chapter: Chapter) => void;
  onOpenPlanEditor: (chapter: Chapter) => void;
};

const getStatusColor = (status: string): string => {
  const colors: Record<string, string> = {
    draft: 'default',
    writing: 'processing',
    completed: 'success',
  };

  return colors[status] || 'default';
};

const getStatusText = (status: string): string => {
  const texts: Record<string, string> = {
    draft: '草稿',
    writing: '写作中',
    completed: '已完成',
  };

  return texts[status] || status;
};

const isAnalysisTaskInProgress = (task?: AnalysisTask | null): boolean => (
  task?.status === 'pending' || task?.status === 'running'
);

const mobileActionButtonStyle: CSSProperties = {
  minHeight: 34,
  paddingInline: 12,
  borderRadius: 12,
};

const { Paragraph, Text, Title } = Typography;

const renderAnalysisStatus = (task?: AnalysisTask) => {
  if (!task) {
    return null;
  }

  switch (task.status) {
    case 'pending':
      return (
        <Tag icon={<SyncOutlined spin />} color="processing">
          {"等待分析"}
        </Tag>
      );
    case 'running': {
      const isRetrying = task.error_code === 'retrying';
      return (
        <Tag
          icon={<SyncOutlined spin />}
          color={isRetrying ? 'warning' : 'processing'}
          title={task.error_message || undefined}
        >
          {isRetrying ? `重试中 ${task.progress}%` : `分析中 ${task.progress}%`}
        </Tag>
      );
    }
    case 'completed':
      return (
        <Tag icon={<CheckCircleOutlined />} color="success">
          {"分析完成"}
        </Tag>
      );
    case 'failed':
      return (
        <Tag icon={<CloseCircleOutlined />} color="error" title={task.error_message || undefined}>
          {"分析失败"}
        </Tag>
      );
    default:
      return null;
  }
};

const areChapterPropsEqual = (left: Chapter, right: Chapter): boolean => (
  left.id === right.id
  && left.chapter_number === right.chapter_number
  && left.title === right.title
  && (left.content ?? '') === (right.content ?? '')
  && (left.word_count ?? 0) === (right.word_count ?? 0)
  && (left.status ?? '') === (right.status ?? '')
  && (left.outline_id ?? null) === (right.outline_id ?? null)
  && (left.outline_order ?? null) === (right.outline_order ?? null)
  && (left.outline_title ?? '') === (right.outline_title ?? '')
  && (left.expansion_plan ?? '') === (right.expansion_plan ?? '')
);

function ChapterListItem({
  chapter,
  variant,
  isMobile,
  showOutlineActions,
  canGenerate,
  generateDisabledReason,
  onOpenReader,
  onOpenEditor,
  onShowAnalysis,
  onOpenSettings,
  onDeleteChapter,
  onShowExpansionPlan,
  onOpenPlanEditor,
}: ChapterListItemProps) {
  const { token } = theme.useToken();
  const alphaColor = (color: string, alpha: number) =>
    `color-mix(in srgb, ${color} ${(alpha * 100).toFixed(0)}%, transparent)`;
  const analysisTask = useChapterAnalysisUiStore((state) => state.tasksMap[chapter.id]);
  const hasContent = Boolean(chapter.content?.trim());
  const isAnalyzing = isAnalysisTaskInProgress(analysisTask);
  const previewLimit = isMobile ? 80 : 150;
  const previewText = chapter.content ? chapter.content.substring(0, previewLimit) : '';
  const hasMorePreview = Boolean(chapter.content && chapter.content.length > previewLimit);
  const titleText = variant === 'flat'
    ? `#${chapter.chapter_number} ${chapter.title}`
    : `第${chapter.chapter_number}章：${chapter.title}`;
  const chapterEyebrow = variant === 'flat' ? 'Chapter Workspace' : 'Outline Chapter';
  const heroBackground = variant === 'flat'
    ? `linear-gradient(135deg, color-mix(in srgb, ${token.colorPrimary} 14%, ${token.colorBgContainer} 86%) 0%, color-mix(in srgb, ${token.colorWarning} 8%, ${token.colorBgContainer} 92%) 100%)`
    : `linear-gradient(135deg, color-mix(in srgb, ${token.colorInfo} 14%, ${token.colorBgContainer} 86%) 0%, color-mix(in srgb, ${token.colorPrimary} 8%, ${token.colorBgContainer} 92%) 100%)`;
  const quietPanelBackground = `linear-gradient(180deg, color-mix(in srgb, ${token.colorBgContainer} 95%, ${token.colorFillAlter} 5%) 0%, color-mix(in srgb, ${token.colorBgContainer} 88%, ${token.colorFillAlter} 12%) 100%)`;

  const itemStyle: CSSProperties = variant === 'flat'
    ? {
        padding: '18px',
        marginBottom: 16,
        background: quietPanelBackground,
        borderRadius: 20,
        border: `1px solid ${alphaColor(token.colorPrimary, 0.08)}`,
        boxShadow: `0 20px 38px ${alphaColor(token.colorText, 0.06)}`,
        flexDirection: isMobile ? 'column' : 'row',
        alignItems: isMobile ? 'flex-start' : 'center',
      }
    : {
        padding: '18px 12px',
        borderRadius: 18,
        transition: 'background 0.3s ease, border-color 0.3s ease',
        background: quietPanelBackground,
        border: `1px solid ${token.colorBorderSecondary}`,
        flexDirection: isMobile ? 'column' : 'row',
        alignItems: isMobile ? 'flex-start' : 'center',
      };

  const analysisButtonTitle = !hasContent ? '暂无内容，无法分析' : isAnalyzing ? '分析中...' : '分析章节';
  const analysisButtonText = isAnalyzing ? '分析中' : '分析';

  const desktopActions = isMobile ? undefined : [
    (
      <Button
        key="read"
        type="text"
        icon={<ReadOutlined />}
        onClick={() => onOpenReader(chapter)}
        disabled={!hasContent}
        title={!hasContent ? '暂无内容可阅读' : '阅读'}
      >
        {"阅读"}
      </Button>
    ),
    (
      <Button
        key="edit"
        type="text"
        icon={<EditOutlined />}
        onClick={() => onOpenEditor(chapter.id)}
      >
        {"编辑"}
      </Button>
    ),
    (
      <Button
        key="analysis"
        type="text"
        icon={isAnalyzing ? <SyncOutlined spin /> : <FundOutlined />}
        onClick={() => onShowAnalysis(chapter.id)}
        disabled={!hasContent || isAnalyzing}
        loading={isAnalyzing}
        title={analysisButtonTitle}
      >
        {analysisButtonText}
      </Button>
    ),
    (
      <Button
        key="settings"
        type="text"
        icon={<SettingOutlined />}
        onClick={() => onOpenSettings(chapter.id)}
      >
        {"设置"}
      </Button>
    ),
    ...(showOutlineActions
      ? [
          (
            <Popconfirm
              key="delete"
              title="确认删除"
              description="删除后无法恢复，确认继续吗？"
              onConfirm={() => onDeleteChapter(chapter.id)}
              okText="确认删除"
              cancelText="取消"
              okButtonProps={{ danger: true }}
            >
              <Button type="text" danger icon={<DeleteOutlined />}>
                {"删除"}
              </Button>
            </Popconfirm>
          ),
        ]
      : []),
  ];

  return (
    <List.Item id={`chapter-item-${chapter.id}`} style={itemStyle} actions={desktopActions}>
      <div style={{ width: '100%' }}>
        <List.Item.Meta
          avatar={!isMobile && (
            <div
              style={{
                width: 42,
                height: 42,
                borderRadius: 14,
                display: 'inline-flex',
                alignItems: 'center',
                justifyContent: 'center',
                background: alphaColor(token.colorPrimary, 0.14),
                color: token.colorPrimary,
              }}
            >
              <FileTextOutlined style={{ fontSize: 22 }} />
            </div>
          )}
          title={
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
                  {chapterEyebrow}
                </Text>
                <Title
                  level={isMobile ? 5 : 4}
                  style={{
                    margin: '6px 0 0',
                    fontFamily: designDisplayFont,
                    fontSize: isMobile ? 16 : 18,
                    lineHeight: 1.35,
                    minWidth: 0,
                    wordBreak: 'break-word',
                    overflowWrap: 'anywhere',
                  }}
                >
                  {titleText}
                </Title>
              </div>

              <Space wrap size={isMobile ? 4 : 8} style={{ minWidth: 0 }}>
                <Tag color={getStatusColor(chapter.status)} style={{ margin: 0, borderRadius: 999 }}>
                  {getStatusText(chapter.status)}
                </Tag>
                <Badge count={`${chapter.word_count || 0}字`} style={{ backgroundColor: token.colorSuccess }} />
                {renderAnalysisStatus(analysisTask)}
                {!canGenerate ? (
                  <Tag icon={<LockOutlined />} color="warning" title={generateDisabledReason} style={{ margin: 0, borderRadius: 999 }}>
                    {"暂不可生成"}
                  </Tag>
                ) : null}
                {showOutlineActions ? (
                  <Space size={4}>
                    {chapter.expansion_plan ? (
                      <InfoCircleOutlined
                        title="查看扩写计划"
                        style={{ color: 'var(--color-primary)', cursor: 'pointer', fontSize: 16 }}
                        onClick={(event) => {
                          event.stopPropagation();
                          onShowExpansionPlan(chapter);
                        }}
                      />
                    ) : null}
                    <FormOutlined
                      title={chapter.expansion_plan ? '编辑章节规划' : '新建章节规划'}
                      style={{ color: 'var(--color-success)', cursor: 'pointer', fontSize: 16 }}
                      onClick={(event) => {
                        event.stopPropagation();
                        onOpenPlanEditor(chapter);
                      }}
                    />
                  </Space>
                ) : null}
              </Space>
            </div>
          }
          description={
            hasContent ? (
              <div
                style={{
                  marginTop: 10,
                  padding: isMobile ? '12px 12px' : '14px 14px',
                  borderRadius: 16,
                  background: token.colorBgContainer,
                  border: `1px solid ${token.colorBorderSecondary}`,
                }}
              >
                <Text style={{ display: 'block', fontSize: 12, color: token.colorTextTertiary, marginBottom: 6 }}>
                  正文摘录
                </Text>
                <Paragraph
                  style={{
                    margin: 0,
                    color: token.colorTextSecondary,
                    lineHeight: 1.7,
                    fontSize: isMobile ? 12 : 14,
                    wordBreak: 'break-word',
                    overflowWrap: 'anywhere',
                  }}
                >
                  {previewText}
                  {hasMorePreview ? '...' : ''}
                </Paragraph>
              </div>
            ) : (
              <div
                style={{
                  marginTop: 10,
                  padding: isMobile ? '12px 12px' : '14px 14px',
                  borderRadius: 16,
                  background: token.colorBgContainer,
                  border: `1px dashed ${token.colorBorderSecondary}`,
                }}
              >
                <Text style={{ color: token.colorTextTertiary, fontSize: isMobile ? 12 : 13 }}>
                  暂无正文，当前条目仍保留阅读、编辑和后续创作入口。
                </Text>
              </div>
            )
          }
        />

        {isMobile ? (
          <Space style={{ marginTop: 12, width: '100%', justifyContent: 'flex-start' }} wrap size={8}>
            <Button
              type="default"
              icon={<ReadOutlined />}
              onClick={() => onOpenReader(chapter)}
              size="middle"
              style={mobileActionButtonStyle}
              disabled={!hasContent}
              title={!hasContent ? '暂无内容可阅读' : '阅读'}
            >
              {"阅读"}
            </Button>
            <Button
              type="default"
              icon={<EditOutlined />}
              onClick={() => onOpenEditor(chapter.id)}
              size="middle"
              style={mobileActionButtonStyle}
              title="编辑"
            >
              {"编辑"}
            </Button>
            <Button
              type="default"
              icon={isAnalyzing ? <SyncOutlined spin /> : <FundOutlined />}
              onClick={() => onShowAnalysis(chapter.id)}
              size="middle"
              style={mobileActionButtonStyle}
              disabled={!hasContent || isAnalyzing}
              loading={isAnalyzing}
              title={analysisButtonTitle}
            >
              {analysisButtonText}
            </Button>
            <Button
              type="default"
              icon={<SettingOutlined />}
              onClick={() => onOpenSettings(chapter.id)}
              size="middle"
              style={mobileActionButtonStyle}
              title="设置"
            >
              {"设置"}
            </Button>
            {showOutlineActions ? (
              <Popconfirm
                title="确认删除"
                description="删除后无法恢复，确认继续吗？"
                onConfirm={() => onDeleteChapter(chapter.id)}
                okText="确认删除"
                cancelText="取消"
                okButtonProps={{ danger: true }}
              >
                <Button
                  type="default"
                  danger
                  icon={<DeleteOutlined />}
                  size="middle"
                  style={mobileActionButtonStyle}
                  title="删除"
                >
                  {"删除"}
                </Button>
              </Popconfirm>
            ) : null}
          </Space>
        ) : null}
      </div>
    </List.Item>
  );
}

export default memo(ChapterListItem, (prevProps, nextProps) => (
  areChapterPropsEqual(prevProps.chapter, nextProps.chapter)
  && prevProps.variant === nextProps.variant
  && prevProps.isMobile === nextProps.isMobile
  && prevProps.showOutlineActions === nextProps.showOutlineActions
  && prevProps.canGenerate === nextProps.canGenerate
  && prevProps.generateDisabledReason === nextProps.generateDisabledReason
  && prevProps.onOpenReader === nextProps.onOpenReader
  && prevProps.onOpenEditor === nextProps.onOpenEditor
  && prevProps.onShowAnalysis === nextProps.onShowAnalysis
  && prevProps.onOpenSettings === nextProps.onOpenSettings
  && prevProps.onDeleteChapter === nextProps.onDeleteChapter
  && prevProps.onShowExpansionPlan === nextProps.onShowExpansionPlan
  && prevProps.onOpenPlanEditor === nextProps.onOpenPlanEditor
));
