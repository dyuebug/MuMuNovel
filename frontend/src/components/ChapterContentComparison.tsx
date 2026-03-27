import React, { useMemo, useState } from 'react';
import { Modal, Button, Card, Statistic, Row, Col, message, theme, Tag, Space, Typography } from 'antd';
import { CheckOutlined, CloseOutlined, SwapOutlined } from '@ant-design/icons';
import ReactDiffViewer from 'react-diff-viewer-continued';
import { chapterApi } from '../services/api';
import type { ChapterCandidateDraftQualityEvidence, ChapterCandidateDraftQualityFacet, ChapterCandidateDraftQualityHighlights } from '../types';

const { Text } = Typography;

interface ChapterContentComparisonProps {
  visible: boolean;
  onClose: () => void;
  chapterId: string;
  projectId?: string;
  chapterTitle: string;
  originalContent: string;
  newContent: string;
  wordCount: number;
  qualityHighlights?: ChapterCandidateDraftQualityHighlights | null;
  onApply?: () => void | Promise<void>;
  onDiscard?: () => void;
  onApplyAction?: () => Promise<boolean | void>;
  showDiscardButton?: boolean;
  applyButtonText?: string;
  discardButtonText?: string;
  modalTitle?: string;
  leftTitle?: string;
  rightTitle?: string;
}

const QUALITY_FACET_META = [
  { key: 'continuity', label: '连续性接力' },
  { key: 'foreshadow', label: '伏笔兑现' },
] as const;

function getFacetStatusColor(status?: string | null): string {
  switch ((status || '').trim().toLowerCase()) {
    case 'ok':
    case 'stable':
    case 'passed':
      return 'success';
    case 'warning':
    case 'pending':
      return 'warning';
    case 'error':
    case 'failed':
      return 'error';
    default:
      return 'default';
  }
}

function renderFacetItems(label: string, items: string[], color: string) {
  if (!items.length) {
    return null;
  }
  return (
    <div style={{ marginTop: 8 }}>
      <Text type="secondary">{label}</Text>
      <div style={{ marginTop: 6 }}>
        <Space size={[4, 8]} wrap>
          {items.map((item) => (
            <Tag key={`${label}-${item}`} color={color} style={{ marginInlineEnd: 0 }}>
              {item}
            </Tag>
          ))}
        </Space>
      </div>
    </div>
  );
}


function renderFacetEvidence(items: ChapterCandidateDraftQualityEvidence[]) {
  if (!items.length) {
    return null;
  }
  return (
    <div style={{ marginTop: 8 }}>
      <Text type="secondary">证据说明</Text>
      <Space direction="vertical" size={8} style={{ width: '100%', marginTop: 6 }}>
        {items.map((item, index) => (
          <Card key={`${item.item}-${index}`} size="small" style={{ background: 'rgba(0,0,0,0.02)' }}>
            <Space direction="vertical" size={4} style={{ width: '100%' }}>
              <Text strong>{item.item}</Text>
              <Text type="secondary">{item.snippet}</Text>
              {item.matched_anchors.length > 0 && (
                <Space size={[4, 4]} wrap>
                  {item.matched_anchors.map((anchor) => (
                    <Tag key={`${item.item}-${anchor}`} color="blue" style={{ marginInlineEnd: 0 }}>
                      {anchor}
                    </Tag>
                  ))}
                </Space>
              )}
            </Space>
          </Card>
        ))}
      </Space>
    </div>
  );
}

