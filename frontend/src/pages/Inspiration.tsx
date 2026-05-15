import React, { useState, useEffect, useLayoutEffect, useRef, useCallback } from 'react';
import { useBusyNavigationGuard } from '../hooks/useBusyNavigationGuard';
import { useNavigate } from 'react-router-dom';
import { Card, Input, Button, Space, Typography, message, Spin, Modal, Switch, theme } from 'antd';
import { SendOutlined, ArrowLeftOutlined, ReloadOutlined } from '@ant-design/icons';
import { backgroundTaskApi, inspirationApi } from '../services/modularApi';
import { AIProjectGenerator, type GenerationConfig } from '../components/AIProjectGenerator';
import {
  GenerationExecutionSettingsPanel,
  useGenerationExecutionSettings,
} from '../components/GenerationExecutionSettings';
import { syncProjectToStoreById } from '../store/hooks';
import { invalidateAllProjectCollectionFreshness } from '../store/projectCollectionRefresh';
import { invalidateProjectCareers } from '../services/projectCareers';

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

type InspirationOptionResponse = {
  prompt?: string;
  options: string[];
  error?: string;
  research_query?: string;
  research_assets?: InspirationResearchAsset[];
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

const Inspiration: React.FC = () => {
  const navigate = useNavigate();
  const [currentStep, setCurrentStep] = useState<Step>('idea');
  const {
    setBusy: setIsGenerationBusy,
    releaseBusy: releaseGenerationBusy,
    shouldDisableNavigation,
  } = useBusyNavigationGuard();
  const [isMobile, setIsMobile] = useState(window.innerWidth <= 768);
  const { token } = theme.useToken();

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

  // ==================== Restore cache on mount ====================

  useEffect(() => {
    if (!cacheLoaded) {
      const requestId = startAsyncRequest();
      void (async () => {
        const restoredGenerating = await restoreGenerationFromStorage(requestId);
        if (!isAsyncRequestActive(requestId)) {
          return;
        }
        const restoredConversation = !restoredGenerating && restoreFromCache(requestId);
        if (!restoredGenerating && !restoredConversation) {
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
    },
    []
  );

  // 重试生成
  const handleRetry = async () => {
    if (!lastFailedRequest) return;

    const requestId = startAsyncRequest();
    setLoading(true);
    try {
      const response = await inspirationApi.generateOptions(lastFailedRequest);
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
      const response = await inspirationApi.refineOptions({
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

        const response = await inspirationApi.generateOptions(requestData);
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
        options: ['✅ 确认创建', '🔄 重新开始']
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

        // 先加载执行设置，再进入生成阶段
        const requestId = startAsyncRequest();
        await loadExecutionDefaults(undefined, requestId);
        if (!isAsyncRequestActive(requestId)) {
          return;
        }
        setExecutionModalOpen(true);
        return;
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
      const response = await inspirationApi.generateOptions(requestData);
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
      const response = await inspirationApi.generateOptions(requestData);
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
      const response = await inspirationApi.generateOptions(requestData);
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

  // 渲染对话界面
  const renderChat = () => (
    <>
      <Card
        ref={chatContainerRef}
        style={{
          height: isMobile ? 'calc(100vh - 280px)' : 600,
          overflowY: 'auto',
          marginBottom: 16,
          boxShadow: `0 8px 24px color-mix(in srgb, ${token.colorTextBase} 20%, transparent)`,
          scrollBehavior: 'smooth'
        }}
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
                maxWidth: '80%',
                padding: '12px 16px',
                borderRadius: 12,
                background: msg.type === 'ai' ? token.colorBgContainer : token.colorPrimary,
                color: msg.type === 'ai' ? token.colorText : token.colorWhite,
                boxShadow: msg.type === 'ai'
                  ? `0 2px 10px color-mix(in srgb, ${token.colorTextBase} 12%, transparent)`
                  : `0 4px 14px color-mix(in srgb, ${token.colorPrimary} 30%, transparent)`,
              }}>
                <Paragraph
                  style={{
                    margin: 0,
                    color: msg.type === 'ai' ? token.colorText : token.colorWhite,
                    whiteSpace: 'pre-wrap'
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
                    {msg.options.map((option, optIndex) => (
                      <Card
                        key={optIndex}
                        hoverable={!msg.optionsDisabled}
                        size="small"
                        onClick={() => !msg.optionsDisabled && handleSelectOption(option, msg.step, index)}
                        style={{
                          cursor: msg.optionsDisabled ? 'not-allowed' : 'pointer',
                          border: msg.isMultiSelect && selectedOptions.includes(option)
                            ? `2px solid ${token.colorPrimary}`
                            : `1px solid ${token.colorBorder}`,
                          background: msg.optionsDisabled
                            ? token.colorBgLayout
                            : msg.isMultiSelect && selectedOptions.includes(option)
                              ? token.colorPrimaryBg
                              : token.colorBgContainer,
                          opacity: msg.optionsDisabled ? 0.6 : 1,
                          animation: 'floatIn 0.6s ease-out',
                          animationDelay: `${optIndex * 0.1}s`,
                          animationFillMode: 'both',
                          transition: 'all 0.3s cubic-bezier(0.4, 0, 0.2, 1)',
                        }}
                        onMouseEnter={(e) => {
                          if (!msg.optionsDisabled) {
                            e.currentTarget.style.transform = 'translateY(-2px) scale(1.02)';
                            e.currentTarget.style.boxShadow = `0 8px 22px color-mix(in srgb, ${token.colorTextBase} 14%, transparent)`;
                          }
                        }}
                        onMouseLeave={(e) => {
                          if (!msg.optionsDisabled) {
                            e.currentTarget.style.transform = 'translateY(0) scale(1)';
                            e.currentTarget.style.boxShadow = 'none';
                          }
                        }}
                      >
                        {option}
                      </Card>
                    ))}

                    {msg.isMultiSelect && (
                      <Button
                        type="primary"
                        block
                        onClick={handleConfirmGenres}
                        disabled={selectedOptions.length === 0}
                      >
                        确认选择 ({selectedOptions.length})
                      </Button>
                    )}

                    {/* 反馈优化区域 - 新增 */}
                    {msg.canRefine && !msg.optionsDisabled && !msg.isMultiSelect && (
                      <div style={{ marginTop: 8, paddingTop: 8, borderTop: `1px dashed ${token.colorBorder}` }}>
                        {showFeedbackInput === index ? (
                          <Space direction="vertical" style={{ width: '100%' }} size="small">
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
                            💡 不太满意？告诉我你的想法
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
              <Spin tip={refining ? "正在根据您的反馈重新生成..." : "正在思考中..."} />
            </div>
          )}

          <div ref={messagesEndRef} />
        </Space>
      </Card>

      <Card
        style={{ boxShadow: `0 4px 12px color-mix(in srgb, ${token.colorTextBase} 14%, transparent)` }}
        styles={{ body: { padding: 12 } }}
      >
        <Space direction="vertical" size={12} style={{ width: '100%', marginBottom: 12 }}>
          <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', gap: 12, flexWrap: 'wrap' }}>
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
                <Space direction="vertical" size={8} style={{ width: '100%' }}>
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
                >
                  补充自定义检索词（可选）
                </Button>
              )}
              {showEnabledResearchPreview && (
                <div
                  data-testid="inspiration-research-preview"
                  style={{
                    borderRadius: 10,
                    padding: '10px 12px',
                    background: `color-mix(in srgb, ${token.colorInfoBg} 75%, ${token.colorBgContainer} 25%)`,
                    border: `1px solid color-mix(in srgb, ${token.colorInfoBorder} 72%, transparent)`,
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
              style={{ fontSize: 12 }}
            >
              当前已缓存 {inspirationResearch.assets.length} 条灵感资料；重新开启后会自动带入创建流程。
            </Text>
          )}
        </Space>
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
            style={{ height: 'auto' }}
          >
            发送
          </Button>
        </Space.Compact>
        <Text type="secondary" style={{ fontSize: 12, marginTop: 8, display: 'block' }}>
          💡 提示：按 Enter 发送，Shift+Enter 换行
        </Text>
      </Card>
    </>
  );

  return (
    <div style={{
      minHeight: '100dvh',
      background: token.colorBgBase,
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

      {/* 顶部标题栏 - 固定不滚动 */}
      <div style={{
        position: 'sticky',
        top: 0,
        zIndex: 100,
        background: token.colorPrimary,
        boxShadow: `0 6px 20px color-mix(in srgb, ${token.colorPrimary} 30%, transparent)`,
      }}>
        <div style={{
          maxWidth: 1200,
          margin: '0 auto',
          display: 'flex',
          alignItems: 'center',
          justifyContent: 'space-between',
          padding: isMobile ? '12px 16px' : '16px 24px',
        }}>
          <Button
            icon={<ArrowLeftOutlined />}
            onClick={handleBack}
            size={isMobile ? 'middle' : 'large'}
            disabled={shouldDisableNavigation(currentStep === 'generating')}
            style={{
              background: `color-mix(in srgb, ${token.colorWhite} 20%, transparent)`,
              borderColor: `color-mix(in srgb, ${token.colorWhite} 30%, transparent)`,
              color: token.colorWhite,
            }}
          >
            {isMobile ? '返回' : '返回首页'}
          </Button>

          <div style={{ textAlign: 'center' }}>
            <Title
              level={isMobile ? 4 : 2}
              style={{
                margin: 0,
                color: token.colorWhite,
                textShadow: '0 2px 4px color-mix(in srgb, var(--ant-color-black) 18%, transparent)',
                lineHeight: 1.2
              }}
            >
              ✨ 灵感模式
            </Title>
          </div>

          {/* 重新开始按钮 - 只在对话进行中显示 */}
          {currentStep !== 'idea' && currentStep !== 'generating' && currentStep !== 'complete' ? (
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
              style={{
                background: `color-mix(in srgb, ${token.colorWhite} 20%, transparent)`,
                borderColor: `color-mix(in srgb, ${token.colorWhite} 30%, transparent)`,
                color: token.colorWhite,
              }}
            >
              {isMobile ? '重新' : '重新开始'}
            </Button>
          ) : (
            <div style={{ width: isMobile ? 60 : 120 }}></div>
          )}
        </div>
      </div>

      <div style={{
        maxWidth: 800,
        margin: '0 auto',
        padding: isMobile ? '16px 12px' : '24px 24px',
      }}>
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
        title="执行设置"
        open={executionModalOpen}
        onCancel={() => setExecutionModalOpen(false)}
        onOk={beginProjectGeneration}
        okText="开始生成"
        cancelText="取消"
        destroyOnHidden
      >
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
      </Modal>
    </div>
  );
};

export default Inspiration;
