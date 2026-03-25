import React, { useState, useEffect } from 'react';
import { Card, List, Button, Space, Empty, Tag, Spin, Alert, Switch, Drawer, message, theme } from 'antd';
import {
  EyeOutlined,
  EyeInvisibleOutlined,
  MenuOutlined,
  LeftOutlined,
  RightOutlined,
  UnorderedListOutlined,
  FundOutlined,
} from '@ant-design/icons';
import { useParams } from 'react-router-dom';
import api, { chapterApi } from '../services/api';
import AnnotatedText, { type MemoryAnnotation } from '../components/AnnotatedText';
import MemorySidebar from '../components/MemorySidebar';
import ProjectQualityTrendPanel from '../components/ProjectQualityTrendPanel';
import type { ChapterAnalysisResponse, ChapterQualityMetrics, ProjectChapterQualityTrendResponse } from '../types';
import {
  renderCompactFactCard,
  renderCompactFactGrid,
  renderCompactListCard,
  renderCompactSelectionSummary,
  renderCompactSettingHint,
} from '../components/storyCreationCommonUi';
import { getMetricRateColor, getOverallScoreColor, renderCompactMetricGrid } from '../components/storyCreationQualityUi';
import {
  formatRepairWeakestMetricHint,
  getQualityMetricItems,
  getQualityProfileDisplayItems,
  getRepairGuidanceDisplay,
  getWeakestQualityMetric,
} from '../utils/storyCreationQualitySummary';


interface ChapterItem {
  id: string;
  chapter_number: number;
  title: string;
  content: string;
  word_count: number;
  status: string;
}

interface AnnotationsData {
  chapter_id: string;
  chapter_number: number;
  title: string;
  word_count: number;
  annotations: MemoryAnnotation[];
  has_analysis: boolean;
  summary: {
    total_annotations: number;
    hooks: number;
    foreshadows: number;
    plot_points: number;
    character_events: number;
  };
}

interface NavigationData {
  current: {
    id: string;
    chapter_number: number;
    title: string;
  };
  previous: {
    id: string;
    chapter_number: number;
    title: string;
  } | null;
  next: {
    id: string;
    chapter_number: number;
    title: string;
  } | null;
}

/**
 * 项目内的章节剧情分析页面
 * 显示章节列表和带标注的章节内容
 */
