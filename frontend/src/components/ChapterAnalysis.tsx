import { useState, useEffect } from 'react';
import { Modal, Spin, Alert, Tabs, Card, Tag, List, Empty, Statistic, Row, Col, Button, message } from 'antd';
import {
  ThunderboltOutlined,
  BulbOutlined,
  FireOutlined,
  HeartOutlined,
  TeamOutlined,
  TrophyOutlined,
  CheckCircleOutlined,
  ClockCircleOutlined,
  CloseCircleOutlined,
  ReloadOutlined,
  EditOutlined
} from '@ant-design/icons';
import type { AnalysisTask, ChapterAnalysisResponse } from '../types';
import { chapterApi } from '../services/api';
import ChapterRegenerationModal from './ChapterRegenerationModal';
import ChapterContentComparison from './ChapterContentComparison';

// 判断是否为移动设备
const isMobileDevice = () => window.innerWidth < 768;

const ANALYSIS_PLOT_STAGE_LABELS: Record<string, string> = {
  development: '发展阶段',
  climax: '高潮阶段',
  ending: '结局阶段',
};

const ANALYSIS_MEMORY_TYPE_LABELS: Record<string, string> = {
  chapter_summary: '章节摘要',
  hook: '钩子',
  foreshadow: '伏笔',
  plot_point: '情节点',
  character_event: '角色事件',
};

const ANALYSIS_INLINE_TERM_LABELS: Array<[string, string]> = [
  ['chapter_summary', '章节摘要'],
  ['character_event', '角色事件'],
  ['plot_point', '情节点'],
  ['foreshadow', '伏笔'],
  ['hook', '钩子'],
  ['revelation', '揭示'],
  ['resolution', '解决'],
  ['transition', '过渡'],
  ['conflict', '冲突'],
  ['planted', '已埋下'],
  ['resolved', '已回收'],
];

const localizeAnalysisPlotStage = (value?: string | null): string => {
  if (!value) {
    return '';
  }
  return ANALYSIS_PLOT_STAGE_LABELS[value] || value;
};

const localizeAnalysisMemoryType = (value?: string | null): string => {
  if (!value) {
    return '';
  }
  return ANALYSIS_MEMORY_TYPE_LABELS[value] || value;
};

const localizeAnalysisMemoryText = (value?: string | null): string => {
  if (!value) {
    return '';
  }
  return ANALYSIS_INLINE_TERM_LABELS.reduce(
    (result, [source, label]) => result.replaceAll(source, label),
    value,
  );
};

interface ChapterAnalysisProps {
  chapterId: string;
  visible: boolean;
  onClose: () => void;
}

