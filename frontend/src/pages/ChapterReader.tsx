import React, { useState, useEffect, useCallback, useRef } from 'react';
import { MAX_CONSECUTIVE_TASK_POLL_ERRORS } from '../utils/taskPolling';
import { isAnalysisTaskRetrying } from '../utils/analysisTasks';
import { useParams, useNavigate } from 'react-router-dom';
import { Card, Spin, Alert, Button, Space, Switch, Drawer, message, Progress, theme } from 'antd';
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
import AnnotatedText, { type MemoryAnnotation } from '../components/AnnotatedText';
import MemorySidebar from '../components/MemorySidebar';

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
      }, 2000);

      analysisPollTimeoutRef.current = window.setTimeout(() => {
        stopAnalysisPolling();
        if (analyzingRef.current) {
          setAnalyzing(false);
          message.warning({ content: '分析超时，请稍后刷新查看结果', key: 'analyze' });
        }
      }, 30000);
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

      if (
        analysisStatus &&
        (analysisStatus.status === 'pending' ||
          analysisStatus.status === 'running' ||
          isAnalysisTaskRetrying(analysisStatus))
      ) {
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
      <div style={{ textAlign: 'center', padding: '100px 0' }}>
        <Spin size="large" tip="加载章节中..." />
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

  return (
    <div style={{ height: '100dvh', minHeight: '100vh', display: 'flex', flexDirection: 'column' }}>
      {/* 顶部工具栏 */}
      <Card
        size="small"
        style={{
          borderRadius: 0,
          borderLeft: 0,
          borderRight: 0,
          borderTop: 0,
        }}
      >
        <div
          style={{
            display: 'flex',
            justifyContent: 'space-between',
            alignItems: isMobile ? 'stretch' : 'center',
            gap: 12,
            flexWrap: 'wrap',
          }}
        >
          <Space wrap size={isMobile ? 8 : 12} style={{ flex: '1 1 420px', minWidth: 0 }}>
            <Button icon={<ArrowLeftOutlined />} onClick={handleBackClick}>
              返回
            </Button>
            <Button
              icon={<LeftOutlined />}
              onClick={handlePreviousChapter}
              disabled={!navigation?.previous}
              title={navigation?.previous ? `上一章: ${navigation.previous.title}` : '已是第一章'}
            >
              上一章
            </Button>
            <span style={{ fontSize: isMobile ? 15 : 16, fontWeight: 600, lineHeight: 1.5, flex: '1 1 auto', minWidth: 0, wordBreak: 'break-word', overflowWrap: 'anywhere' }}>
              第{chapter.chapter_number}章: {chapter.title}
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

          <Space wrap size={isMobile ? 8 : 12} style={{ justifyContent: isMobile ? 'flex-start' : 'flex-end' }}>
            <Button
              icon={<ReloadOutlined />}
              onClick={handleReanalyze}
              loading={analyzing}
              disabled={analyzing}
            >
              {analyzing ? '分析中...' : '重新分析'}
            </Button>
            {hasAnnotations && (
              <>
                <Switch
                  checked={showAnnotations}
                  onChange={setShowAnnotations}
                  checkedChildren={<EyeOutlined />}
                  unCheckedChildren={<EyeInvisibleOutlined />}
                />
                <span style={{ fontSize: 13, color: token.colorTextSecondary }}>显示标注</span>
                <Button
                  icon={<MenuOutlined />}
                  onClick={() => setSidebarVisible(true)}
                  style={{ display: isMobile ? 'inline-block' : 'none' }}
                >
                  分析
                </Button>
              </>
            )}
          </Space>
        </div>

        {analyzing && (
          <div style={{ marginTop: 12 }}>
            <Progress percent={analysisProgress} size="small" status="active" />
            <span style={{ fontSize: 12, color: token.colorTextSecondary, marginLeft: 8 }}>
              正在分析章节...
            </span>
          </div>
        )}

        {!analyzing && hasAnnotations && annotationsData && (
          <div style={{ marginTop: 12, fontSize: 12, color: token.colorTextTertiary }}>
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

      {/* 主内容区域 */}
      <div style={{ flex: 1, display: 'flex', overflow: 'hidden' }}>
        {/* 左侧：章节内容 */}
        <div
          style={{
            flex: 1,
            overflowY: 'auto',
            padding: isMobile ? '16px 12px 24px' : '32px 40px',
            maxWidth: desktopSidebarVisible ? 'calc(100% - 360px)' : '100%',
          }}
        >
          <Card>
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
              background: token.colorBgLayout,
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
