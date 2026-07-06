import React, { useEffect, useMemo, useRef, useState } from 'react';
import { Modal, Button, Card, Statistic, Row, Col, message, theme, Tag, Space, Typography } from 'antd';
import { CheckOutlined, CloseOutlined, SwapOutlined } from '@ant-design/icons';
import ReactDiffViewer from 'react-diff-viewer-continued';
import { chapterApi } from '../services/modularApi';
import type { ChapterCandidateDraftQualityEvidence, ChapterCandidateDraftQualityFacet, ChapterCandidateDraftQualityHighlights } from '../types';
import { designDisplayFont } from '../theme/themeConfig';

const { Text, Paragraph, Title } = Typography;

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
  const mountedRef = useRef(true);
  const applyRequestIdRef = useRef(0);
  const triggerAnalysisTimerRef = useRef<number | null>(null);

  useEffect(() => {
    mountedRef.current = true;
    return () => {
      mountedRef.current = false;
      applyRequestIdRef.current += 1;
      if (triggerAnalysisTimerRef.current) {
        window.clearTimeout(triggerAnalysisTimerRef.current);
        triggerAnalysisTimerRef.current = null;
      }
    };
  }, []);

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
  const heroBackground = `linear-gradient(135deg,
    color-mix(in srgb, ${token.colorPrimary} 78%, #6f3d2f 22%) 0%,
    color-mix(in srgb, ${token.colorInfo} 34%, #1f262e 66%) 100%)`;
  const panelBorder = `1px solid color-mix(in srgb, ${token.colorBorderSecondary} 88%, white 12%)`;
  const quietPanelBackground = `linear-gradient(180deg,
    color-mix(in srgb, ${token.colorBgContainer} 96%, white 4%) 0%,
    color-mix(in srgb, ${token.colorFillAlter} 36%, ${token.colorBgContainer} 64%) 100%)`;
  const modalSurfaceStyles = {
    header: {
      padding: '18px 24px 0',
      borderBottom: 'none',
      background: quietPanelBackground,
    },
    body: {
      padding: 20,
      background: quietPanelBackground,
    },
    footer: {
      padding: '0 24px 20px',
      borderTop: 'none',
      background: quietPanelBackground,
    },
    content: {
      borderRadius: 24,
      overflow: 'hidden',
      border: panelBorder,
      boxShadow: `0 24px 52px color-mix(in srgb, ${token.colorText} 12%, transparent)`,
    },
  } as const;
  const comparisonGuideSteps = [
    '先读顶部焦点与字数摘要，确认这次是在做候选稿采纳判断，而不是直接进入逐行修改。',
    '再结合质量摘要看连续性和伏笔兑现，再下钻到 diff 里核对原文与候选稿差异。',
    '最后再决定应用、放弃或切换视图，原有应用、放弃和分析触发逻辑保持不变。',
  ];
  const comparisonFocus = applying
    ? {
        title: '当前正在应用候选稿，请等待写入与后续分析触发完成',
        note: '提交过程中不需要重复点击，现有应用、关闭与章节分析触发逻辑保持不变。',
        tags: [
          { label: '应用中', color: 'processing' },
          { label: viewMode === 'split' ? '分栏对照' : '统一视图', color: 'blue' },
        ],
      }
    : hasQualityHighlights
      ? {
          title: '先判断质量摘要，再决定是否采纳候选稿',
          note: '这次更适合先读质量摘要中的连续性与伏笔提示，再回到逐行 diff 做最终判断。',
          tags: [
            { label: `质量维度 ${visibleQualityFacets.length} 项`, color: 'gold' },
            { label: viewMode === 'split' ? '分栏对照' : '统一视图', color: 'blue' },
            showDiscardButton ? { label: '可放弃候选稿', color: 'default' } : { label: '仅支持应用或关闭', color: 'green' },
          ],
        }
      : {
          title: '当前适合直接对照原文与候选稿差异',
          note: '没有额外质量摘要时，可以先看字数变化与视图模式，再进入逐行比对做采纳判断。',
          tags: [
            { label: '直接 diff 审阅', color: 'processing' },
            { label: viewMode === 'split' ? '分栏对照' : '统一视图', color: 'blue' },
          ],
        };

  const renderModalHero = (eyebrow: string, title: string, description: string) => (
    <Card
      bordered={false}
      style={{
        marginBottom: 16,
        borderRadius: 20,
        overflow: 'hidden',
        background: heroBackground,
      }}
      styles={{ body: { padding: 20 } }}
    >
      <Text style={{ color: 'color-mix(in srgb, #ffffff 68%, transparent)', letterSpacing: '0.14em', textTransform: 'uppercase' }}>
        {eyebrow}
      </Text>
      <Title level={5} style={{ margin: '8px 0 10px', color: '#f7f1e8', fontFamily: designDisplayFont, letterSpacing: '-0.03em' }}>
        {title}
      </Title>
      <Paragraph style={{ margin: 0, color: 'color-mix(in srgb, #ffffff 82%, transparent)', lineHeight: 1.7 }}>
        {description}
      </Paragraph>
    </Card>
  );

  const renderGuidePanel = (
    guideLabel: string,
    guideTitle: string,
    guideDescription: string,
    guideSteps: string[],
    focusTitle: string,
    focusNote: string,
    focusTags: Array<{ label: string; color: string }>,
  ) => (
    <Card
      bordered={false}
      style={{
        marginBottom: 16,
        borderRadius: 18,
        background: quietPanelBackground,
        border: panelBorder,
      }}
      styles={{ body: { padding: 18 } }}
    >
      <div
        style={{
          display: 'grid',
          gridTemplateColumns: 'repeat(auto-fit, minmax(220px, 1fr))',
          gap: 16,
        }}
      >
        <div>
          <Text style={{ fontSize: 12, letterSpacing: '0.12em', textTransform: 'uppercase', color: token.colorTextTertiary }}>
            {guideLabel}
          </Text>
          <Title level={5} style={{ margin: '6px 0 8px', fontFamily: designDisplayFont }}>
            {guideTitle}
          </Title>
          <Paragraph style={{ margin: 0, color: token.colorTextSecondary, lineHeight: 1.75 }}>
            {guideDescription}
          </Paragraph>
          <div style={{ display: 'flex', flexWrap: 'wrap', gap: 8, marginTop: 12 }}>
            {guideSteps.map((item, index) => (
              <span
                key={item}
                style={{
                  display: 'inline-flex',
                  alignItems: 'center',
                  gap: 8,
                  padding: '6px 12px',
                  borderRadius: 999,
                  background: token.colorBgContainer,
                  border: `1px solid ${token.colorBorderSecondary}`,
                  color: token.colorTextSecondary,
                  fontSize: 12,
                }}
              >
                <span style={{ color: token.colorPrimary, fontWeight: 700 }}>{index + 1}</span>
                {item}
              </span>
            ))}
          </div>
        </div>
        <div
          style={{
            borderRadius: 16,
            padding: '16px 18px',
            background: token.colorBgContainer,
            border: `1px solid ${token.colorBorderSecondary}`,
          }}
        >
          <Text style={{ display: 'block', fontSize: 12, letterSpacing: '0.12em', textTransform: 'uppercase', color: token.colorTextTertiary }}>
            当前工作焦点
          </Text>
          <Title level={5} style={{ margin: '8px 0 6px', fontFamily: designDisplayFont }}>
            {focusTitle}
          </Title>
          <Paragraph style={{ margin: 0, color: token.colorTextSecondary, lineHeight: 1.75 }}>
            {focusNote}
          </Paragraph>
          <Space wrap size={[8, 8]} style={{ marginTop: 12 }}>
            {focusTags.map((tag) => (
              <Tag key={`${tag.color}-${tag.label}`} color={tag.color} style={{ margin: 0, borderRadius: 999, paddingInline: 10 }}>
                {tag.label}
              </Tag>
            ))}
          </Space>
        </div>
      </div>
    </Card>
  );

  const renderWorkspacePanel = (label: string, title: string, description: string, children: React.ReactNode) => (
    <Card
      bordered={false}
      style={{
        borderRadius: 18,
        background: token.colorBgContainer,
        border: panelBorder,
      }}
      styles={{ body: { padding: 18 } }}
    >
      <div style={{ marginBottom: 14 }}>
        <Text style={{ fontSize: 12, letterSpacing: '0.12em', textTransform: 'uppercase', color: token.colorTextTertiary }}>
          {label}
        </Text>
        <Title level={5} style={{ margin: '6px 0 8px', fontFamily: designDisplayFont }}>
          {title}
        </Title>
        <Paragraph style={{ margin: 0, color: token.colorTextSecondary, lineHeight: 1.7 }}>
          {description}
        </Paragraph>
      </div>
      {children}
    </Card>
  );

  const handleApply = async () => {
    applyRequestIdRef.current += 1;
    const requestId = applyRequestIdRef.current;
    setApplying(true);
    try {
      if (onApplyAction) {
        const result = await onApplyAction();
        if (!mountedRef.current || applyRequestIdRef.current !== requestId) {
          return;
        }
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

      if (!mountedRef.current || applyRequestIdRef.current !== requestId) {
        return;
      }
      message.success('新内容已应用！');
      await Promise.resolve(onApply?.());
      if (!mountedRef.current || applyRequestIdRef.current !== requestId) {
        return;
      }

      if (triggerAnalysisTimerRef.current) {
        window.clearTimeout(triggerAnalysisTimerRef.current);
      }
      triggerAnalysisTimerRef.current = window.setTimeout(async () => {
        try {
          if (!mountedRef.current || applyRequestIdRef.current !== requestId) {
            return;
          }
          await chapterApi.triggerChapterAnalysis(chapterId, projectId);
          if (!mountedRef.current || applyRequestIdRef.current !== requestId) {
            return;
          }
          message.success('章节分析已开始，请稍后查看结果');
        } catch (analysisError) {
          if (!mountedRef.current || applyRequestIdRef.current !== requestId) {
            return;
          }
          console.error('Failed to trigger chapter analysis:', analysisError);
          message.warning('章节分析触发失败，您可以手动触发分析');
        }
      }, 500);

      onClose();
    } catch (error: unknown) {
      if (!mountedRef.current || applyRequestIdRef.current !== requestId) {
        return;
      }
      const err = error as Error;
      message.error(err.message || '应用失败');
    } finally {
      if (mountedRef.current && applyRequestIdRef.current === requestId) {
        setApplying(false);
      }
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
        title={null}
        open={visible}
        onCancel={onClose}
        width="95%"
        centered
        style={{ maxWidth: 1600 }}
        footer={footerActions}
        styles={modalSurfaceStyles}
      >
        {renderModalHero(
          'Draft Review',
          resolvedModalTitle,
          '这是候选稿采纳工作台。原有应用、放弃、关闭与分析触发逻辑都保持不变，这里只补充导览层，帮助你先看摘要，再判断是否采纳。'
        )}
        {renderGuidePanel(
          'Comparison Guide',
          '先看摘要，再进入逐行 diff 审阅',
          '这个弹窗更像一次章节候选稿审校台，而不是直接覆盖内容的单步操作。先判断整体质量，再进入 diff 对照，能更稳定地做采纳决策。',
          comparisonGuideSteps,
          comparisonFocus.title,
          comparisonFocus.note,
          comparisonFocus.tags,
        )}
        {renderWorkspacePanel(
          'Comparison Workspace',
          '候选稿对比工作区',
          '顶部先看字数和质量摘要，底部继续使用原有 diff 视图进行逐行比对；所有应用与放弃操作仍走原有逻辑。',
          <>
            <Card size="small" style={{ marginBottom: 16 }}>
              <Row gutter={[16, 16]}>
                <Col xs={12} md={6}>
                  <Statistic
                    title="原内容字数"
                    value={originalWordCount}
                    suffix="字"
                  />
                </Col>
                <Col xs={12} md={6}>
                  <Statistic
                    title="新内容字数"
                    value={wordCount}
                    suffix="字"
                  />
                </Col>
                <Col xs={12} md={6}>
                  <Statistic
                    title="字数变化"
                    value={wordCountDiff}
                    suffix="字"
                    valueStyle={{ color: wordCountDiff > 0 ? 'var(--color-success)' : 'var(--color-error)' }}
                    prefix={wordCountDiff > 0 ? '+' : ''}
                  />
                </Col>
                <Col xs={12} md={6}>
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
              border: `1px solid ${token.colorBorderSecondary}`,
              borderRadius: 16,
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
          </>,
        )}
      </Modal>
    </>
  );
};

export default ChapterContentComparison;