const ChapterContentComparison: React.FC<ChapterContentComparisonProps> = ({
  visible,
  onClose,
  chapterId,
  projectId,
  chapterTitle,
  originalContent,
  newContent,
  wordCount,
  qualityHighlights,
  onApply,
  onDiscard,
  onApplyAction,
  showDiscardButton = true,
  applyButtonText = '应用新内容',
  discardButtonText = '放弃新内容',
  modalTitle,
  leftTitle = '原内容',
  rightTitle = '新内容',
}) => {
  const { token } = theme.useToken();
  const [applying, setApplying] = useState(false);
  const [viewMode, setViewMode] = useState<'split' | 'unified'>('split');
  const [modal, contextHolder] = Modal.useModal();

  const visibleQualityFacets = useMemo(
    () => QUALITY_FACET_META
      .map(({ key, label }) => ({
        key,
        label,
        facet: (qualityHighlights?.[key] || null) as ChapterCandidateDraftQualityFacet | null,
      }))
      .filter((item) => item.facet && (item.facet.summary || item.facet.matched_items.length || item.facet.missing_items.length || item.facet.repair_targets.length || (item.facet.matched_evidence?.length ?? 0))),
    [qualityHighlights]
  );

  const originalWordCount = originalContent.length;
  const wordCountDiff = wordCount - originalWordCount;
  const wordCountDiffPercent = originalWordCount > 0
    ? ((wordCountDiff / originalWordCount) * 100).toFixed(1)
    : (wordCount === 0 ? '0.0' : '100.0');
  const resolvedModalTitle = modalTitle || `内容对比 - ${chapterTitle}`;
  const hasQualityHighlights = visibleQualityFacets.length > 0;
  const diffMaxHeight = hasQualityHighlights ? 'calc(90vh - 560px)' : 'calc(90vh - 300px)';

  const handleApply = async () => {
    setApplying(true);
    try {
      if (onApplyAction) {
        const result = await onApplyAction();
        if (result === false) {
          return;
        }
        onClose();
        return;
      }

      const response = await fetch(`/api/chapters/${chapterId}`, {
        method: 'PUT',
        headers: {
          'Content-Type': 'application/json',
        },
        body: JSON.stringify({
          content: newContent,
        }),
      });

      if (!response.ok) {
        throw new Error('应用新内容失败');
      }

      message.success('新内容已应用！');
      await Promise.resolve(onApply?.());

      setTimeout(async () => {
        try {
          await chapterApi.triggerChapterAnalysis(chapterId, projectId);
          message.success('章节分析已开始，请稍后查看结果');
        } catch (analysisError) {
          console.error('Failed to trigger chapter analysis:', analysisError);
          message.warning('章节分析触发失败，您可以手动触发分析');
        }
      }, 500);

      onClose();
    } catch (error: unknown) {
      const err = error as Error;
      message.error(err.message || '应用失败');
    } finally {
      setApplying(false);
    }
  };

  const handleDiscard = () => {
    if (!onDiscard) {
      onClose();
      return;
    }

    modal.confirm({
      title: '确认放弃',
      content: '确定要放弃新生成的内容吗？此操作不可恢复。',
      centered: true,
      okText: '确定放弃',
      cancelText: '取消',
      okButtonProps: { danger: true },
      onOk: () => {
        onDiscard();
        onClose();
        message.info('已放弃新内容');
      },
    });
  };

  const footerActions = [
    ...(showDiscardButton && onDiscard
      ? [
          <Button
            key="discard"
            danger
            icon={<CloseOutlined />}
            onClick={handleDiscard}
          >
            {discardButtonText}
          </Button>,
        ]
      : []),
    <Button key="close" onClick={onClose}>
      {'关闭'}
    </Button>,
    <Button
      key="toggle"
      icon={<SwapOutlined />}
      onClick={() => setViewMode(viewMode === 'split' ? 'unified' : 'split')}
    >
      {'切换视图'}
    </Button>,
    <Button
      key="apply"
      type="primary"
      icon={<CheckOutlined />}
      loading={applying}
      onClick={handleApply}
    >
      {applyButtonText}
    </Button>,
  ];

  return (
    <>
      {contextHolder}
      <Modal
        title={resolvedModalTitle}
        open={visible}
        onCancel={onClose}
        width="95%"
        centered
        style={{ maxWidth: 1600 }}
        footer={footerActions}
      >
        <Card size="small" style={{ marginBottom: 16 }}>
          <Row gutter={16}>
            <Col span={6}>
              <Statistic
                title="原内容字数"
                value={originalWordCount}
                suffix="字"
              />
            </Col>
            <Col span={6}>
              <Statistic
                title="新内容字数"
                value={wordCount}
                suffix="字"
              />
            </Col>
            <Col span={6}>
              <Statistic
                title="字数变化"
                value={wordCountDiff}
                suffix="字"
                valueStyle={{ color: wordCountDiff > 0 ? 'var(--color-success)' : 'var(--color-error)' }}
                prefix={wordCountDiff > 0 ? '+' : ''}
              />
            </Col>
            <Col span={6}>
              <Statistic
                title="变化比例"
                value={wordCountDiffPercent}
                suffix="%"
                valueStyle={{ color: Math.abs(parseFloat(wordCountDiffPercent)) < 10 ? 'var(--color-primary)' : 'var(--color-warning)' }}
                prefix={wordCountDiff > 0 ? '+' : ''}
              />
            </Col>
          </Row>
        </Card>

        {hasQualityHighlights && (
          <Card size="small" title="候选稿质量摘要" style={{ marginBottom: 16 }}>
            <Row gutter={[16, 16]}>
              {visibleQualityFacets.map(({ key, label, facet }) => {
                if (!facet) {
                  return null;
                }
                return (
                  <Col xs={24} md={12} key={key}>
                    <Card
                      size="small"
                      style={{
                        height: '100%',
                        background: token.colorBgLayout,
                        borderColor: token.colorBorderSecondary,
                      }}
                    >
                      <Space direction="vertical" size={6} style={{ width: '100%' }}>
                        <Space wrap>
                          <Text strong>{label}</Text>
                          <Tag color={getFacetStatusColor(facet.status)}>{facet.status || 'unknown'}</Tag>
                        </Space>
                        {facet.summary ? <Text>{facet.summary}</Text> : <Text type="secondary">{'暂无质量摘要'}</Text>}
                        {renderFacetItems('已命中', facet.matched_items, 'success')}
                        {renderFacetItems('待补齐', facet.missing_items, 'warning')}
                        {renderFacetItems('修复目标', facet.repair_targets, 'processing')}
                        {renderFacetEvidence(facet.matched_evidence || [])}
                      </Space>
                    </Card>
                  </Col>
                );
              })}
            </Row>
          </Card>
        )}

        <div style={{
          maxHeight: diffMaxHeight,
          overflow: 'auto',
          border: '1px solid var(--color-border)',
          borderRadius: 4,
        }}>
          <ReactDiffViewer
            oldValue={originalContent}
            newValue={newContent}
            splitView={viewMode === 'split'}
            leftTitle={leftTitle}
            rightTitle={rightTitle}
            showDiffOnly={false}
            useDarkTheme={false}
            styles={{
              variables: {
                light: {
                  diffViewerBackground: token.colorBgContainer,
                  addedBackground: 'var(--color-success-bg)',
                  addedColor: 'var(--color-text-primary)',
                  removedBackground: 'var(--color-error-bg)',
                  removedColor: 'var(--color-text-primary)',
                  wordAddedBackground: 'var(--color-success-border)',
                  wordRemovedBackground: 'var(--color-error-border)',
                  addedGutterBackground: 'var(--color-success-bg)',
                  removedGutterBackground: 'var(--color-error-bg)',
                  gutterBackground: 'var(--color-bg-layout)',
                  gutterBackgroundDark: 'var(--color-bg-container)',
                  highlightBackground: 'var(--color-warning-bg)',
                  highlightGutterBackground: 'var(--color-warning-border)',
                },
              },
              line: {
                padding: '10px 2px',
                fontSize: '14px',
                lineHeight: '20px',
                whiteSpace: 'pre-wrap',
                wordBreak: 'break-word',
              },
            }}
          />
        </div>
      </Modal>
    </>
  );
};

export default ChapterContentComparison;
