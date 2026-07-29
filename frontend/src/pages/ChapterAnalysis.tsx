import React, { Suspense, lazy, useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { Card, List, Button, Space, Empty, Tag, Alert, Switch, Drawer, message, theme, Typography, Row, Col, Divider } from 'antd';
import {
  EyeOutlined,
  EyeInvisibleOutlined,
  MenuOutlined,
  LeftOutlined,
  RightOutlined,
  UnorderedListOutlined,
} from '@ant-design/icons';
import { useParams } from 'react-router-dom';
import { useStore } from '../store';
import { api, chapterApi } from '../services/modularApi';
import { getAxiosErrorStatus, isRequestCancelledError, silentRequestConfig } from '../services/core/httpClient';
import type { MemoryAnnotation } from '../components/AnnotatedText';
import InlineDeferredPanel from '../components/InlineDeferredPanel';
import WorkflowEntryFallback from '../components/WorkflowEntryFallback';
import type { ChapterAnalysisResponse, ChapterCandidateDraftQualityHighlights, ChapterQualityMetrics, ProjectChapterQualityTrendResponse } from '../types';
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
import { designDisplayFont } from '../theme/themeConfig';

const { Title, Paragraph, Text } = Typography;


const LazyProjectQualityTrendPanel = lazy(() => import('../components/ProjectQualityTrendPanel'));
const LazyChapterContentComparison = lazy(() => import('../components/ChapterContentComparison'));
const LazyAnnotatedText = lazy(() => import('../components/AnnotatedText'));
const LazyMemorySidebar = lazy(() => import('../components/MemorySidebar'));

const toChapterItem = (chapter: {
  id: string;
  chapter_number: number;
  title: string;
  content?: string | null;
  word_count?: number | null;
  status?: string | null;
}): ChapterItem => ({
  id: chapter.id,
  chapter_number: chapter.chapter_number,
  title: chapter.title,
  content: chapter.content ?? '',
  word_count: chapter.word_count ?? 0,
  status: chapter.status ?? 'draft',
});

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

const getCandidateGenerationPathLabel = (value?: string | null): string => {
  switch (value) {
    case 'single_pass':
      return '单轮直出';
    case 'rerank_retry':
      return '重排复选';
    case 'word_budget_repair':
      return '字数修复';
    default:
      return value ? value : '';
  }
};

const getCandidateAttemptKindLabel = (value?: string | null): string => {
  switch (value) {
    case 'initial_candidate':
      return '初始候选';
    case 'rerank_candidate':
      return '重排候选';
    case 'word_budget_repair':
      return '字数修复';
    default:
      return value ? value : '';
  }
};

const renderAnalysisInlineFallback = (
  options: {
    eyebrow: string;
    title: string;
    message: string;
    minHeight?: number;
    tags?: Array<{ label: string; color?: string }>;
  },
) => (
  <InlineDeferredPanel
    eyebrow={options.eyebrow}
    title={options.title}
    message={options.message}
    minHeight={options.minHeight ?? 220}
    tags={options.tags ?? []}
  />
);

const comparisonWorkflowFallback = (
  <WorkflowEntryFallback
    eyebrow="Candidate Comparison"
    title="正在接入候选稿对比工作台"
    message="系统正在恢复候选稿差异视图、质量摘要与应用入口，原有应用、关闭和触发分析逻辑保持不变。"
    tags={[
      { label: '候选稿对比', color: 'purple' },
      { label: '质量摘要恢复中', color: 'processing' },
      { label: '应用逻辑保持原样', color: 'green' },
    ]}
  />
);

const isChapterAnalysisNotFoundError = (error: unknown) => {
  if (getAxiosErrorStatus(error) !== 404) {
    return false;
  }

  const detail = (error as { response?: { data?: { detail?: unknown } } }).response?.data?.detail;
  return detail === 'Chapter analysis not found' || (error as Error).message === 'Chapter analysis not found';
};

/**
 * 项目内的章节剧情分析页面
 * 显示章节列表和带标注的章节内容
 */
const ChapterAnalysis: React.FC = () => {
  const { projectId } = useParams<{ projectId: string }>();
  const currentProject = useStore((state) => state.currentProject);
  
  const [chapters, setChapters] = useState<ChapterItem[]>([]);
  const [selectedChapter, setSelectedChapter] = useState<ChapterItem | null>(null);
  const [annotationsData, setAnnotationsData] = useState<AnnotationsData | null>(null);
  const [analysisDetail, setAnalysisDetail] = useState<ChapterAnalysisResponse | null>(null);
  const [projectQualityTrend, setProjectQualityTrend] = useState<ProjectChapterQualityTrendResponse | null>(null);
  const [navigation, setNavigation] = useState<NavigationData | null>(null);
  const [loading, setLoading] = useState(true);
  const [trendLoading, setTrendLoading] = useState(false);
  const [contentLoading, setContentLoading] = useState(false);
  const [contentMetaLoading, setContentMetaLoading] = useState(false);
  const [navigationLoading, setNavigationLoading] = useState(false);
  const [annotationsLoading, setAnnotationsLoading] = useState(false);
  const [applyingCandidateDraft, setApplyingCandidateDraft] = useState(false);
  const [candidateComparisonVisible, setCandidateComparisonVisible] = useState(false);
  const [candidateComparisonLoading, setCandidateComparisonLoading] = useState(false);
  const [candidateComparisonContent, setCandidateComparisonContent] = useState('');
  const [candidateComparisonWordCount, setCandidateComparisonWordCount] = useState(0);
  const [candidateComparisonHighlights, setCandidateComparisonHighlights] = useState<ChapterCandidateDraftQualityHighlights | null>(null);
  const [showAnnotations, setShowAnnotations] = useState(true);
  const [activeAnnotationId, setActiveAnnotationId] = useState<string | undefined>();
  const [sidebarVisible, setSidebarVisible] = useState(false);
  const [chapterListVisible, setChapterListVisible] = useState(false);
  const [scrollToContentAnnotation, setScrollToContentAnnotation] = useState<string | undefined>();
  const [scrollToSidebarAnnotation, setScrollToSidebarAnnotation] = useState<string | undefined>();
  const [isMobile, setIsMobile] = useState(window.innerWidth < 768);
  const { token } = theme.useToken();
  const initialChapterRequestRef = useRef<string | null>(null);
  const selectedChapterIdRef = useRef<string | null>(null);
  const chapterLoadAbortRef = useRef<AbortController | null>(null);
  const chapterListAbortRef = useRef<AbortController | null>(null);
  const trendRequestIdRef = useRef(0);
  const mountedRef = useRef(true);
  const contentLoadingRef = useRef(false);
  const contentScrollResetTimerRef = useRef<number | null>(null);
  const sidebarScrollResetTimerRef = useRef<number | null>(null);
  const getCachedProjectChapterItems = useCallback((): ChapterItem[] => {
    if (!projectId) {
      return [];
    }

    return useStore
      .getState()
      .chapters
      .filter((chapter) => chapter.project_id === projectId)
      .map(toChapterItem);
  }, [projectId]);

  const clearScrollResetTimers = useCallback(() => {
    if (contentScrollResetTimerRef.current !== null) {
      window.clearTimeout(contentScrollResetTimerRef.current);
      contentScrollResetTimerRef.current = null;
    }
    if (sidebarScrollResetTimerRef.current !== null) {
      window.clearTimeout(sidebarScrollResetTimerRef.current);
      sidebarScrollResetTimerRef.current = null;
    }
  }, []);

  const abortPendingChapterLoad = useCallback(() => {
    chapterLoadAbortRef.current?.abort();
    chapterLoadAbortRef.current = null;
  }, []);

  const abortPendingChapterListLoad = useCallback(() => {
    chapterListAbortRef.current?.abort();
    chapterListAbortRef.current = null;
  }, []);

  // 监听窗口大小变化
  useEffect(() => {
    const handleResize = () => {
      setIsMobile(window.innerWidth < 768);
    };
    
    window.addEventListener('resize', handleResize);
    return () => window.removeEventListener('resize', handleResize);
  }, []);

  useEffect(() => {
    mountedRef.current = true;

    return () => {
      mountedRef.current = false;
      clearScrollResetTimers();
    };
  }, [clearScrollResetTimers]);

  useEffect(() => {
    selectedChapterIdRef.current = selectedChapter?.id ?? null;
  }, [selectedChapter?.id]);

  useEffect(() => {
    contentLoadingRef.current = contentLoading;
  }, [contentLoading]);

  // 加载章节内容和标注
  const loadChapterContent = useCallback(async (chapterId: string) => {
    if (!chapterId) {
      return;
    }
    if (initialChapterRequestRef.current === chapterId && contentLoadingRef.current) {
      return;
    }

    abortPendingChapterLoad();
    const abortController = new AbortController();
    chapterLoadAbortRef.current = abortController;
    initialChapterRequestRef.current = chapterId;

    try {
      setContentLoading(true);
      setContentMetaLoading(true);
      setNavigationLoading(true);
      setAnnotationsLoading(true);
      setAnnotationsData(null);
      setAnalysisDetail(null);
      setNavigation(null);
      setCandidateComparisonVisible(false);
      setCandidateComparisonContent('');
      setCandidateComparisonWordCount(0);
      const requestConfig = { signal: abortController.signal };

      const chapterResponse = await api.get(`/chapters/${chapterId}`, requestConfig);

      if (abortController.signal.aborted || chapterLoadAbortRef.current !== abortController) {
        return;
      }

      const normalizedChapterResponse = chapterResponse.data || chapterResponse;
      setSelectedChapter(normalizedChapterResponse);
      setChapters((prev) => prev.map((item) => (
        item.id === normalizedChapterResponse.id
          ? { ...item, ...normalizedChapterResponse }
          : item
      )));
      setContentLoading(false);

      void api.get(`/chapters/${chapterId}/annotations`, requestConfig)
        .then((annotationsResponse) => {
          if (abortController.signal.aborted || chapterLoadAbortRef.current !== abortController) {
            return;
          }
          const normalizedAnnotationsResponse = (annotationsResponse.data || annotationsResponse) as AnnotationsData;
          setAnnotationsData(normalizedAnnotationsResponse);
          setAnnotationsLoading(false);

          if (!normalizedAnnotationsResponse.has_analysis) {
            setAnalysisDetail(null);
            return;
          }

          void chapterApi.getChapterAnalysis(chapterId, false, silentRequestConfig(requestConfig))
            .then((analysisResponse) => {
              if (abortController.signal.aborted || chapterLoadAbortRef.current !== abortController) {
                return;
              }
              const normalizedAnalysisResponse = analysisResponse && typeof analysisResponse === 'object' && 'data' in analysisResponse
                ? (analysisResponse as { data?: ChapterAnalysisResponse }).data ?? analysisResponse
                : analysisResponse;
              setAnalysisDetail((normalizedAnalysisResponse ?? null) as ChapterAnalysisResponse | null);
            })
            .catch((error) => {
              if (isRequestCancelledError(error) || abortController.signal.aborted) {
                return;
              }
              if (isChapterAnalysisNotFoundError(error)) {
                setAnalysisDetail(null);
                return;
              }
              console.error('加载章节分析失败，已降级为空数据:', error);
            });
        })
        .catch((error) => {
          if (isRequestCancelledError(error) || abortController.signal.aborted) {
            return;
          }
          console.error('加载章节标注失败，已降级为空数据:', error);
          setAnnotationsLoading(false);
        });

      void api.get(`/chapters/${chapterId}/navigation`, requestConfig)
        .then((navigationResponse) => {
          if (abortController.signal.aborted || chapterLoadAbortRef.current !== abortController) {
            return;
          }
          setNavigation((navigationResponse.data || navigationResponse) as NavigationData);
          setNavigationLoading(false);
        })
        .catch((error) => {
          if (isRequestCancelledError(error) || abortController.signal.aborted) {
            return;
          }
          console.error('加载章节导航失败，已降级为空数据:', error);
          setNavigationLoading(false);
        });
    } catch (error) {
      if (isRequestCancelledError(error) || abortController.signal.aborted) {
        return;
      }
      setAnnotationsData(null);
      setAnalysisDetail(null);
      setNavigation(null);
      setAnnotationsLoading(false);
      setNavigationLoading(false);
      console.error('加载章节内容失败:', error);
      message.error('加载章节内容失败');
    } finally {
      if (chapterLoadAbortRef.current === abortController) {
        chapterLoadAbortRef.current = null;
        initialChapterRequestRef.current = null;
        setContentLoading(false);
        setContentMetaLoading(false);
      }
    }
  }, [abortPendingChapterLoad]);

  useEffect(() => {
    if (!projectId) {
      return;
    }
    const cachedProjectChapterItems = getCachedProjectChapterItems();

    if (cachedProjectChapterItems.length === 0) {
      return;
    }

    setChapters((prev) => (prev.length > 0 ? prev : cachedProjectChapterItems));
    setLoading(false);
  }, [getCachedProjectChapterItems, projectId]);

  // 加载章节列表
  useEffect(() => {
    const loadChapters = async () => {
      if (!projectId) return;

      abortPendingChapterListLoad();
      const abortController = new AbortController();
      chapterListAbortRef.current = abortController;
      const trendRequestId = ++trendRequestIdRef.current;

      try {
        setLoading((prev) => prev && chapters.length === 0);
        setTrendLoading(true);
        const response = await api.get(`/chapters/project/${projectId}`, { signal: abortController.signal });

        if (abortController.signal.aborted || chapterListAbortRef.current !== abortController) {
          return;
        }
        // API 拦截器已经解析了 response.data，所以直接使用
        const data = response.data || response;
        const chapterList = (data.items || []) as ChapterItem[];
        setChapters(chapterList);

        const currentSelectedChapterId = selectedChapterIdRef.current;
        const hasSelectedChapterInList = currentSelectedChapterId
          ? chapterList.some((chapter) => chapter.id === currentSelectedChapterId)
          : false;
        const firstChapterWithContent = chapterList.find((ch: ChapterItem) => ch.content && ch.content.trim() !== '');
        if (
          firstChapterWithContent
          && (!currentSelectedChapterId || !hasSelectedChapterInList)
          && initialChapterRequestRef.current !== firstChapterWithContent.id
        ) {
          void loadChapterContent(firstChapterWithContent.id);
        }

        void chapterApi.getProjectChapterQualityTrend(projectId, 12)
          .then((trendResponse) => {
            if (
              abortController.signal.aborted
              || trendRequestIdRef.current !== trendRequestId
              || !mountedRef.current
            ) {
              return;
            }
            setProjectQualityTrend(trendResponse);
          })
          .catch((trendError) => {
            if (isRequestCancelledError(trendError) || abortController.signal.aborted) {
              return;
            }
            console.error('加载项目质量趋势失败:', trendError);
            if (
              trendRequestIdRef.current !== trendRequestId
              || !mountedRef.current
            ) {
              return;
            }
            setProjectQualityTrend(null);
          })
          .finally(() => {
            if (trendRequestIdRef.current === trendRequestId) {
              setTrendLoading(false);
            }
          });
      } catch (error) {
        if (isRequestCancelledError(error) || abortController.signal.aborted) {
          return;
        }
        setProjectQualityTrend(null);
        console.error('加载章节列表失败:', error);
        message.error('加载章节列表失败');
      } finally {
        if (chapterListAbortRef.current === abortController) {
          chapterListAbortRef.current = null;
          setLoading(false);
        }
      }
    };

    void loadChapters();
    return () => {
      abortPendingChapterListLoad();
    };
  }, [abortPendingChapterListLoad, loadChapterContent, projectId]);

  useEffect(() => {
    return () => {
      abortPendingChapterLoad();
      abortPendingChapterListLoad();
      clearScrollResetTimers();
    };
  }, [abortPendingChapterListLoad, abortPendingChapterLoad, clearScrollResetTimers]);

  const applyCandidateDraft = async (): Promise<boolean> => {
    if (!selectedChapter || !analysisDetail?.candidate_draft) {
      return false;
    }

    const candidateDraft = analysisDetail.candidate_draft;
    const confirmSections: string[] = [];
    const applyRiskItems = candidateDraft.apply_risk?.items ?? [];
    if (applyRiskItems.length > 0) {
      const riskSummary = candidateDraft.apply_risk?.summary?.trim()
        || '恢复前请先确认这些一致性 / 质量风险是否可接受。';
      const riskList = applyRiskItems
        .map((item, index) => `${index + 1}. ${item}`)
        .join('\n');
      confirmSections.push(`${riskSummary}\n${riskList}`);
    }
    if (candidateDraft.is_stale) {
      confirmSections.push('候选草稿早于当前章节内容，恢复会覆盖现有正文。');
    }
    if (confirmSections.length > 0) {
      const confirmed = window.confirm(`${confirmSections.join('\n\n')}\n\n是否继续恢复？`);
      if (!confirmed) {
        return false;
      }
    }

    try {
      setApplyingCandidateDraft(true);
      const response = await chapterApi.applyCandidateDraft(selectedChapter.id, {
        attempt_id: candidateDraft.attempt_id,
        allow_stale: candidateDraft.is_stale,
      });
      message.success(response.message || '候选草稿已恢复');
      await loadChapterContent(selectedChapter.id);
      if (projectId) {
        try {
          const trendResponse = await chapterApi.getProjectChapterQualityTrend(projectId, 12);
          setProjectQualityTrend(trendResponse);
        } catch (trendError) {
          console.error('Failed to refresh project quality trend:', trendError);
        }
      }
      return true;
    } catch (error) {
      console.error('Failed to apply candidate draft:', error);
      const errorMessage = typeof error === 'object' && error !== null && 'response' in error
        ? ((error as { response?: { data?: { detail?: string } } }).response?.data?.detail || '恢复候选草稿失败')
        : '恢复候选草稿失败';
      message.error(errorMessage);
      return false;
    } finally {
      setApplyingCandidateDraft(false);
    }
  };

  const handleApplyCandidateDraft = async () => {
    await applyCandidateDraft();
  };

  const handlePreviewCandidateDraftComparison = async () => {
    if (!selectedChapter || !analysisDetail?.candidate_draft) {
      return;
    }

    const candidateDraft = analysisDetail.candidate_draft;
    if (!candidateDraft.can_apply) {
      message.warning('当前候选稿只保留了预览，无法进行正文对比。');
      return;
    }

    try {
      setCandidateComparisonLoading(true);
      setCandidateComparisonHighlights(null);
      const response = await chapterApi.getCandidateDraft(selectedChapter.id, candidateDraft.attempt_id);
      const detailDraft = response.candidate_draft;
      if (!detailDraft?.content) {
        message.warning('当前候选稿缺少完整正文，暂时无法进行对比。');
        return;
      }
      setCandidateComparisonContent(detailDraft.content);
      setCandidateComparisonWordCount(detailDraft.word_count || detailDraft.content.length);
      setCandidateComparisonHighlights(detailDraft.quality_highlights || null);
      setCandidateComparisonVisible(true);
    } catch (error) {
      console.error('Failed to load candidate draft detail:', error);
      const errorMessage = typeof error === 'object' && error !== null && 'response' in error
        ? ((error as { response?: { data?: { detail?: string } } }).response?.data?.detail || '加载候选稿正文失败')
        : '加载候选稿正文失败';
      message.error(errorMessage);
    } finally {
      setCandidateComparisonLoading(false);
    }
  };

  const handleChapterSelect = (chapterId: string) => {
    if (chapterId === selectedChapterIdRef.current || chapterId === initialChapterRequestRef.current) {
      if (isMobile) {
        setChapterListVisible(false);
      }
      return;
    }
    void loadChapterContent(chapterId);
    if (isMobile) {
      setChapterListVisible(false);
    }
  };

  const handlePreviousChapter = () => {
    if (navigation?.previous) {
      void loadChapterContent(navigation.previous.id);
    }
  };

  const handleNextChapter = () => {
    if (navigation?.next) {
      void loadChapterContent(navigation.next.id);
    }
  };

  const handleAnnotationClick = (annotation: MemoryAnnotation, source: 'content' | 'sidebar' = 'content') => {
    setActiveAnnotationId(annotation.id);
    
    if (source === 'content') {
      // 从内容区点击，滚动到侧边栏
      setScrollToSidebarAnnotation(annotation.id);
      if (sidebarScrollResetTimerRef.current !== null) {
        window.clearTimeout(sidebarScrollResetTimerRef.current);
      }
      sidebarScrollResetTimerRef.current = window.setTimeout(() => {
        sidebarScrollResetTimerRef.current = null;
        if (!mountedRef.current) {
          return;
        }
        setScrollToSidebarAnnotation(undefined);
      }, 100);
      
      if (isMobile) {
        setSidebarVisible(true);
      }
    } else {
      // 从侧边栏点击，滚动到内容区
      setScrollToContentAnnotation(annotation.id);
      if (contentScrollResetTimerRef.current !== null) {
        window.clearTimeout(contentScrollResetTimerRef.current);
      }
      contentScrollResetTimerRef.current = window.setTimeout(() => {
        contentScrollResetTimerRef.current = null;
        if (!mountedRef.current) {
          return;
        }
        setScrollToContentAnnotation(undefined);
      }, 100);
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
  const candidateDraft = analysisDetail?.candidate_draft ?? null;
  const candidateSelection = candidateDraft?.candidate_selection ?? null;
  const candidateRepairTargets = candidateDraft?.repair_targets ?? [];
  const candidatePreserveStrengths = candidateDraft?.preserve_strengths ?? [];
  const candidateFailedMetricLabels = (candidateDraft?.failed_metrics ?? [])
    .map((item) => item.label)
    .filter((label): label is string => Boolean(label));
  const candidateApplyRisk = candidateDraft?.apply_risk ?? null;
  const candidateApplyRiskItems = candidateApplyRisk?.items ?? [];
  const candidateDraftStateLabel = !candidateDraft
    ? ''
    : (!candidateDraft.can_apply
      ? '仅预览'
      : (candidateDraft.is_stale ? '已过期' : '可恢复'));
  const candidateSourceLabel = !candidateDraft
    ? ''
    : (candidateDraft.source === 'batch'
      ? '批量生成'
      : (candidateDraft.source === 'chapter' ? '单章生成' : candidateDraft.source));
  const candidateSelectionSummaryItems = candidateDraft ? [
    { label: '状态', value: candidateDraftStateLabel, color: !candidateDraft.can_apply ? 'gold' : (candidateDraft.is_stale ? 'gold' : 'green') },
    { label: '来源', value: candidateSourceLabel },
    { label: '草稿字数', value: `${candidateDraft.word_count}` },
    ...(candidateSelection?.candidate_index && candidateSelection?.candidate_count
      ? [{ label: '候选位次', value: `${candidateSelection.candidate_index}/${candidateSelection.candidate_count}` }]
      : []),
    ...(typeof candidateSelection?.selection_score === 'number'
      ? [{ label: '选择分', value: `${candidateSelection.selection_score.toFixed(1)}` }]
      : []),
    ...(candidateSelection?.generation_path
      ? [{ label: '生成路径', value: getCandidateGenerationPathLabel(candidateSelection.generation_path) }]
      : []),
    ...(candidateSelection?.attempt_kind
      ? [{ label: '尝试类型', value: getCandidateAttemptKindLabel(candidateSelection.attempt_kind) }]
      : []),
    ...(candidateSelection?.winner_candidate_index
      ? [{ label: '胜出候选', value: `${candidateSelection.winner_candidate_index}` }]
      : []),
  ] : [];
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
    || candidateDraft
    || qualityProfileItems.length > 0,
  );
  const qualityAcceptancePending = contentMetaLoading && !hasQualityAcceptanceData;
  const showMissingAnalysisAlert = !annotationsLoading && !contentMetaLoading && !hasAnnotations;
  const annotationSummaryText = useMemo(() => {
    if (!hasAnnotations || !annotationsData) {
      return '';
    }

    const parts = [`共有 ${annotationsData.summary.total_annotations} 个标注：`];
    if (annotationsData.summary.hooks > 0) {
      parts.push(`🎣${annotationsData.summary.hooks}个钩子`);
    }
    if (annotationsData.summary.foreshadows > 0) {
      parts.push(`🌟${annotationsData.summary.foreshadows}个伏笔`);
    }
    if (annotationsData.summary.plot_points > 0) {
      parts.push(`💎${annotationsData.summary.plot_points}个情节点`);
    }
    if (annotationsData.summary.character_events > 0) {
      parts.push(`👤${annotationsData.summary.character_events}个角色事件`);
    }
    return parts.join(' ');
  }, [annotationsData, hasAnnotations]);

  const chapterListNode = useMemo(() => {
    if (chapters.length === 0) {
      return <Empty description="暂无章节" style={{ marginTop: 60 }} />;
    }

    return (
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
              title={(
                <span style={{ fontSize: 14, fontWeight: selectedChapter?.id === chapter.id ? 600 : 400 }}>
                  第{chapter.chapter_number}章: {chapter.title}
                </span>
              )}
              description={(
                <Space size={4}>
                  <Tag color={chapter.content && chapter.content.trim() !== '' ? 'success' : 'default'}>
                    {chapter.word_count || 0}字
                  </Tag>
                </Space>
              )}
            />
          </List.Item>
        )}
      />
    );
  }, [chapters, handleChapterSelect, selectedChapter?.id, token.colorPrimary, token.colorPrimaryBg]);

  const toolbarNode = useMemo(() => {
    if (!selectedChapter) {
      return null;
    }

    return (
      <Card size="small" style={{ marginBottom: isMobile ? 8 : 16 }}>
        {isMobile ? (
          <div style={{ display: 'flex', flexDirection: 'column', gap: 12 }}>
            <div
              style={{
                display: 'flex',
                justifyContent: 'space-between',
                alignItems: 'center',
                gap: 8,
              }}
            >
              <Button
                icon={<LeftOutlined />}
                onClick={handlePreviousChapter}
                disabled={navigationLoading || !navigation?.previous}
                title={navigation?.previous ? `上一章: ${navigation.previous.title}` : '已是第一章'}
                size="small"
              />
              <span
                style={{
                  fontSize: 14,
                  fontWeight: 600,
                  flex: 1,
                  textAlign: 'center',
                  whiteSpace: 'nowrap',
                  overflow: 'hidden',
                  textOverflow: 'ellipsis',
                  padding: '0 8px',
                }}
              >
                第{selectedChapter.chapter_number}章: {selectedChapter.title}
              </span>
              <Button
                icon={<RightOutlined />}
                onClick={handleNextChapter}
                disabled={navigationLoading || !navigation?.next}
                title={navigation?.next ? `下一章: ${navigation.next.title}` : '已是最后一章'}
                size="small"
              />
            </div>

            <div
              style={{
                display: 'flex',
                justifyContent: 'space-between',
                alignItems: 'center',
                gap: 8,
              }}
            >
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
                      lineHeight: '16px',
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
          <div
            style={{
              display: 'flex',
              justifyContent: 'space-between',
              alignItems: 'center',
            }}
          >
            <Space>
              <Button
                icon={<LeftOutlined />}
                onClick={handlePreviousChapter}
                disabled={navigationLoading || !navigation?.previous}
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
                disabled={navigationLoading || !navigation?.next}
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

        {annotationSummaryText && (
          <div
            style={{
              marginTop: 12,
              fontSize: isMobile ? 11 : 12,
              color: token.colorTextTertiary,
              lineHeight: 1.5,
            }}
          >
            {annotationSummaryText}
          </div>
        )}
      </Card>
    );
  }, [
    annotationSummaryText,
    handleNextChapter,
    handlePreviousChapter,
    hasAnnotations,
    isMobile,
    navigation,
    navigationLoading,
    selectedChapter,
    showAnnotations,
    token.colorTextSecondary,
    token.colorTextTertiary,
  ]);

  const qualityAcceptanceCardNode = useMemo(() => (
    <Card
      title="质量验收"
      size={isMobile ? 'small' : 'default'}
      style={{ marginBottom: 16 }}
    >
      {qualityAcceptancePending ? (
        renderCompactSettingHint(
          '质量验收结果正在汇总',
          '系统正在整理质量指标、修复建议、候选草稿与验收摘要；当前章节内容不会因此被改写。',
          { style: { marginBottom: 0 } },
        )
      ) : !hasQualityAcceptanceData ? (
        renderCompactSettingHint(
          '暂无质量验收数据',
          '运行章节分析或重新生成后，这里会汇总质量指标、质检优先项、自动修订草稿与质量画像。',
          { style: { marginBottom: 0 } },
        )
      ) : (
        <>
          {normalizedChapterQualityMetrics ? (
            <>
              {renderCompactSelectionSummary(qualityAcceptanceSummaryItems, { style: { marginBottom: 10 } })}
              {qualityRepairGuidance?.summary && renderCompactSettingHint(
                '修复建议',
                qualityRepairGuidance.summary,
                { style: { marginBottom: 10 } },
              )}
              {qualityRepairWeakestMetricHint && (
                <div style={{ marginBottom: 10 }}>
                  {renderCompactFactCard('当前最弱项', qualityRepairWeakestMetricHint)}
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
                      '下一轮修复',
                      qualityRepairGuidance.repairTargets,
                      { tagText: `${qualityRepairGuidance.repairTargets.length}项`, tagColor: 'gold', style: { height: '100%' } },
                    )}
                  </div>
                  <div style={{ minWidth: 0 }}>
                    {qualityRepairGuidance.preserveStrengths.length > 0 && renderCompactListCard(
                      '保留优势',
                      qualityRepairGuidance.preserveStrengths,
                      { tagText: `${qualityRepairGuidance.preserveStrengths.length}项`, tagColor: 'green', style: { height: '100%' } },
                    )}
                  </div>
                  <div style={{ minWidth: 0 }}>
                    {qualityRepairGuidance.focusAreas.length > 0 && renderCompactListCard(
                      '关注重点',
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
              '暂无质量指标',
              '当前章节还没有生成质量指标，但你仍可查看质检结论、自动修订草稿和质量画像。',
              { style: { marginBottom: checkerResult || draftResult || qualityProfileItems.length > 0 ? 12 : 0 } },
            )
          )}

          {checkerResult && (
            <>
              {renderCompactSettingHint(
                '质检优先处理',
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
                '优先处理',
                checkerPriorityActions,
                { tagText: `${checkerPriorityActions.length}项`, tagColor: 'red', style: { marginBottom: 12 } },
              )}
            </>
          )}

          {draftResult && (
            <>
              {renderCompactSettingHint(
                '自动修订草稿',
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
                '未解决项',
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

          {candidateDraft && (
            <>
              {renderCompactSettingHint(
                '候选草稿',
                candidateDraft.repair_summary || '质量门禁已保留一份候选草稿，可恢复到正文后再继续润色。',
                {
                  tone: candidateDraft.can_apply ? (candidateDraft.is_stale ? 'warning' : 'success') : 'warning',
                  style: { marginBottom: 10 },
                },
              )}
              {candidateSelectionSummaryItems.length > 0 && renderCompactSelectionSummary(
                candidateSelectionSummaryItems,
                { style: { marginBottom: 10 } },
              )}
              {candidateRepairTargets.length > 0 && renderCompactListCard(
                '修复目标',
                candidateRepairTargets,
                { tagText: `${candidateRepairTargets.length}项`, tagColor: 'blue', style: { marginBottom: 10 } },
              )}
              {candidatePreserveStrengths.length > 0 && renderCompactListCard(
                '保留优势',
                candidatePreserveStrengths,
                { tagText: `${candidatePreserveStrengths.length}项`, tagColor: 'green', style: { marginBottom: 10 } },
              )}
              {candidateFailedMetricLabels.length > 0 && renderCompactListCard(
                '门禁关注项',
                candidateFailedMetricLabels,
                { tagText: `${candidateFailedMetricLabels.length}项`, tagColor: 'red', style: { marginBottom: 10 } },
              )}
              {candidateApplyRisk && renderCompactSettingHint(
                '应用风险',
                candidateApplyRisk.summary || '应用前请先确认角色状态、关键设定与章节事实是否一致。',
                { tone: 'warning', style: { marginBottom: 10 } },
              )}
              {candidateApplyRiskItems.length > 0 && renderCompactListCard(
                '风险清单',
                candidateApplyRiskItems,
                { tagText: `${candidateApplyRiskItems.length}项`, tagColor: 'orange', style: { marginBottom: 10 } },
              )}
              {candidateDraft.content_preview && (
                <div
                  style={{
                    padding: '8px 10px',
                    border: '1px solid #f0f0f0',
                    borderRadius: 8,
                    marginBottom: 10,
                  }}
                >
                  <div style={{ fontWeight: 600, fontSize: 13, marginBottom: 6 }}>候选预览</div>
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
                    {candidateDraft.content_preview}
                  </div>
                </div>
              )}
              <Space size={8} style={{ marginBottom: qualityProfileItems.length > 0 ? 12 : 0 }}>
                <Button
                  size="small"
                  onClick={handlePreviewCandidateDraftComparison}
                  loading={candidateComparisonLoading}
                  disabled={!candidateDraft.can_apply}
                >
                  对比预览
                </Button>
                <Button
                  type="primary"
                  size="small"
                  onClick={handleApplyCandidateDraft}
                  loading={applyingCandidateDraft}
                  disabled={!candidateDraft.can_apply}
                >
                  恢复到正文
                </Button>
                {!candidateDraft.can_apply && (
                  <span style={{ fontSize: 12, color: token.colorTextTertiary }}>
                    当前只保留了预览，旧候选草稿暂无法直接恢复。
                  </span>
                )}
              </Space>
            </>
          )}

          {qualityProfileItems.length > 0 && (
            <>
              {renderCompactSettingHint(
                '质量画像摘要',
                '质量画像汇总了风格、维度与主要优化方向，可用来校准后续章节生成偏好。',
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
  ), [
    analysisDetail?.checker_created_at,
    applyingCandidateDraft,
    candidateApplyRisk,
    candidateApplyRiskItems,
    candidateComparisonLoading,
    candidateDraft,
    candidateFailedMetricLabels,
    candidatePreserveStrengths,
    candidateRepairTargets,
    candidateSelectionSummaryItems,
    checkerCriticalCount,
    checkerIssueTotal,
    checkerMajorCount,
    checkerMinorCount,
    checkerPriorityActions,
    checkerResult,
    draftAppliedIssueCount,
    draftPriorityIssueCount,
    draftResult,
    draftUnresolvedIssues,
    handleApplyCandidateDraft,
    handlePreviewCandidateDraftComparison,
    hasQualityAcceptanceData,
    hasQualityRepairBreakdown,
    isMobile,
    normalizedChapterQualityMetrics,
    qualityAcceptancePending,
    qualityAcceptanceSummaryItems,
    qualityMetricItems,
    qualityProfileItems,
    qualityRepairGuidance,
    qualityRepairWeakestMetricHint,
    token.colorTextTertiary,
  ]);

  const contentAreaNode = useMemo(() => {
    if (!selectedChapter) {
      return null;
    }

    return (
      <div
        style={{
          flex: 1,
          display: 'flex',
          gap: isMobile ? 0 : 16,
          overflow: 'hidden',
        }}
      >
        <Card
          style={{ flex: 1, overflow: 'auto' }}
          bodyStyle={{ padding: isMobile ? '12px' : '24px' }}
          loading={contentLoading}
        >
          {!contentLoading && (
            <>
              {showMissingAnalysisAlert && (
                <Alert
                  message="暂无分析数据"
                  description="该章节尚未进行章节分析，无法显示记忆标注。"
                  type="info"
                  showIcon
                  style={{ marginBottom: 24 }}
                />
              )}

              {showAnnotations && hasAnnotations && annotationsData ? (
                <Suspense
                  fallback={renderAnalysisInlineFallback({
                    eyebrow: 'Annotated Reading',
                    title: '正在整理正文标注阅读区',
                    message: '系统正在恢复正文高亮、标注定位与点击联动，原有正文与标注交互逻辑保持不变。',
                    minHeight: 260,
                    tags: [
                      { label: '正文标注', color: 'processing' },
                      { label: '定位联动恢复中', color: 'cyan' },
                      { label: '交互逻辑保持原样', color: 'green' },
                    ],
                  })}
                >
                  <LazyAnnotatedText
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
                </Suspense>
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

        {hasAnnotations && annotationsData && !isMobile && (
          <Card
            style={{ width: 400, overflow: 'auto' }}
            bodyStyle={{ padding: 0 }}
          >
            <Suspense
              fallback={renderAnalysisInlineFallback({
                eyebrow: 'Memory Sidebar',
                title: '正在展开记忆标注侧栏',
                message: '系统正在恢复标注列表、定位入口与侧栏滚动位置，原有侧栏联动逻辑保持不变。',
                minHeight: 320,
                tags: [
                  { label: '记忆标注侧栏', color: 'gold' },
                  { label: '定位入口恢复中', color: 'processing' },
                  { label: '联动逻辑保持原样', color: 'green' },
                ],
              })}
            >
              <LazyMemorySidebar
                annotations={annotationsData.annotations}
                activeAnnotationId={activeAnnotationId}
                onAnnotationClick={(annotation) => handleAnnotationClick(annotation, 'sidebar')}
                scrollToAnnotation={scrollToSidebarAnnotation}
              />
            </Suspense>
          </Card>
        )}
      </div>
    );
  }, [
    activeAnnotationId,
    annotationsData,
    contentLoading,
    handleAnnotationClick,
    hasAnnotations,
    isMobile,
    scrollToContentAnnotation,
    scrollToSidebarAnnotation,
    selectedChapter,
    showAnnotations,
    showMissingAnalysisAlert,
  ]);

  const annotationDrawerNode = useMemo(() => {
    if (!hasAnnotations || !annotationsData) {
      return null;
    }

    return (
      <Drawer
        title="章节分析"
        placement="right"
        onClose={() => setSidebarVisible(false)}
        open={sidebarVisible}
        width={isMobile ? '90%' : '80%'}
      >
        <Suspense
          fallback={renderAnalysisInlineFallback({
            eyebrow: 'Memory Drawer',
            title: '正在接入移动端标注抽屉',
            message: '系统正在恢复移动端标注列表、点击回跳与抽屉阅读区，原有标注选择与关闭逻辑保持不变。',
            minHeight: 320,
            tags: [
              { label: '移动端标注抽屉', color: 'blue' },
              { label: '回跳入口恢复中', color: 'processing' },
              { label: '抽屉逻辑保持原样', color: 'green' },
            ],
          })}
        >
          <LazyMemorySidebar
            annotations={annotationsData.annotations}
            activeAnnotationId={activeAnnotationId}
            onAnnotationClick={(annotation) => {
              handleAnnotationClick(annotation, 'sidebar');
              setSidebarVisible(false);
            }}
            scrollToAnnotation={scrollToSidebarAnnotation}
          />
        </Suspense>
      </Drawer>
    );
  }, [
    activeAnnotationId,
    annotationsData,
    handleAnnotationClick,
    hasAnnotations,
    isMobile,
    scrollToSidebarAnnotation,
    sidebarVisible,
  ]);

  const candidateComparisonNode = useMemo(() => {
    if (!selectedChapter || !candidateComparisonVisible) {
      return null;
    }

    return (
      <Suspense fallback={comparisonWorkflowFallback}>
        <LazyChapterContentComparison
          visible={candidateComparisonVisible}
          onClose={() => {
            setCandidateComparisonVisible(false);
            setCandidateComparisonHighlights(null);
          }}
          chapterId={selectedChapter.id}
          projectId={projectId}
          chapterTitle={selectedChapter.title}
          originalContent={selectedChapter.content || ''}
          newContent={candidateComparisonContent}
          wordCount={candidateComparisonWordCount || candidateComparisonContent.length}
          qualityHighlights={candidateComparisonHighlights}
          onApplyAction={applyCandidateDraft}
          showDiscardButton={false}
          applyButtonText="恢复到正文"
          modalTitle={`候选稿对比 - ${selectedChapter.title}`}
          leftTitle="当前正文"
          rightTitle="候选稿"
        />
      </Suspense>
    );
  }, [
    applyCandidateDraft,
    candidateComparisonContent,
    candidateComparisonHighlights,
    candidateComparisonVisible,
    candidateComparisonWordCount,
    projectId,
    selectedChapter,
  ]);


  if (loading) {
    return (
      <div
        style={{
          minHeight: '100vh',
          padding: isMobile ? '24px 12px' : '32px 16px',
          background: `linear-gradient(180deg, ${token.colorBgLayout} 0%, color-mix(in srgb, ${token.colorPrimary} 6%, ${token.colorBgLayout} 94%) 100%)`,
          display: 'flex',
          alignItems: 'center',
          justifyContent: 'center',
        }}
      >
        <div style={{ width: 'min(680px, 100%)' }}>
          {renderAnalysisInlineFallback({
            eyebrow: 'Analysis Workspace',
            title: '恢复章节分析与候选稿工作台',
            message: '当前正在读取章节列表、正文标注、导航信息与趋势数据。原有章节切换、候选稿对比和分析结果恢复逻辑保持不变。',
            minHeight: 320,
            tags: [
              { label: '章节列表恢复中', color: 'processing' },
              { label: '标注与正文同步', color: 'blue' },
              { label: '趋势数据预热', color: 'default' },
            ],
          })}
        </div>
      </div>
    );
  }

  const heroBackground = `linear-gradient(135deg,
    color-mix(in srgb, ${token.colorPrimary} 74%, #6f4638 26%) 0%,
    color-mix(in srgb, ${token.colorInfo} 26%, #18242d 74%) 100%)`;
  const editorialInk = '#fff9f0';
  const panelBackground = `linear-gradient(180deg,
    color-mix(in srgb, ${token.colorBgContainer} 95%, white 5%) 0%,
    color-mix(in srgb, ${token.colorFillAlter} 44%, ${token.colorBgContainer} 56%) 100%)`;
  const panelBorder = `1px solid color-mix(in srgb, ${token.colorBorderSecondary} 88%, white 12%)`;
  const summaryItems: Array<{ label: string; value: number | string; accent: string; compact?: boolean }> = [
    { label: '章节总数', value: chapters.length, accent: editorialInk },
    { label: '当前章节', value: selectedChapter ? `第${selectedChapter.chapter_number}章` : '未选择', accent: token.colorSuccess, compact: true },
    { label: '标注总数', value: annotationsData?.summary.total_annotations ?? 0, accent: token.colorInfo },
    { label: '趋势数据', value: projectQualityTrend?.items?.length ?? 0, accent: editorialInk },
  ];

  return (
    <div style={{ display: 'flex', flexDirection: 'column', minHeight: '100%', gap: 16, overflow: 'visible', paddingBottom: 24 }}>
      <Card
        variant="borderless"
        style={{
          background: heroBackground,
          borderRadius: 28,
          border: `1px solid color-mix(in srgb, ${token.colorBgContainer} 12%, transparent)`,
          boxShadow: `0 26px 52px color-mix(in srgb, ${token.colorText} 20%, transparent)`,
          overflow: 'hidden',
          position: 'relative',
          flexShrink: 0,
        }}
        styles={{ body: { padding: isMobile ? 20 : 24 } }}
      >
        <div style={{ position: 'absolute', top: -56, right: -28, width: 170, height: 170, borderRadius: '50%', background: 'rgba(255,255,255,0.08)', pointerEvents: 'none' }} />
        <div style={{ position: 'absolute', bottom: -30, left: isMobile ? '56%' : '28%', width: 120, height: 120, borderRadius: '50%', background: 'rgba(255,255,255,0.05)', pointerEvents: 'none' }} />
        <Row gutter={[24, 18]} align="middle" style={{ position: 'relative', zIndex: 1 }}>
          <Col xs={24} lg={14}>
            <Space direction="vertical" size={8} style={{ width: '100%' }}>
              <Text style={{ color: 'rgba(255,255,255,0.72)', fontSize: 11, letterSpacing: '0.18em', textTransform: 'uppercase' }}>
                Analysis Deck
              </Text>
              <Title level={2} style={{ margin: 0, color: editorialInk, fontFamily: designDisplayFont, letterSpacing: '-0.03em' }}>
                剧情分析
              </Title>
              <Paragraph style={{ margin: 0, color: 'rgba(255,255,255,0.82)', fontSize: 15, lineHeight: 1.8 }}>
                查看章节质量、记忆标注、候选稿状态和趋势数据。
              </Paragraph>
              <Space wrap size={[10, 10]}>
                <Tag style={{ borderRadius: 999, paddingInline: 12, border: '1px solid rgba(255,255,255,0.12)', background: 'rgba(255,255,255,0.08)', color: editorialInk }}>
                  {currentProject?.title ?? '当前项目'}
                </Tag>
                <Tag style={{ borderRadius: 999, paddingInline: 12, border: '1px solid rgba(255,255,255,0.12)', background: 'rgba(255,255,255,0.08)', color: editorialInk }}>
                  {selectedChapter ? `${selectedChapter.title}` : '等待选择章节'}
                </Tag>
                {isMobile && (
                  <Button
                    icon={<UnorderedListOutlined />}
                    onClick={() => setChapterListVisible(true)}
                    style={{
                      borderRadius: 999,
                      borderColor: 'rgba(255,255,255,0.18)',
                      background: 'rgba(255,255,255,0.08)',
                      color: editorialInk,
                    }}
                  >
                    章节列表
                  </Button>
                )}
              </Space>
            </Space>
          </Col>
          <Col xs={24} lg={10}>
            <Row gutter={[12, 12]}>
              {summaryItems.map((item) => (
                <Col xs={12} key={item.label}>
                  <div
                    style={{
                      minHeight: 92,
                      borderRadius: 18,
                      padding: '12px 14px',
                      background: 'rgba(255,255,255,0.08)',
                      border: '1px solid rgba(255,255,255,0.1)',
                      backdropFilter: 'blur(10px)',
                      display: 'flex',
                      flexDirection: 'column',
                      justifyContent: 'space-between',
                    }}
                  >
                    <Text style={{ color: 'rgba(255,255,255,0.72)', fontSize: 12, display: 'block' }}>{item.label}</Text>
                    <Text style={{ color: item.accent, fontWeight: 700, fontSize: item.compact ? 15 : 24, lineHeight: 1.2, wordBreak: 'break-word' }}>
                      {item.value}
                    </Text>
                  </div>
                </Col>
              ))}
            </Row>
          </Col>
        </Row>
      </Card>

      <Card
        variant="borderless"
        style={{
          flex: '1 0 auto',
          overflow: 'hidden',
          background: panelBackground,
          borderRadius: 24,
          border: panelBorder,
          boxShadow: `0 18px 36px color-mix(in srgb, ${token.colorText} 8%, transparent)`,
        }}
        styles={{ body: { height: '100%', padding: isMobile ? 16 : 20 } }}
      >
        <Space direction="vertical" size={16} style={{ width: '100%', height: '100%' }}>
          <div
            style={{
              display: 'flex',
              justifyContent: 'space-between',
              alignItems: isMobile ? 'flex-start' : 'center',
              gap: 12,
              flexDirection: isMobile ? 'column' : 'row',
            }}
          >
            <Space direction="vertical" size={4}>
              <Text style={{ fontSize: 12, letterSpacing: '0.12em', textTransform: 'uppercase', color: token.colorTextTertiary }}>
                Quality Workspace
              </Text>
              <Title level={4} style={{ margin: 0, fontFamily: designDisplayFont, color: token.colorTextBase }}>
                章节分析工作区
              </Title>
              <Paragraph style={{ margin: 0, color: token.colorTextSecondary }}>
                左侧维护章节上下文，右侧集中查看质量趋势、验收摘要、正文标注和候选稿对比。
              </Paragraph>
            </Space>
            {!isMobile && (
              <Tag color="blue" style={{ borderRadius: 999, paddingInline: 12 }}>
                选择章节后开始查看分析细节
              </Tag>
            )}
          </div>

          <Divider style={{ margin: 0, borderColor: token.colorBorderSecondary }} />
      
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
          style={{ width: 280, height: '100%', overflow: 'hidden', borderRadius: 20, border: `1px solid ${token.colorBorderSecondary}` }}
          bodyStyle={{ padding: 0, height: 'calc(100% - 57px)', overflow: 'auto' }}
        >
          {chapterListNode}
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
          {chapterListNode}
        </Drawer>
        )}

        {/* 右侧内容区域 */}
        <div style={{ flex: 1, display: 'flex', flexDirection: 'column', height: '100%', overflow: 'hidden' }}>
        {!selectedChapter ? (
          <Card
            variant="borderless"
            style={{
              height: '100%',
              borderRadius: 22,
              background: `linear-gradient(180deg, ${token.colorBgContainer} 0%, ${token.colorFillAlter} 100%)`,
              border: `1px dashed ${token.colorBorder}`,
            }}
          >
            <Empty description="请先选择一个章节，再查看对应的质量分析、标注和候选稿。" style={{ marginTop: 100 }}>
              {isMobile ? (
                <Button type="primary" icon={<UnorderedListOutlined />} onClick={() => setChapterListVisible(true)}>
                  打开章节列表
                </Button>
              ) : null}
            </Empty>
          </Card>
        ) : (
          <>
            {toolbarNode}

            <Suspense
              fallback={(
                <div style={{ marginBottom: 16 }}>
                  {renderAnalysisInlineFallback({
                    eyebrow: 'Quality Trend',
                    title: '正在加载章节质量趋势',
                    message: '系统正在恢复质量曲线、章节维度趋势与紧凑视图，原有趋势查询和展示逻辑保持不变。',
                    minHeight: isMobile ? 220 : 240,
                    tags: [
                      { label: '质量趋势', color: 'processing' },
                      { label: '趋势曲线恢复中', color: 'cyan' },
                      { label: '查询逻辑保持原样', color: 'green' },
                    ],
                  })}
                </div>
              )}
            >
              <LazyProjectQualityTrendPanel
                trendData={projectQualityTrend}
                loading={loading || trendLoading}
                compact={isMobile}
              />
            </Suspense>

            {qualityAcceptanceCardNode}

            {contentAreaNode}

            {annotationDrawerNode}
            {candidateComparisonNode}
          </>
        )}
        </div>
      </div>
        </Space>
      </Card>
    </div>
  );
};

export default ChapterAnalysis;
