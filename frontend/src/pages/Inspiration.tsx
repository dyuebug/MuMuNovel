import React, { useState, useEffect, useLayoutEffect, useRef, useCallback } from 'react';
import { useBusyNavigationGuard } from '../hooks/useBusyNavigationGuard';
import { useLocation, useNavigate } from 'react-router-dom';
import { Alert, Card, Input, Button, Space, Typography, message, Modal, Switch, Tag, theme } from 'antd';
import { SendOutlined, ArrowLeftOutlined, ReloadOutlined, LoadingOutlined } from '@ant-design/icons';
import { backgroundTaskApi, inspirationApi, type BackgroundTaskStatus } from '../services/modularApi';
import { AIProjectGenerator, type GenerationConfig } from '../components/AIProjectGenerator';
import {
  GenerationExecutionSettingsPanel,
  useGenerationExecutionSettings,
} from '../components/GenerationExecutionSettings';
import { syncProjectToStoreById } from '../store/hooks';
import { invalidateAllProjectCollectionFreshness } from '../store/projectCollectionRefresh';
import { invalidateProjectCareers } from '../services/projectCareers';
import type { InspirationOptionResponse, InspirationQuickGenerateResponse } from '../services/modules/inspiration';
import { waitForBackgroundTaskCompletion } from '../utils/taskPolling';
import { designDisplayFont } from '../theme/themeConfig';

const { Title, Text, Paragraph } = Typography;
const { TextArea } = Input;

type Step = 'idea' | 'title' | 'description' | 'theme' | 'genre' | 'perspective' | 'outline_mode' | 'confirm' | 'generating' | 'complete';
type InspirationOptionStep = 'title' | 'description' | 'theme' | 'genre';

type InspirationResearchAsset = {
  title: string;
  source?: string;
  summary?: string;
};

type InspirationOptionRequest = {
  step: InspirationOptionStep;
  context: Partial<WizardData> & {
    initial_idea?: string;
  };
  enable_web_research?: boolean;
  web_research_query?: string;
};

type InspirationResearchBundle = {
  query: string;
  assets: InspirationResearchAsset[];
};


interface Message {
  type: 'ai' | 'user';
  content: string;
  options?: string[];
  isMultiSelect?: boolean;
  optionsDisabled?: boolean; // 标记选项是否已禁用
  canRefine?: boolean; // 是否可以优化（用于支持多轮对话）
  step?: Step; // 当前步骤（用于反馈）
}

interface WizardData {
  title: string;
  description: string;
  theme: string;
  genre: string[];
  narrative_perspective: string;
  outline_mode: 'one-to-one' | 'one-to-many';
}

// 缓存数据接口
interface CacheData {
  messages: Message[];
  currentStep: Step;
  wizardData: Partial<WizardData>;
  initialIdea: string;
  selectedOptions: string[];
  executionEnableWebResearch: boolean;
  executionWebResearchQuery: string;
  inspirationResearch: InspirationResearchBundle;
  lastFailedRequest: InspirationOptionRequest | null;
  timestamp: number;
}

// 缓存键
const CACHE_KEY = 'inspiration_conversation_cache';
// 缓存有效期：24小时
const CACHE_EXPIRY = 24 * 60 * 60 * 1000;

const isRecord = (value: unknown): value is Record<string, unknown> => (
  Boolean(value) && typeof value === 'object' && !Array.isArray(value)
);

const isInspirationOptionStep = (value: unknown): value is InspirationOptionStep => (
  value === 'title' || value === 'description' || value === 'theme' || value === 'genre'
);

const toStringArray = (value: unknown): string[] => {
  if (typeof value === 'string') {
    return value
      .split(/[、,\n]/)
      .map((item) => item.trim())
      .filter(Boolean);
  }

  if (!Array.isArray(value)) {
    return [];
  }

  return value
    .map((item) => (typeof item === 'string' ? item.trim() : ''))
    .filter(Boolean);
};

const normalizeInspirationOptionResponse = (value: unknown): InspirationOptionResponse | null => {
  if (!isRecord(value)) {
    return null;
  }

  const options = toStringArray(value.options);
  return {
    prompt: typeof value.prompt === 'string' ? value.prompt : undefined,
    options,
    error: typeof value.error === 'string' ? value.error : undefined,
    research_query: typeof value.research_query === 'string' ? value.research_query : undefined,
    research_assets: Array.isArray(value.research_assets)
      ? value.research_assets
        .filter(isRecord)
        .map((asset) => ({
          title: typeof asset.title === 'string' ? asset.title : '',
          source: typeof asset.source === 'string' ? asset.source : undefined,
          summary: typeof asset.summary === 'string' ? asset.summary : undefined,
        }))
      : undefined,
  };
};

const normalizeInspirationQuickGenerateResponse = (value: unknown): InspirationQuickGenerateResponse | null => {
  if (!isRecord(value)) {
    return null;
  }

  return {
    title: typeof value.title === 'string' ? value.title : '',
    description: typeof value.description === 'string' ? value.description : '',
    theme: typeof value.theme === 'string' ? value.theme : '',
    genre: toStringArray(value.genre),
    narrative_perspective: typeof value.narrative_perspective === 'string'
      ? value.narrative_perspective
      : '',
    error: typeof value.error === 'string' ? value.error : undefined,
  };
};

const getInspirationTaskStep = (
  task: BackgroundTaskStatus,
  fallbackRequest?: InspirationOptionRequest | null,
): InspirationOptionStep | null => {
  const checkpointStep = task.checkpoint?.inspiration_step;
  if (isInspirationOptionStep(checkpointStep)) {
    return checkpointStep;
  }

  if (isInspirationOptionStep(fallbackRequest?.step)) {
    return fallbackRequest.step;
  }

  return null;
};

const areSameStringArray = (left?: string[], right?: string[]) => (
  JSON.stringify(left ?? []) === JSON.stringify(right ?? [])
);

const formatInspirationPrompt = (fallbackPrompt: string, response?: InspirationOptionResponse) => {
  const prompt = response?.prompt?.trim() || fallbackPrompt;
  const query = response?.research_query?.trim();
  const assets = Array.isArray(response?.research_assets) ? response.research_assets : [];
  if (!query && assets.length === 0) {
    return prompt;
  }

  const assetTitles = assets
    .map((asset) => asset.title?.trim() || asset.source?.trim() || '')
    .filter(Boolean)
    .slice(0, 2);
  const notes = [`已结合联网检索${query ? `：${query}` : ''}`];
  if (assetTitles.length > 0) {
    notes.push(`参考资料：${assetTitles.join(' / ')}`);
  }
  return `${prompt}\n\n${notes.join('\n')}`;
};

const clampPreviewText = (value: string | undefined, fallback: string, maxLength = 72) => {
  const normalized = value?.trim();
  if (!normalized) {
    return fallback;
  }
  return normalized.length > maxLength
    ? `${normalized.slice(0, maxLength).trim()}...`
    : normalized;
};