const ChapterAnalysis: React.FC = () => {
  const { projectId } = useParams<{ projectId: string }>();
  
  const [chapters, setChapters] = useState<ChapterItem[]>([]);
  const [selectedChapter, setSelectedChapter] = useState<ChapterItem | null>(null);
  const [annotationsData, setAnnotationsData] = useState<AnnotationsData | null>(null);
  const [analysisDetail, setAnalysisDetail] = useState<ChapterAnalysisResponse | null>(null);
  const [projectQualityTrend, setProjectQualityTrend] = useState<ProjectChapterQualityTrendResponse | null>(null);
  const [navigation, setNavigation] = useState<NavigationData | null>(null);
  const [loading, setLoading] = useState(true);
  const [contentLoading, setContentLoading] = useState(false);
  const [showAnnotations, setShowAnnotations] = useState(true);
  const [activeAnnotationId, setActiveAnnotationId] = useState<string | undefined>();
  const [sidebarVisible, setSidebarVisible] = useState(false);
  const [chapterListVisible, setChapterListVisible] = useState(false);
  const [scrollToContentAnnotation, setScrollToContentAnnotation] = useState<string | undefined>();
  const [scrollToSidebarAnnotation, setScrollToSidebarAnnotation] = useState<string | undefined>();
  const [isMobile, setIsMobile] = useState(window.innerWidth < 768);
  const { token } = theme.useToken();

  // 监听窗口大小变化
  useEffect(() => {
    const handleResize = () => {
      setIsMobile(window.innerWidth < 768);
    };
    
    window.addEventListener('resize', handleResize);
    return () => window.removeEventListener('resize', handleResize);
  }, []);

  // 加载章节列表
  useEffect(() => {
    const loadChapters = async () => {
      if (!projectId) return;
      
      try {
        setLoading(true);
        const [response, trendResponse] = await Promise.all([
          api.get(`/chapters/project/${projectId}`),
          chapterApi.getProjectChapterQualityTrend(projectId, 12).catch((trendError) => {
            console.error('加载项目质量趋势失败:', trendError);
            return null;
          }),
        ]);
        // API 拦截器已经解析了 response.data，所以直接使用
        const data = response.data || response;
        const chapterList = data.items || [];
        setChapters(chapterList);
        setProjectQualityTrend(trendResponse);
        
        // 自动选择第一个有内容的章节
        const firstChapterWithContent = chapterList.find((ch: ChapterItem) => ch.content && ch.content.trim() !== '');
        if (firstChapterWithContent) {
          loadChapterContent(firstChapterWithContent.id);
        }
      } catch (error) {
        setProjectQualityTrend(null);
        console.error('加载章节列表失败:', error);
        message.error('加载章节列表失败');
      } finally {
        setLoading(false);
      }
    };

    loadChapters();
  }, [projectId]);

  // 加载章节内容和标注
  const loadChapterContent = async (chapterId: string) => {
    try {
      setContentLoading(true);
      setAnalysisDetail(null);
      
      const [chapterResponse, annotationsResponse, analysisResponse, navigationResponse] = await Promise.all([
        api.get(`/chapters/${chapterId}`),
        api.get(`/chapters/${chapterId}/annotations`).catch(() => null),
        chapterApi.getChapterAnalysis(chapterId).catch(() => null),
        api.get(`/chapters/${chapterId}/navigation`).catch(() => null),
      ]);

      // Compatible with axios interceptors returning data directly.
      const normalizedAnalysisResponse = analysisResponse && typeof analysisResponse === 'object' && 'data' in analysisResponse
        ? (analysisResponse as { data?: ChapterAnalysisResponse }).data ?? analysisResponse
        : analysisResponse;
      setSelectedChapter(chapterResponse.data || chapterResponse);
      setAnnotationsData(annotationsResponse ? (annotationsResponse.data || annotationsResponse) : null);
      setAnalysisDetail(normalizedAnalysisResponse ?? null);
      setNavigation(navigationResponse ? (navigationResponse.data || navigationResponse) : null);
    } catch (error) {
      setAnalysisDetail(null);
      console.error('加载章节内容失败:', error);
      message.error('加载章节内容失败');
    } finally {
      setContentLoading(false);
    }
  };

  const handleChapterSelect = (chapterId: string) => {
    loadChapterContent(chapterId);
    if (isMobile) {
      setChapterListVisible(false);
    }
  };

  const handlePreviousChapter = () => {
    if (navigation?.previous) {
      loadChapterContent(navigation.previous.id);
    }
  };

  const handleNextChapter = () => {
    if (navigation?.next) {
      loadChapterContent(navigation.next.id);
    }
  };

  const handleAnnotationClick = (annotation: MemoryAnnotation, source: 'content' | 'sidebar' = 'content') => {
    setActiveAnnotationId(annotation.id);
    
    if (source === 'content') {
      // 从内容区点击，滚动到侧边栏
      setScrollToSidebarAnnotation(annotation.id);
      // 清除滚动状态
      setTimeout(() => setScrollToSidebarAnnotation(undefined), 100);
      
      if (isMobile) {
        setSidebarVisible(true);
      }
    } else {
      // 从侧边栏点击，滚动到内容区
      setScrollToContentAnnotation(annotation.id);
      // 清除滚动状态
      setTimeout(() => setScrollToContentAnnotation(undefined), 100);
    }
  };

  const hasAnnotations = annotationsData && annotationsData.annotations.length > 0;
  const chapterQualityMetrics = analysisDetail?.quality_metrics ?? null;
  const normalizedChapterQualityMetrics: ChapterQualityMetrics | null = chapterQualityMetrics ? {
    overall_score: chapterQualityMetrics.overall_score ?? 0,
    conflict_chain_hit_rate: chapterQualityMetrics.conflict_chain_hit_rate ?? 0,
    rule_grounding_hit_rate: chapterQualityMetrics.rule_grounding_hit_rate ?? 0,
    outline_alignment_rate: chapterQualityMetrics.outline_alignment_rate ?? 0,
    dialogue_naturalness_rate: chapterQualityMetrics.dialogue_naturalness_rate ?? 0,
    opening_hook_rate: chapterQualityMetrics.opening_hook_rate ?? 0,
    payoff_chain_rate: chapterQualityMetrics.payoff_chain_rate ?? 0,
    cliffhanger_rate: chapterQualityMetrics.cliffhanger_rate ?? 0,
    repair_guidance: chapterQualityMetrics.repair_guidance ?? null,
  } : null;
  const checkerResult = analysisDetail?.checker_result ?? null;
  const draftResult = analysisDetail?.auto_revision_draft ?? null;
  const weakestQualityMetric = normalizedChapterQualityMetrics ? getWeakestQualityMetric(normalizedChapterQualityMetrics) : null;
  const qualityRepairGuidance = getRepairGuidanceDisplay(normalizedChapterQualityMetrics?.repair_guidance ?? null);
  const qualityRepairWeakestMetricHint = formatRepairWeakestMetricHint(qualityRepairGuidance);
  const qualityMetricItems = normalizedChapterQualityMetrics ? getQualityMetricItems(normalizedChapterQualityMetrics) : [];
  const qualityProfileItems = getQualityProfileDisplayItems(
    analysisDetail?.quality_profile_summary
      ?? checkerResult?.quality_profile_summary
      ?? draftResult?.quality_profile_summary
      ?? null,
  );
  const checkerPriorityActions = checkerResult?.priority_actions ?? [];
  const checkerIssues = checkerResult?.issues ?? [];
  const checkerCriticalCount = checkerResult?.severity_counts?.critical ?? 0;
  const checkerMajorCount = checkerResult?.severity_counts?.major ?? 0;
  const checkerMinorCount = checkerResult?.severity_counts?.minor ?? 0;
  const checkerIssueTotal = checkerIssues.length;
  const draftUnresolvedIssues = draftResult?.unresolved_issues ?? [];
  const draftPriorityIssueCount = draftResult?.priority_issue_count ?? ((draftResult?.critical_count ?? 0) + (draftResult?.major_count ?? 0));
  const draftAppliedIssueCount = draftResult?.applied_issue_count ?? draftResult?.applied_critical_count ?? 0;
  const hasQualityRepairBreakdown = Boolean(
    qualityRepairGuidance && (
      qualityRepairGuidance.repairTargets.length > 0
      || qualityRepairGuidance.preserveStrengths.length > 0
      || qualityRepairGuidance.focusAreas.length > 0
    ),
  );
  const qualityAcceptanceSummaryItems = normalizedChapterQualityMetrics ? [
    { label: '综合得分', value: `${normalizedChapterQualityMetrics.overall_score.toFixed(1)}`, color: getOverallScoreColor(normalizedChapterQualityMetrics.overall_score) },
    ...(weakestQualityMetric
      ? [{
          label: '最弱项',
          value: `${weakestQualityMetric.label} ${weakestQualityMetric.value}%`,
          color: getMetricRateColor(weakestQualityMetric.value),
        }]
      : []),
    {
      label: '生成时间',
      value: chapterQualityMetrics?.generated_at ? new Date(chapterQualityMetrics.generated_at).toLocaleString() : '尚未生成',
    },
  ] : [];
  const hasQualityAcceptanceData = Boolean(
    normalizedChapterQualityMetrics
    || checkerResult
    || draftResult
    || qualityProfileItems.length > 0,
  );


  if (loading) {
    return (
      <div style={{ textAlign: 'center', padding: '100px 0' }}>
        <Spin size="large" tip="加载章节中..." />
      </div>
    );
  }

  return (
    <div style={{ display: 'flex', flexDirection: 'column', height: '100%' }}>
      {/* 页面标题 - 仅桌面端显示 */}
      {!isMobile && (
        <div style={{
          padding: '16px 0',
          marginBottom: 16,
          borderBottom: `1px solid ${token.colorBorderSecondary}`
        }}>
          <h2 style={{ margin: 0, fontSize: 24 }}>
            <FundOutlined style={{ marginRight: 8 }} />
            剧情分析
          </h2>
        </div>
      )}
      
      <div style={{
        flex: 1,
        display: 'flex',
        gap: isMobile ? 0 : 16,
        flexDirection: isMobile ? 'column' : 'row',
        overflow: 'hidden'
      }}>
        {/* 左侧章节列表 - 桌面端 */}
        {!isMobile && (
        <Card
          title="章节列表"
          style={{ width: 280, height: '100%', overflow: 'hidden' }}
          bodyStyle={{ padding: 0, height: 'calc(100% - 57px)', overflow: 'auto' }}
        >
          {chapters.length === 0 ? (
            <Empty description="暂无章节" style={{ marginTop: 60 }} />
          ) : (
            <List
              dataSource={chapters}
              renderItem={(chapter) => (
                <List.Item
                  key={chapter.id}
                  onClick={() => handleChapterSelect(chapter.id)}
                  style={{
                    cursor: 'pointer',
                    padding: '12px 16px',
                    background: selectedChapter?.id === chapter.id ? token.colorPrimaryBg : 'transparent',
                    borderLeft: selectedChapter?.id === chapter.id ? `3px solid ${token.colorPrimary}` : '3px solid transparent',
                  }}
                >
                  <List.Item.Meta
                    title={
                      <span style={{ fontSize: 14, fontWeight: selectedChapter?.id === chapter.id ? 600 : 400 }}>
                        第{chapter.chapter_number}章: {chapter.title}
                      </span>
                    }
                    description={
                      <Space size={4}>
                        <Tag color={chapter.content && chapter.content.trim() !== '' ? 'success' : 'default'}>
                          {chapter.word_count || 0}字
                        </Tag>
                      </Space>
                    }
                  />
                </List.Item>
              )}
            />
          )}
        </Card>
        )}

        {/* 移动端章节列表抽屉 */}
      {isMobile && (
        <Drawer
          title="章节列表"
          placement="left"
          onClose={() => setChapterListVisible(false)}
          open={chapterListVisible}
          width="85%"
          styles={{ body: { padding: 0 } }}
        >
          {chapters.length === 0 ? (
            <Empty description="暂无章节" style={{ marginTop: 60 }} />
          ) : (
            <List
              dataSource={chapters}
              renderItem={(chapter) => (
                <List.Item
                  key={chapter.id}
                  onClick={() => handleChapterSelect(chapter.id)}
                  style={{
                    cursor: 'pointer',
                    padding: '12px 16px',
                    background: selectedChapter?.id === chapter.id ? token.colorPrimaryBg : 'transparent',
                    borderLeft: selectedChapter?.id === chapter.id ? `3px solid ${token.colorPrimary}` : '3px solid transparent',
                  }}
                >
                  <List.Item.Meta
                    title={
                      <span style={{ fontSize: 14, fontWeight: selectedChapter?.id === chapter.id ? 600 : 400 }}>
                        第{chapter.chapter_number}章: {chapter.title}
                      </span>
                    }
                    description={
                      <Space size={4}>
                        <Tag color={chapter.content && chapter.content.trim() !== '' ? 'success' : 'default'}>
                          {chapter.word_count || 0}字
                        </Tag>
                      </Space>
                    }
                  />
                </List.Item>
              )}
            />
          )}
        </Drawer>
        )}

        {/* 右侧内容区域 */}
        <div style={{ flex: 1, display: 'flex', flexDirection: 'column', height: '100%', overflow: 'hidden' }}>
        {!selectedChapter ? (
          <Card style={{ height: '100%' }}>
            <Empty description="请从左侧选择一个章节查看" style={{ marginTop: 100 }} />
          </Card>
        ) : (
          <>
            {/* 工具栏 */}
            <Card size="small" style={{ marginBottom: isMobile ? 8 : 16 }}>
              {isMobile ? (
                // 移动端布局：两行显示
                <div style={{ display: 'flex', flexDirection: 'column', gap: 12 }}>
                  {/* 第一行：标题和翻页按钮 */}
                  <div style={{
                    display: 'flex',
                    justifyContent: 'space-between',
                    alignItems: 'center',
                    gap: 8
                  }}>
                    <Button
                      icon={<LeftOutlined />}
                      onClick={handlePreviousChapter}
                      disabled={!navigation?.previous}
                      title={navigation?.previous ? `上一章: ${navigation.previous.title}` : '已是第一章'}
                      size="small"
                    />
                    <span style={{
                      fontSize: 14,
                      fontWeight: 600,
                      flex: 1,
                      textAlign: 'center',
                      whiteSpace: 'nowrap',
                      overflow: 'hidden',
                      textOverflow: 'ellipsis',
                      padding: '0 8px'
                    }}>
                      第{selectedChapter.chapter_number}章: {selectedChapter.title}
                    </span>
                    <Button
                      icon={<RightOutlined />}
                      onClick={handleNextChapter}
                      disabled={!navigation?.next}
                      title={navigation?.next ? `下一章: ${navigation.next.title}` : '已是最后一章'}
                      size="small"
                    />
                  </div>

                  {/* 第二行：章节、开关、分析按钮 */}
                  <div style={{
                    display: 'flex',
                    justifyContent: 'space-between',
                    alignItems: 'center',
                    gap: 8
                  }}>
                    <Button
                      icon={<UnorderedListOutlined />}
                      onClick={() => setChapterListVisible(true)}
                      size="small"
                    >
                      章节
                    </Button>

                    {hasAnnotations && (
                      <>
                        <Switch
                          checked={showAnnotations}
                          onChange={setShowAnnotations}
                          checkedChildren={<EyeOutlined />}
                          unCheckedChildren={<EyeInvisibleOutlined />}
                          size="small"
                          style={{
                            flexShrink: 0,
                            height: 16,
                            minHeight: 16,
                            lineHeight: '16px'
                          }}
                        />
                        <Button
                          icon={<MenuOutlined />}
                          onClick={() => setSidebarVisible(true)}
                          size="small"
                        >
                          分析
                        </Button>
                      </>
                    )}
                  </div>
                </div>
              ) : (
                // 桌面端布局：保持原样
                <div style={{
                  display: 'flex',
                  justifyContent: 'space-between',
                  alignItems: 'center'
                }}>
                  <Space>
                    <Button
                      icon={<LeftOutlined />}
                      onClick={handlePreviousChapter}
                      disabled={!navigation?.previous}
                      title={navigation?.previous ? `上一章: ${navigation.previous.title}` : '已是第一章'}
                    >
                      上一章
                    </Button>
                    <span style={{ fontSize: 16, fontWeight: 600 }}>
                      第{selectedChapter.chapter_number}章: {selectedChapter.title}
                    </span>
                    <Button
                      icon={<RightOutlined />}
                      onClick={handleNextChapter}
                      disabled={!navigation?.next}
                      title={navigation?.next ? `下一章: ${navigation.next.title}` : '已是最后一章'}
                    >
                      下一章
                    </Button>
                  </Space>

                  <Space>
                    {hasAnnotations && (
                      <>
                        <Switch
                          checked={showAnnotations}
                          onChange={setShowAnnotations}
                          checkedChildren={<EyeOutlined />}
                          unCheckedChildren={<EyeInvisibleOutlined />}
                        />
                        <span style={{ fontSize: 13, color: token.colorTextSecondary }}>显示标注</span>
                      </>
                    )}
                  </Space>
                </div>
              )}

              {hasAnnotations && annotationsData && (
                <div style={{
                  marginTop: 12,
                  fontSize: isMobile ? 11 : 12,
                  color: token.colorTextTertiary,
                  lineHeight: 1.5
                }}>
                  共有 {annotationsData.summary.total_annotations} 个标注：
                  {annotationsData.summary.hooks > 0 && ` 🎣${annotationsData.summary.hooks}个钩子`}
                  {annotationsData.summary.foreshadows > 0 &&
                    ` 🌟${annotationsData.summary.foreshadows}个伏笔`}
                  {annotationsData.summary.plot_points > 0 &&
                    ` 💎${annotationsData.summary.plot_points}个情节点`}
                  {annotationsData.summary.character_events > 0 &&
                    ` 👤${annotationsData.summary.character_events}个角色事件`}
                </div>
              )}
            </Card>

            <ProjectQualityTrendPanel
              trendData={projectQualityTrend}
              loading={loading}
              compact={isMobile}
            />

            <Card
              title="质量验收"
              size={isMobile ? 'small' : 'default'}
              loading={contentLoading}
              style={{ marginBottom: 16 }}
            >
              {!hasQualityAcceptanceData ? (
                renderCompactSettingHint(
                  "暂无质量验收数据",
                  "运行章节分析或重新生成后，这里会汇总质量指标、质检优先项、自动修订草稿与质量画像。",
                  { style: { marginBottom: 0 } },
                )
              ) : (
                <>
                  {normalizedChapterQualityMetrics ? (
                    <>
                      {renderCompactSelectionSummary(qualityAcceptanceSummaryItems, { style: { marginBottom: 10 } })}
                      {qualityRepairGuidance?.summary && renderCompactSettingHint(
                        "修复建议",
                        qualityRepairGuidance.summary,
                        { style: { marginBottom: 10 } },
                      )}
                      {qualityRepairWeakestMetricHint && (
                        <div style={{ marginBottom: 10 }}>
                          {renderCompactFactCard("当前最弱项", qualityRepairWeakestMetricHint)}
                        </div>
                      )}
                      {hasQualityRepairBreakdown && qualityRepairGuidance && (
                        <div
                          style={{
                            display: 'grid',
                            gridTemplateColumns: 'repeat(auto-fit, minmax(220px, 1fr))',
                            gap: 8,
                            marginBottom: 10,
                          }}
                        >
                          <div style={{ minWidth: 0 }}>
                            {qualityRepairGuidance.repairTargets.length > 0 && renderCompactListCard(
                              "下一轮修复",
                              qualityRepairGuidance.repairTargets,
                              { tagText: `${qualityRepairGuidance.repairTargets.length}项`, tagColor: 'gold', style: { height: '100%' } },
                            )}
                          </div>
                          <div style={{ minWidth: 0 }}>
                            {qualityRepairGuidance.preserveStrengths.length > 0 && renderCompactListCard(
                              "保留优势",
                              qualityRepairGuidance.preserveStrengths,
                              { tagText: `${qualityRepairGuidance.preserveStrengths.length}项`, tagColor: 'green', style: { height: '100%' } },
                            )}
                          </div>
                          <div style={{ minWidth: 0 }}>
                            {qualityRepairGuidance.focusAreas.length > 0 && renderCompactListCard(
                              "关注重点",
                              qualityRepairGuidance.focusAreas,
                              { tagText: `${qualityRepairGuidance.focusAreas.length}项`, tagColor: 'blue', style: { height: '100%' } },
                            )}
                          </div>
                        </div>
                      )}
                      {renderCompactMetricGrid(qualityMetricItems, {
                        style: { marginBottom: checkerResult || draftResult || qualityProfileItems.length > 0 ? 12 : 0 },
                      })}
                    </>
                  ) : (
                    renderCompactSettingHint(
                      "暂无质量指标",
                      "当前章节还没有生成质量指标，但你仍可查看质检结论、自动修订草稿和质量画像。",
                      { style: { marginBottom: checkerResult || draftResult || qualityProfileItems.length > 0 ? 12 : 0 } },
                    )
                  )}

                  {checkerResult && (
                    <>
                      {renderCompactSettingHint(
                        "质检优先处理",
                        checkerResult.overall_assessment || '已完成文本质检。',
                        {
                          tone: checkerCriticalCount > 0 ? 'warning' : checkerMajorCount > 0 ? 'info' : 'success',
                          style: { marginBottom: 10 },
                        },
                      )}
                      {renderCompactSelectionSummary(
                        [
                          { label: '严重', value: `${checkerCriticalCount}`, color: checkerCriticalCount > 0 ? 'red' : 'default' },
                          { label: '重要', value: `${checkerMajorCount}`, color: checkerMajorCount > 0 ? 'gold' : 'default' },
                          { label: '一般', value: `${checkerMinorCount}`, color: checkerMinorCount > 0 ? 'blue' : 'default' },
                          { label: '问题总数', value: `${checkerIssueTotal}` },
                          ...(analysisDetail?.checker_created_at
                            ? [{ label: '质检时间', value: new Date(analysisDetail.checker_created_at).toLocaleString() }]
                            : []),
                        ],
                        { style: { marginBottom: checkerPriorityActions.length > 0 ? 10 : 12 } },
                      )}
                      {checkerPriorityActions.length > 0 && renderCompactListCard(
                        "优先处理",
                        checkerPriorityActions,
                        { tagText: `${checkerPriorityActions.length}项`, tagColor: 'red', style: { marginBottom: 12 } },
                      )}
                    </>
                  )}

                  {draftResult && (
                    <>
                      {renderCompactSettingHint(
                        "自动修订草稿",
                        draftResult.change_summary || '系统已根据高优先问题生成自动修订草稿，可先复核再决定是否应用。',
                        {
                          tone: draftResult.is_stale ? 'warning' : 'success',
                          style: { marginBottom: 10 },
                        },
                      )}
                      {renderCompactSelectionSummary(
                        [
                          { label: '高优先问题', value: `${draftPriorityIssueCount}`, color: draftPriorityIssueCount > 0 ? 'red' : 'green' },
                          { label: '已处理', value: `${draftAppliedIssueCount}`, color: 'green' },
                          { label: '草稿字数', value: `${draftResult.revised_word_count}` },
                          { label: '状态', value: draftResult.is_stale ? '已过期' : '可应用', color: draftResult.is_stale ? 'gold' : 'green' },
                        ],
                        { style: { marginBottom: draftUnresolvedIssues.length > 0 || draftResult.revised_text_preview ? 10 : 12 } },
                      )}
                      {draftUnresolvedIssues.length > 0 && renderCompactListCard(
                        "未解决项",
                        draftUnresolvedIssues,
                        { tagText: `${draftUnresolvedIssues.length}项`, tagColor: 'gold', style: { marginBottom: 10 } },
                      )}
                      {draftResult.revised_text_preview && (
                        <div
                          style={{
                            padding: '8px 10px',
                            border: '1px solid #f0f0f0',
                            borderRadius: 8,
                            marginBottom: qualityProfileItems.length > 0 ? 12 : 0,
                          }}
                        >
                          <div style={{ fontWeight: 600, fontSize: 13, marginBottom: 6 }}>修订预览</div>
                          <div
                            style={{
                              color: 'var(--color-text-secondary)',
                              fontSize: 12,
                              lineHeight: 1.7,
                              whiteSpace: 'pre-wrap',
                              wordBreak: 'break-word',
                              maxHeight: 160,
                              overflowY: 'auto',
                            }}
                          >
                            {draftResult.revised_text_preview}
                          </div>
                        </div>
                      )}
                    </>
                  )}

                  {qualityProfileItems.length > 0 && (
                    <>
                      {renderCompactSettingHint(
                        "质量画像摘要",
                        "质量画像汇总了风格、维度与主要优化方向，可用来校准后续章节生成偏好。",
                        { tone: 'success', style: { marginBottom: 10 } },
                      )}
                      {renderCompactFactGrid(
                        qualityProfileItems.map((item) => [item.label, item.description] as [string, string]),
                      )}
                    </>
                  )}
                </>
              )}
            </Card>

            {/* 内容区域 */}
            <div style={{
              flex: 1,
              display: 'flex',
              gap: isMobile ? 0 : 16,
              overflow: 'hidden'
            }}>
              {/* 章节内容 */}
              <Card
                style={{ flex: 1, overflow: 'auto' }}
                bodyStyle={{ padding: isMobile ? '12px' : '24px' }}
                loading={contentLoading}
              >
                {!contentLoading && (
                  <>
                    {!hasAnnotations && (
                      <Alert
                        message="暂无分析数据"
                        description="该章节尚未进行章节分析，无法显示记忆标注。"
                        type="info"
                        showIcon
                        style={{ marginBottom: 24 }}
                      />
                    )}

                    {showAnnotations && hasAnnotations && annotationsData ? (
                      <AnnotatedText
                        content={selectedChapter.content}
                        annotations={annotationsData.annotations}
                        onAnnotationClick={(annotation) => handleAnnotationClick(annotation, 'content')}
                        activeAnnotationId={activeAnnotationId}
                        scrollToAnnotation={scrollToContentAnnotation}
                        style={{
                          lineHeight: isMobile ? 1.8 : 2,
                          fontSize: isMobile ? 14 : 16,
                        }}
                      />
                    ) : (
                      <div
                        style={{
                          lineHeight: isMobile ? 1.8 : 2,
                          fontSize: isMobile ? 14 : 16,
                          whiteSpace: 'pre-wrap',
                          wordBreak: 'break-word',
                        }}
                      >
                        {selectedChapter.content}
                      </div>
                    )}
                  </>
                )}
              </Card>

              {/* 右侧记忆侧边栏（桌面端） */}
              {hasAnnotations && annotationsData && !isMobile && (
                <Card
                  style={{ width: 400, overflow: 'auto' }}
                  bodyStyle={{ padding: 0 }}
                >
                  <MemorySidebar
                    annotations={annotationsData.annotations}
                    activeAnnotationId={activeAnnotationId}
                    onAnnotationClick={(annotation) => handleAnnotationClick(annotation, 'sidebar')}
                    scrollToAnnotation={scrollToSidebarAnnotation}
                  />
                </Card>
              )}
            </div>

            {/* 移动端抽屉 */}
            {hasAnnotations && annotationsData && (
              <Drawer
                title="章节分析"
                placement="right"
                onClose={() => setSidebarVisible(false)}
                open={sidebarVisible}
                width={isMobile ? '90%' : '80%'}
              >
                <MemorySidebar
                  annotations={annotationsData.annotations}
                  activeAnnotationId={activeAnnotationId}
                  onAnnotationClick={(annotation) => {
                    handleAnnotationClick(annotation, 'sidebar');
                    setSidebarVisible(false);
                  }}
                  scrollToAnnotation={scrollToSidebarAnnotation}
                />
              </Drawer>
            )}
          </>
        )}
        </div>
      </div>
    </div>
  );
};

export default ChapterAnalysis;