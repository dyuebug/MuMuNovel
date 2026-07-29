import React, { useState, useEffect, useCallback, useRef } from 'react';
import { MAX_CONSECUTIVE_TASK_POLL_ERRORS } from '../utils/taskPolling';
import { isAnalysisTaskRetrying } from '../utils/analysisTasks';
import { useParams, useNavigate } from 'react-router-dom';
import { Card, Alert, Button, Space, Switch, Drawer, message, Progress, theme, Typography, Row, Col, Tag } from 'antd';
import {
  ArrowLeftOutlined,
  EyeOutlined,
  EyeInvisibleOutlined,
  MenuOutlined,
  ReloadOutlined,
  LeftOutlined,
  RightOutlined,
} from '@ant-design/icons';
import { api, chapterApi } from '../services/modularApi';
import { isRequestCancelledError } from '../services/core/httpClient';
import type { AnalysisTask } from '../types';
import AnnotatedText, { type MemoryAnnotation } from '../components/AnnotatedText';
import InlineDeferredPanel from '../components/InlineDeferredPanel';
import MemorySidebar from '../components/MemorySidebar';
import { designDisplayFont } from '../theme/themeConfig';

const { Title, Paragraph, Text } = Typography;
const CHAPTER_ANALYSIS_POLL_INTERVAL_MS = 2000;
const CHAPTER_ANALYSIS_POLL_TIMEOUT_MS = 16 * 60 * 1000;

type ActiveAnalysisTask = Pick<AnalysisTask, 'status' | 'error_code' | 'progress'>;
type AnalysisTaskActivityState = ActiveAnalysisTask | null | undefined;

const isAnalysisTaskActive = (task: AnalysisTaskActivityState): task is ActiveAnalysisTask => (
  task?.status === 'pending' ||
  task?.status === 'running' ||
  isAnalysisTaskRetrying(task)
);