const Inspiration: React.FC = () => {
  const navigate = useNavigate();
  const location = useLocation();
  const [currentStep, setCurrentStep] = useState<Step>('idea');
  const {
    setBusy: setIsGenerationBusy,
    releaseBusy: releaseGenerationBusy,
    shouldDisableNavigation,
  } = useBusyNavigationGuard();
  const [isMobile, setIsMobile] = useState(window.innerWidth <= 768);
  const { token } = theme.useToken();
  const alphaColor = (color: string, alpha: number) => `color-mix(in srgb, ${color} ${(alpha * 100).toFixed(0)}%, transparent)`;

  useEffect(() => {
    const handleResize = () => {
      setIsMobile(window.innerWidth <= 768);
    };
    window.addEventListener('resize', handleResize);
    return () => window.removeEventListener('resize', handleResize);
  }, []);

  const [messages, setMessages] = useState<Message[]>([
    {
      type: 'ai',
      content: '你好！我是你的AI创作助手。\n\n告诉我你的核心灵感吧，最好带上这三点：主角是谁、眼前冲突是什么、失败代价是什么。',
    }
  ]);
  const [inputValue, setInputValue] = useState('');
  const [loading, setLoading] = useState(false);
  const [selectedOptions, setSelectedOptions] = useState<string[]>([]);
  const [inspirationResearch, setInspirationResearch] = useState<InspirationResearchBundle>({
    query: '',
    assets: [],
  });

  // 收集的数据
  const [wizardData, setWizardData] = useState<Partial<WizardData>>({});
  // 保存用户的原始想法，用于保持上下文一致性
  const [initialIdea, setInitialIdea] = useState<string>('');
  
  // 反馈相关状态
  const [feedbackValue, setFeedbackValue] = useState('');
  const [showFeedbackInput, setShowFeedbackInput] = useState<number | null>(null); // 当前显示反馈输入的消息索引
  const [refining, setRefining] = useState(false); // 正在优化选项

  // 生成配置
  const [generationConfig, setGenerationConfig] = useState<GenerationConfig | null>(null);
  const [resumeProjectId, setResumeProjectId] = useState<string | null>(null);
  const [executionModalOpen, setExecutionModalOpen] = useState(false);
  const [executionModel, setExecutionModel] = useState<string | undefined>();
  const [executionEnableMcp, setExecutionEnableMcp] = useState(true);
  const [executionEnableWebResearch, setExecutionEnableWebResearch] = useState(false);
  const [executionWebResearchQuery, setExecutionWebResearchQuery] = useState('');
  const [showResearchQueryEditor, setShowResearchQueryEditor] = useState(false);
  const {
    availableModels,
    fetchingModels,
    runtimeProvider,
    currentSettingsModel,
    loadDefaults,
  } = useGenerationExecutionSettings();


  // Modal hook
  const [modal, contextHolder] = Modal.useModal();
  const mountedRef = useRef(true);
  const requestEpochRef = useRef(0);
  const restoredTaskIdRef = useRef<string | null>(null);

  useEffect(() => {
    return () => {
      mountedRef.current = false;
      requestEpochRef.current += 1;
    };
  }, []);

  const startAsyncRequest = useCallback(() => {
    requestEpochRef.current += 1;
    return requestEpochRef.current;
  }, []);

  const invalidateAsyncRequests = useCallback(() => {
    requestEpochRef.current += 1;
  }, []);

  const isAsyncRequestActive = useCallback((requestId: number) => {
    return mountedRef.current && requestEpochRef.current === requestId;
  }, []);

  const loadExecutionDefaults = useCallback(async (options?: { syncWebResearch?: boolean }, requestId?: number) => {
    try {
      const { model, webResearchEnabled } = await loadDefaults();
      if ((requestId !== undefined && !isAsyncRequestActive(requestId)) || !mountedRef.current) {
        return;
      }
      setExecutionEnableMcp(true);
      setExecutionModel(model);
      if (options?.syncWebResearch) {
        setExecutionEnableWebResearch(webResearchEnabled);
      }
    } catch (error) {
      if (requestId !== undefined && !isAsyncRequestActive(requestId)) {
        return;
      }
      console.warn('Failed to load inspiration execution settings:', error);
    }
  }, [isAsyncRequestActive, loadDefaults]);

  const mergeInspirationResearch = useCallback((response?: InspirationOptionResponse) => {
    const query = response?.research_query?.trim() || '';
    const responseAssets = Array.isArray(response?.research_assets) ? response.research_assets : [];
    if (!query && responseAssets.length === 0) {
      return;
    }

    setInspirationResearch((prev) => {
      const mergedAssets = [...prev.assets];
      const seenKeys = new Set(
        prev.assets.map((asset) => `${asset.title?.trim() || ''}::${asset.source?.trim() || ''}`.toLowerCase())
      );

      for (const asset of responseAssets) {
        const title = asset.title?.trim() || asset.source?.trim() || '';
        const source = asset.source?.trim() || '';
        const summary = asset.summary?.trim() || '';
        if (!title && !source && !summary) {
          continue;
        }
        const dedupeKey = `${title}::${source}`.toLowerCase();
        if (seenKeys.has(dedupeKey)) {
          continue;
        }
        seenKeys.add(dedupeKey);
        mergedAssets.push({
          title: title || source || '参考资料',
          source: source || undefined,
          summary: summary || undefined,
        });
      }

      return {
        query: query || prev.query,
        assets: mergedAssets.slice(0, 6),
      };
    });
  }, []);

  const beginProjectGeneration = useCallback(() => {
    const data = wizardData as WizardData;
    const fallbackResearchQuery = executionEnableWebResearch
      ? executionWebResearchQuery.trim() || inspirationResearch.query.trim() || undefined
      : undefined;
    const config: GenerationConfig = {
      title: data.title,
      description: data.description,
      theme: data.theme,
      genre: data.genre,
      narrative_perspective: data.narrative_perspective,
      target_words: 100000,
      chapter_count: 3,
      character_count: 5,
      outline_mode: data.outline_mode,
      provider: runtimeProvider,
      model: executionModel,
      enable_mcp: executionEnableMcp,
      enable_web_research: executionEnableWebResearch,
      web_research_query: fallbackResearchQuery,
      reference_research_assets: executionEnableWebResearch
        ? inspirationResearch.assets.slice(0, 6)
        : undefined,
    };
    try {
      localStorage.removeItem(CACHE_KEY);
    } catch (error) {
      console.error('清除对话缓存失败:', error);
    }
    setResumeProjectId(null);
    setGenerationConfig(config);
    setExecutionModalOpen(false);
    setCurrentStep('generating');
  }, [
    executionEnableMcp,
    executionEnableWebResearch,
    executionModel,
    executionWebResearchQuery,
    inspirationResearch.assets,
    inspirationResearch.query,
    runtimeProvider,
    wizardData,
  ]);

  // 滚动容器引用
  const messagesEndRef = useRef<HTMLDivElement>(null);
  const chatContainerRef = useRef<HTMLDivElement>(null);

  // 记录上次失败的请求参数，用于重试
  const [lastFailedRequest, setLastFailedRequest] = useState<InspirationOptionRequest | null>(null);

  // 标记是否已经加载缓存
  const [cacheLoaded, setCacheLoaded] = useState(false);

  // ==================== 缓存管理函数 ====================

  // 清除缓存
  const clearCache = useCallback(() => {
    try {
      localStorage.removeItem(CACHE_KEY);
      console.log('🗑️ 缓存已清除');
    } catch (error) {
      console.error('清除缓存失败:', error);
    }
  }, []);

  // Clear generation resume cache
  const clearGenerationResumeStorage = useCallback(() => {
    try {
      localStorage.removeItem('inspiration_project_id');
      localStorage.removeItem('inspiration_generation_data');
      localStorage.removeItem('inspiration_current_step');
      localStorage.removeItem('inspiration_task_id');
    } catch (error) {
      console.error('清除灵感模式生成恢复缓存失败:', error);
    }
  }, []);


  // 保存到缓存
  const saveToCache = useCallback(() => {
    try {
      // 只在对话阶段保存，生成阶段不保存
      if (currentStep === 'generating' || currentStep === 'complete') {
        return;
      }

      // 只有用户有输入时才保存（至少两条消息：AI问候+用户回复）
      if (messages.length <= 1) {
        return;
      }

      const cacheData: CacheData = {
        messages,
        currentStep,
        wizardData,
        initialIdea,
        selectedOptions,
        executionEnableWebResearch,
        executionWebResearchQuery,
        inspirationResearch,
        lastFailedRequest,
        timestamp: Date.now()
      };

      localStorage.setItem(CACHE_KEY, JSON.stringify(cacheData));
      console.log('💾 对话已自动保存');
    } catch (error) {
      console.error('保存缓存失败:', error);
    }
  }, [
    currentStep,
    executionEnableWebResearch,
    executionWebResearchQuery,
    initialIdea,
    inspirationResearch,
    lastFailedRequest,
    messages,
    selectedOptions,
    wizardData,
  ]);


  // Restore generation state from storage
  const restoreGenerationFromStorage = useCallback(async (requestId?: number): Promise<boolean> => {
    try {
      const storedStep = localStorage.getItem('inspiration_current_step');
      const rawConfig = localStorage.getItem('inspiration_generation_data');
      const storedTaskId = localStorage.getItem('inspiration_task_id')?.trim();
      if (storedStep !== 'generating' || !rawConfig) {
        if (storedStep || rawConfig || storedTaskId) {
          clearGenerationResumeStorage();
        }
        return false;
      }

      const parsed = JSON.parse(rawConfig) as GenerationConfig | null;
      if (!parsed || typeof parsed !== 'object') {
        clearGenerationResumeStorage();
        return false;
      }

      const storedProjectId = localStorage.getItem('inspiration_project_id');
      if (storedTaskId && !storedProjectId?.trim()) {
        const task = await backgroundTaskApi.getTaskStatus(storedTaskId);
        if (requestId !== undefined && !isAsyncRequestActive(requestId)) {
          return false;
        }
        if (task.error === 'task_missing' || task.task_type === 'unknown') {
          clearGenerationResumeStorage();
          message.warning('上一次后台任务已失效，请重新开始生成', 2);
          return false;
        }
      }

      if (requestId !== undefined && !isAsyncRequestActive(requestId)) {
        return false;
      }
      setGenerationConfig(parsed);
      setResumeProjectId(storedProjectId?.trim() ? storedProjectId : null);
      setCurrentStep('generating');
      message.success('已恢复上次的生成进度', 2);
      return true;
    } catch (error) {
      if (requestId !== undefined && !isAsyncRequestActive(requestId)) {
        return false;
      }
      console.error('恢复灵感模式生成进度失败:', error);
      clearGenerationResumeStorage();
      return false;
    }
  }, [clearGenerationResumeStorage, isAsyncRequestActive]);

  const restoreFromCache = useCallback((requestId?: number): boolean => {
    try {
      const cached = localStorage.getItem(CACHE_KEY);
      if (!cached) {
        return false;
      }

      const cacheData: CacheData = JSON.parse(cached);
      const age = Date.now() - cacheData.timestamp;

      // Cache expired
      if (age > CACHE_EXPIRY) {
        console.log('Cache expired, clearing');
        clearCache();
        return false;
      }

      // 必须有有效的对话数据
      if (!cacheData.messages || cacheData.messages.length <= 1) {
        return false;
      }

      if (requestId !== undefined && !isAsyncRequestActive(requestId)) {
        return false;
      }
      // 恢复所有状态
      setMessages(cacheData.messages);
      setCurrentStep(cacheData.currentStep);
      setWizardData(cacheData.wizardData);
      setInitialIdea(cacheData.initialIdea);
      setSelectedOptions(cacheData.selectedOptions);
      setExecutionEnableWebResearch(Boolean(cacheData.executionEnableWebResearch));
      setExecutionWebResearchQuery(cacheData.executionWebResearchQuery || '');
      setInspirationResearch(cacheData.inspirationResearch || { query: '', assets: [] });
      if (cacheData.lastFailedRequest) {
        setLastFailedRequest(cacheData.lastFailedRequest);
      }

      console.log('✅ 已恢复上次的对话进度');
      message.success('已恢复上次的对话进度', 2);
      return true;
    } catch (error) {
      if (requestId !== undefined && !isAsyncRequestActive(requestId)) {
        return false;
      }
      console.error('恢复缓存失败:', error);
      clearCache();
      return false;
    }
  }, [clearCache, isAsyncRequestActive]);

  const restoreInspirationTaskFromUrl = useCallback(async (requestId: number): Promise<boolean> => {
    const taskId = new URLSearchParams(location.search).get('task_id')?.trim();
    if (!taskId || restoredTaskIdRef.current === taskId) {
      return false;
    }

    try {
      setLoading(true);
      let task = await backgroundTaskApi.getTaskStatus(taskId);
      if (!isAsyncRequestActive(requestId)) {
        return false;
      }

      if (task.error === 'task_missing' || task.task_type === 'unknown') {
        message.warning('这个灵感后台任务已失效，请重新生成', 2);
        navigate('/inspiration', { replace: true });
        return true;
      }

      if (!task.task_type.startsWith('inspiration_')) {
        return false;
      }

      restoreFromCache(requestId);
      if (!isAsyncRequestActive(requestId)) {
        return false;
      }

      if (task.status === 'pending' || task.status === 'running') {
        message.info('灵感任务仍在后台执行，完成后会自动恢复选项', 2);
        task = await waitForBackgroundTaskCompletion<typeof task, BackgroundTaskStatus>(task, {
          pollTask: backgroundTaskApi.getTaskStatus,
          progressMessage: '正在等待灵感任务完成',
          resolveValue: (latestTask) => latestTask,
        });
        if (!isAsyncRequestActive(requestId)) {
          return false;
        }
      }

      if (task.status === 'failed' || task.status === 'cancelled') {
        const fallback = task.status === 'cancelled' ? '灵感任务已取消' : '灵感任务执行失败';
        message.error(task.error || task.message || fallback);
        navigate('/inspiration', { replace: true });
        return true;
      }

      if (task.task_type === 'inspiration_quick_generate') {
        const response = normalizeInspirationQuickGenerateResponse(task.result);
        if (!response) {
          message.warning('灵感补全任务结果格式异常，请重新生成', 2);
          navigate('/inspiration', { replace: true });
          return true;
        }

        if (response.error) {
          message.error(response.error);
          navigate('/inspiration', { replace: true });
          return true;
        }

        const restoredData: WizardData = {
          title: response.title || wizardData.title || '',
          description: response.description || wizardData.description || '',
          theme: response.theme || wizardData.theme || '',
          genre: response.genre.length > 0 ? response.genre : wizardData.genre || [],
          narrative_perspective: response.narrative_perspective || wizardData.narrative_perspective || '第三人称',
          outline_mode: wizardData.outline_mode || 'one-to-one',
        };
        setWizardData(restoredData);
        setCurrentStep('confirm');
        setMessages((prev) => {
          const content = `灵感补全任务已完成，下面按补全后的设定继续：\n\n📖 书名：${restoredData.title}\n📝 简介：${restoredData.description}\n🎯 主题：${restoredData.theme}\n🏷️ 类型：${restoredData.genre.join('、')}\n👁️ 视角：${restoredData.narrative_perspective}\n\n请选择下一步操作：`;
          if (prev.some((item) => item.type === 'ai' && item.content === content)) {
            return prev;
          }
          return [...prev, {
            type: 'ai',
            content,
            options: ['✅ 确认创建', '⚡ 智能补全并创建', '🔄 重新开始'],
          }];
        });
        restoredTaskIdRef.current = taskId;
        message.success('已恢复灵感补全结果', 2);
        navigate('/inspiration', { replace: true });
        return true;
      }

      const response = normalizeInspirationOptionResponse(task.result);
      const step = getInspirationTaskStep(task, lastFailedRequest);
      if (!response || !step) {
        message.warning('灵感任务结果缺少步骤信息，请重新生成', 2);
        navigate('/inspiration', { replace: true });
        return true;
      }

      if (response.error) {
        message.error(response.error);
        navigate('/inspiration', { replace: true });
        return true;
      }

      if (response.options.length === 0) {
        message.warning('灵感任务没有返回可选项，请重新生成', 2);
        navigate('/inspiration', { replace: true });
        return true;
      }

      mergeInspirationResearch(response);
      const aiMessage: Message = {
        type: 'ai',
        content: formatInspirationPrompt('任务已完成，请选择一个选项，或者输入你自己的：', response),
        options: response.options,
        isMultiSelect: step === 'genre',
        canRefine: true,
        step,
      };

      setMessages((prev) => {
        const alreadyRestored = prev.some((item) => (
          item.type === 'ai'
          && item.step === step
          && areSameStringArray(item.options, aiMessage.options)
        ));
        if (alreadyRestored) {
          return prev;
        }
        return [...prev, aiMessage];
      });
      setCurrentStep(step);
      setSelectedOptions([]);
      setLastFailedRequest(null);
      restoredTaskIdRef.current = taskId;
      message.success('已恢复灵感选项结果', 2);
      navigate('/inspiration', { replace: true });
      return true;
    } catch (error) {
      if (!isAsyncRequestActive(requestId)) {
        return false;
      }
      console.error('恢复灵感后台任务失败:', error);
      message.error('恢复灵感后台任务失败，请稍后重试');
      navigate('/inspiration', { replace: true });
      return true;
    } finally {
      if (isAsyncRequestActive(requestId)) {
        setLoading(false);
      }
    }
  }, [
    isAsyncRequestActive,
    lastFailedRequest,
    location.search,
    mergeInspirationResearch,
    navigate,
    restoreFromCache,
    wizardData.description,
    wizardData.genre,
    wizardData.narrative_perspective,
    wizardData.outline_mode,
    wizardData.theme,
    wizardData.title,
  ]);

  // ==================== Restore cache on mount ====================

  useEffect(() => {
    if (!cacheLoaded) {
      const requestId = startAsyncRequest();
      void (async () => {
        const restoredTask = await restoreInspirationTaskFromUrl(requestId);
        if (!isAsyncRequestActive(requestId)) {
          return;
        }
        const restoredGenerating = !restoredTask && await restoreGenerationFromStorage(requestId);
        if (!isAsyncRequestActive(requestId)) {
          return;
        }
        const restoredConversation = !restoredTask && !restoredGenerating && restoreFromCache(requestId);
        if (!restoredTask && !restoredGenerating && !restoredConversation) {
          await loadExecutionDefaults({ syncWebResearch: true }, requestId);
        }
        if (!isAsyncRequestActive(requestId)) {
          return;
        }
        setCacheLoaded(true);
      })();
    }
  }, [
    cacheLoaded,
    isAsyncRequestActive,
    loadExecutionDefaults,
    restoreFromCache,
    restoreGenerationFromStorage,
    restoreInspirationTaskFromUrl,
    startAsyncRequest,
  ]);

  useEffect(() => {
    if (!cacheLoaded) {
      return;
    }

    const taskId = new URLSearchParams(location.search).get('task_id')?.trim();
    if (!taskId || restoredTaskIdRef.current === taskId) {
      return;
    }

    const requestId = startAsyncRequest();
    void restoreInspirationTaskFromUrl(requestId);
  }, [
    cacheLoaded,
    location.search,
    restoreInspirationTaskFromUrl,
    startAsyncRequest,
  ]);

  // ==================== 自动保存：状态变化时保存 ====================

  useLayoutEffect(() => {
    if (cacheLoaded) {
      saveToCache();
    }
  }, [
    cacheLoaded,
    currentStep,
    executionEnableWebResearch,
    executionWebResearchQuery,
    initialIdea,
    lastFailedRequest,
    messages,
    saveToCache,
    selectedOptions,
    wizardData,
  ]);

  // 自动滚动到底部
  const scrollToBottom = () => {
    setTimeout(() => {
      if (chatContainerRef.current) {
        chatContainerRef.current.scrollTo({
          top: chatContainerRef.current.scrollHeight,
          behavior: 'smooth'
        });
      }
    }, 100);
  };

  // 当消息更新时自动滚动
  useEffect(() => {
    scrollToBottom();
  }, [messages]);

  const buildChatResearchFields = useCallback(() => {
    const trimmedQuery = executionWebResearchQuery.trim();
    return {
      enable_web_research: executionEnableWebResearch,
      web_research_query: executionEnableWebResearch && trimmedQuery ? trimmedQuery : undefined,
    };
  }, [executionEnableWebResearch, executionWebResearchQuery]);

  const trimmedExecutionResearchQuery = executionWebResearchQuery.trim();
  const previewResearchQuery = trimmedExecutionResearchQuery || inspirationResearch.query.trim();
  const previewResearchAssets = inspirationResearch.assets.slice(0, 3);
  const previewResearchOverflowCount = Math.max(0, inspirationResearch.assets.length - previewResearchAssets.length);
  const showEnabledResearchPreview = executionEnableWebResearch && (previewResearchQuery || previewResearchAssets.length > 0);
  const showDisabledResearchHint = !executionEnableWebResearch && inspirationResearch.assets.length > 0;

  useEffect(() => {
    if (!executionEnableWebResearch) {
      setShowResearchQueryEditor(false);
      return;
    }

    if (trimmedExecutionResearchQuery) {
      setShowResearchQueryEditor(true);
    }
  }, [executionEnableWebResearch, trimmedExecutionResearchQuery]);

  const buildInspirationRequest = useCallback(
    (step: InspirationOptionStep, context: InspirationOptionRequest['context']): InspirationOptionRequest => ({
      step,
      context,
      ...buildChatResearchFields(),
    }),
    [buildChatResearchFields]
  );

  const buildInspirationPrompt = useCallback(
    (fallbackPrompt: string, response?: InspirationOptionResponse) => {
      return formatInspirationPrompt(fallbackPrompt, response);
    },
    []
  );

  // 重试生成
  const handleRetry = async () => {
    if (!lastFailedRequest) return;

    const requestId = startAsyncRequest();
    setLoading(true);
    try {
      const response = await inspirationApi.generateOptionsInBackground(lastFailedRequest);
      if (!isAsyncRequestActive(requestId)) {
        return;
      }

      if (response.error) {
        message.error(response.error);
        return;
      }

      mergeInspirationResearch(response);

      setMessages(prev => {
        const newMessages = [...prev];
        if (newMessages[newMessages.length - 1].type === 'ai' &&
          (newMessages[newMessages.length - 1].content.includes('生成失败') ||
            newMessages[newMessages.length - 1].content.includes('出错了'))) {
          newMessages.pop();
        }
        return newMessages;
      });

      const aiMessage: Message = {
        type: 'ai',
        content: buildInspirationPrompt('请选择一个选项，或者输入你自己的：', response),
        options: response.options || [],
        isMultiSelect: lastFailedRequest.step === 'genre'
      };
      setMessages(prev => [...prev, aiMessage]);
      setLastFailedRequest(null);
    } catch (error: unknown) {
      if (!isAsyncRequestActive(requestId)) {
        return;
      }
      console.error('重试失败:', error);
      message.error('重试失败，请稍后再试');
    } finally {
      if (isAsyncRequestActive(requestId)) {
        setLoading(false);
      }
    }
  };

  // 处理用户反馈，重新生成选项
  const handleRefineOptions = async (messageIndex: number, feedback: string) => {
    if (!feedback.trim()) {
      message.warning('请输入您的反馈意见');
      return;
    }

    const targetMessage = messages[messageIndex];
    if (!targetMessage.options || !targetMessage.step) {
      return;
    }

    const requestId = startAsyncRequest();
    setRefining(true);
    setShowFeedbackInput(null);
    setFeedbackValue('');

    // 先禁用旧的选项
    setMessages(prev => {
      const newMessages = [...prev];
      if (newMessages[messageIndex]) {
        newMessages[messageIndex] = {
          ...newMessages[messageIndex],
          optionsDisabled: true,
          canRefine: false, // 同时禁用反馈功能
        };
      }
      return newMessages;
    });

    try {
      // 添加用户反馈消息
      const feedbackMessage: Message = {
        type: 'user',
        content: `💭 ${feedback}`,
      };
      setMessages(prev => [...prev, feedbackMessage]);

      const step = targetMessage.step as InspirationOptionStep;
      
      // 构建上下文
      const context: Partial<WizardData> & { initial_idea?: string } = {
        initial_idea: initialIdea,
        title: wizardData.title,
        description: wizardData.description,
        theme: wizardData.theme,
      };

      // 调用refine接口
      const response = await inspirationApi.refineOptionsInBackground({
        step,
        context,
        feedback,
        previous_options: targetMessage.options,
        ...buildChatResearchFields(),
      });
      if (!isAsyncRequestActive(requestId)) {
        return;
      }

      if (response.error) {
        message.error(response.error);
        return;
      }

      // 添加新的AI消息
      mergeInspirationResearch(response);

      const aiMessage: Message = {
        type: 'ai',
        content: buildInspirationPrompt(`根据你的反馈，我重做了这批${step === 'title' ? '书名' : step === 'description' ? '简介' : step === 'theme' ? '主题' : '类型'}选项，冲突和句式会更拉开：`, response),
        options: response.options || [],
        isMultiSelect: step === 'genre',
        canRefine: true,
        step: step,
      };
      setMessages(prev => [...prev, aiMessage]);

      message.success('已根据您的反馈重新生成选项');
    } catch (error: unknown) {
      if (!isAsyncRequestActive(requestId)) {
        return;
      }
      console.error('优化选项失败:', error);
      const errMsg = error instanceof Error ? error.message : '优化失败，请重试';
      const axiosError = error as { response?: { data?: { detail?: string } } };
      message.error(axiosError.response?.data?.detail || errMsg);
    } finally {
      if (isAsyncRequestActive(requestId)) {
        setRefining(false);
      }
    }
  };

  // 步骤顺序
  const stepOrder: Step[] = ['idea', 'title', 'description', 'theme', 'genre', 'perspective', 'outline_mode', 'confirm'];

  const handleSendMessage = async () => {
    if (!inputValue.trim()) {
      message.warning('请输入内容');
      return;
    }

    const userMessage: Message = {
      type: 'user',
      content: inputValue,
    };
    setMessages(prev => [...prev, userMessage]);

    const userInput = inputValue;
    setInputValue('');

    try {
      if (currentStep === 'idea') {
        const requestId = startAsyncRequest();
        setLoading(true);
        setInitialIdea(userInput);

        const requestData = buildInspirationRequest('title', {
          initial_idea: userInput,
          description: userInput,
        });

        const response = await inspirationApi.generateOptionsInBackground(requestData);
        if (!isAsyncRequestActive(requestId)) {
          return;
        }

        if (response.error || !response.options || response.options.length < 3) {
          const errorMessage: Message = {
            type: 'ai',
            content: response.error
              ? `生成书名时出错：${response.error}\n\n你可以选择：`
              : `生成的选项格式不正确（至少需要3个有效选项）\n\n你可以选择：`,
            options: response.options && response.options.length > 0 ? response.options : ['重新生成', '我自己输入书名']
          };
          setMessages(prev => [...prev, errorMessage]);
          setLastFailedRequest(requestData);
          return;
        }

        mergeInspirationResearch(response);

        const aiMessage: Message = {
          type: 'ai',
          content: buildInspirationPrompt('请选择一个更有记忆点的书名，或者输入你自己的：', response),
          options: response.options,
          canRefine: true,
          step: 'title'
        };
        setMessages(prev => [...prev, aiMessage]);
        setCurrentStep('title');
        setLastFailedRequest(null);
      } else {
        await handleCustomInput(userInput);
        return;
      }
    } catch (error: unknown) {
      console.error('发送消息失败:', error);
      const errMsg = error instanceof Error ? error.message : '生成失败，请重试';
      const axiosError = error as { response?: { data?: { detail?: string } } };
      message.error(axiosError.response?.data?.detail || errMsg);
    } finally {
      setLoading(false);
    }
  };

  const handleSelectOption = async (option: string, optionStep?: Step, messageIndex?: number) => {
    const activeStep = optionStep ?? currentStep;
    const openExecutionSettings = async (activeRequestId = startAsyncRequest()) => {
      await loadExecutionDefaults(undefined, activeRequestId);
      if (!isAsyncRequestActive(activeRequestId)) {
        return;
      }
      setExecutionModalOpen(true);
    };

    if (option === '重新生成' && lastFailedRequest) {
      await handleRetry();
      return;
    }

    if (option === '我自己输入书名' || option === '我自己输入') {
      message.info('请在下方输入框中输入您的内容');
      return;
    }

    // 对于多选类型，不立即禁用选项
    if (activeStep === 'genre') {
      const newSelected = selectedOptions.includes(option)
        ? selectedOptions.filter(o => o !== option)
        : [...selectedOptions, option];
      setSelectedOptions(newSelected);
      return;
    }

    // 立即禁用当前消息的选项（单选场景）
    setMessages(prev => {
      const newMessages = [...prev];
      const targetMessageIndex = typeof messageIndex === 'number'
        ? messageIndex
        : newMessages.map((m, i) => m.type === 'ai' && m.options ? i : -1).filter(i => i >= 0).pop();
      if (targetMessageIndex !== undefined && targetMessageIndex >= 0) {
        newMessages[targetMessageIndex] = {
          ...newMessages[targetMessageIndex],
          optionsDisabled: true
        };
      }
      return newMessages;
    });

    if (activeStep === 'perspective') {
      const userMessage: Message = {
        type: 'user',
        content: option,
      };
      setMessages(prev => [...prev, userMessage]);

      const updatedData = { ...wizardData, narrative_perspective: option };
      setWizardData(updatedData);

      // 询问大纲模式
      const aiMessage: Message = {
        type: 'ai',
        content: `很好！现在请选择你想要的大纲模式：

📋 一对一模式：传统模式，一个大纲对应一个章节，适合结构清晰、章节独立的小说。

📚 一对多模式：细化模式，一个大纲可以展开成多个章节，适合需要详细展开情节的小说。

请选择：`,
        options: ['📋 一对一模式', '📚 一对多模式']
      };
      setMessages(prev => [...prev, aiMessage]);
      setCurrentStep('outline_mode');
      return;
    }

    if (activeStep === 'outline_mode') {
      const userMessage: Message = {
        type: 'user',
        content: option,
      };
      setMessages(prev => [...prev, userMessage]);

      // 将选项转换为实际的模式值
      const modeValue: 'one-to-one' | 'one-to-many' =
        option === '📋 一对一模式' ? 'one-to-one' : 'one-to-many';

      const updatedData = {
        ...wizardData,
        outline_mode: modeValue,
        genre: wizardData.genre || []
      } as WizardData;
      setWizardData(updatedData);

      // 显示摘要
      const modeText = modeValue === 'one-to-one' ? '一对一模式' : '一对多模式';
      const summary = `
太棒了！你的小说设定已完成，请确认：

📖 书名：${updatedData.title}
📝 简介：${updatedData.description}
🎯 主题：${updatedData.theme}
🏷️ 类型：${updatedData.genre.join('、')}
👁️ 视角：${updatedData.narrative_perspective}
📋 大纲模式：${modeText}

请选择下一步操作：
      `.trim();

      const aiMessage: Message = {
        type: 'ai',
        content: summary,
        options: ['✅ 确认创建', '⚡ 智能补全并创建', '🔄 重新开始']
      };
      setMessages(prev => [...prev, aiMessage]);
      setCurrentStep('confirm');
      return;
    }

    if (activeStep === 'confirm') {
      if (option === '✅ 确认创建') {
        const userMessage: Message = {
          type: 'user',
          content: '确认创建',
        };
        setMessages(prev => [...prev, userMessage]);

        const aiMessage: Message = {
          type: 'ai',
          content: '好的！请先确认执行设置，然后开始创建项目。'
        };
        setMessages(prev => [...prev, aiMessage]);

        await openExecutionSettings();
        return;
      } else if (option === '⚡ 智能补全并创建') {
        const requestId = startAsyncRequest();
        const userMessage: Message = {
          type: 'user',
          content: '智能补全并创建',
        };
        setMessages(prev => [...prev, userMessage]);
        setLoading(true);

        try {
          const response = await inspirationApi.quickGenerateInBackground({
            title: wizardData.title,
            description: wizardData.description,
            theme: wizardData.theme,
            genre: wizardData.genre,
            narrative_perspective: wizardData.narrative_perspective,
          });
          if (!isAsyncRequestActive(requestId)) {
            return;
          }
          if (response.error) {
            message.error(response.error);
            return;
          }

          const completedData: WizardData = {
            title: response.title || wizardData.title || '',
            description: response.description || wizardData.description || '',
            theme: response.theme || wizardData.theme || '',
            genre: Array.isArray(response.genre)
              ? response.genre
              : response.genre
                ? [response.genre]
                : wizardData.genre || [],
            narrative_perspective: response.narrative_perspective || wizardData.narrative_perspective || '第三人称',
            outline_mode: wizardData.outline_mode || 'one-to-one',
          };
          setWizardData(completedData);

          const aiMessage: Message = {
            type: 'ai',
            content: `已完成智能补全，下面按补全后的设定创建项目：\n\n📖 书名：${completedData.title}\n📝 简介：${completedData.description}\n🎯 主题：${completedData.theme}\n🏷️ 类型：${completedData.genre.join('、')}\n👁️ 视角：${completedData.narrative_perspective}\n\n请先确认执行设置，然后开始创建项目。`,
          };
          setMessages(prev => [...prev, aiMessage]);
          await openExecutionSettings(requestId);
        } catch (error: unknown) {
          if (!isAsyncRequestActive(requestId)) {
            return;
          }
          console.error('智能补全失败:', error);
          const errMsg = error instanceof Error ? error.message : '智能补全失败，请重试';
          const axiosError = error as { response?: { data?: { detail?: string } } };
          message.error(axiosError.response?.data?.detail || errMsg);
        } finally {
          if (isAsyncRequestActive(requestId)) {
            setLoading(false);
          }
        }
        return;
      } else if (option === '🔄 重新开始') {
        handleRestart();
        return;
      }
    }

    const userMessage: Message = {
      type: 'user',
      content: option,
    };
    setMessages(prev => [...prev, userMessage]);
    const requestId = startAsyncRequest();
    setLoading(true);

    try {
      const updatedData = { ...wizardData };
      if (activeStep === 'title') {
        updatedData.title = option;
      } else if (activeStep === 'description') {
        updatedData.description = option;
      } else if (activeStep === 'theme') {
        updatedData.theme = option;
      }
      setWizardData(updatedData);

      await generateNextStep(activeStep, updatedData, requestId);
    } catch (error: unknown) {
      if (!isAsyncRequestActive(requestId)) {
        return;
      }
      console.error('选择选项失败:', error);
      const errMsg = error instanceof Error ? error.message : '生成失败，请重试';
      const axiosError = error as { response?: { data?: { detail?: string } } };
      const detail = axiosError.response?.data?.detail || errMsg;
      message.error(detail);
      setMessages(prev => [...prev, {
        type: 'ai',
        content: `继续生成下一步时出错：${detail}\n\n你可以直接输入，或点击“重新开始”后重试。`
      }]);
    } finally {
      if (isAsyncRequestActive(requestId)) {
        setLoading(false);
      }
    }
  };

  const handleCustomInput = async (input: string) => {
    const activeStep = currentStep;
    const requestId = startAsyncRequest();
    setLoading(true);
    try {
      const updatedData = { ...wizardData };

      if (activeStep === 'title') {
        updatedData.title = input;
      } else if (activeStep === 'description') {
        updatedData.description = input;
      } else if (activeStep === 'theme') {
        updatedData.theme = input;
      } else if (activeStep === 'genre') {
        updatedData.genre = [input];
      } else if (activeStep === 'perspective') {
        updatedData.narrative_perspective = input;
        setWizardData(updatedData);
        
        // 直接进入大纲模式选择
        const aiMessage: Message = {
          type: 'ai',
          content: `很好！现在请选择你想要的大纲模式：

📋 一对一模式：传统模式，一个大纲对应一个章节，适合结构清晰、章节独立的小说。

📚 一对多模式：细化模式，一个大纲可以展开成多个章节，适合需要详细展开情节的小说。

请选择：`,
          options: ['📋 一对一模式', '📚 一对多模式']
        };
        setMessages(prev => [...prev, aiMessage]);
        setCurrentStep('outline_mode');
        if (isAsyncRequestActive(requestId)) {
          setLoading(false);
        }
        return;
      } else if (activeStep === 'outline_mode') {
        // 大纲模式不支持自定义输入
        message.warning('请从选项中选择一个大纲模式');
        if (isAsyncRequestActive(requestId)) {
          setLoading(false);
        }
        return;
      }

      setWizardData(updatedData);
      await generateNextStep(activeStep, updatedData, requestId);
    } catch (error: unknown) {
      if (!isAsyncRequestActive(requestId)) {
        return;
      }
      console.error('处理自定义输入失败:', error);
      const errMsg = error instanceof Error ? error.message : '处理失败，请重试';
      const axiosError = error as { response?: { data?: { detail?: string } } };
      const detail = axiosError.response?.data?.detail || errMsg;
      message.error(detail);
      setMessages(prev => [...prev, {
        type: 'ai',
        content: `继续生成下一步时出错：${detail}\n\n你可以修改输入后再试，或重新开始当前灵感流程。`
      }]);
    } finally {
      if (isAsyncRequestActive(requestId)) {
        setLoading(false);
      }
    }
  };

  const handleConfirmGenres = async () => {
    if (selectedOptions.length === 0) {
      message.warning('请至少选择一个类型');
      return;
    }

    // 禁用类型选择的选项
    setMessages(prev => {
      const newMessages = [...prev];
      const lastAiMessageIndex = newMessages.map((m, i) => m.type === 'ai' && m.options ? i : -1).filter(i => i >= 0).pop();
      if (lastAiMessageIndex !== undefined && lastAiMessageIndex >= 0) {
        newMessages[lastAiMessageIndex] = {
          ...newMessages[lastAiMessageIndex],
          optionsDisabled: true
        };
      }
      return newMessages;
    });

    const userMessage: Message = {
      type: 'user',
      content: selectedOptions.join('、'),
    };
    setMessages(prev => [...prev, userMessage]);

    const updatedData = { ...wizardData, genre: selectedOptions };
    setWizardData(updatedData);
    setSelectedOptions([]);

    setLoading(true);
    try {
      const aiMessage: Message = {
        type: 'ai',
        content: '很好！接下来，请选择小说的叙事视角：',
        options: ['第一人称', '第三人称', '全知视角']
      };
      setMessages(prev => [...prev, aiMessage]);
      setCurrentStep('perspective');
    } finally {
      setLoading(false);
    }
  };

  const generateNextStep = async (fromStep: Step, data: Partial<WizardData>, requestId?: number) => {
    const currentIndex = stepOrder.indexOf(fromStep);
    const nextStep = stepOrder[currentIndex + 1];

    if (nextStep === 'perspective') {
      // genre 步骤完成后，进入 perspective
      const aiMessage: Message = {
        type: 'ai',
        content: '很好！接下来，请选择小说的叙事视角：',
        options: ['第一人称', '第三人称', '全知视角']
      };
      setMessages(prev => [...prev, aiMessage]);
      setCurrentStep('perspective');
    } else if (nextStep === 'description') {
      const activeRequestId = requestId ?? startAsyncRequest();
      const requestData = buildInspirationRequest('description', {
        initial_idea: initialIdea,
        title: data.title,
      });
      const response = await inspirationApi.generateOptionsInBackground(requestData);
      if (!isAsyncRequestActive(activeRequestId)) {
        return;
      }

      if (response.error || !response.options || response.options.length < 3) {
        const errorMessage: Message = {
          type: 'ai',
          content: response.error
            ? `生成简介时出错：${response.error}\n\n你可以选择：`
            : `生成的选项格式不正确（至少需要3个有效选项）\n\n你可以选择：`,
          options: response.options && response.options.length > 0 ? response.options : ['重新生成', '我自己输入']
        };
        setMessages(prev => [...prev, errorMessage]);
        setLastFailedRequest(requestData);
        return;
      }

      mergeInspirationResearch(response);

      const aiMessage: Message = {
        type: 'ai',
        content: buildInspirationPrompt('请选择一个冲突更强、开场更快的简介，或者输入你自己的：', response),
        options: response.options,
        canRefine: true,
        step: 'description'
      };
      setMessages(prev => [...prev, aiMessage]);
      setCurrentStep('description');
      setLastFailedRequest(null);

    } else if (nextStep === 'theme') {
      const activeRequestId = requestId ?? startAsyncRequest();
      const requestData = buildInspirationRequest('theme', {
        initial_idea: initialIdea,
        title: data.title,
        description: data.description,
      });
      const response = await inspirationApi.generateOptionsInBackground(requestData);
      if (!isAsyncRequestActive(activeRequestId)) {
        return;
      }

      if (response.error || !response.options || response.options.length < 3) {
        const errorMessage: Message = {
          type: 'ai',
          content: response.error
            ? `生成主题时出错：${response.error}\n\n你可以选择：`
            : `生成的选项格式不正确（至少需要3个有效选项）\n\n你可以选择：`,
          options: response.options && response.options.length > 0 ? response.options : ['重新生成', '我自己输入']
        };
        setMessages(prev => [...prev, errorMessage]);
        setLastFailedRequest(requestData);
        return;
      }

      mergeInspirationResearch(response);

      const aiMessage: Message = {
        type: 'ai',
        content: buildInspirationPrompt('请选择一个价值冲突最清晰的主题，或者输入你自己的：', response),
        options: response.options,
        canRefine: true,
        step: 'theme'
      };
      setMessages(prev => [...prev, aiMessage]);
      setCurrentStep('theme');
      setLastFailedRequest(null);

    } else if (nextStep === 'genre') {
      const activeRequestId = requestId ?? startAsyncRequest();
      const requestData = buildInspirationRequest('genre', {
        initial_idea: initialIdea,
        title: data.title,
        description: data.description,
        theme: data.theme,
      });
      const response = await inspirationApi.generateOptionsInBackground(requestData);
      if (!isAsyncRequestActive(activeRequestId)) {
        return;
      }

      if (response.error || !response.options || response.options.length < 3) {
        const errorMessage: Message = {
          type: 'ai',
          content: response.error
            ? `生成类型时出错：${response.error}\n\n你可以选择：`
            : `生成的选项格式不正确（至少需要3个有效选项）\n\n你可以选择：`,
          options: response.options && response.options.length > 0 ? response.options : ['重新生成', '我自己输入'],
          isMultiSelect: false
        };
        setMessages(prev => [...prev, errorMessage]);
        setLastFailedRequest(requestData);
        return;
      }

      mergeInspirationResearch(response);

      const aiMessage: Message = {
        type: 'ai',
        content: buildInspirationPrompt('请选择类型标签（可多选）：', response),
        options: response.options,
        isMultiSelect: true,
        canRefine: true,
        step: 'genre'
      };
      setMessages(prev => [...prev, aiMessage]);
      setCurrentStep('genre');
      setLastFailedRequest(null);
    }
  };

  const handleRestart = () => {
    // Reset conversation state
    invalidateAsyncRequests();
    clearCache();
    clearGenerationResumeStorage();

    setCurrentStep('idea');
    setResumeProjectId(null);
    setGenerationConfig(null);
    setMessages([
      {
        type: 'ai',
        content: '好的，我们重新开始。\n\n请告诉我你的灵感：主角身份、第一冲突、最怕失去的东西。',
      }
    ]);
    setWizardData({});
    setInitialIdea('');
    setSelectedOptions([]);
    setInspirationResearch({ query: '', assets: [] });
    setLoading(false);
  };

  const handleBack = () => {
    invalidateAsyncRequests();
    navigate('/projects');
  };

  // Completion callback
  const syncCompletedProject = async (projectId: string) => {
    try {
      invalidateAllProjectCollectionFreshness(projectId);
      invalidateProjectCareers(projectId);
      await syncProjectToStoreById(projectId);
    } catch (error) {
      console.error('同步灵感模式完成项目到 store 失败:', error);
    }
  };

  const handleComplete = async (projectId: string) => {
    console.log('灵感模式项目创建完成:', projectId);
    invalidateAsyncRequests();
    clearCache();
    clearGenerationResumeStorage();
    setResumeProjectId(null);
    await syncCompletedProject(projectId);
    releaseGenerationBusy();
    setCurrentStep('complete');
  };

  // Back to chat page
  const handleBackToChat = () => {
    invalidateAsyncRequests();
    clearCache();
    clearGenerationResumeStorage();
    releaseGenerationBusy();
    setCurrentStep('idea');
    setResumeProjectId(null);
    setGenerationConfig(null);
    handleRestart();
  };

  const editorialInk = token.colorText;
  const heroBackground = `linear-gradient(135deg, #171411 0%, color-mix(in srgb, #171411 68%, ${token.colorPrimary} 32%) 100%)`;
  const quietPanelBackground = `linear-gradient(180deg, color-mix(in srgb, ${token.colorBgContainer} 98%, ${token.colorBgLayout} 2%) 0%, color-mix(in srgb, ${token.colorBgContainer} 92%, ${token.colorBgLayout} 8%) 100%)`;
  const panelBorder = `1px solid color-mix(in srgb, ${token.colorPrimary} 12%, ${token.colorBorder} 88%)`;
  const outlineButtonStyle = {
    borderRadius: 999,
    background: 'color-mix(in srgb, var(--ant-color-bg-container) 14%, transparent)',
    border: '1px solid color-mix(in srgb, var(--ant-color-bg-container) 20%, transparent)',
    color: editorialInk,
    boxShadow: `0 10px 18px color-mix(in srgb, ${token.colorText} 18%, transparent)`,
    backdropFilter: 'blur(8px)',
  } as const;
  const stepTitleMap: Record<Step, string> = {
    idea: '灵感起点',
    title: '标题探索',
    description: '简介打磨',
    theme: '主题确认',
    genre: '题材组合',
    perspective: '视角选择',
    outline_mode: '结构模式',
    confirm: '最终确认',
    generating: '项目生成中',
    complete: '项目已完成',
  };
  const currentStepLabel = stepTitleMap[currentStep];
  const isConversationStage = currentStep !== 'generating' && currentStep !== 'complete';
  const heroSummaryItems = [
    { label: '当前阶段', value: currentStepLabel },
    { label: '消息数', value: `${messages.length}` },
    { label: '联网研究', value: executionEnableWebResearch ? '已开启' : '未开启' },
  ];
  const chatBubbleBaseBorder = `1px solid ${alphaColor(token.colorBorderSecondary, 0.82)}`;
  const aiBubbleBackground = `linear-gradient(180deg, ${alphaColor(token.colorBgContainer, 0.98)} 0%, ${alphaColor(token.colorFillQuaternary, 0.4)} 100%)`;
  const userBubbleBackground = `linear-gradient(135deg, ${alphaColor(token.colorPrimary, 0.92)} 0%, ${alphaColor(token.colorInfo, 0.74)} 100%)`;
  const researchPanelBackground = `linear-gradient(180deg, ${alphaColor(token.colorBgContainer, 0.98)} 0%, ${alphaColor(token.colorFillAlter, 0.58)} 100%)`;
  const controlPanelStyle = {
    borderRadius: 16,
    border: chatBubbleBaseBorder,
    background: `linear-gradient(180deg, ${alphaColor(token.colorBgElevated, 0.98)} 0%, ${alphaColor(token.colorFillQuaternary, 0.42)} 100%)`,
    padding: '12px 14px',
  } as const;
  const conversationStepOrder: Step[] = [
    'idea',
    'title',
    'description',
    'theme',
    'genre',
    'perspective',
    'outline_mode',
    'confirm',
  ];
  const currentStageIndex = conversationStepOrder.indexOf(currentStep);
  const selectedOutlineModeLabel = wizardData.outline_mode === 'one-to-many' ? '细化模式' : '传统模式';
  const selectedGenreLabel = wizardData.genre?.length ? wizardData.genre.join('、') : '待确认';
  const stepProgressItems = conversationStepOrder.map((stepKey, index) => {
    const isCurrent = currentStageIndex === index;
    const isCompleted = currentStageIndex > index;
    return {
      key: stepKey,
      label: stepTitleMap[stepKey],
      caption: index === 0 ? '输入灵感' : isCompleted ? '已沉淀' : isCurrent ? '进行中' : '待推进',
      isCurrent,
      isCompleted,
    };
  });
  const projectBriefItems = [
    {
      label: '灵感原点',
      value: clampPreviewText(initialIdea, '等待你输入第一句真正想写的冲突与代价', 56),
    },
    {
      label: '标题方向',
      value: clampPreviewText(wizardData.title, '还没有沉淀出正式书名', 32),
    },
    {
      label: '一句话简介',
      value: clampPreviewText(wizardData.description, '简介还在对话中提炼', 68),
    },
    {
      label: '主题钩子',
      value: clampPreviewText(wizardData.theme, '主题尚未确认', 42),
    },
    {
      label: '题材组合',
      value: selectedGenreLabel,
    },
    {
      label: '叙事结构',
      value: `${wizardData.narrative_perspective || '待确认'} · ${selectedOutlineModeLabel}`,
    },
  ];
  const researchStatusLabel = executionEnableWebResearch
    ? previewResearchQuery || '将自动生成检索词'
    : inspirationResearch.assets.length > 0
      ? `已缓存 ${inspirationResearch.assets.length} 条资料，待重新启用`
      : '当前不带入联网研究';

  // 渲染对话界面
  const renderChat = () => (
    <>
      <Card
        ref={chatContainerRef}
        style={{
          height: isMobile ? 'calc(100vh - 280px)' : 600,
          overflowY: 'auto',
          marginBottom: 16,
          borderRadius: isMobile ? 20 : 24,
          border: panelBorder,
          background: quietPanelBackground,
          boxShadow: `0 18px 36px color-mix(in srgb, ${token.colorTextBase} 12%, transparent)`,
          scrollBehavior: 'smooth'
        }}
        styles={{ body: { padding: isMobile ? 14 : 18 } }}
      >
        <Space direction="vertical" style={{ width: '100%' }} size="large">
          {messages.map((msg, index) => (
            <div
              key={index}
              style={{
                display: 'flex',
                justifyContent: msg.type === 'ai' ? 'flex-start' : 'flex-end',
                alignItems: 'flex-start',
                animation: 'fadeInUp 0.5s ease-out',
                animationFillMode: 'both',
                animationDelay: `${index * 0.1}s`
              }}
            >
              <div style={{
                maxWidth: isMobile ? '90%' : '82%',
                padding: isMobile ? '12px 14px' : '14px 16px',
                borderRadius: 18,
                border: msg.type === 'ai' ? chatBubbleBaseBorder : `1px solid ${alphaColor(token.colorPrimary, 0.22)}`,
                background: msg.type === 'ai' ? aiBubbleBackground : userBubbleBackground,
                color: msg.type === 'ai' ? token.colorText : token.colorWhite,
                boxShadow: msg.type === 'ai'
                  ? `0 12px 28px ${alphaColor(token.colorTextBase, 0.08)}`
                  : `0 14px 32px ${alphaColor(token.colorPrimary, 0.22)}`,
              }}>
                <Text
                  style={{
                    display: 'block',
                    marginBottom: 8,
                    fontSize: 11,
                    letterSpacing: '0.08em',
                    textTransform: 'uppercase',
                    color: msg.type === 'ai' ? token.colorTextTertiary : 'rgba(255,255,255,0.72)',
                  }}
                >
                  {msg.type === 'ai' ? 'Muse' : 'You'}
                </Text>
                <Paragraph
                  style={{
                    margin: 0,
                    color: msg.type === 'ai' ? token.colorText : token.colorWhite,
                    whiteSpace: 'pre-wrap',
                    lineHeight: 1.8,
                  }}
                >
                  {msg.content}
                </Paragraph>

                {msg.options && msg.options.length > 0 && (
                  <Space
                    direction="vertical"
                    style={{ width: '100%', marginTop: 12 }}
                    size="small"
                  >
                    <div
                      style={{
                        ...controlPanelStyle,
                        padding: '10px 12px',
                      }}
                    >
                      <Text strong style={{ display: 'block', fontSize: 12 }}>
                        {msg.isMultiSelect
                          ? `已选 ${selectedOptions.length} 项，可继续多选后统一确认`
                          : '点击任意提案即可继续推进这轮灵感对话'}
                      </Text>
                      <Text type="secondary" style={{ display: 'block', marginTop: 4, fontSize: 12 }}>
                        {msg.isMultiSelect
                          ? '这一轮更像编辑筛选题材组合，先圈住你想要的方向，再统一提交。'
                          : '每张卡片都对应一条可直接采纳的编辑建议，你也可以先改写再发送。'}
                      </Text>
                    </div>
                    {msg.options.map((option, optIndex) => (
                      <Card
                        key={optIndex}
                        hoverable={!msg.optionsDisabled}
                        size="small"
                        onClick={() => !msg.optionsDisabled && handleSelectOption(option, msg.step, index)}
                        style={{
                          cursor: msg.optionsDisabled ? 'not-allowed' : 'pointer',
                          border: msg.isMultiSelect && selectedOptions.includes(option)
                            ? `2px solid ${alphaColor(token.colorPrimary, 0.42)}`
                            : `1px solid ${alphaColor(token.colorBorderSecondary, 0.86)}`,
                          background: msg.optionsDisabled
                            ? alphaColor(token.colorBgLayout, 0.98)
                            : msg.isMultiSelect && selectedOptions.includes(option)
                              ? `linear-gradient(180deg, ${alphaColor(token.colorPrimaryBg, 0.94)} 0%, ${alphaColor(token.colorBgContainer, 0.98)} 100%)`
                              : `linear-gradient(180deg, ${alphaColor(token.colorBgContainer, 0.98)} 0%, ${alphaColor(token.colorFillQuaternary, 0.4)} 100%)`,
                          opacity: msg.optionsDisabled ? 0.6 : 1,
                          animation: 'floatIn 0.6s ease-out',
                          animationDelay: `${optIndex * 0.1}s`,
                          animationFillMode: 'both',
                          transition: 'all 0.3s cubic-bezier(0.4, 0, 0.2, 1)',
                          borderRadius: 16,
                          boxShadow: msg.isMultiSelect && selectedOptions.includes(option)
                            ? `0 10px 24px ${alphaColor(token.colorPrimary, 0.14)}`
                            : 'none',
                        }}
                        onMouseEnter={(e) => {
                          if (!msg.optionsDisabled) {
                            e.currentTarget.style.transform = 'translateY(-2px) scale(1.02)';
                            e.currentTarget.style.boxShadow = `0 12px 24px ${alphaColor(token.colorTextBase, 0.12)}`;
                          }
                        }}
                        onMouseLeave={(e) => {
                          if (!msg.optionsDisabled) {
                            e.currentTarget.style.transform = 'translateY(0) scale(1)';
                            e.currentTarget.style.boxShadow = 'none';
                          }
                        }}
                      >
                        <Text
                          style={{
                            display: 'block',
                            marginBottom: 6,
                            fontSize: 11,
                            letterSpacing: '0.08em',
                            textTransform: 'uppercase',
                            color: msg.isMultiSelect && selectedOptions.includes(option)
                              ? token.colorPrimary
                              : token.colorTextTertiary,
                          }}
                        >
                          {`Option ${String(optIndex + 1).padStart(2, '0')}`}
                        </Text>
                        <Paragraph style={{ margin: 0, color: token.colorText, lineHeight: 1.8 }}>
                          {option}
                        </Paragraph>
                        {msg.isMultiSelect && selectedOptions.includes(option) && (
                          <Text style={{ display: 'block', marginTop: 8, fontSize: 12, color: token.colorPrimary }}>
                            已加入本轮候选清单
                          </Text>
                        )}
                      </Card>
                    ))}

                    {msg.isMultiSelect && (
                      <Button
                        type="primary"
                        block
                        onClick={handleConfirmGenres}
                        disabled={selectedOptions.length === 0}
                        style={{ borderRadius: 14, height: 40 }}
                      >
                        确认选择 ({selectedOptions.length})
                      </Button>
                    )}

                    {/* 反馈优化区域 - 新增 */}
                    {msg.canRefine && !msg.optionsDisabled && !msg.isMultiSelect && (
                      <div
                        style={{
                          marginTop: 8,
                          paddingTop: 10,
                          borderTop: `1px dashed ${alphaColor(token.colorBorderSecondary, 0.9)}`,
                        }}
                      >
                        {showFeedbackInput === index ? (
                          <Space direction="vertical" style={{ ...controlPanelStyle, width: '100%' }} size="small">
                            <TextArea
                              value={feedbackValue}
                              onChange={(e) => setFeedbackValue(e.target.value)}
                              placeholder="例如：冲突再狠一点、开场更快、少讲设定多动作、结尾留更强钩子"
                              autoSize={{ minRows: 2, maxRows: 3 }}
                              disabled={refining}
                              onPressEnter={(e) => {
                                if (!e.shiftKey && feedbackValue.trim()) {
                                  e.preventDefault();
                                  handleRefineOptions(index, feedbackValue);
                                }
                              }}
                            />
                            <Space style={{ width: '100%', justifyContent: 'flex-end' }}>
                              <Button
                                size="small"
                                onClick={() => {
                                  setShowFeedbackInput(null);
                                  setFeedbackValue('');
                                }}
                                disabled={refining}
                              >
                                取消
                              </Button>
                              <Button
                                type="primary"
                                size="small"
                                onClick={() => handleRefineOptions(index, feedbackValue)}
                                loading={refining}
                                disabled={!feedbackValue.trim()}
                              >
                                重新生成
                              </Button>
                            </Space>
                          </Space>
                        ) : (
                          <Button
                            type="link"
                            size="small"
                            onClick={() => setShowFeedbackInput(index)}
                            style={{ padding: 0, height: 'auto' }}
                          >
                            不太满意？告诉我你的想法
                          </Button>
                        )}
                      </div>
                    )}
                  </Space>
                )}
              </div>
            </div>
          ))}

          {(loading || refining) && (
            <div style={{
              textAlign: 'center',
              padding: 20,
              animation: 'fadeIn 0.3s ease-in'
            }}>
              <div
                style={{
                  display: 'inline-block',
                  padding: '14px 18px',
                  borderRadius: 16,
                  border: chatBubbleBaseBorder,
                  background: aiBubbleBackground,
                }}
              >
                <Space direction="vertical" size={8} style={{ alignItems: 'flex-start' }}>
                  <Tag color="processing" style={{ margin: 0, borderRadius: 999, paddingInline: 10 }}>
                    {refining ? '灵感重排中' : '灵感编排中'}
                  </Tag>
                  <Space size={10} align="start">
                    <LoadingOutlined spin style={{ color: token.colorPrimary, fontSize: 16, marginTop: 2 }} />
                    <div style={{ textAlign: 'left' }}>
                      <Text strong style={{ display: 'block' }}>
                        {refining ? '正在根据你的反馈重写候选方向' : '正在整理下一组灵感候选'}
                      </Text>
                      <Text type="secondary" style={{ display: 'block', marginTop: 4, lineHeight: 1.7, maxWidth: 320 }}>
                        {refining
                          ? '系统会保留当前上下文，只重排这一轮不满意的表达、冲突和节奏建议。'
                          : '系统正在结合当前对话、联网研究和已选信息，生成下一步最适合你继续判断的候选。'}
                      </Text>
                    </div>
                  </Space>
                </Space>
              </div>
            </div>
          )}

          <div ref={messagesEndRef} />
        </Space>
      </Card>

      <Card
        style={{
          borderRadius: isMobile ? 20 : 24,
          border: panelBorder,
          background: researchPanelBackground,
          boxShadow: `0 18px 36px color-mix(in srgb, ${token.colorTextBase} 12%, transparent)`,
        }}
        styles={{ body: { padding: 14 } }}
      >
        <Text
          style={{
            display: 'block',
            marginBottom: 6,
            fontSize: 11,
            letterSpacing: '0.08em',
            textTransform: 'uppercase',
            color: token.colorTextTertiary,
          }}
        >
          Research Assist
        </Text>
        <Text strong style={{ display: 'block', marginBottom: 8, fontSize: 16 }}>
          联网研究与输入工作台
        </Text>
        <Text type="secondary" style={{ display: 'block', marginBottom: 14, lineHeight: 1.7 }}>
          这里负责给灵感对话补充题材资料、趋势参考和自定义检索词，同时也是你继续推进对话的主输入区。
        </Text>
        <Space direction="vertical" size={12} style={{ width: '100%', marginBottom: 12 }}>
          <div style={{ ...controlPanelStyle, display: 'flex', justifyContent: 'space-between', alignItems: 'center', gap: 12, flexWrap: 'wrap' }}>
            <div>
              <Text strong>联网搜索增强</Text>
              <Text type="secondary" style={{ display: 'block', fontSize: 12, marginTop: 4 }}>
                为灵感生成补充实时趋势、题材资料与风格参考；留空时会自动按当前创意生成检索词。
              </Text>
            </div>
            <Switch
              checked={executionEnableWebResearch}
              onChange={setExecutionEnableWebResearch}
              checkedChildren="开启"
              unCheckedChildren="关闭"
            />
          </div>
          {executionEnableWebResearch && (
            <>
              {showResearchQueryEditor ? (
                <Space direction="vertical" size={8} style={{ ...controlPanelStyle, width: '100%' }}>
                  <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', gap: 8, flexWrap: 'wrap' }}>
                    <Text strong style={{ fontSize: 12 }}>自定义检索词（可选）</Text>
                    <Button
                      type="link"
                      size="small"
                      style={{ paddingInline: 0, height: 'auto' }}
                      onClick={() => {
                        if (!trimmedExecutionResearchQuery) {
                          setShowResearchQueryEditor(false);
                        }
                      }}
                      disabled={Boolean(trimmedExecutionResearchQuery)}
                    >
                      收起
                    </Button>
                  </div>
                  <Input
                    data-testid="inspiration-research-query-input"
                    value={executionWebResearchQuery}
                    onChange={(e) => setExecutionWebResearchQuery(e.target.value)}
                    placeholder="例如：2026 女频悬疑爆款趋势、法医职业细节、时间循环题材读者偏好"
                    maxLength={400}
                    showCount
                    allowClear
                  />
                </Space>
              ) : (
                <Button
                  data-testid="inspiration-research-query-toggle"
                  type="dashed"
                  block
                  onClick={() => setShowResearchQueryEditor(true)}
                  style={{ borderRadius: 14, height: 42 }}
                >
                  补充自定义检索词（可选）
                </Button>
              )}
              {showEnabledResearchPreview && (
                <div
                  data-testid="inspiration-research-preview"
                  style={{
                    borderRadius: 16,
                    padding: '12px 14px',
                    background: `linear-gradient(180deg, ${alphaColor(token.colorInfoBg, 0.82)} 0%, ${alphaColor(token.colorBgContainer, 0.98)} 100%)`,
                    border: `1px solid ${alphaColor(token.colorInfo, 0.14)}`,
                  }}
                >
                  <Text strong style={{ display: 'block', marginBottom: 6 }}>
                    将带入生成链路的研究上下文
                  </Text>
                  {previewResearchQuery && (
                    <Text type="secondary" style={{ display: 'block', marginBottom: previewResearchAssets.length > 0 ? 8 : 0 }}>
                      检索词：{previewResearchQuery}
                    </Text>
                  )}
                  {previewResearchAssets.length > 0 && (
                    <Space direction="vertical" size={4} style={{ width: '100%' }}>
                      {previewResearchAssets.map((asset, index) => (
                        <Text key={`${asset.title}-${asset.source || index}`} style={{ display: 'block', fontSize: 12 }}>
                          - {asset.title}{asset.source ? ` · ${asset.source}` : ''}
                        </Text>
                      ))}
                      {previewResearchOverflowCount > 0 && (
                        <Text type="secondary" style={{ fontSize: 12 }}>
                          还有 {previewResearchOverflowCount} 条已缓存资料会一并带入。
                        </Text>
                      )}
                    </Space>
                  )}
                </div>
              )}
            </>
          )}
          {showDisabledResearchHint && (
            <Text
              data-testid="inspiration-research-preview-disabled"
              type="secondary"
              style={{ fontSize: 12, ...controlPanelStyle }}
            >
              当前已缓存 {inspirationResearch.assets.length} 条灵感资料；重新开启后会自动带入创建流程。
            </Text>
          )}
        </Space>
        <div style={{ ...controlPanelStyle, padding: 12 }}>
          <Space.Compact style={{ width: '100%' }}>
            <TextArea
              value={inputValue}
              onChange={(e) => setInputValue(e.target.value)}
              placeholder={
                currentStep === 'idea'
                  ? '例如：女法医穿回案发前一天，必须在24小时内洗清自己杀人嫌疑...'
                  : '输入自定义内容，或点击上方选项卡片...'
              }
              autoSize={{ minRows: 2, maxRows: 4 }}
              onPressEnter={(e) => {
                if (!e.shiftKey) {
                  e.preventDefault();
                  handleSendMessage();
                }
              }}
              disabled={loading}
            />
            <Button
              type="primary"
              icon={<SendOutlined />}
              onClick={handleSendMessage}
              loading={loading}
              style={{ height: 'auto', minWidth: 92, borderRadius: 12 }}
            >
              发送
            </Button>
          </Space.Compact>
          <Text type="secondary" style={{ fontSize: 12, marginTop: 8, display: 'block' }}>
            提示：按 Enter 发送，Shift+Enter 换行
          </Text>
        </div>
      </Card>
    </>
  );

  return (
    <div style={{
      minHeight: '100dvh',
      background: `linear-gradient(180deg, ${token.colorBgLayout} 0%, ${token.colorBgBase} 100%)`,
    }}>
      {contextHolder}
      <style>
        {`
          @keyframes fadeInUp {
            from {
              opacity: 0;
              transform: translateY(20px);
            }
            to {
              opacity: 1;
              transform: translateY(0);
            }
          }
          
          @keyframes floatIn {
            0% {
              opacity: 0;
              transform: translateY(10px) scale(0.95);
            }
            60% {
              transform: translateY(-5px) scale(1.02);
            }
            100% {
              opacity: 1;
              transform: translateY(0) scale(1);
            }
          }
          
          @keyframes fadeIn {
            from {
              opacity: 0;
            }
            to {
              opacity: 1;
            }
          }
        `}
      </style>

      <div style={{
        maxWidth: 1080,
        margin: '0 auto',
        padding: isMobile ? '18px 14px 32px' : '24px 24px 40px',
      }}>
        <Card
          variant="borderless"
          style={{
            background: heroBackground,
            borderRadius: isMobile ? 22 : 30,
            border: `1px solid color-mix(in srgb, ${token.colorBgContainer} 12%, transparent)`,
            boxShadow: `0 26px 52px color-mix(in srgb, ${token.colorText} 20%, transparent)`,
            overflow: 'hidden',
            position: 'relative',
            marginBottom: 18,
          }}
          styles={{ body: { padding: isMobile ? 18 : 24 } }}
        >
          <div style={{ position: 'absolute', top: -56, right: -40, width: 180, height: 180, borderRadius: '50%', background: 'rgba(255,255,255,0.08)', pointerEvents: 'none' }} />
          <div style={{ position: 'absolute', bottom: -30, left: '24%', width: 110, height: 110, borderRadius: '50%', background: 'rgba(255,255,255,0.05)', pointerEvents: 'none' }} />
          <div style={{ position: 'relative', zIndex: 1 }}>
            <div style={{ display: 'flex', justifyContent: 'space-between', gap: 12, alignItems: 'flex-start', flexWrap: 'wrap' }}>
              <div style={{ flex: '1 1 520px' }}>
                <Text style={{ color: 'rgba(255,255,255,0.72)', fontSize: 11, letterSpacing: '0.18em', textTransform: 'uppercase' }}>
                  Creative Entry
                </Text>
                <Title
                  level={isMobile ? 3 : 2}
                  style={{
                    margin: '8px 0 10px',
                    color: editorialInk,
                    fontFamily: designDisplayFont,
                    letterSpacing: '-0.03em',
                  }}
                >
                  灵感模式
                </Title>
                <Paragraph style={{ margin: 0, color: 'rgba(255,255,255,0.82)', fontSize: isMobile ? 13 : 15, lineHeight: 1.8, maxWidth: 680 }}>
                  这里是项目创作的第一道入口。你可以像和编辑沟通一样，把主角、冲突、代价与题材气质一步步聊出来，再把结果无缝推进到完整项目生成。
                </Paragraph>
              </div>
              <div style={{ flex: '1 1 280px', minWidth: isMobile ? '100%' : 280 }}>
                <Space direction="vertical" size={12} style={{ width: '100%' }}>
                  {heroSummaryItems.map((item) => (
                    <div
                      key={item.label}
                      style={{
                        display: 'flex',
                        justifyContent: 'space-between',
                        alignItems: 'center',
                        gap: 12,
                        borderRadius: 18,
                        padding: '12px 14px',
                        background: 'rgba(255,255,255,0.08)',
                        border: '1px solid rgba(255,255,255,0.1)',
                        backdropFilter: 'blur(10px)',
                      }}
                    >
                      <Text style={{ color: 'rgba(255,255,255,0.72)', fontSize: 12 }}>{item.label}</Text>
                      <Text style={{ color: editorialInk, fontWeight: 600 }}>{item.value}</Text>
                    </div>
                  ))}
                </Space>
              </div>
            </div>
            <Space wrap size={[10, 10]} style={{ marginTop: 20 }}>
              <Button
                icon={<ArrowLeftOutlined />}
                onClick={handleBack}
                size={isMobile ? 'middle' : 'large'}
                disabled={shouldDisableNavigation(currentStep === 'generating')}
                style={outlineButtonStyle}
              >
                {isMobile ? '返回' : '返回首页'}
              </Button>
              {isConversationStage && currentStep !== 'idea' ? (
                <Button
                  icon={<ReloadOutlined />}
                  onClick={() => {
                    modal.confirm({
                      title: '确认重新开始',
                      content: '确定要重新开始吗？当前的对话进度将会丢失。',
                      okText: '确认',
                      cancelText: '取消',
                      centered: true,
                      okButtonProps: { danger: true },
                      onOk: () => {
                        handleRestart();
                      },
                    });
                  }}
                  size={isMobile ? 'middle' : 'large'}
                  style={outlineButtonStyle}
                >
                  {isMobile ? '重新' : '重新开始'}
                </Button>
              ) : null}
            </Space>
          </div>
        </Card>

        {isConversationStage && (
          <div
            style={{
              display: 'grid',
              gridTemplateColumns: isMobile ? '1fr' : 'minmax(0, 1.3fr) minmax(320px, 0.95fr)',
              gap: 14,
              marginBottom: 16,
            }}
          >
            <Card
              style={{
                borderRadius: isMobile ? 20 : 24,
                border: panelBorder,
                background: quietPanelBackground,
                boxShadow: `0 18px 36px color-mix(in srgb, ${token.colorTextBase} 10%, transparent)`,
              }}
              styles={{ body: { padding: isMobile ? 14 : 18 } }}
            >
              <Text
                style={{
                  display: 'block',
                  marginBottom: 6,
                  fontSize: 11,
                  letterSpacing: '0.08em',
                  textTransform: 'uppercase',
                  color: token.colorTextTertiary,
                }}
              >
                Creative Route
              </Text>
              <Text strong style={{ display: 'block', marginBottom: 8, fontSize: 16 }}>
                创作路线图
              </Text>
              <Text type="secondary" style={{ display: 'block', marginBottom: 14, lineHeight: 1.7 }}>
                先把冲突讲明，再逐步收束成标题、简介、主题、题材与结构，让灵感像编辑工作台一样一格格落地。
              </Text>
              <div style={{ display: 'flex', flexWrap: 'wrap', gap: 10 }}>
                {stepProgressItems.map((item, index) => (
                  <div
                    key={item.key}
                    style={{
                      flex: isMobile ? '1 1 calc(50% - 8px)' : '1 1 150px',
                      minWidth: isMobile ? 0 : 150,
                      borderRadius: 18,
                      padding: '12px 14px',
                      border: item.isCurrent
                        ? `1px solid ${alphaColor(token.colorPrimary, 0.24)}`
                        : `1px solid ${alphaColor(token.colorBorderSecondary, 0.84)}`,
                      background: item.isCurrent
                        ? `linear-gradient(180deg, ${alphaColor(token.colorPrimaryBg, 0.92)} 0%, ${alphaColor(token.colorBgContainer, 0.98)} 100%)`
                        : item.isCompleted
                          ? `linear-gradient(180deg, ${alphaColor(token.colorSuccessBg, 0.9)} 0%, ${alphaColor(token.colorBgContainer, 0.98)} 100%)`
                          : `linear-gradient(180deg, ${alphaColor(token.colorBgContainer, 0.98)} 0%, ${alphaColor(token.colorFillQuaternary, 0.42)} 100%)`,
                      boxShadow: item.isCurrent
                        ? `0 10px 24px ${alphaColor(token.colorPrimary, 0.12)}`
                        : 'none',
                    }}
                  >
                    <Text
                      style={{
                        display: 'block',
                        marginBottom: 6,
                        fontSize: 11,
                        letterSpacing: '0.08em',
                        textTransform: 'uppercase',
                        color: item.isCurrent
                          ? token.colorPrimary
                          : item.isCompleted
                            ? token.colorSuccess
                            : token.colorTextTertiary,
                      }}
                    >
                      {`Step ${String(index + 1).padStart(2, '0')}`}
                    </Text>
                    <Text strong style={{ display: 'block', marginBottom: 4 }}>
                      {item.label}
                    </Text>
                    <Text type="secondary" style={{ fontSize: 12 }}>
                      {item.caption}
                    </Text>
                  </div>
                ))}
              </div>
            </Card>

            <Card
              style={{
                borderRadius: isMobile ? 20 : 24,
                border: panelBorder,
                background: researchPanelBackground,
                boxShadow: `0 18px 36px color-mix(in srgb, ${token.colorTextBase} 10%, transparent)`,
              }}
              styles={{ body: { padding: isMobile ? 14 : 18 } }}
            >
              <Text
                style={{
                  display: 'block',
                  marginBottom: 6,
                  fontSize: 11,
                  letterSpacing: '0.08em',
                  textTransform: 'uppercase',
                  color: token.colorTextTertiary,
                }}
              >
                Project Brief
              </Text>
              <Text strong style={{ display: 'block', marginBottom: 8, fontSize: 16 }}>
                当前项目摘要
              </Text>
              <Text type="secondary" style={{ display: 'block', marginBottom: 14, lineHeight: 1.7 }}>
                这里汇总对话已经沉淀出的核心设定，方便你一边聊天，一边看到项目骨架是怎样逐步成形的。
              </Text>
              <Space direction="vertical" size={10} style={{ width: '100%' }}>
                {projectBriefItems.map((item) => (
                  <div
                    key={item.label}
                    style={{
                      borderRadius: 16,
                      padding: '12px 14px',
                      border: `1px solid ${alphaColor(token.colorBorderSecondary, 0.84)}`,
                      background: `linear-gradient(180deg, ${alphaColor(token.colorBgContainer, 0.98)} 0%, ${alphaColor(token.colorFillQuaternary, 0.34)} 100%)`,
                    }}
                  >
                    <Text style={{ display: 'block', marginBottom: 4, fontSize: 12, color: token.colorTextTertiary }}>
                      {item.label}
                    </Text>
                    <Text strong style={{ lineHeight: 1.7 }}>
                      {item.value}
                    </Text>
                  </div>
                ))}
                <div style={{ ...controlPanelStyle, padding: '12px 14px' }}>
                  <Text strong style={{ display: 'block', marginBottom: 4 }}>
                    研究带入状态
                  </Text>
                  <Text type="secondary" style={{ display: 'block', lineHeight: 1.7 }}>
                    {researchStatusLabel}
                  </Text>
                </div>
              </Space>
            </Card>
          </div>
        )}

        {(currentStep === 'idea' || currentStep === 'title' || currentStep === 'description' ||
          currentStep === 'theme' || currentStep === 'genre' || currentStep === 'perspective' ||
          currentStep === 'outline_mode' || currentStep === 'confirm') && renderChat()}
        {(currentStep === 'generating' || currentStep === 'complete') && generationConfig && (
          <AIProjectGenerator
            config={generationConfig}
            storagePrefix="inspiration"
            onComplete={handleComplete}
            onBack={handleBackToChat}
            onBusyChange={setIsGenerationBusy}
            backButtonText="返回灵感首页"
            isMobile={isMobile}
            resumeProjectId={resumeProjectId ?? undefined}
          />
        )}
      </div>

      <Modal
        title={(
          <div>
            <Text style={{ display: 'block', fontSize: 11, letterSpacing: '0.08em', textTransform: 'uppercase', color: token.colorTextTertiary, marginBottom: 4 }}>
              Launch Review
            </Text>
            <Text strong style={{ display: 'block', fontSize: 18 }}>
              执行设置
            </Text>
            <Text type="secondary" style={{ display: 'block', marginTop: 4, lineHeight: 1.7 }}>
              在正式开始生成前，最后确认本次项目骨架的执行方式、模型策略和联网研究状态。
            </Text>
          </div>
        )}
        open={executionModalOpen}
        onCancel={() => setExecutionModalOpen(false)}
        onOk={beginProjectGeneration}
        okText="开始生成"
        cancelText="取消"
        destroyOnHidden
        width={760}
        styles={{
          content: {
            borderRadius: 24,
            border: `1px solid ${alphaColor(token.colorBorderSecondary, 0.88)}`,
            background: `linear-gradient(180deg, ${alphaColor(token.colorBgContainer, 0.98)} 0%, ${alphaColor(token.colorFillQuaternary, 0.52)} 100%)`,
            boxShadow: `0 28px 56px ${alphaColor(token.colorTextBase, 0.14)}`,
          },
          header: {
            paddingBottom: 0,
            background: 'transparent',
            borderBottom: 'none',
          },
          body: {
            paddingTop: 16,
          },
          footer: {
            borderTop: 'none',
            paddingTop: 8,
          },
        }}
      >
        <Card
          size="small"
          style={{
            marginBottom: 14,
            borderRadius: 18,
            border: `1px solid ${alphaColor(token.colorBorderSecondary, 0.85)}`,
            background: `linear-gradient(135deg, ${alphaColor(token.colorPrimaryBg, 0.9)} 0%, ${alphaColor(token.colorBgContainer, 0.98)} 100%)`,
          }}
          styles={{ body: { padding: 14 } }}
        >
          <Text style={{ display: 'block', fontSize: 11, letterSpacing: '0.08em', textTransform: 'uppercase', color: token.colorTextTertiary, marginBottom: 6 }}>
            Project Snapshot
          </Text>
          <Text strong style={{ display: 'block', marginBottom: 8 }}>
            本次将生成的项目摘要
          </Text>
          <Text type="secondary" style={{ display: 'block', lineHeight: 1.7, marginBottom: 12 }}>
            这里展示的是灵感对话已经收束出的核心设定，方便你在点击“开始生成”前做最后核对。
          </Text>
          <Space wrap size={[8, 8]}>
            <Tag color="blue" style={{ borderRadius: 999 }}>书名：{wizardData.title || '待确认'}</Tag>
            <Tag color="purple" style={{ borderRadius: 999 }}>题材：{selectedGenreLabel}</Tag>
            <Tag color="gold" style={{ borderRadius: 999 }}>视角：{wizardData.narrative_perspective || '待确认'}</Tag>
            <Tag color="green" style={{ borderRadius: 999 }}>结构：{selectedOutlineModeLabel}</Tag>
          </Space>
        </Card>

        <GenerationExecutionSettingsPanel
          card={false}
          enableMcp={executionEnableMcp}
          onEnableMcpChange={setExecutionEnableMcp}
          model={executionModel}
          onModelChange={setExecutionModel}
          fetchingModels={fetchingModels}
          availableModels={availableModels}
          runtimeProvider={runtimeProvider}
          currentSettingsModel={currentSettingsModel}
        />

        <Card
          size="small"
          style={{
            marginTop: 14,
            borderRadius: 18,
            border: `1px solid ${alphaColor(token.colorBorderSecondary, 0.85)}`,
            background: `linear-gradient(180deg, ${alphaColor(token.colorBgElevated, 0.98)} 0%, ${alphaColor(token.colorFillAlter, 0.5)} 100%)`,
          }}
          styles={{ body: { padding: 14 } }}
        >
          <Text style={{ display: 'block', fontSize: 11, letterSpacing: '0.08em', textTransform: 'uppercase', color: token.colorTextTertiary, marginBottom: 6 }}>
            Research Context
          </Text>
          <Text strong style={{ display: 'block', marginBottom: 8 }}>
            联网研究与灵感资料
          </Text>
          <Text type="secondary" style={{ display: 'block', lineHeight: 1.7, marginBottom: 12 }}>
            如果已经在灵感阶段积累了题材资料，这里会告诉你哪些研究上下文会被带进正式的项目创建流程。
          </Text>
          <Alert
            type={executionEnableWebResearch ? 'success' : 'info'}
            showIcon
            style={{
              marginBottom: 12,
              borderRadius: 14,
              border: `1px solid ${alphaColor(executionEnableWebResearch ? token.colorSuccess : token.colorInfo, 0.12)}`,
              background: executionEnableWebResearch
                ? `linear-gradient(135deg, ${alphaColor(token.colorSuccessBg, 0.88)} 0%, ${alphaColor(token.colorBgContainer, 0.98)} 100%)`
                : `linear-gradient(135deg, ${alphaColor(token.colorInfoBg, 0.88)} 0%, ${alphaColor(token.colorBgContainer, 0.98)} 100%)`,
            }}
            message={executionEnableWebResearch
              ? '已开启联网研究，生成时会携带检索上下文与灵感资料。'
              : '当前未开启联网研究；本次将只依据对话中沉淀的设定生成项目。'}
          />
          <Space wrap size={[8, 8]} style={{ marginBottom: 12 }}>
            <Tag color={executionEnableWebResearch ? 'green' : 'default'} style={{ borderRadius: 999 }}>
              联网研究：{executionEnableWebResearch ? '开启' : '关闭'}
            </Tag>
            <Tag color="blue" style={{ borderRadius: 999 }}>
              检索词：{previewResearchQuery || '未设置'}
            </Tag>
            <Tag color="purple" style={{ borderRadius: 999 }}>
              已缓存资料：{inspirationResearch.assets.length} 条
            </Tag>
          </Space>
          {previewResearchAssets.length > 0 ? (
            <Space direction="vertical" size={4} style={{ width: '100%' }}>
              {previewResearchAssets.map((asset, index) => (
                <Text key={`${asset.title}-${asset.source || index}`} style={{ display: 'block', fontSize: 12 }}>
                  - {asset.title}{asset.source ? ` · ${asset.source}` : ''}
                </Text>
              ))}
              {previewResearchOverflowCount > 0 ? (
                <Text type="secondary" style={{ fontSize: 12 }}>
                  还有 {previewResearchOverflowCount} 条资料会在生成时一并带入。
                </Text>
              ) : null}
            </Space>
          ) : (
            <Text type="secondary" style={{ fontSize: 12 }}>
              当前没有额外研究资料预览，系统会直接使用现有灵感对话内容。
            </Text>
          )}
        </Card>
      </Modal>
    </div>
  );
};

export default Inspiration;
