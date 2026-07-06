import { Suspense, lazy, useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { useNavigate } from 'react-router-dom';
import {
  Button,
  Card,
  Col,
  message,
  Popconfirm,
  Row,
  Space,
  Steps,
  Tag,
  Typography,
  theme,
} from 'antd';
import { InboxOutlined, ReloadOutlined } from '@ant-design/icons';
import { bookImportApi } from '../services/modularApi';
import { isRequestCancelledError } from '../services/core/httpClient';
import { MAX_CONSECUTIVE_TASK_POLL_ERRORS } from '../utils/taskPolling';
import { syncProjectToStoreById } from '../store/hooks';
import InlineDeferredPanel from '../components/InlineDeferredPanel';
import { designDisplayFont } from '../theme/themeConfig';
import type {
  BookImportApplyPayload,
  BookImportPreview,
  BookImportStepFailure,
  BookImportTask,
} from '../types';

const { Text, Title, Paragraph } = Typography;

const LazyBookImportUploadStep = lazy(() => import('../components/BookImportUploadStep'));
const LazyBookImportTaskStatusStep = lazy(() => import('../components/BookImportTaskStatusStep'));
const LazyBookImportPreviewStep = lazy(() => import('../components/BookImportPreviewStep'));
const LazyBookImportProgressStep = lazy(() => import('../components/BookImportProgressStep'));

const renderBookImportLazyFallback = (step: number) => {
  const fallbackByStep = [
    {
      eyebrow: 'Book Import Upload',
      title: '正在整理导入素材入口',
      message: '系统正在恢复文件上传、任务启动与批量导入说明，原有导入任务创建逻辑保持不变。',
      tags: [
        { label: '上传入口', color: 'processing' },
        { label: '导入任务创建', color: 'volcano' },
        { label: '启动逻辑保持原样', color: 'green' },
      ],
    },
    {
      eyebrow: 'Book Import Status',
      title: '正在接入导入任务状态面板',
      message: '系统正在恢复任务轮询、取消入口与状态提示，原有后台任务状态流保持不变。',
      tags: [
        { label: '任务状态', color: 'cyan' },
        { label: '后台轮询恢复中', color: 'processing' },
        { label: '状态流保持原样', color: 'green' },
      ],
    },
    {
      eyebrow: 'Book Import Preview',
      title: '正在展开拆书预览工作区',
      message: '系统正在恢复章节预览、字段校对与应用入口，原有预览数据与确认逻辑保持不变。',
      tags: [
        { label: '拆书预览', color: 'gold' },
        { label: '预览工作区恢复中', color: 'processing' },
        { label: '确认逻辑保持原样', color: 'green' },
      ],
    },
    {
      eyebrow: 'Book Import Apply',
      title: '正在接入导入结果与重试工作区',
      message: '系统正在恢复导入进度、失败步骤重试与结果提示，原有任务收口和重试逻辑保持不变。',
      tags: [
        { label: '导入结果', color: 'blue' },
        { label: '失败步骤重试', color: 'purple' },
        { label: '结果逻辑保持原样', color: 'green' },
      ],
    },
  ] as const;

  const fallback = fallbackByStep[step] ?? fallbackByStep[0];

  return (
    <InlineDeferredPanel
      eyebrow={fallback.eyebrow}
      title={fallback.title}
      message={fallback.message}
      minHeight={260}
      tags={[...fallback.tags]}
    />
  );
};

const syncCompletedProjectToStore = async (projectId: string) => {
  try {
    await syncProjectToStoreById(projectId);
  } catch (error) {
    console.error('同步导入完成项目到 store 失败:', error);
  }
};

const BOOK_IMPORT_CACHE_KEY = 'book_import_page_cache_v1';

type BookImportPageCache = {
  taskId: string | null;
  taskStatus: BookImportTask | null;
  preview: BookImportPreview | null;
  applyProgress: number;
  applyMessage: string;
  applyError: string | null;
  isApplyComplete: boolean;
  failedSteps: BookImportStepFailure[];
  retrying: boolean;
  retryProgress: number;
  retryMessage: string;
  importedProjectId: string | null;
  cachedAt: number;
};

function loadBookImportCache(): BookImportPageCache | null {
  try {
    const raw = sessionStorage.getItem(BOOK_IMPORT_CACHE_KEY);
    if (!raw) return null;
    return JSON.parse(raw) as BookImportPageCache;
  } catch (error) {
    console.warn('读取拆书页面缓存失败:', error);
    return null;
  }
}

function saveBookImportCache(cache: BookImportPageCache) {
  try {
    sessionStorage.setItem(BOOK_IMPORT_CACHE_KEY, JSON.stringify(cache));
  } catch (error) {
    const isQuotaExceeded =
      error instanceof DOMException &&
      (error.name === 'QuotaExceededError' || error.name === 'NS_ERROR_DOM_QUOTA_REACHED');

    if (isQuotaExceeded) {
      // 发生容量溢出时降级为轻量缓存（不保存预览正文），避免持续报错
      try {
        const lightweightCache: BookImportPageCache = {
          ...cache,
          preview: null,
        };
        sessionStorage.setItem(BOOK_IMPORT_CACHE_KEY, JSON.stringify(lightweightCache));
        return;
      } catch (fallbackError) {
        console.warn('写入轻量拆书页面缓存失败:', fallbackError);
        try {
          sessionStorage.removeItem(BOOK_IMPORT_CACHE_KEY);
        } catch {
          // ignore
        }
      }
    }

    console.warn('写入拆书页面缓存失败:', error);
  }
}

function clearBookImportCache() {
  try {
    sessionStorage.removeItem(BOOK_IMPORT_CACHE_KEY);
  } catch (error) {
    console.warn('清理拆书页面缓存失败:', error);
  }
}

function isNotFoundError(error: unknown): boolean {
  if (!error || typeof error !== 'object') return false;
  const maybeError = error as { response?: { status?: number } };
  return maybeError.response?.status === 404;
}

export default function BookImport() {
  const navigate = useNavigate();
  const { token } = theme.useToken();
  const isMobile = window.innerWidth <= 768;
  const [file, setFile] = useState<File | null>(null);

  const [taskId, setTaskId] = useState<string | null>(null);
  const [taskStatus, setTaskStatus] = useState<BookImportTask | null>(null);
  const [preview, setPreview] = useState<BookImportPreview | null>(null);

  const [creatingTask, setCreatingTask] = useState(false);
  const [loadingPreview, setLoadingPreview] = useState(false);
  const [applying, setApplying] = useState(false);
  const [applyProgress, setApplyProgress] = useState(0);
  const [applyMessage, setApplyMessage] = useState('');
  const [applyError, setApplyError] = useState<string | null>(null);
  const [isApplyComplete, setIsApplyComplete] = useState(false);
  const [cacheReady, setCacheReady] = useState(false);

  // 步骤级失败和重试相关状态
  const [failedSteps, setFailedSteps] = useState<BookImportStepFailure[]>([]);
  const [retrying, setRetrying] = useState(false);
  const [retryProgress, setRetryProgress] = useState(0);
  const [retryMessage, setRetryMessage] = useState('');
  const importedProjectId = useRef<string | null>(null);
  const taskPollErrorCountRef = useRef(0);
  const mountedRef = useRef(true);
  const pageSessionRef = useRef(0);
  const applyRunRef = useRef(0);
  const retryRunRef = useRef(0);

  useEffect(() => {
    return () => {
      mountedRef.current = false;
      pageSessionRef.current += 1;
      applyRunRef.current += 1;
      retryRunRef.current += 1;
    };
  }, []);

  const beginPageSession = useCallback(() => {
    pageSessionRef.current += 1;
    return pageSessionRef.current;
  }, []);

  const invalidatePageSession = useCallback(() => {
    pageSessionRef.current += 1;
  }, []);

  const isPageSessionActive = useCallback((sessionId: number) => {
    return mountedRef.current && pageSessionRef.current === sessionId;
  }, []);

  const beginApplyRun = useCallback(() => {
    applyRunRef.current += 1;
    return applyRunRef.current;
  }, []);

  const invalidateApplyRun = useCallback(() => {
    applyRunRef.current += 1;
  }, []);

  const isApplyRunActive = useCallback((runId: number) => {
    return mountedRef.current && applyRunRef.current === runId;
  }, []);

  const beginRetryRun = useCallback(() => {
    retryRunRef.current += 1;
    return retryRunRef.current;
  }, []);

  const invalidateRetryRun = useCallback(() => {
    retryRunRef.current += 1;
  }, []);

  const isRetryRunActive = useCallback((runId: number) => {
    return mountedRef.current && retryRunRef.current === runId;
  }, []);

  const isTaskTerminal = useMemo(() => {
    return !!taskStatus && ['completed', 'failed', 'cancelled'].includes(taskStatus.status);
  }, [taskStatus]);

  const currentStep = useMemo(() => {
    if (!taskId) return 0;
    if (taskStatus && ['pending', 'running'].includes(taskStatus.status)) return 1;
    if (applying || retrying || isApplyComplete || Boolean(applyError) || failedSteps.length > 0) return 3;
    if (preview) return 2;
    return 1;
  }, [taskId, taskStatus, preview, applying, retrying, isApplyComplete, applyError, failedSteps]);

  const canRestart = useMemo(() => {
    return Boolean(
      file ||
      taskId ||
      taskStatus ||
      preview ||
      applyProgress > 0 ||
      applyMessage ||
      applyError ||
      isApplyComplete ||
      failedSteps.length > 0 ||
      retrying
    );
  }, [
    file,
    taskId,
    taskStatus,
    preview,
    applyProgress,
    applyMessage,
    applyError,
    isApplyComplete,
    failedSteps,
    retrying,
  ]);

  const stepItems = [
    { title: '上传文件' },
    { title: '解析中' },
    { title: '预览修改' },
    { title: '生成导入' },
  ];
  const currentStepText = stepItems[currentStep]?.title || '上传文件';
  const heroStats = [
    { label: '当前步骤', value: currentStepText, compact: true },
    { label: '预览章节', value: preview?.chapters?.length ?? 0 },
    { label: '失败步骤', value: failedSteps.length },
    { label: '导入状态', value: isApplyComplete ? '已完成' : retrying ? '重试中' : applying ? '导入中' : '待处理', compact: true },
  ];
  const importGuideSteps = [
    '先看当前步骤与统计卡，确认现在是在上传、解析、预览还是正式导入阶段。',
    '再进入对应步骤面板处理内容，只在预览阶段改章节细节，在导入阶段观察进度和失败项。',
    '最后再决定是否重试失败步骤或重新开始，避免在任务仍可恢复时过早清空当前流水线。',
  ];
  const importFocus = isApplyComplete
    ? {
        title: failedSteps.length > 0 ? '检查失败步骤并决定是否补跑' : '导入已完成，可以回看结果',
        note: failedSteps.length > 0
          ? `当前导入主流程已经结束，但还有 ${failedSteps.length} 个失败步骤待处理，适合先判断是否重试或跳过。`
          : '当前导入已经完成，适合回看生成结果并确认项目内容是否已正确落库。',
      }
    : retrying
      ? {
          title: '等待失败步骤重试回流',
          note: '当前正在补跑失败步骤，建议先观察进度反馈，避免重复点击或提前重开任务。',
        }
      : applying
        ? {
            title: '关注导入与生成进度',
            note: '当前已经进入正式导入阶段，适合优先观察应用进度、错误提示和失败步骤列表。',
          }
        : currentStep === 2
          ? {
              title: '逐章校对预览内容',
              note: '当前处在预览修改阶段，适合集中检查章节切分、标题和局部内容，再决定是否正式导入。',
            }
          : currentStep === 1
            ? {
                title: '等待解析结果生成',
                note: '当前任务还在解析中，先关注状态刷新与任务可恢复性，等预览数据就绪后再进入内容校对。',
              }
            : {
                title: taskId ? '继续当前导入流水线' : '从上传文件开始建立任务',
                note: taskId
                  ? '当前已经有导入任务上下文，适合沿着现有流水线继续推进，而不是重复创建新任务。'
                  : '当前还在上传入口，先选定源文件并启动解析任务，再进入后续预览与导入步骤。',
              };

  useEffect(() => {
    const sessionId = beginPageSession();
    const cache = loadBookImportCache();
    if (!isPageSessionActive(sessionId)) {
      return;
    }
    if (cache) {
      const cacheAgeMs = typeof cache.cachedAt === 'number'
        ? Date.now() - cache.cachedAt
        : Number.POSITIVE_INFINITY;

      // 6 小时后不再恢复旧缓存，避免误接回历史 taskId
      if (cacheAgeMs > 6 * 60 * 60 * 1000) {
        clearBookImportCache();
      } else {
        setTaskId(cache.taskId);
        setTaskStatus(cache.taskStatus);
        setPreview(cache.preview);
        setApplyProgress(cache.applyProgress);
        setApplyError(cache.applyError);
        setIsApplyComplete(cache.isApplyComplete);
        const restoredFailedSteps = Array.isArray(cache.failedSteps) ? cache.failedSteps : [];
        const hadRetryInFlight = Boolean(cache.retrying);
        setFailedSteps(restoredFailedSteps);
        setRetrying(false);
        setRetryProgress(hadRetryInFlight ? 0 : (cache.retryProgress || 0));
        setRetryMessage(hadRetryInFlight ? '' : (cache.retryMessage || ''));
        importedProjectId.current = cache.importedProjectId || null;
        setApplyMessage(
          hadRetryInFlight && restoredFailedSteps.length > 0
            ? '检测到上次重试在刷新前中断，请重新点击“重试失败步骤”'
            : (cache.applyMessage || (cache.applyProgress > 0 && !cache.isApplyComplete
              ? '已恢复导入进度，请继续等待当前任务完成'
              : ''))
        );
        message.info('已恢复上次的导入进度');
      }
    }
    if (isPageSessionActive(sessionId)) {
      setCacheReady(true);
    }
  }, [beginPageSession, isPageSessionActive]);

  useEffect(() => {
    if (!cacheReady) return;

    const shouldClearCompletedCache = isApplyComplete && failedSteps.length === 0 && !retrying && !applyError;
    if (shouldClearCompletedCache) {
      clearBookImportCache();
      return;
    }

    const hasCacheData = Boolean(
      taskId ||
      taskStatus ||
      preview ||
      applyError ||
      applyProgress > 0 ||
      applyMessage ||
      failedSteps.length > 0 ||
      retrying ||
      retryProgress > 0 ||
      retryMessage ||
      importedProjectId.current
    );

    if (!hasCacheData) {
      clearBookImportCache();
      return;
    }

    saveBookImportCache({
      taskId,
      taskStatus,
      // preview 体积较大，不再写入 sessionStorage 缓存
      // 恢复时基于 taskId + taskStatus 重新获取 preview
      preview: null,
      applyProgress,
      applyMessage,
      applyError,
      isApplyComplete,
      failedSteps,
      retrying,
      retryProgress,
      retryMessage,
      importedProjectId: importedProjectId.current,
      cachedAt: Date.now(),
    });
  }, [
    cacheReady,
    taskId,
    taskStatus,
    preview,
    applyProgress,
    applyMessage,
    applyError,
    isApplyComplete,
    failedSteps,
    retrying,
    retryProgress,
    retryMessage,
  ]);

  useEffect(() => {
    if (!taskId) return;
    if (isTaskTerminal) return;

    let disposed = false;
    const sessionId = beginPageSession();
    taskPollErrorCountRef.current = 0;

    const timer = setInterval(async () => {
      try {
        const status = await bookImportApi.getTaskStatus(taskId);
        if (disposed || !isPageSessionActive(sessionId)) {
          return;
        }
        taskPollErrorCountRef.current = 0;
        setTaskStatus(status);
      } catch (error) {
        if (disposed || isRequestCancelledError(error) || !isPageSessionActive(sessionId)) {
          return;
        }
        console.error('轮询任务状态失败:', error);
        if (isNotFoundError(error)) {
          clearBookImportCache();
          setTaskId(null);
          setTaskStatus(null);
          setPreview(null);
          setApplyProgress(0);
          setApplyMessage('');
          setApplyError(null);
          setIsApplyComplete(false);
          message.warning('拆书任务已失效（可能因服务重启），请重新上传TXT并开始解析');
          return;
        }
        taskPollErrorCountRef.current += 1;
        if (taskPollErrorCountRef.current < MAX_CONSECUTIVE_TASK_POLL_ERRORS) {
          return;
        }
        window.clearInterval(timer);
        setTaskStatus((prev) => prev ? { ...prev, message: '任务状态同步失败，请刷新页面确认最新结果' } : prev);
        message.error('拆书任务状态同步失败，请刷新页面确认最新结果');
      }
    }, 1500);

    return () => {
      disposed = true;
      clearInterval(timer);
    };
  }, [beginPageSession, isPageSessionActive, taskId, isTaskTerminal]);

  useEffect(() => {
    const sessionId = beginPageSession();
    const fetchPreview = async () => {
      if (!taskId || !taskStatus) return;
      if (taskStatus.status !== 'completed' || preview) return;

      try {
        setLoadingPreview(true);
        const data = await bookImportApi.getPreview(taskId);
        if (!isPageSessionActive(sessionId)) {
          return;
        }
        setPreview(data);
      } catch (error) {
        if (!isPageSessionActive(sessionId)) {
          return;
        }
        console.error('获取预览失败:', error);
        if (isNotFoundError(error)) {
          clearBookImportCache();
          setTaskId(null);
          setTaskStatus(null);
          setPreview(null);
          setApplyProgress(0);
          setApplyMessage('');
          setApplyError(null);
          setIsApplyComplete(false);
          message.warning('拆书任务预览不存在（可能因服务重启），已清空缓存，请重新上传TXT');
        } else {
          message.error('获取预览失败');
        }
      } finally {
        if (isPageSessionActive(sessionId)) {
          setLoadingPreview(false);
        }
      }
    };

    fetchPreview();
  }, [beginPageSession, isPageSessionActive, preview, taskId, taskStatus]);

  const startTask = async () => {
    if (!file) {
      message.warning('请先选择 TXT 文件');
      return;
    }

    const sessionId = beginPageSession();
    invalidateApplyRun();
    invalidateRetryRun();
    try {
      setCreatingTask(true);
      setPreview(null);
      setTaskStatus(null);

      const response = await bookImportApi.createTask({
        file,
      });

      if (!isPageSessionActive(sessionId)) {
        return;
      }
      setTaskId(response.task_id);
      message.success('拆书任务已创建');
    } catch (error) {
      if (!isPageSessionActive(sessionId)) {
        return;
      }
      console.error('创建任务失败:', error);
      message.error('创建拆书任务失败');
    } finally {
      if (isPageSessionActive(sessionId)) {
        setCreatingTask(false);
      }
    }
  };

  const refreshStatus = async () => {
    if (!taskId) return;
    const sessionId = beginPageSession();
    try {
      const status = await bookImportApi.getTaskStatus(taskId);
      if (!isPageSessionActive(sessionId)) {
        return;
      }
      setTaskStatus(status);
    } catch (error) {
      if (isRequestCancelledError(error) || !isPageSessionActive(sessionId)) {
        return;
      }
      console.error('刷新状态失败:', error);
      if (isNotFoundError(error)) {
        clearBookImportCache();
        setTaskId(null);
        setTaskStatus(null);
        setPreview(null);
        setApplyProgress(0);
        setApplyMessage('');
        setApplyError(null);
        setIsApplyComplete(false);
        message.warning('任务不存在，已清空本地缓存，请重新创建拆书任务');
      }
    }
  };

  const cancelTask = async () => {
    if (!taskId) return;
    const sessionId = beginPageSession();
    try {
      await bookImportApi.cancelTask(taskId);
      if (!isPageSessionActive(sessionId)) {
        return;
      }
      message.success('任务已取消');
      await refreshStatus();
    } catch (error) {
      if (!isPageSessionActive(sessionId)) {
        return;
      }
      console.error('取消任务失败:', error);
      message.error('取消任务失败');
    }
  };

  const applyImport = async () => {
    if (!taskId || !preview) return;

    const payload: BookImportApplyPayload = {
      project_suggestion: preview.project_suggestion,
      chapters: preview.chapters,
      outlines: preview.outlines,
      import_mode: 'append',
    };

    const runId = beginApplyRun();
    try {
      setApplying(true);
      setApplyProgress(0);
      setApplyMessage('准备导入...');
      setApplyError(null);
      setIsApplyComplete(false);
      setFailedSteps([]);

      const result = await bookImportApi.applyImportInBackground(taskId, payload);
      if (!isApplyRunActive(runId)) {
        return;
      }

      importedProjectId.current = result.project_id;
      const generatedCareers = result.statistics?.generated_careers ?? 0;
      const generatedEntities = result.statistics?.generated_entities ?? 0;
      const nextFailedSteps = Array.isArray(result.failed_steps) ? result.failed_steps : [];

      setApplyProgress(100);
      setApplyMessage(nextFailedSteps.length > 0 ? '导入完成，但部分生成步骤失败' : '导入完成！');
      setFailedSteps(nextFailedSteps);
      setIsApplyComplete(true);
      setApplying(false);

      if (nextFailedSteps.length === 0) {
        message.success(`导入成功：已生成职业${generatedCareers}个，角色/组织${generatedEntities}个`);
        clearBookImportCache();
        void syncCompletedProjectToStore(result.project_id);
        setTimeout(() => {
          if (isApplyRunActive(runId)) {
            navigate(`/project/${result.project_id}/chapters`);
          }
        }, 1000);
      } else {
        message.warning(`导入完成，但有 ${nextFailedSteps.length} 个生成步骤失败，可点击重试`);
      }
    } catch (error) {
      if (!isApplyRunActive(runId)) {
        return;
      }
      console.error('确认导入失败:', error);
      setApplyError('确认导入失败，无法连接到服务器');
      message.error('确认导入失败');
      setApplying(false);
    }
  };

  const retryFailedSteps = useCallback(async () => {
    if (!taskId || failedSteps.length === 0) return;

    const stepsToRetry = failedSteps.map(f => f.step_name);
    const runId = beginRetryRun();

    try {
      setRetrying(true);
      setRetryProgress(0);
      setRetryMessage('正在重试失败的生成步骤...');

      const result = await bookImportApi.retryFailedStepsInBackground(taskId, stepsToRetry);
      if (!isRetryRunActive(runId)) {
        return;
      }

      setRetrying(false);
      setRetryProgress(100);
      setRetryMessage('重试完成');

      if (result.still_failed && result.still_failed.length > 0) {
        setFailedSteps(result.still_failed);
        message.warning(`重试完成，仍有 ${result.still_failed.length} 个步骤失败`);
      } else {
        setFailedSteps([]);
        message.success('所有步骤重试成功！');
        clearBookImportCache();
        const projectId = result.project_id || importedProjectId.current;
        if (projectId) {
          void syncCompletedProjectToStore(projectId);
          setTimeout(() => {
            if (isRetryRunActive(runId)) {
              navigate(`/project/${projectId}/chapters`);
            }
          }, 1000);
        }
      }
    } catch (error) {
      if (!isRetryRunActive(runId)) {
        return;
      }
      console.error('重试请求失败:', error);
      message.error('重试请求失败，无法连接到服务器');
      setRetrying(false);
    }
  }, [beginRetryRun, failedSteps, isRetryRunActive, navigate, taskId]);

  const skipFailedSteps = useCallback(() => {
    invalidateApplyRun();
    invalidateRetryRun();
    setFailedSteps([]);
    clearBookImportCache();
    const projectId = importedProjectId.current;
    if (projectId) {
      message.info('已跳过失败步骤，正在跳转到项目...');
      navigate(`/project/${projectId}/chapters`);
    }
  }, [invalidateApplyRun, invalidateRetryRun, navigate]);

  const restartImport = useCallback(() => {
    invalidatePageSession();
    invalidateApplyRun();
    invalidateRetryRun();
    clearBookImportCache();
    importedProjectId.current = null;

    setFile(null);
    setTaskId(null);
    setTaskStatus(null);
    setPreview(null);

    setCreatingTask(false);
    setLoadingPreview(false);
    setApplying(false);
    setApplyProgress(0);
    setApplyMessage('');
    setApplyError(null);
    setIsApplyComplete(false);

    setFailedSteps([]);
    setRetrying(false);
    setRetryProgress(0);
    setRetryMessage('');

    message.success('已重新开始，请重新上传 TXT 并解析');
  }, [invalidateApplyRun, invalidatePageSession, invalidateRetryRun]);

  const updateChapter = (index: number, patch: Partial<BookImportPreview['chapters'][number]>) => {
    setPreview(prev => {
      if (!prev) return prev;
      const next = [...prev.chapters];
      next[index] = { ...next[index], ...patch };
      return { ...prev, chapters: next };
    });
  };

  return (
    <div
      style={{
        minHeight: '90vh',
        overflow: 'auto',
        background: `linear-gradient(180deg, ${token.colorBgLayout} 0%, ${token.colorFillSecondary} 100%)`,
        padding: isMobile ? '20px 16px 70px' : '24px 24px 70px',
      }}
    >
      <div style={{ maxWidth: 1400, margin: '0 auto', width: '100%' }}>
        <Card
          variant="borderless"
          style={{
            background: `linear-gradient(135deg,
              color-mix(in srgb, ${token.colorPrimary} 78%, #6f4537 22%) 0%,
              color-mix(in srgb, ${token.colorInfo} 24%, #162129 76%) 100%)`,
            borderRadius: isMobile ? 16 : 20,
            boxShadow: `0 26px 52px color-mix(in srgb, ${token.colorText} 20%, transparent)`,
            marginBottom: isMobile ? 14 : 16,
            border: `1px solid color-mix(in srgb, ${token.colorBgContainer} 12%, transparent)`,
            position: 'relative',
            overflow: 'hidden',
          }}
        >
          <div style={{ position: 'absolute', top: -48, right: -48, width: 160, height: 160, borderRadius: '50%', background: token.colorWhite, opacity: 0.08, pointerEvents: 'none' }} />
          <div style={{ position: 'absolute', bottom: -40, left: '26%', width: 110, height: 110, borderRadius: '50%', background: token.colorWhite, opacity: 0.05, pointerEvents: 'none' }} />

          <Row align="middle" justify="space-between" gutter={[16, 16]} style={{ position: 'relative', zIndex: 1 }}>
            <Col xs={24} sm={12}>
              <Space direction="vertical" size={4}>
                <Title level={isMobile ? 3 : 2} style={{ margin: 0, color: token.colorWhite, fontFamily: designDisplayFont, letterSpacing: '-0.03em', textShadow: `0 2px 4px ${token.colorBgMask}` }}>
                  <InboxOutlined style={{ color: token.colorWhite, opacity: 0.9, marginRight: 8 }} />
                  拆书导入
                </Title>
                <Text style={{ fontSize: isMobile ? 12 : 14, color: token.colorTextLightSolid, opacity: 0.85, marginLeft: isMobile ? 40 : 48 }}>
                  上传 TXT 并自动解析为章节、预览并导入项目。这里像一条导入流水线：上传、解析、预览、应用和失败重试都在同一处完成。
                </Text>
              </Space>
            </Col>
            <Col xs={24} sm={12}>
              <Space
                size={12}
                style={{
                  width: '100%',
                  display: 'flex',
                  justifyContent: isMobile ? 'flex-start' : 'flex-end',
                }}
              >
                <Tag
                  style={{
                    marginInlineEnd: 0,
                    background: token.colorWhite,
                    border: `1px solid ${token.colorWhite}`,
                    color: token.colorPrimary,
                    fontWeight: 600,
                    borderRadius: 8,
                    paddingInline: 10,
                  }}
                >
                  当前进度：{currentStepText}
                </Tag>
                <Popconfirm
                  title="确认重新开始？"
                  description="将清空当前拆书任务与缓存，并回到上传文件步骤。"
                  onConfirm={restartImport}
                  okText="重新开始"
                  cancelText="取消"
                  disabled={!canRestart}
                >
                  <Button
                    danger
                    type="primary"
                    icon={<ReloadOutlined />}
                    disabled={!canRestart}
                    style={{ boxShadow: '0 6px 16px rgba(0, 0, 0, 0.2)', borderRadius: 10 }}
                  >
                    重新开始
                  </Button>
                </Popconfirm>
              </Space>
            </Col>
          </Row>

          <Row gutter={[12, 12]} style={{ marginTop: isMobile ? 14 : 18, position: 'relative', zIndex: 1 }}>
            {heroStats.map((item) => (
              <Col xs={12} md={6} key={item.label}>
                <div
                  style={{
                    minHeight: 88,
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
                  <Text style={{ color: token.colorWhite, fontWeight: 700, fontSize: item.compact ? 15 : 24, lineHeight: 1.2, wordBreak: 'break-word' }}>
                    {item.value}
                  </Text>
                </div>
              </Col>
            ))}
          </Row>

          <Card
            variant="borderless"
            style={{
              marginTop: isMobile ? 14 : 18,
              borderRadius: 12,
              background: `linear-gradient(180deg, ${token.colorBgContainer} 0%, ${token.colorFillAlter} 100%)`,
              border: `1px solid ${token.colorBorderSecondary}`,
              boxShadow: token.boxShadow,
            }}
            styles={{ body: { padding: isMobile ? '10px 12px' : '12px 16px' } }}
          >
            <Steps current={currentStep} size={isMobile ? 'small' : 'default'} items={stepItems} />
          </Card>
        </Card>

        <Card
          variant="borderless"
          style={{
            borderRadius: 22,
            background: `linear-gradient(135deg, color-mix(in srgb, ${token.colorPrimary} 10%, white 90%) 0%, color-mix(in srgb, ${token.colorInfo} 10%, white 90%) 100%)`,
            border: `1px solid color-mix(in srgb, ${token.colorPrimary} 16%, white 84%)`,
            boxShadow: `0 18px 36px color-mix(in srgb, ${token.colorText} 8%, transparent)`,
            marginBottom: 16,
          }}
          styles={{ body: { padding: isMobile ? 16 : 18 } }}
        >
          <Row gutter={[16, 16]}>
            <Col xs={24} lg={15}>
              <Space direction="vertical" size={8} style={{ width: '100%' }}>
                <Text style={{ color: token.colorTextTertiary, fontSize: 12, letterSpacing: '0.12em', textTransform: 'uppercase' }}>
                  Import Guide
                </Text>
                <Paragraph style={{ margin: 0, color: token.colorText, lineHeight: 1.75 }}>
                  这个页面更像拆书导入流水线的控制台。原有缓存恢复、任务轮询、预览修改和失败重试逻辑都保持不变，这里只把每一步应该先看什么、后做什么说明得更清楚。
                </Paragraph>
                <div style={{ display: 'flex', flexWrap: 'wrap', gap: 8 }}>
                  {importGuideSteps.map((item, index) => (
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
                        color: token.colorTextBase,
                        fontSize: 12,
                      }}
                    >
                      <span style={{ color: token.colorPrimary, fontWeight: 700 }}>{index + 1}</span>
                      {item}
                    </span>
                  ))}
                </div>
              </Space>
            </Col>
            <Col xs={24} lg={9}>
              <div
                style={{
                  height: '100%',
                  borderRadius: 18,
                  padding: isMobile ? '14px 14px 12px' : '16px 18px 14px',
                  background: `linear-gradient(180deg, ${token.colorBgContainer} 0%, ${token.colorFillAlter} 100%)`,
                  border: `1px solid ${token.colorBorderSecondary}`,
                }}
              >
                <Text style={{ display: 'block', color: token.colorTextTertiary, fontSize: 12, letterSpacing: '0.12em', textTransform: 'uppercase' }}>
                  当前导入焦点
                </Text>
                <Title level={5} style={{ margin: '8px 0 6px', color: token.colorTextBase, fontFamily: designDisplayFont }}>
                  {importFocus.title}
                </Title>
                <Paragraph style={{ margin: 0, color: token.colorTextSecondary, lineHeight: 1.75 }}>
                  {importFocus.note}
                </Paragraph>
              </div>
            </Col>
          </Row>
        </Card>


{currentStep === 0 ? (
  <Suspense fallback={renderBookImportLazyFallback(0)}>
    <LazyBookImportUploadStep
      file={file}
      creatingTask={creatingTask}
      taskId={taskId}
      onFileSelect={setFile}
      onFileRemove={() => {
        setFile(null);
      }}
      onStartTask={startTask}
    />
  </Suspense>
) : null}

{currentStep === 1 ? (
  <Suspense fallback={renderBookImportLazyFallback(1)}>
    <LazyBookImportTaskStatusStep
      taskId={taskId}
      taskStatus={taskStatus}
      onRefreshStatus={refreshStatus}
      onCancelTask={cancelTask}
    />
  </Suspense>
) : null}

      <Card
        variant="borderless"
        style={{
          borderRadius: 24,
          background: `linear-gradient(180deg, ${token.colorBgContainer} 0%, ${token.colorFillAlter} 100%)`,
          border: `1px solid ${token.colorBorderSecondary}`,
          boxShadow: `0 18px 36px color-mix(in srgb, ${token.colorText} 8%, transparent)`,
        }}
        styles={{ body: { padding: isMobile ? 14 : 18 } }}
      >
        {currentStep === 2 ? (
          <Suspense fallback={renderBookImportLazyFallback(2)}>
            <LazyBookImportPreviewStep
              applying={applying}
              loadingPreview={loadingPreview}
              preview={preview}
              setPreview={setPreview}
              updateChapter={updateChapter}
              onApplyImport={applyImport}
            />
          </Suspense>
        ) : null}

        {currentStep === 3 ? (
          <Suspense fallback={renderBookImportLazyFallback(3)}>
            <LazyBookImportProgressStep
              applyProgress={applyProgress}
              applyMessage={applyMessage}
              applyError={applyError}
              failedSteps={failedSteps}
              isApplyComplete={isApplyComplete}
              retryProgress={retryProgress}
              retrying={retrying}
              retryMessage={retryMessage}
              onRetryFailedSteps={retryFailedSteps}
              onSkipFailedSteps={skipFailedSteps}
            />
          </Suspense>
        ) : null}
      </Card>


      </div>
    </div>
  );
}