interface ChapterData {
  id: string;
  project_id?: string;
  chapter_number: number;
  title: string;
  content: string;
  word_count: number;
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
 * 章节阅读器页面
 * 展示带有记忆标注的章节内容
 */
const ChapterReader: React.FC = () => {
  const { chapterId } = useParams<{ chapterId: string }>();
  const navigate = useNavigate();

  const { token } = theme.useToken();

  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [chapter, setChapter] = useState<ChapterData | null>(null);
  const [annotationsData, setAnnotationsData] = useState<AnnotationsData | null>(null);
  const [showAnnotations, setShowAnnotations] = useState(true);
  const [activeAnnotationId, setActiveAnnotationId] = useState<string | undefined>();
  const [sidebarVisible, setSidebarVisible] = useState(false);
  const [analyzing, setAnalyzing] = useState(false);
  const [analysisProgress, setAnalysisProgress] = useState(0);
  const [navigation, setNavigation] = useState<NavigationData | null>(null);
  const [isMobile, setIsMobile] = useState(() => window.innerWidth < 768);
  const analysisPollIntervalRef = useRef<number | null>(null);
  const analysisPollTimeoutRef = useRef<number | null>(null);
  const analysisPollErrorCountRef = useRef(0);
  const analyzingRef = useRef(false);
  const loadAbortRef = useRef<AbortController | null>(null);
  const chapterIdRef = useRef<string | undefined>(chapterId);

  const sanitizeAnnotationsData = useCallback(
    (loadedAnnotationsData: AnnotationsData | null, chapterContent: string) => {
      if (!loadedAnnotationsData) {
        return null;
      }

      const validAnnotations = loadedAnnotationsData.annotations.filter(
        (annotation: MemoryAnnotation) =>
          annotation.position >= 0 && annotation.position < chapterContent.length,
      );
      const invalidCount = loadedAnnotationsData.annotations.length - validAnnotations.length;

      if (invalidCount > 0) {
        console.warn(
          `${invalidCount} annotation positions are invalid; rendering ${validAnnotations.length} valid annotations only.`,
        );
      }

      return {
        ...loadedAnnotationsData,
        annotations: validAnnotations,
      };
    },
    [],
  );

  const abortPendingLoad = useCallback(() => {
    loadAbortRef.current?.abort();
    loadAbortRef.current = null;
  }, []);

  const stopAnalysisPolling = () => {
    if (analysisPollIntervalRef.current) {
      window.clearInterval(analysisPollIntervalRef.current);
      analysisPollIntervalRef.current = null;
    }
    if (analysisPollTimeoutRef.current) {
      window.clearTimeout(analysisPollTimeoutRef.current);
      analysisPollTimeoutRef.current = null;
    }
    analysisPollErrorCountRef.current = 0;
  };

  useEffect(() => {
    chapterIdRef.current = chapterId;
  }, [chapterId]);

  const startAnalysisPolling = useCallback(
    (resolvedProjectId?: string, chapterContent?: string) => {
      if (!chapterId) {
        return;
      }

      stopAnalysisPolling();
      analysisPollErrorCountRef.current = 0;

      const poll = async () => {
        try {
          if (chapterIdRef.current !== chapterId) {
            stopAnalysisPolling();
            return;
          }
          const statusRes = await chapterApi.getChapterAnalysisStatus(chapterId, resolvedProjectId);
          analysisPollErrorCountRef.current = 0;
          const { status, progress, error_message, auto_recovered } = statusRes;

          setAnalysisProgress(progress || 0);

          if (status === 'completed') {
            stopAnalysisPolling();
            setAnalyzing(false);
            message.success({ content: '分析完成！', key: 'analyze' });

            const annotationsRes = await api.get<unknown, AnnotationsData>(
              `/chapters/${chapterId}/annotations`,
            );
            if (chapterIdRef.current !== chapterId) {
              return;
            }
            const nextChapterContent = chapterContent ?? chapter?.content ?? '';
            setAnnotationsData(sanitizeAnnotationsData(annotationsRes, nextChapterContent));
            return;
          }

          if (status === 'failed') {
            if (isAnalysisTaskRetrying(statusRes)) {
              message.loading({
                content: error_message || '章节分析正在自动重试，请稍候...',
                key: 'analyze',
                duration: 0,
              });
              return;
            }

            stopAnalysisPolling();
            setAnalyzing(false);

            if (auto_recovered) {
              message.warning({
                content: `分析任务已自动恢复：${error_message || '请稍后重试'}`,
                key: 'analyze',
              });
              return;
            }

            message.error({
              content: `分析失败：${error_message || '未知错误'}`,
              key: 'analyze',
            });
          }
        } catch (err) {
          if (isRequestCancelledError(err)) {
            return;
          }

          analysisPollErrorCountRef.current += 1;
          console.error('轮询分析状态失败:', err);
          if (analysisPollErrorCountRef.current < MAX_CONSECUTIVE_TASK_POLL_ERRORS) {
            return;
          }

          stopAnalysisPolling();
          setAnalyzing(false);
          message.error({
            content: '章节分析状态同步失败，请刷新页面确认最新结果',
            key: 'analyze',
          });
        }
      };

      void poll();
      analysisPollIntervalRef.current = window.setInterval(() => {
        void poll();
      }, CHAPTER_ANALYSIS_POLL_INTERVAL_MS);

      analysisPollTimeoutRef.current = window.setTimeout(() => {
        stopAnalysisPolling();
        if (analyzingRef.current) {
          setAnalyzing(false);
          message.warning({ content: '分析耗时较长，请稍后刷新查看结果', key: 'analyze' });
        }
      }, CHAPTER_ANALYSIS_POLL_TIMEOUT_MS);
    },
    [chapter?.content, chapterId, sanitizeAnnotationsData],
  );

  const loadChapterData = useCallback(async () => {
    if (!chapterId) {
      return;
    }

    abortPendingLoad();
    const abortController = new AbortController();
    loadAbortRef.current = abortController;

    try {
      setLoading(true);
      setError(null);
      setAnalyzing(false);
      setAnalysisProgress(0);
      message.destroy('analyze');

      const requestConfig = { signal: abortController.signal };

      // Load chapter content, annotations, navigation, and analysis status in parallel
      // The API interceptor already unwraps response.data
      const [chapterData, loadedAnnotationsData, navigationData, analysisStatus] = await Promise.all([
        api.get<unknown, ChapterData>(`/chapters/${chapterId}`, requestConfig).catch(err => {
          console.error('Failed to load chapter:', err);
          throw err;
        }),
        api.get<unknown, AnnotationsData>(`/chapters/${chapterId}/annotations`, requestConfig).catch(err => {
          if (isRequestCancelledError(err)) {
            throw err;
          }
          console.warn('Failed to load annotations:', err);
          return null;
        }),
        api.get<unknown, NavigationData>(`/chapters/${chapterId}/navigation`, requestConfig).catch(err => {
          if (isRequestCancelledError(err)) {
            throw err;
          }
          console.warn('Failed to load chapter navigation:', err);
          return null;
        }),
        chapterApi.getChapterAnalysisStatus(chapterId, undefined, requestConfig).catch(err => {
          if (isRequestCancelledError(err)) {
            throw err;
          }
          console.warn('Failed to load chapter analysis status:', err);
          return null;
        }),
      ]);

      if (abortController.signal.aborted || loadAbortRef.current !== abortController) {
        return;
      }

      console.log('Chapter payload:', chapterData);
      console.log('Annotations payload:', loadedAnnotationsData);
      console.log('Navigation payload:', navigationData);
      console.log('Chapter analysis status payload:', analysisStatus);

      // Validate chapter payload
      if (!chapterData || !chapterData.content) {
        throw new Error('\u7ae0\u8282\u6570\u636e\u65e0\u6548\uff1a\u7f3a\u5c11\u5185\u5bb9');
      }

      setChapter(chapterData);
      setNavigation(navigationData);
      setAnnotationsData(sanitizeAnnotationsData(loadedAnnotationsData, chapterData.content));

      if (isAnalysisTaskActive(analysisStatus)) {
        setAnalyzing(true);
        setAnalysisProgress(analysisStatus.progress || 0);
        message.loading({ content: '正在恢复章节分析状态...', key: 'analyze', duration: 0 });
        startAnalysisPolling(chapterData.project_id, chapterData.content);
      } else {
        setAnalyzing(false);
        setAnalysisProgress(0);
      }
    } catch (err: unknown) {
      if (isRequestCancelledError(err) || abortController.signal.aborted) {
        return;
      }
      console.error('Failed to load chapter reader data:', err);
      const loadError = err as { response?: { data?: { detail?: string } }; message?: string };
      setError(loadError.response?.data?.detail || loadError.message || '\u52a0\u8f7d\u5931\u8d25');
    } finally {
      if (loadAbortRef.current === abortController) {
        loadAbortRef.current = null;
        setLoading(false);
      }
    }
  }, [abortPendingLoad, chapterId, sanitizeAnnotationsData, startAnalysisPolling]);

  useEffect(() => {
    analyzingRef.current = analyzing;
  }, [analyzing]);

  useEffect(() => {
    const handleResize = () => {
      setIsMobile(window.innerWidth < 768);
    };

    window.addEventListener('resize', handleResize);
    return () => {
      window.removeEventListener('resize', handleResize);
    };
  }, []);

  useEffect(() => {
    if (chapterId) {
      void loadChapterData();
    }
    return () => {
      abortPendingLoad();
      stopAnalysisPolling();
    };
  }, [abortPendingLoad, chapterId, loadChapterData]);

  const handleAnnotationClick = (annotation: MemoryAnnotation) => {
    setActiveAnnotationId(annotation.id);
    // 移动端显示侧边栏
    if (isMobile) {
      setSidebarVisible(true);
    }
  };

  const handleBackClick = () => {
    navigate(-1);
  };

  const handlePreviousChapter = () => {
    if (navigation?.previous) {
      navigate(`/chapters/${navigation.previous.id}/reader`);
    }
  };

  const handleNextChapter = () => {
    if (navigation?.next) {
      navigate(`/chapters/${navigation.next.id}/reader`);
    }
  };

  const handleReanalyze = async () => {
    if (!chapterId) return;

    stopAnalysisPolling();

    try {
      const existingStatus = await chapterApi.getChapterAnalysisStatus(chapterId, chapter?.project_id);
      if (isAnalysisTaskActive(existingStatus)) {
        setAnalyzing(true);
        setAnalysisProgress(existingStatus.progress || 0);
        message.loading({
          content: '章节分析仍在进行，已恢复进度同步...',
          key: 'analyze',
          duration: 0,
        });
        startAnalysisPolling(chapter?.project_id, chapter?.content);
        return;
      }

      setAnalyzing(true);
      setAnalysisProgress(0);
      message.loading({ content: '开始分析章节...', key: 'analyze', duration: 0 });

      await chapterApi.triggerChapterAnalysis(chapterId, chapter?.project_id);
      startAnalysisPolling(chapter?.project_id, chapter?.content);
    } catch (err: unknown) {
      stopAnalysisPolling();
      setAnalyzing(false);
      const error = err as { response?: { data?: { detail?: string } } };
      message.error({
        content: error.response?.data?.detail || '触发分析失败',
        key: 'analyze'
      });
    }
  };

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
        <div style={{ width: 'min(640px, 100%)' }}>
          <InlineDeferredPanel
            eyebrow="Reading Desk"
            title="恢复章节正文与标注工作台"
            message="当前正在读取章节正文、记忆标注、上下章导航与分析状态。原有章节加载、分析恢复和导航切换逻辑保持不变。"
            minHeight={300}
            tags={[
              { label: '正文加载中', color: 'processing' },
              { label: '标注与记忆恢复', color: 'blue' },
              { label: '导航状态同步', color: 'default' },
            ]}
          />
        </div>
      </div>
    );
  }

  if (error || !chapter) {
    return (
      <div style={{ padding: 24 }}>
        <Alert
          message="加载失败"
          description={error || '章节不存在'}
          type="error"
          showIcon
        />
        <Button onClick={handleBackClick} style={{ marginTop: 16 }}>
          返回
        </Button>
      </div>
    );
  }

  const annotationItems = annotationsData?.annotations ?? [];
  const hasAnnotations = annotationItems.length > 0;
  const desktopSidebarVisible = Boolean(hasAnnotations && !isMobile);
  const heroBackground = `linear-gradient(135deg,
    color-mix(in srgb, ${token.colorPrimary} 70%, #6f4737 30%) 0%,
    color-mix(in srgb, ${token.colorInfo} 24%, #162129 76%) 100%)`;
  const editorialInk = '#fff9f0';
  const actionButtonStyle = {
    borderRadius: 999,
    height: 40,
    paddingInline: 14,
    borderColor: 'rgba(255,255,255,0.18)',
    background: 'rgba(255,255,255,0.08)',
    color: editorialInk,
    boxShadow: 'none',
  } as const;
  const panelBackground = `linear-gradient(180deg,
    color-mix(in srgb, ${token.colorBgContainer} 95%, white 5%) 0%,
    color-mix(in srgb, ${token.colorFillAlter} 44%, ${token.colorBgContainer} 56%) 100%)`;
  const panelBorder = `1px solid color-mix(in srgb, ${token.colorBorderSecondary} 88%, white 12%)`;
  const annotationSummaryItems = annotationsData ? [
    { label: '标注总数', value: annotationsData.summary.total_annotations, accent: editorialInk },
    { label: '钩子', value: annotationsData.summary.hooks, accent: token.colorSuccess },
    { label: '伏笔', value: annotationsData.summary.foreshadows, accent: token.colorInfo },
    { label: '情节点', value: annotationsData.summary.plot_points, accent: editorialInk },
  ] : [
    { label: '标注总数', value: 0, accent: editorialInk },
    { label: '钩子', value: 0, accent: token.colorSuccess },
    { label: '伏笔', value: 0, accent: token.colorInfo },
    { label: '情节点', value: 0, accent: editorialInk },
  ];
  return (
    <div style={{ height: '100dvh', minHeight: '100vh', display: 'flex', flexDirection: 'column', gap: 16, overflow: 'hidden', padding: isMobile ? 12 : 16 }}>
      <Card
        variant="borderless"
        style={{
          background: heroBackground,
          borderRadius: 28,
          border: `1px solid color-mix(in srgb, ${token.colorBgContainer} 12%, transparent)`,
          boxShadow: `0 26px 52px color-mix(in srgb, ${token.colorText} 20%, transparent)`,
          overflow: 'hidden',
          position: 'relative',
        }}
        styles={{ body: { padding: isMobile ? 20 : 24 } }}
      >
        <div style={{ position: 'absolute', top: -56, right: -30, width: 170, height: 170, borderRadius: '50%', background: 'rgba(255,255,255,0.08)', pointerEvents: 'none' }} />
        <div style={{ position: 'absolute', bottom: -30, left: isMobile ? '56%' : '26%', width: 120, height: 120, borderRadius: '50%', background: 'rgba(255,255,255,0.05)', pointerEvents: 'none' }} />
        <Row gutter={[24, 18]} align="middle" style={{ position: 'relative', zIndex: 1 }}>
          <Col xs={24} lg={15}>
            <Space direction="vertical" size={8} style={{ width: '100%' }}>
              <Text style={{ color: 'rgba(255,255,255,0.72)', fontSize: 11, letterSpacing: '0.18em', textTransform: 'uppercase' }}>
                Reading Desk
              </Text>
              <Title level={2} style={{ margin: 0, color: editorialInk, fontFamily: designDisplayFont, letterSpacing: '-0.03em' }}>
                第{chapter.chapter_number}章 · {chapter.title}
              </Title>
              <Paragraph style={{ margin: 0, color: 'rgba(255,255,255,0.82)', fontSize: 15, lineHeight: 1.8 }}>
                这里是沉浸式阅读与标注回看工作台。你可以直接阅读正文、检查章节分析记忆点，并在需要时快速切换上下章或重新触发分析。
              </Paragraph>
              <Space wrap size={[10, 10]}>
                <Tag style={{ borderRadius: 999, paddingInline: 12, border: '1px solid rgba(255,255,255,0.12)', background: 'rgba(255,255,255,0.08)', color: editorialInk }}>
                  {chapter.word_count} 字
                </Tag>
                <Tag style={{ borderRadius: 999, paddingInline: 12, border: '1px solid rgba(255,255,255,0.12)', background: 'rgba(255,255,255,0.08)', color: editorialInk }}>
                  {hasAnnotations ? '已生成记忆标注' : '暂无分析标注'}
                </Tag>
                {navigation?.current && (
                  <Tag style={{ borderRadius: 999, paddingInline: 12, border: '1px solid rgba(255,255,255,0.12)', background: 'rgba(255,255,255,0.08)', color: editorialInk }}>
                    Chapter {navigation.current.chapter_number}
                  </Tag>
                )}
              </Space>
            </Space>
          </Col>
          <Col xs={24} lg={9}>
            <Row gutter={[12, 12]}>
              {annotationSummaryItems.map((item) => (
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
                    <Text style={{ color: item.accent, fontWeight: 700, fontSize: 24 }}>{item.value}</Text>
                  </div>
                </Col>
              ))}
            </Row>
          </Col>
        </Row>

        <Space wrap size={[10, 10]} style={{ marginTop: 20, position: 'relative', zIndex: 1 }}>
          <Button icon={<ArrowLeftOutlined />} onClick={handleBackClick} style={actionButtonStyle}>
            返回
          </Button>
          <Button
            icon={<LeftOutlined />}
            onClick={handlePreviousChapter}
            disabled={!navigation?.previous}
            title={navigation?.previous ? `上一章: ${navigation.previous.title}` : '已是第一章'}
            style={actionButtonStyle}
          >
            上一章
          </Button>
          <Button
            icon={<RightOutlined />}
            onClick={handleNextChapter}
            disabled={!navigation?.next}
            title={navigation?.next ? `下一章: ${navigation.next.title}` : '已是最后一章'}
            style={actionButtonStyle}
          >
            下一章
          </Button>
          <Button
            icon={<ReloadOutlined />}
            onClick={handleReanalyze}
            loading={analyzing}
            disabled={analyzing}
            style={actionButtonStyle}
          >
            {analyzing ? '分析中...' : '重新分析'}
          </Button>
          {hasAnnotations && (
            <>
              <Space
                size={8}
                style={{
                  borderRadius: 999,
                  padding: '0 12px',
                  height: 40,
                  border: '1px solid rgba(255,255,255,0.12)',
                  background: 'rgba(255,255,255,0.08)',
                  color: editorialInk,
                }}
              >
                <Switch
                  checked={showAnnotations}
                  onChange={setShowAnnotations}
                  checkedChildren={<EyeOutlined />}
                  unCheckedChildren={<EyeInvisibleOutlined />}
                />
                <Text style={{ color: editorialInk, fontSize: 13 }}>显示标注</Text>
              </Space>
              <Button
                icon={<MenuOutlined />}
                onClick={() => setSidebarVisible(true)}
                style={{ ...actionButtonStyle, display: isMobile ? 'inline-flex' : 'none' }}
              >
                分析
              </Button>
            </>
          )}
        </Space>

        {analyzing && (
          <div style={{ marginTop: 16, position: 'relative', zIndex: 1 }}>
            <Progress percent={analysisProgress} size="small" status="active" />
            <span style={{ fontSize: 12, color: 'rgba(255,255,255,0.78)', marginLeft: 8 }}>
              正在分析章节...
            </span>
          </div>
        )}

        {!analyzing && hasAnnotations && annotationsData && (
          <div style={{ marginTop: 16, fontSize: 12, color: 'rgba(255,255,255,0.78)', position: 'relative', zIndex: 1 }}>
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

      <div
        style={{
          flex: 1,
          display: 'flex',
          overflow: 'hidden',
          background: panelBackground,
          borderRadius: 24,
          border: panelBorder,
          boxShadow: `0 18px 36px color-mix(in srgb, ${token.colorText} 8%, transparent)`,
        }}
      >
        {/* 左侧：章节内容 */}
        <div
          style={{
            flex: 1,
            overflowY: 'auto',
            padding: isMobile ? '16px 12px 24px' : '32px 40px',
            maxWidth: desktopSidebarVisible ? 'calc(100% - 360px)' : '100%',
          }}
        >
          <Card
            variant="borderless"
            style={{
              borderRadius: 24,
              background: token.colorBgContainer,
              border: `1px solid ${token.colorBorderSecondary}`,
              boxShadow: `0 18px 32px color-mix(in srgb, ${token.colorText} 6%, transparent)`,
            }}
            styles={{ body: { padding: isMobile ? 18 : 28 } }}
          >
            <div style={{ maxWidth: 800, margin: '0 auto' }}>
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
                  content={chapter.content}
                  annotations={annotationItems}
                  onAnnotationClick={handleAnnotationClick}
                  activeAnnotationId={activeAnnotationId}
                />
              ) : (
                <div
                  style={{
                    lineHeight: 2,
                    fontSize: 16,
                    whiteSpace: 'pre-wrap',
                    wordBreak: 'break-word',
                    color: token.colorTextBase,
                  }}
                >
                  {chapter.content}
                </div>
              )}

              {/* 底部翻页按钮 */}
              <div style={{ marginTop: 48, paddingTop: 24, borderTop: `1px solid ${token.colorBorderSecondary}` }}>
                <div
                  style={{
                    display: 'flex',
                    width: '100%',
                    justifyContent: 'space-between',
                    gap: 12,
                    flexWrap: isMobile ? 'wrap' : 'nowrap',
                  }}
                >
                  <Button
                    size={isMobile ? 'middle' : 'large'}
                    style={isMobile ? { flex: '1 1 100%', height: 'auto', whiteSpace: 'normal', textAlign: 'left', justifyContent: 'flex-start' } : undefined}
                    icon={<LeftOutlined />}
                    onClick={handlePreviousChapter}
                    disabled={!navigation?.previous}
                  >
                    {navigation?.previous
                      ? (isMobile ? `上一章 · 第${navigation.previous.chapter_number}章` : `上一章: 第${navigation.previous.chapter_number}章 ${navigation.previous.title}`)
                      : '已是第一章'}
                  </Button>
                  <Button
                    size={isMobile ? 'middle' : 'large'}
                    style={isMobile ? { flex: '1 1 100%', height: 'auto', whiteSpace: 'normal', textAlign: 'left', justifyContent: 'space-between' } : undefined}
                    type="primary"
                    icon={<RightOutlined />}
                    onClick={handleNextChapter}
                    disabled={!navigation?.next}
                    iconPosition="end"
                  >
                    {navigation?.next
                      ? (isMobile ? `下一章 · 第${navigation.next.chapter_number}章` : `下一章: 第${navigation.next.chapter_number}章 ${navigation.next.title}`)
                      : '已是最后一章'}
                  </Button>
                </div>
              </div>
            </div>
          </Card>
        </div>

        {/* 右侧：记忆侧边栏（桌面端） */}
        {desktopSidebarVisible && (
          <div
            style={{
              width: 360,
              borderLeft: `1px solid ${token.colorBorderSecondary}`,
              overflowY: 'auto',
              background: `linear-gradient(180deg, ${token.colorFillAlter} 0%, ${token.colorBgContainer} 100%)`,
            }}
          >
            <MemorySidebar
              annotations={annotationItems}
              activeAnnotationId={activeAnnotationId}
              onAnnotationClick={handleAnnotationClick}
            />
          </div>
        )}
      </div>

      {/* 移动端抽屉 */}
      {hasAnnotations && annotationsData && (
        <Drawer
          title="章节分析"
          placement="right"
          onClose={() => setSidebarVisible(false)}
          open={sidebarVisible}
          width={isMobile ? 'min(320px, calc(100vw - 24px))' : 420}
        >
          <MemorySidebar
            annotations={annotationItems}
            activeAnnotationId={activeAnnotationId}
            onAnnotationClick={(annotation) => {
              handleAnnotationClick(annotation);
              setSidebarVisible(false);
            }}
          />
        </Drawer>
      )}
    </div>
  );
};

export default ChapterReader;