export default function ChapterAnalysis({ chapterId, visible, onClose }: ChapterAnalysisProps) {
  const [task, setTask] = useState<AnalysisTask | null>(null);
  const [analysis, setAnalysis] = useState<ChapterAnalysisResponse | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [isMobile, setIsMobile] = useState(isMobileDevice());
  const [regenerationModalVisible, setRegenerationModalVisible] = useState(false);
  const [comparisonModalVisible, setComparisonModalVisible] = useState(false);
  const [draftPreviewVisible, setDraftPreviewVisible] = useState(false);
  const [draftPreviewLoading, setDraftPreviewLoading] = useState(false);
  const [applyingDraft, setApplyingDraft] = useState(false);
  const [chapterInfo, setChapterInfo] = useState<{ title: string; chapter_number: number; content: string; project_id?: string } | null>(null);
  const [draftContent, setDraftContent] = useState('');
  const [newGeneratedContent, setNewGeneratedContent] = useState('');
  const [newContentWordCount, setNewContentWordCount] = useState(0);

  useEffect(() => {
    if (visible && chapterId) {
      fetchAnalysisStatus();
    }

    // 监听窗口大小变化
    const handleResize = () => {
      setIsMobile(isMobileDevice());
    };

    window.addEventListener('resize', handleResize);

    // 清理函数：组件卸载或关闭时清除轮询
    return () => {
      window.removeEventListener('resize', handleResize);
      // 清除可能存在的轮询
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [visible, chapterId]);

  // 🔧 新增：独立的章节信息加载函数
  const loadChapterInfo = async () => {
    try {
      const chapterData = await chapterApi.getChapter(chapterId);
      setChapterInfo({
        title: chapterData.title,
        chapter_number: chapterData.chapter_number,
        content: chapterData.content || '',
        project_id: chapterData.project_id,
      });
      return chapterData;
    } catch (error) {
      console.error('❌ 加载章节信息失败:', error);
      return null;
    }
  };

  const fetchAnalysisStatus = async () => {
    try {
      setLoading(true);
      setError(null);

      const chapterData = await loadChapterInfo();
      const taskData: AnalysisTask = await chapterApi.getChapterAnalysisStatus(
        chapterId,
        chapterData?.project_id || chapterInfo?.project_id
      );

      // 如果状态为 none（无任务），设置 task 为 null，让前端显示"开始分析"按钮
      if (taskData.status === 'none' || !taskData.has_task) {
        setTask(null);
        setError(null); // 清除错误，这不是错误状态
        return;
      }

      setTask(taskData);

      if (taskData.status === 'completed') {
        await fetchAnalysisResult();
      } else if (taskData.status === 'running' || taskData.status === 'pending') {
        // 开始轮询
        startPolling();
      }
    } catch (err) {
      setError((err as Error).message);
    } finally {
      setLoading(false);
    }
  };

  const fetchAnalysisResult = async () => {
    try {
      const data: ChapterAnalysisResponse = await chapterApi.getChapterAnalysis(chapterId, false);
      setAnalysis(data);
    } catch (err) {
      setError((err as Error).message);
    }
  };

  const startPolling = () => {
    const pollInterval = setInterval(async () => {
      try {
        const taskData: AnalysisTask = await chapterApi.getChapterAnalysisStatus(
          chapterId,
          chapterInfo?.project_id
        );
        if (taskData.status === 'none' || !taskData.has_task) return;
        setTask(taskData);

        if (taskData.status === 'completed') {
          clearInterval(pollInterval);
          await fetchAnalysisResult();
          // 🔧 分析完成后刷新章节内容，确保显示最新内容
          await loadChapterInfo();
        } else if (taskData.status === 'failed') {
          clearInterval(pollInterval);
          setError(taskData.error_message || '分析失败');
        }
      } catch (err) {
        console.error('轮询错误:', err);
      }
    }, 2000);

    // 5分钟超时
    setTimeout(() => clearInterval(pollInterval), 300000);
  };

  const triggerAnalysis = async () => {
    try {
      setLoading(true);
      setError(null);

      const chapterData = await loadChapterInfo();
      await chapterApi.triggerChapterAnalysis(
        chapterId,
        chapterData?.project_id || chapterInfo?.project_id
      );

      // 触发成功后立即关闭Modal，让父组件的状态管理接管
      onClose();
    } catch (err) {
      setError((err as Error).message);
    } finally {
      setLoading(false);
    }
  };


  const renderStatusIcon = () => {
    if (!task) return null;

    switch (task.status) {
      case 'pending':
        return <ClockCircleOutlined style={{ color: 'var(--color-warning)' }} />;
      case 'running':
        return <Spin />;
      case 'completed':
        return <CheckCircleOutlined style={{ color: 'var(--color-success)' }} />;
      case 'failed':
        return <CloseCircleOutlined style={{ color: 'var(--color-error)' }} />;
      default:
        return null;
    }
  };

  const renderProgress = () => {
    if (!task || task.status === 'completed') return null;

    return (
      <div style={{
        padding: '40px',
        display: 'flex',
        flexDirection: 'column',
        alignItems: 'center',
        justifyContent: 'center',
        minHeight: '300px'
      }}>
        {/* 标题和图标 */}
        <div style={{
          textAlign: 'center',
          marginBottom: 32
        }}>
          {renderStatusIcon()}
          <div style={{
            fontSize: 20,
            fontWeight: 'bold',
            marginTop: 16,
            color: task.status === 'failed' ? 'var(--color-error)' : 'var(--color-text-primary)'
          }}>
            {task.status === 'pending' && '等待分析...'}
            {task.status === 'running' && '正在分析中...'}
            {task.status === 'failed' && '分析失败'}
          </div>
        </div>

        {/* 进度条 */}
        <div style={{
          width: '100%',
          maxWidth: '500px',
          marginBottom: 16
        }}>
          <div style={{
            height: 12,
            background: 'var(--color-bg-layout)',
            borderRadius: 6,
            overflow: 'hidden',
            marginBottom: 12
          }}>
            <div style={{
              height: '100%',
              background: task.status === 'failed'
                ? 'var(--color-error)'
                : task.progress === 100
                  ? 'var(--color-success)'
                  : 'var(--color-primary)',
              width: `${task.progress}%`,
              transition: 'all 0.3s ease',
              borderRadius: 6,
              boxShadow: task.progress > 0 && task.status !== 'failed'
                ? '0 0 10px rgba(24, 144, 255, 0.3)'
                : 'none'
            }} />
          </div>

          {/* 进度百分比 */}
          <div style={{
            textAlign: 'center',
            fontSize: 32,
            fontWeight: 'bold',
            color: task.status === 'failed' ? 'var(--color-error)' :
              task.progress === 100 ? 'var(--color-success)' : 'var(--color-primary)',
            marginBottom: 8
          }}>
            {task.progress}%
          </div>
        </div>

        {/* 状态消息 */}
        <div style={{
          textAlign: 'center',
          fontSize: 16,
          color: 'var(--color-text-secondary)',
          minHeight: 24,
          marginBottom: 16
        }}>
          {task.status === 'pending' && '分析任务已创建，正在队列中...'}
          {task.status === 'running' && '正在提取关键信息和记忆片段...'}
        </div>

        {/* 错误信息 */}
        {task.status === 'failed' && task.error_message && (
          <Alert
            message="分析失败"
            description={task.error_message}
            type="error"
            showIcon
            style={{
              marginTop: 16,
              maxWidth: '500px',
              width: '100%'
            }}
          />
        )}

        {/* 提示文字 */}
        {task.status !== 'failed' && (
          <div style={{
            textAlign: 'center',
            fontSize: 13,
            color: 'var(--color-text-tertiary)',
            marginTop: 16
          }}>
            分析过程需要一定时间，请耐心等待
          </div>
        )}
      </div>
    );
  };

  // 将分析建议转换为重新生成组件需要的格式
  const convertSuggestionsForRegeneration = () => {
    if (!analysis?.analysis?.suggestions) return [];

    return analysis.analysis.suggestions.map((suggestion, index) => ({
      category: '改进建议',
      content: suggestion,
      priority: index < 3 ? 'high' : 'medium'
    }));
  };

  const checkerResult = analysis?.checker_result;
  const draftResult = analysis?.auto_revision_draft;
  const checkerIssues = checkerResult?.issues || [];
  const checkerPriorityActions = checkerResult?.priority_actions || [];
  const checkerSeverityCounts = checkerResult?.severity_counts;
  const checkerCriticalCount = checkerSeverityCounts?.critical || 0;
  const checkerMajorCount = checkerSeverityCounts?.major || 0;
  const checkerMinorCount = checkerSeverityCounts?.minor || 0;
  const checkerIssueTotal = checkerIssues.length;
  const draftUnresolvedIssues = draftResult?.unresolved_issues || [];
  const draftCriticalCount = draftResult?.critical_count ?? 0;
  const draftMajorCount = draftResult?.major_count ?? 0;
  const draftPriorityIssueCount = draftResult?.priority_issue_count ?? (draftCriticalCount + draftMajorCount);
  const draftAppliedIssueCount = draftResult?.applied_issue_count ?? draftResult?.applied_critical_count ?? 0;

  const getSeverityTagColor = (severity?: string) => {
    switch ((severity || '').toLowerCase()) {
      case 'critical':
        return 'red';
      case 'major':
      case 'warning':
        return 'orange';
      case 'minor':
      case 'info':
        return 'blue';
      default:
        return 'default';
    }
  };

  const getSeverityLabel = (severity?: string) => {
    switch ((severity || '').toLowerCase()) {
      case 'critical':
        return '严重';
      case 'major':
        return '重要';
      case 'warning':
        return '警告';
      case 'minor':
        return '一般';
      case 'info':
        return '提示';
      default:
        return severity || '未知';
    }
  };

  const resetDraftPreviewState = () => {
    setDraftPreviewVisible(false);
    setDraftPreviewLoading(false);
    setDraftContent('');
  };

  const openDraftPreview = async () => {
    if (!draftResult?.history_id) {
      message.warning('当前草稿缺少历史记录标识，无法查看全文');
      return;
    }

    try {
      setDraftPreviewLoading(true);
      setDraftPreviewVisible(true);
      const response = await chapterApi.getAutoRevisionDraft(chapterId, draftResult.history_id);
      setDraftContent(response.auto_revision_draft.revised_text || response.auto_revision_draft.revised_text_preview || '');
    } catch (err) {
      message.error((err as Error).message || '加载自动修订草稿失败');
      setDraftPreviewVisible(false);
    } finally {
      setDraftPreviewLoading(false);
    }
  };

  const refreshAfterDraftApplied = async () => {
    setAnalysis(null);
    setTask(null);
    await loadChapterInfo();
    await fetchAnalysisStatus();
  };

  const applyDraft = async (allowStale = false) => {
    if (!draftResult?.history_id) {
      message.warning('当前草稿缺少历史记录标识，无法应用');
      return;
    }

    try {
      setApplyingDraft(true);
      await chapterApi.applyAutoRevisionDraft(chapterId, {
        history_id: draftResult.history_id,
        allow_stale: allowStale,
      });
      message.success('自动修订草稿已应用到章节正文');
      resetDraftPreviewState();
      await refreshAfterDraftApplied();
    } catch (err) {
      const error = err as Error & { response?: { status?: number; data?: { detail?: string } } };
      const status = error.response?.status;
      const detail = error.response?.data?.detail || error.message;

      if (status === 409 && !allowStale) {
        Modal.confirm({
          title: '草稿已过期',
          content: detail || '自动修订草稿已过期，是否仍要强制应用？',
          okText: '仍要应用',
          cancelText: '取消',
          centered: true,
          onOk: async () => {
            await applyDraft(true);
          },
        });
        return;
      }

      message.error(detail || '应用自动修订草稿失败');
    } finally {
      setApplyingDraft(false);
    }
  };

  const renderAnalysisResult = () => {
    if (!analysis) return null;

    const { analysis: analysis_data, memories } = analysis;
    const hasSuggestions = !!(analysis_data.suggestions && analysis_data.suggestions.length > 0);
    const hasCheckerResult = !!checkerResult;
    const hasDraftResult = !!draftResult;

    return (
      <Tabs
        defaultActiveKey="overview"
        style={{ height: '100%' }}
        items={[
          {
            key: 'overview',
            label: '概览',
            icon: <TrophyOutlined />,
            children: (
              <div style={{ height: isMobile ? 'calc(80vh - 180px)' : 'calc(90vh - 220px)', overflowY: 'auto', paddingRight: '8px' }}>
                {/* 根据建议重新生成按钮 */}
                {hasSuggestions && (
                  <Alert
                    message="发现改进建议"
                    description={
                      <div>
                        <p style={{ marginBottom: 12 }}>已分析出 {analysis_data.suggestions.length} 条改进建议，您可以根据这些建议重新生成章节内容。</p>
                        <Button
                          type="primary"
                          icon={<EditOutlined />}
                          onClick={() => setRegenerationModalVisible(true)}
                          size={isMobile ? 'small' : 'middle'}
                        >
                          根据建议重新生成
                        </Button>
                      </div>
                    }
                    type="info"
                    showIcon
                    style={{ marginBottom: 16 }}
                  />
                )}

                {hasDraftResult && (
                  <Alert
                    message="发现自动修订草稿"
                    description={
                      <div>
                        <p style={{ marginBottom: 8 }}>
                          {draftResult.change_summary || '系统已根据高优先问题生成一份自动修订草稿，您可以先预览，再决定是否应用。'}
                        </p>
                        <div style={{ display: 'flex', flexWrap: 'wrap', gap: 8, marginBottom: draftUnresolvedIssues.length > 0 ? 12 : 0 }}>
                          <Tag color="red">高优先问题 {draftPriorityIssueCount}</Tag>
                          {draftMajorCount > 0 && <Tag color="orange">中等问题 {draftMajorCount}</Tag>}
                          <Tag color="green">已处理 {draftAppliedIssueCount}</Tag>
                          <Tag color="blue">草稿字数 {draftResult.revised_word_count}</Tag>
                          {draftResult.is_stale && <Tag color="orange">草稿已过期</Tag>}
                        </div>
                        {draftUnresolvedIssues.length > 0 && (
                          <div style={{ marginBottom: 12 }}>
                            <div style={{ fontWeight: 500, marginBottom: 4 }}>仍待处理</div>
                            <ul style={{ margin: 0, paddingLeft: 20 }}>
                              {draftUnresolvedIssues.map((issue, index) => (
                                <li key={`${issue}-${index}`}>{issue}</li>
                              ))}
                            </ul>
                          </div>
                        )}
                        <div style={{ display: 'flex', flexWrap: 'wrap', gap: 8 }}>
                          <Button
                            onClick={openDraftPreview}
                            loading={draftPreviewLoading}
                            size={isMobile ? 'small' : 'middle'}
                          >
                            查看修订草稿
                          </Button>
                          <Button
                            type="primary"
                            onClick={() => applyDraft(false)}
                            loading={applyingDraft}
                            size={isMobile ? 'small' : 'middle'}
                          >
                            应用自动修订草稿
                          </Button>
                          {hasSuggestions && (
                            <Button
                              icon={<EditOutlined />}
                              onClick={() => setRegenerationModalVisible(true)}
                              size={isMobile ? 'small' : 'middle'}
                            >
                              根据建议重新生成
                            </Button>
                          )}
                        </div>
                      </div>
                    }
                    type={draftResult.is_stale ? 'warning' : 'success'}
                    showIcon
                    style={{ marginBottom: 16 }}
                  />
                )}

                {hasCheckerResult && (
                  <Card title="文本质检结果" style={{ marginBottom: 16 }} size={isMobile ? 'small' : 'default'}>
                    <div style={{ marginBottom: 16 }}>
                      <div style={{ fontWeight: 500, marginBottom: 8 }}>{checkerResult.overall_assessment || '已完成文本质检'}</div>
                      <Row gutter={isMobile ? 8 : 16}>
                        <Col span={isMobile ? 8 : 6}>
                          <Statistic title="严重" value={checkerCriticalCount} valueStyle={{ color: 'var(--color-error)' }} />
                        </Col>
                        <Col span={isMobile ? 8 : 6}>
                          <Statistic title="重要" value={checkerMajorCount} valueStyle={{ color: 'var(--color-warning)' }} />
                        </Col>
                        <Col span={isMobile ? 8 : 6}>
                          <Statistic title="一般" value={checkerMinorCount} valueStyle={{ color: 'var(--color-primary)' }} />
                        </Col>
                        <Col span={isMobile ? 24 : 6}>
                          <Statistic title="问题总数" value={checkerIssueTotal} />
                        </Col>
                      </Row>
                    </div>

                    {checkerPriorityActions.length > 0 && (
                      <Card type="inner" title="优先修复项" size="small" style={{ marginBottom: 16 }}>
                        <List
                          dataSource={checkerPriorityActions}
                          renderItem={(item, index) => (
                            <List.Item>
                              <span>{index + 1}. {item}</span>
                            </List.Item>
                          )}
                        />
                      </Card>
                    )}

                    <Card type="inner" title={`问题列表 (${checkerIssueTotal})`} size="small">
                      {checkerIssueTotal > 0 ? (
                        <List
                          dataSource={checkerIssues}
                          renderItem={(issue, index) => {
                            const issueTitle = (issue as { title?: string }).title || issue.category || `问题 ${index + 1}`;
                            const issueLocation = issue.location || (issue as { impact?: string }).impact || '';
                            return (
                              <List.Item>
                                <List.Item.Meta
                                  title={
                                    <div style={{ display: 'flex', flexWrap: 'wrap', alignItems: 'center', gap: 8 }}>
                                      <Tag color={getSeverityTagColor(issue.severity)}>{getSeverityLabel(issue.severity)}</Tag>
                                      <span>{issueTitle}</span>
                                      {issueLocation && <Tag>{issueLocation}</Tag>}
                                    </div>
                                  }
                                  description={
                                    <div>
                                      {issue.evidence && <p style={{ marginBottom: 8 }}><strong>证据：</strong>{issue.evidence}</p>}
                                      {issue.impact && <p style={{ marginBottom: 8 }}><strong>影响：</strong>{issue.impact}</p>}
                                      {issue.suggestion && <p style={{ marginBottom: 0 }}><strong>建议：</strong>{issue.suggestion}</p>}
                                    </div>
                                  }
                                />
                              </List.Item>
                            );
                          }}
                        />
                      ) : (
                        <Empty description="未发现需要修复的问题" />
                      )}
                    </Card>
                  </Card>
                )}

                <Card title="整体评分" style={{ marginBottom: 16 }} size={isMobile ? 'small' : 'default'}>
                  <Row gutter={isMobile ? 8 : 16}>
                    <Col span={isMobile ? 12 : 6}>
                      <Statistic
                        title="整体质量"
                        value={analysis_data.overall_quality_score || 0}
                        suffix="/ 10"
                        valueStyle={{ color: 'var(--color-success)' }}
                      />
                    </Col>
                    <Col span={isMobile ? 12 : 6}>
                      <Statistic
                        title="节奏把控"
                        value={analysis_data.pacing_score || 0}
                        suffix="/ 10"
                      />
                    </Col>
                    <Col span={isMobile ? 12 : 6}>
                      <Statistic
                        title="吸引力"
                        value={analysis_data.engagement_score || 0}
                        suffix="/ 10"
                      />
                    </Col>
                    <Col span={isMobile ? 12 : 6}>
                      <Statistic
                        title="连贯性"
                        value={analysis_data.coherence_score || 0}
                        suffix="/ 10"
                      />
                    </Col>
                  </Row>
                </Card>

                {analysis_data.analysis_report && (
                  <Card title="分析摘要" style={{ marginBottom: 16 }} size={isMobile ? 'small' : 'default'}>
                    <pre style={{ whiteSpace: 'pre-wrap', fontFamily: 'inherit', fontSize: isMobile ? 13 : 14 }}>
                      {analysis_data.analysis_report}
                    </pre>
                  </Card>
                )}

                {hasSuggestions && (
                  <Card title={<><BulbOutlined /> 改进建议</>} size={isMobile ? 'small' : 'default'}>
                    <List
                      dataSource={analysis_data.suggestions}
                      renderItem={(item, index) => (
                        <List.Item>
                          <span>{index + 1}. {item}</span>
                        </List.Item>
                      )}
                    />
                  </Card>
                )}
              </div>
            )
          },
          {
            key: 'hooks',
            label: `钩子 (${analysis_data.hooks?.length || 0})`,
            icon: <ThunderboltOutlined />,
            children: (
              <div style={{ height: isMobile ? 'calc(80vh - 180px)' : 'calc(90vh - 220px)', overflowY: 'auto', paddingRight: '8px' }}>
                <Card size={isMobile ? 'small' : 'default'}>
                  {analysis_data.hooks && analysis_data.hooks.length > 0 ? (
                    <List
                      dataSource={analysis_data.hooks}
                      renderItem={(hook) => (
                        <List.Item>
                          <List.Item.Meta
                            title={
                              <div>
                                <Tag color="blue">{hook.type}</Tag>
                                <Tag color="orange">{hook.position}</Tag>
                                <Tag color="red">强度: {hook.strength}/10</Tag>
                              </div>
                            }
                            description={hook.content}
                          />
                        </List.Item>
                      )}
                    />
                  ) : (
                    <Empty description="暂无钩子" />
                  )}
                </Card>
              </div>
            )
          },
          {
            key: 'foreshadows',
            label: `伏笔 (${analysis_data.foreshadows?.length || 0})`,
            icon: <FireOutlined />,
            children: (
              <div style={{ height: isMobile ? 'calc(80vh - 180px)' : 'calc(90vh - 220px)', overflowY: 'auto', paddingRight: '8px' }}>
                <Card size={isMobile ? 'small' : 'default'}>
                  {analysis_data.foreshadows && analysis_data.foreshadows.length > 0 ? (
                    <List
                      dataSource={analysis_data.foreshadows}
                      renderItem={(foreshadow) => (
                        <List.Item>
                          <List.Item.Meta
                            title={
                              <div>
                                <Tag color={foreshadow.type === 'planted' ? 'green' : 'purple'}>
                                  {foreshadow.type === 'planted' ? '已埋下' : '已回收'}
                                </Tag>
                                <Tag>强度: {foreshadow.strength}/10</Tag>
                                <Tag>隐藏度: {foreshadow.subtlety}/10</Tag>
                                {foreshadow.reference_chapter && (
                                  <Tag color="cyan">呼应第{foreshadow.reference_chapter}章</Tag>
                                )}
                              </div>
                            }
                            description={foreshadow.content}
                          />
                        </List.Item>
                      )}
                    />
                  ) : (
                    <Empty description="暂无伏笔" />
                  )}
                </Card>
              </div>
            )
          },
          {
            key: 'emotion',
            label: '情感曲线',
            icon: <HeartOutlined />,
            children: (
              <div style={{ height: isMobile ? 'calc(80vh - 180px)' : 'calc(90vh - 220px)', overflowY: 'auto', paddingRight: '8px' }}>
                <Card size={isMobile ? 'small' : 'default'}>
                  {analysis_data.emotional_tone ? (
                    <div>
                      <Row gutter={isMobile ? 8 : 16} style={{ marginBottom: isMobile ? 16 : 24 }}>
                        <Col span={isMobile ? 24 : 12}>
                          <Statistic
                            title="主导情绪"
                            value={analysis_data.emotional_tone}
                          />
                        </Col>
                        <Col span={isMobile ? 24 : 12}>
                          <Statistic
                            title="情感强度"
                            value={(analysis_data.emotional_intensity * 10).toFixed(1)}
                            suffix="/ 10"
                          />
                        </Col>
                      </Row>
                      <Card type="inner" title="剧情阶段" size="small">
                        <p><strong>阶段：</strong>{localizeAnalysisPlotStage(analysis_data.plot_stage)}</p>
                        <p><strong>冲突等级：</strong>{analysis_data.conflict_level} / 10</p>
                        {analysis_data.conflict_types && analysis_data.conflict_types.length > 0 && (
                          <div style={{ marginTop: 8 }}>
                            <strong>冲突类型：</strong>
                            {analysis_data.conflict_types.map((type, idx) => (
                              <Tag key={idx} color="red" style={{ margin: 4 }}>
                                {type}
                              </Tag>
                            ))}
                          </div>
                        )}
                      </Card>
                    </div>
                  ) : (
                    <Empty description="暂无情感分析" />
                  )}
                </Card>
              </div>
            )
          },
          {
            key: 'characters',
            label: `角色 (${analysis_data.character_states?.length || 0})`,
            icon: <TeamOutlined />,
            children: (
              <div style={{ height: isMobile ? 'calc(80vh - 180px)' : 'calc(90vh - 220px)', overflowY: 'auto', paddingRight: '8px' }}>
                <Card size={isMobile ? 'small' : 'default'}>
                  {analysis_data.character_states && analysis_data.character_states.length > 0 ? (
                    <List
                      dataSource={analysis_data.character_states}
                      renderItem={(char) => (
                        <List.Item>
                          <Card
                            type="inner"
                            title={char.character_name}
                            size="small"
                            style={{ width: '100%' }}
                          >
                            <p><strong>状态变化：</strong>{char.state_before} → {char.state_after}</p>
                            <p><strong>心理变化：</strong>{char.psychological_change}</p>
                            <p><strong>关键事件：</strong>{char.key_event}</p>
                            {char.relationship_changes && Object.keys(char.relationship_changes).length > 0 && (
                              <div>
                                <strong>关系变化：</strong>
                                {Object.entries(char.relationship_changes).map(([name, change]) => (
                                  <Tag key={name} color="blue" style={{ margin: 4 }}>
                                    与{name}: {change}
                                  </Tag>
                                ))}
                              </div>
                            )}
                          </Card>
                        </List.Item>
                      )}
                    />
                  ) : (
                    <Empty description="暂无角色分析" />
                  )}
                </Card>
              </div>
            )
          },
          {
            key: 'memories',
            label: `记忆 (${memories?.length || 0})`,
            icon: <FireOutlined />,
            children: (
              <div style={{ height: isMobile ? 'calc(80vh - 180px)' : 'calc(90vh - 220px)', overflowY: 'auto', paddingRight: '8px' }}>
                <Card size={isMobile ? 'small' : 'default'}>
                  {memories && memories.length > 0 ? (
                    <List
                      dataSource={memories}
                      renderItem={(memory) => (
                        <List.Item>
                          <List.Item.Meta
                            title={
                              <div>
                                <Tag color="blue">{localizeAnalysisMemoryType(memory.type)}</Tag>
                                <Tag color="orange">重要性: {memory.importance.toFixed(1)}</Tag>
                                {memory.is_foreshadow === 1 && <Tag color="green">已埋下伏笔</Tag>}
                                {memory.is_foreshadow === 2 && <Tag color="purple">已回收伏笔</Tag>}
                                <span style={{ marginLeft: 8 }}>{localizeAnalysisMemoryText(memory.title)}</span>
                              </div>
                            }
                            description={
                              <div>
                                <p>{memory.content}</p>
                                <div>
                                  {memory.tags.map((tag, idx) => (
                                    <Tag key={idx} style={{ margin: 2 }}>{localizeAnalysisMemoryText(tag)}</Tag>
                                  ))}
                                </div>
                              </div>
                            }
                          />
                        </List.Item>
                      )}
                    />
                  ) : (
                    <Empty description="暂无记忆片段" />
                  )}
                </Card>
              </div>
            )
          }
        ]}
      />
    );
  };

  return (
    <Modal
      title="章节分析"
      open={visible}
      onCancel={onClose}
      width={isMobile ? 'calc(100vw - 32px)' : '90%'}
      centered
      style={{
        maxWidth: isMobile ? 'calc(100vw - 32px)' : '1400px',
        margin: isMobile ? '0 auto' : undefined,
        padding: isMobile ? '0 16px' : undefined
      }}
      styles={{
        body: {
          padding: isMobile ? '12px' : '24px',
          paddingBottom: 0,
          maxHeight: isMobile ? 'calc(100vh - 200px)' : 'calc(90vh - 150px)',
          overflowY: 'auto'
        }
      }}
      footer={[
        <Button key="close" onClick={onClose} size={isMobile ? 'small' : 'middle'}>
          关闭
        </Button>,
        !task && !loading && (
          <Button
            key="analyze"
            type="primary"
            icon={<ReloadOutlined />}
            onClick={triggerAnalysis}
            loading={loading}
            size={isMobile ? 'small' : 'middle'}
          >
            开始分析
          </Button>
        ),
        task && (task.status === 'failed') && (
          <Button
            key="reanalyze"
            type="primary"
            icon={<ReloadOutlined />}
            onClick={triggerAnalysis}
            loading={loading}
            danger
            size={isMobile ? 'small' : 'middle'}
          >
            重新分析
          </Button>
        ),
        task && task.status === 'completed' && (
          <Button
            key="reanalyze"
            type="default"
            icon={<ReloadOutlined />}
            onClick={triggerAnalysis}
            loading={loading}
            size={isMobile ? 'small' : 'middle'}
          >
            重新分析
          </Button>
        )
      ].filter(Boolean)}
    >
      {loading && !task && (
        <div style={{ textAlign: 'center', padding: '48px' }}>
          <Spin size="large" />
          <p style={{ marginTop: 16 }}>加载中...</p>
        </div>
      )}

      {error && (
        <Alert
          message="错误"
          description={error}
          type="error"
          showIcon
        />
      )}

      {task && task.status !== 'completed' && renderProgress()}
      {task && task.status === 'completed' && analysis && renderAnalysisResult()}

      {/* 自动修订草稿预览 */}
      <Modal
        title="自动修订草稿预览"
        open={draftPreviewVisible}
        onCancel={resetDraftPreviewState}
        width={isMobile ? 'calc(100vw - 32px)' : '80%'}
        centered
        style={{ maxWidth: '1200px' }}
        footer={[
          <Button key="close-draft-preview" onClick={resetDraftPreviewState} disabled={applyingDraft}>
            关闭
          </Button>,
          <Button
            key="apply-draft"
            type="primary"
            loading={applyingDraft}
            onClick={() => applyDraft(false)}
          >
            应用自动修订草稿
          </Button>
        ]}
      >
        {draftPreviewLoading ? (
          <div style={{ textAlign: 'center', padding: '48px 0' }}>
            <Spin size="large" />
            <p style={{ marginTop: 16 }}>正在加载修订草稿...</p>
          </div>
        ) : (
          <div>
            {draftResult && (
              <Card size="small" style={{ marginBottom: 16 }}>
                <Row gutter={16}>
                  <Col span={8}>
                    <Statistic title="高优先问题" value={draftPriorityIssueCount} valueStyle={{ color: 'var(--color-error)' }} />
                  </Col>
                  <Col span={8}>
                    <Statistic title="已处理" value={draftAppliedIssueCount} valueStyle={{ color: 'var(--color-success)' }} />
                  </Col>
                  <Col span={8}>
                    <Statistic title="草稿字数" value={draftResult.revised_word_count} />
                  </Col>
                </Row>
                {draftResult.change_summary && (
                  <div style={{ marginTop: 16 }}>
                    <strong>修订摘要：</strong>{draftResult.change_summary}
                  </div>
                )}
              </Card>
            )}
            <Card size="small" title="修订正文">
              <pre style={{ whiteSpace: 'pre-wrap', fontFamily: 'inherit', fontSize: isMobile ? 13 : 14, margin: 0 }}>
                {draftContent || draftResult?.revised_text_preview || '暂无可预览内容'}
              </pre>
            </Card>
          </div>
        )}
      </Modal>

      {/* 重新生成Modal */}
      {chapterInfo && (
        <ChapterRegenerationModal
          visible={regenerationModalVisible}
          onCancel={() => setRegenerationModalVisible(false)}
          onSuccess={(newContent: string, wordCount: number) => {
            // 保存新生成的内容
            setNewGeneratedContent(newContent);
            setNewContentWordCount(wordCount);
            // 关闭重新生成对话框
            setRegenerationModalVisible(false);
            // 打开对比界面
            setComparisonModalVisible(true);
          }}
          chapterId={chapterId}
          chapterTitle={chapterInfo.title}
          chapterNumber={chapterInfo.chapter_number}
          suggestions={convertSuggestionsForRegeneration()}
          hasAnalysis={true}
        />
      )}

      {/* 内容对比组件 */}
      {chapterInfo && comparisonModalVisible && (
        <ChapterContentComparison
          visible={comparisonModalVisible}
          onClose={() => setComparisonModalVisible(false)}
          chapterId={chapterId}
          projectId={chapterInfo.project_id}
          chapterTitle={chapterInfo.title}
          originalContent={chapterInfo.content}
          newContent={newGeneratedContent}
          wordCount={newContentWordCount}
          onApply={async () => {
            // 应用新内容后刷新章节信息和分析
            setChapterInfo(null);
            setAnalysis(null);

            // 重新加载章节内容
            await loadChapterInfo();

            // 刷新分析状态
            await fetchAnalysisStatus();
          }}
          onDiscard={() => {
            // 放弃新内容，清空状态
            setNewGeneratedContent('');
            setNewContentWordCount(0);
          }}
        />
      )}
    </Modal>
  );
}
