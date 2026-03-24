import { memo } from 'react';
import type { CSSProperties } from 'react';
import { Badge, Button, List, Popconfirm, Space, Tag } from 'antd';
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

type ChapterListItemVariant = 'flat' | 'grouped';

type ChapterListItemProps = {
  chapter: Chapter;
  variant: ChapterListItemVariant;
  isMobile: boolean;
  showOutlineActions: boolean;
  analysisTask?: AnalysisTask;
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
    draft: '??',
    writing: '???',
    completed: '???',
  };

  return texts[status] || status;
};

const isAnalysisTaskInProgress = (task?: AnalysisTask | null): boolean => (
  task?.status === 'pending' || task?.status === 'running'
);

const renderAnalysisStatus = (task?: AnalysisTask) => {
  if (!task) {
    return null;
  }

  switch (task.status) {
    case 'pending':
      return (
        <Tag icon={<SyncOutlined spin />} color="processing">
          ???
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
          {isRetrying ? `??? ${task.progress}%` : `??? ${task.progress}%`}
        </Tag>
      );
    }
    case 'completed':
      return (
        <Tag icon={<CheckCircleOutlined />} color="success">
          ???
        </Tag>
      );
    case 'failed':
      return (
        <Tag icon={<CloseCircleOutlined />} color="error" title={task.error_message || undefined}>
          ??
        </Tag>
      );
    default:
      return null;
  }
};



function ChapterListItem({
  chapter,
  variant,
  isMobile,
  showOutlineActions,
  analysisTask,
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
  const hasContent = Boolean(chapter.content?.trim());
  const isAnalyzing = isAnalysisTaskInProgress(analysisTask);
  const previewLimit = isMobile ? 80 : 150;
  const previewText = chapter.content ? chapter.content.substring(0, previewLimit) : '';
  const hasMorePreview = Boolean(chapter.content && chapter.content.length > previewLimit);
  const titleText = variant === 'flat'
    ? `#${chapter.chapter_number} ${chapter.title}`
    : `?${chapter.chapter_number}??${chapter.title}`;

  const itemStyle: CSSProperties = variant === 'flat'
    ? {
        padding: '16px',
        marginBottom: 16,
        background: '#fff',
        borderRadius: 8,
        border: '1px solid #f0f0f0',
        flexDirection: isMobile ? 'column' : 'row',
        alignItems: isMobile ? 'flex-start' : 'center',
      }
    : {
        padding: '16px 0',
        borderRadius: 8,
        transition: 'background 0.3s ease',
        flexDirection: isMobile ? 'column' : 'row',
        alignItems: isMobile ? 'flex-start' : 'center',
      };

  const analysisButtonTitle = !hasContent ? '???????' : isAnalyzing ? '???...' : '????';
  const analysisButtonText = isAnalyzing ? '???' : '??';

  const desktopActions = isMobile ? undefined : [
    (
      <Button
        key="read"
        type="text"
        icon={<ReadOutlined />}
        onClick={() => onOpenReader(chapter)}
        disabled={!hasContent}
        title={!hasContent ? '????' : '??'}
      >
        ??
      </Button>
    ),
    (
      <Button
        key="edit"
        type="text"
        icon={<EditOutlined />}
        onClick={() => onOpenEditor(chapter.id)}
      >
        ??
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
        ??
      </Button>
    ),
    ...(showOutlineActions
      ? [
          (
            <Popconfirm
              key="delete"
              title="??????"
              description="?????????????"
              onConfirm={() => onDeleteChapter(chapter.id)}
              okText="??"
              cancelText="??"
              okButtonProps={{ danger: true }}
            >
              <Button type="text" danger icon={<DeleteOutlined />}>
                ??
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
          avatar={!isMobile && <FileTextOutlined style={{ fontSize: 32, color: 'var(--color-primary)' }} />}
          title={
            <div
              style={{
                display: 'flex',
                flexDirection: isMobile ? 'column' : 'row',
                alignItems: isMobile ? 'flex-start' : 'center',
                gap: isMobile ? 6 : 12,
                width: '100%',
              }}
            >
              <span style={{ fontSize: isMobile ? 14 : 16, fontWeight: 500, flexShrink: 0 }}>
                {titleText}
              </span>
              <Space wrap size={isMobile ? 4 : 8}>
                <Tag color={getStatusColor(chapter.status)}>{getStatusText(chapter.status)}</Tag>
                <Badge count={`${chapter.word_count || 0}?`} style={{ backgroundColor: 'var(--color-success)' }} />
                {renderAnalysisStatus(analysisTask)}
                {!canGenerate ? (
                  <Tag icon={<LockOutlined />} color="warning" title={generateDisabledReason}>
                    ?????
                  </Tag>
                ) : null}
                {showOutlineActions ? (
                  <Space size={4}>
                    {chapter.expansion_plan ? (
                      <InfoCircleOutlined
                        title="??????"
                        style={{ color: 'var(--color-primary)', cursor: 'pointer', fontSize: 16 }}
                        onClick={(event) => {
                          event.stopPropagation();
                          onShowExpansionPlan(chapter);
                        }}
                      />
                    ) : null}
                    <FormOutlined
                      title={chapter.expansion_plan ? '??????' : '??????'}
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
              <div style={{ marginTop: 8, color: 'rgba(0,0,0,0.65)', lineHeight: 1.6, fontSize: isMobile ? 12 : 14 }}>
                {previewText}
                {hasMorePreview ? '...' : ''}
              </div>
            ) : (
              <span style={{ color: 'rgba(0,0,0,0.45)', fontSize: isMobile ? 12 : 14 }}>????</span>
            )
          }
        />

        {isMobile ? (
          <Space style={{ marginTop: 12, width: '100%', justifyContent: 'flex-end' }} wrap>
            <Button
              type="text"
              icon={<ReadOutlined />}
              onClick={() => onOpenReader(chapter)}
              size="small"
              disabled={!hasContent}
              title={!hasContent ? '????' : '??'}
            />
            <Button
              type="text"
              icon={<EditOutlined />}
              onClick={() => onOpenEditor(chapter.id)}
              size="small"
              title="??"
            />
            <Button
              type="text"
              icon={isAnalyzing ? <SyncOutlined spin /> : <FundOutlined />}
              onClick={() => onShowAnalysis(chapter.id)}
              size="small"
              disabled={!hasContent || isAnalyzing}
              loading={isAnalyzing}
              title={analysisButtonTitle}
            />
            <Button
              type="text"
              icon={<SettingOutlined />}
              onClick={() => onOpenSettings(chapter.id)}
              size="small"
              title="??"
            />
            {showOutlineActions ? (
              <Popconfirm
                title="??????"
                description="?????????????"
                onConfirm={() => onDeleteChapter(chapter.id)}
                okText="??"
                cancelText="??"
                okButtonProps={{ danger: true }}
              >
                <Button
                  type="text"
                  danger
                  icon={<DeleteOutlined />}
                  size="small"
                  title="??"
                />
              </Popconfirm>
            ) : null}
          </Space>
        ) : null}
      </div>
    </List.Item>
  );
}

export default memo(ChapterListItem);
