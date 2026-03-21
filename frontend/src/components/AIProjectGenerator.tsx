import React, { useState, useEffect, useRef } from 'react';
import { useNavigate } from 'react-router-dom';
import { Card, Button, Space, Typography, message, Progress } from 'antd';
import { CheckCircleOutlined, LoadingOutlined } from '@ant-design/icons';
import { backgroundTaskApi, wizardStreamApi } from '../services/api';
import type { SSEClientOptions } from '../utils/sseClient';
import type { ApiError, ResearchAssetSummary } from '../types';

const { Title, Paragraph, Text } = Typography;

export interface GenerationConfig {
  title: string;
  description: string;
  theme: string;
  genre: string | string[];
  narrative_perspective: string;
  target_words: number;
  chapter_count: number;
  character_count: number;
  outline_mode?: 'one-to-one' | 'one-to-many';
  enable_web_research?: boolean;
  web_research_query?: string;
  world_building_research_query?: string;
  careers_research_query?: string;
  characters_research_query?: string;
  outline_research_query?: string;
}

interface AIProjectGeneratorProps {
  config: GenerationConfig;
  storagePrefix: 'wizard' | 'inspiration';
  onComplete: (projectId: string) => void;
  onBack?: () => void;
  isMobile?: boolean;
  resumeProjectId?: string;
}

type GenerationStep = 'pending' | 'processing' | 'completed' | 'error';

interface GenerationSteps {
  worldBuilding: GenerationStep;
  careers: GenerationStep;
  characters: GenerationStep;
  outline: GenerationStep;
}

type ResearchStepKey = keyof GenerationSteps;

interface StepResearchSummary {
  query?: string;
  assets: ResearchAssetSummary[];
}

interface WorldBuildingResult {
  project_id: string;
  time_period: string;
  location: string;
  atmosphere: string;
  rules: string;
  research_query?: string;
  research_assets?: ResearchAssetSummary[];
}

export const AIProjectGenerator: React.FC<AIProjectGeneratorProps> = ({
  config,
  storagePrefix,
  onComplete,
  isMobile = false,
  resumeProjectId
}) => {
  const navigate = useNavigate();

  // 状态管理
  const [loading, setLoading] = useState(false);
  const [projectId, setProjectId] = useState<string>('');

  // SSE流式进度状态
  const [progress, setProgress] = useState(0);
  const [progressMessage, setProgressMessage] = useState('');
  const [errorDetails, setErrorDetails] = useState<string>('');
  const [currentTaskId, setCurrentTaskId] = useState<string | null>(null);
  const [isCancelling, setIsCancelling] = useState(false);
  const [generationSteps, setGenerationSteps] = useState<GenerationSteps>({
    worldBuilding: 'pending',
    careers: 'pending',
    characters: 'pending',
    outline: 'pending'
  });

  // 保存生成数据，用于重试
  const [generationData, setGenerationData] = useState<GenerationConfig | null>(null);
  // 保存世界观生成结果，用于后续步骤
  const [worldBuildingResult, setWorldBuildingResult] = useState<WorldBuildingResult | null>(null);
  const [researchSummaries, setResearchSummaries] = useState<Partial<Record<ResearchStepKey, StepResearchSummary>>>({});
  const cancelledByUserRef = useRef(false);
  // 【修复】操作锁，防止并发调用
  const operationLockRef = useRef(false);

  // LocalStorage 键名
  const storageKeys = {
    projectId: `${storagePrefix}_project_id`,
    generationData: `${storagePrefix}_generation_data`,
    currentStep: `${storagePrefix}_current_step`
  };

  // 保存进度到localStorage
  const saveProgress = (projectId: string, data: GenerationConfig, step: string) => {
    try {
      localStorage.setItem(storageKeys.projectId, projectId);
      localStorage.setItem(storageKeys.generationData, JSON.stringify(data));
      localStorage.setItem(storageKeys.currentStep, step);
    } catch (error) {
      console.error('保存进度失败:', error);
    }
  };

  // 清理localStorage
  const clearStorage = () => {
    localStorage.removeItem(storageKeys.projectId);
    localStorage.removeItem(storageKeys.generationData);
    localStorage.removeItem(storageKeys.currentStep);
  };

  const buildResearchFields = (data: GenerationConfig, step: ResearchStepKey) => {
    const stepQueryMap: Record<ResearchStepKey, string | undefined> = {
      worldBuilding: data.world_building_research_query,
      careers: data.careers_research_query,
      characters: data.characters_research_query,
      outline: data.outline_research_query,
    };
    return {
      enable_web_research: data.enable_web_research,
      web_research_query: (stepQueryMap[step] || data.web_research_query)?.trim() || undefined,
    };
  };

  const buildWorldBuildingPayload = (data: GenerationConfig) => {
    const genreString = Array.isArray(data.genre) ? data.genre.join('、') : data.genre;
    return {
      title: data.title,
      description: data.description,
      theme: data.theme,
      genre: genreString,
      narrative_perspective: data.narrative_perspective,
      target_words: data.target_words,
      chapter_count: data.chapter_count,
      character_count: data.character_count,
      outline_mode: data.outline_mode || 'one-to-many',
      ...buildResearchFields(data, 'worldBuilding'),
    };
  };

  const buildCareerPayload = (pid: string, data: GenerationConfig) => ({
    project_id: pid,
    ...buildResearchFields(data, 'careers'),
  });

  const buildCharactersPayload = (pid: string, data: GenerationConfig, worldResult: WorldBuildingResult) => {
    const genreString = Array.isArray(data.genre) ? data.genre.join('、') : data.genre;
    return {
      project_id: pid,
      count: data.character_count,
      world_context: {
        time_period: worldResult.time_period || '',
        location: worldResult.location || '',
        atmosphere: worldResult.atmosphere || '',
        rules: worldResult.rules || '',
      },
      theme: data.theme,
      genre: genreString,
      ...buildResearchFields(data, 'characters'),
    };
  };

  const buildOutlinePayload = (pid: string, data: GenerationConfig) => ({
    project_id: pid,
    chapter_count: data.chapter_count,
    narrative_perspective: data.narrative_perspective,
    target_words: data.target_words,
    ...buildResearchFields(data, 'outline'),
  });


  const updateStepResearch = (
    step: ResearchStepKey,
    payload?: { research_query?: string; research_assets?: ResearchAssetSummary[] }
  ) => {
    if (!payload) return;
    const query = payload.research_query?.trim() || '';
    const assets = Array.isArray(payload.research_assets) ? payload.research_assets.slice(0, 5) : [];
    if (!query && assets.length === 0) return;
    setResearchSummaries((prev) => ({
      ...prev,
      [step]: { query, assets },
    }));
  };

  const isTaskCancelledError = (error: unknown) => {
    const e = error as { name?: string; code?: string; message?: string };
    return cancelledByUserRef.current || e?.code === 'TASK_CANCELLED' || e?.name === 'TaskCancelledError' || e?.message?.includes('取消');
  };

  const buildTaskOptions = (options: SSEClientOptions): SSEClientOptions => ({
    ...options,
    onTaskCreated: (taskId: string) => {
      cancelledByUserRef.current = false;
      setCurrentTaskId(taskId);
      options.onTaskCreated?.(taskId);
    },
    onCancelled: (cancelMsg: string) => {
      cancelledByUserRef.current = true;
      setCurrentTaskId(null);
      setProgressMessage(cancelMsg || '后台任务已取消');
      setLoading(false);
      setIsCancelling(false);
      // 【修复】释放操作锁
      operationLockRef.current = false;
      options.onCancelled?.(cancelMsg);
    },
    onComplete: () => {
      setCurrentTaskId(null);
      setIsCancelling(false);
      // 【修复】释放操作锁
      operationLockRef.current = false;
      options.onComplete?.();
    },
  });

  const handleCancelCurrentTask = async (): Promise<boolean> => {
    // 【修复】防止并发调用
    if (!currentTaskId || isCancelling || operationLockRef.current) return false;

    operationLockRef.current = true;
    setIsCancelling(true);
    setProgressMessage('正在取消后台任务...');

    try {
      await backgroundTaskApi.cancelTask(currentTaskId);
      message.info('后台任务已取消');

      // 【修复】立即清理状态，移除硬编码延迟
      cancelledByUserRef.current = true;
      setCurrentTaskId(null);
      setIsCancelling(false);
      setLoading(false);
      operationLockRef.current = false;

      return true;
    } catch (error) {
      console.error('取消后台任务失败:', error);
      message.error('取消任务失败，请重试');
      setIsCancelling(false);
      operationLockRef.current = false;
      return false;
    }
  };

  // 开始自动化生成流程
  useEffect(() => {
    if (config) {
      if (resumeProjectId) {
        // 恢复生成模式
        handleResumeGenerate(config, resumeProjectId);
      } else {
        // 新建项目模式
        handleAutoGenerate(config);
      }
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [config, resumeProjectId]);

  // 恢复未完成项目的生成
  const handleResumeGenerate = async (data: GenerationConfig, projectIdParam: string) => {
    try {
      cancelledByUserRef.current = false;
      setCurrentTaskId(null);
      setIsCancelling(false);
      setLoading(true);
      setProgress(0);
      setProgressMessage('检查项目状态...');
      setErrorDetails('');
      setGenerationData(data);
      setProjectId(projectIdParam);

      // 获取项目信息,判断当前完成到哪一步
      const response = await fetch(`/api/projects/${projectIdParam}`, {
        credentials: 'include'
      });
      if (!response.ok) {
        throw new Error('获取项目信息失败');
      }
      const project = await response.json();
      const wizardStep = project.wizard_step || 0;

      // 根据wizard_step判断从哪里继续
      // wizard_step: 0=未开始, 1=世界观已完成, 2=职业体系已完成, 3=角色已完成, 4=大纲已完成
      // 获取世界观数据（用于后续步骤）
      const worldResult = {
        project_id: projectIdParam,
        time_period: project.world_time_period || '',
        location: project.world_location || '',
        atmosphere: project.world_atmosphere || '',
        rules: project.world_rules || ''
      };

      if (wizardStep === 0) {
        // 从世界观开始
        message.info('从世界观步骤开始生成...');
        setGenerationSteps({ worldBuilding: 'processing', careers: 'pending', characters: 'pending', outline: 'pending' });
        await resumeFromWorldBuilding(data);
      } else if (wizardStep === 1) {
        // 世界观已完成，从职业体系开始
        message.info('世界观已完成，从职业体系步骤继续...');
        setGenerationSteps({ worldBuilding: 'completed', careers: 'processing', characters: 'pending', outline: 'pending' });
        setWorldBuildingResult(worldResult);
        setProgress(20);
        await resumeFromCareers(data, worldResult);
      } else if (wizardStep === 2) {
        // 职业体系已完成，从角色开始
        message.info('职业体系已完成，从角色步骤继续...');
        setGenerationSteps({ worldBuilding: 'completed', careers: 'completed', characters: 'processing', outline: 'pending' });
        setWorldBuildingResult(worldResult);
        setProgress(40);
        await resumeFromCharacters(data, worldResult);
      } else if (wizardStep === 3) {
        // 角色已完成，从大纲开始
        message.info('角色已完成，从大纲步骤继续...');
        setGenerationSteps({ worldBuilding: 'completed', careers: 'completed', characters: 'completed', outline: 'processing' });
        setProgress(70);
        await resumeFromOutline(data, projectIdParam);
      } else {
        // 已全部完成
        message.success('项目已完成,正在跳转...');
        setProgress(100);
        onComplete(projectIdParam);
        setTimeout(() => {
          navigate(`/project/${projectIdParam}`);
        }, 1000);
      }
    } catch (error) {
      if (isTaskCancelledError(error)) {
        message.info('后台任务已取消');
        setLoading(false);
        setIsCancelling(false);
        setCurrentTaskId(null);
        // 【修复】释放操作锁
        operationLockRef.current = false;
        return;
      }
      const apiError = error as ApiError;
      const errorMsg = apiError.response?.data?.detail || apiError.message || '未知错误';
      console.error('恢复生成失败:', errorMsg);
      setErrorDetails(errorMsg);
      message.error('恢复生成失败：' + errorMsg);
      setLoading(false);
      // 【修复】释放操作锁
      operationLockRef.current = false;
    }
  };

  // 恢复:从世界观步骤开始
  const resumeFromWorldBuilding = async (data: GenerationConfig) => {
    const worldResult = await wizardStreamApi.generateWorldBuildingStream(
      buildWorldBuildingPayload(data),
      buildTaskOptions({
        onProgress: (msg, prog) => {
          // 直接使用后端返回的进度值
          setProgress(prog);
          setProgressMessage(msg);
        },
        onResult: (result) => {
          setWorldBuildingResult(result);
          updateStepResearch('worldBuilding', result);
          setGenerationSteps(prev => ({ ...prev, worldBuilding: 'completed' }));
        },
        onError: (error) => {
          console.error('世界观生成失败:', error);
          setErrorDetails(`世界观生成失败: ${error}`);
          setGenerationSteps(prev => ({ ...prev, worldBuilding: 'error' }));
          setLoading(false);
          throw new Error(error);
        },
        onComplete: () => {
          console.log('世界观生成完成');
        }
      })
    );

    await resumeFromCareers(data, worldResult);
  };

  // 恢复:从职业体系步骤继续
  const resumeFromCareers = async (data: GenerationConfig, worldResult: WorldBuildingResult) => {
    const pid = projectId || worldResult.project_id;

    setGenerationSteps(prev => ({ ...prev, careers: 'processing' }));
    setProgressMessage('正在生成职业体系...');

    await wizardStreamApi.generateCareerSystemStream(
      buildCareerPayload(pid, data),
      buildTaskOptions({
        onProgress: (msg, prog) => {
          setProgress(prog);
          setProgressMessage(msg);
        },
        onResult: (result) => {
          console.log(`成功生成职业体系：主职业${result.main_careers_count}个，副职业${result.sub_careers_count}个`);
          setGenerationSteps(prev => ({ ...prev, careers: 'completed' }));
        },
        onError: (error) => {
          console.error('职业体系生成失败:', error);
          setErrorDetails(`职业体系生成失败: ${error}`);
          setGenerationSteps(prev => ({ ...prev, careers: 'error' }));
          setLoading(false);
          throw new Error(error);
        },
        onComplete: () => {
          console.log('职业体系生成完成');
        }
      })
    );

    await resumeFromCharacters(data, worldResult);
  };

  // 恢复:从角色步骤继续
  const resumeFromCharacters = async (data: GenerationConfig, worldResult: WorldBuildingResult) => {
    const pid = projectId || worldResult.project_id;

    setGenerationSteps(prev => ({ ...prev, characters: 'processing' }));
    setProgressMessage('正在生成角色...');

    await wizardStreamApi.generateCharactersStream(
      buildCharactersPayload(pid, data, worldResult),
      buildTaskOptions({
        onProgress: (msg, prog) => {
          // 直接使用后端返回的进度值
          setProgress(prog);
          setProgressMessage(msg);
        },
        onResult: (result) => {
          console.log(`成功生成${result.characters?.length || 0}个角色`);
          setGenerationSteps(prev => ({ ...prev, characters: 'completed' }));
        },
        onError: (error) => {
          console.error('角色生成失败:', error);
          setErrorDetails(`角色生成失败: ${error}`);
          setGenerationSteps(prev => ({ ...prev, characters: 'error' }));
          setLoading(false);
          throw new Error(error);
        },
        onComplete: () => {
          console.log('角色生成完成');
        }
      })
    );

    await resumeFromOutline(data, pid);
  };

  // 恢复:从大纲步骤继续
  const resumeFromOutline = async (data: GenerationConfig, pid: string) => {
    setGenerationSteps(prev => ({ ...prev, outline: 'processing' }));
    setProgressMessage('正在生成大纲...');

    await wizardStreamApi.generateCompleteOutlineStream(
      buildOutlinePayload(pid, data),
      buildTaskOptions({
        onProgress: (msg, prog) => {
          // 直接使用后端返回的进度值
          setProgress(prog);
          setProgressMessage(msg);
        },
        onResult: () => {
          console.log('大纲生成完成');
          setGenerationSteps(prev => ({ ...prev, outline: 'completed' }));
        },
        onError: (error) => {
          console.error('大纲生成失败:', error);
          setErrorDetails(`大纲生成失败: ${error}`);
          setGenerationSteps(prev => ({ ...prev, outline: 'error' }));
          setLoading(false);
          throw new Error(error);
        },
        onComplete: () => {
          console.log('大纲生成完成');
        }
      })
    );

    // 全部完成
    setProgress(100);
    setProgressMessage('项目创建完成！正在跳转...');
    message.success('项目创建成功！正在进入项目...');
    clearStorage();
    setLoading(false);

    onComplete(pid);
    setTimeout(() => {
      navigate(`/project/${pid}`);
    }, 1000);
  };

  // 自动化生成流程
  const handleAutoGenerate = async (data: GenerationConfig) => {
    try {
      cancelledByUserRef.current = false;
      setCurrentTaskId(null);
      setIsCancelling(false);
      setLoading(true);
      setProgress(0);
      setProgressMessage('开始创建项目...');
      setErrorDetails('');
      setGenerationData(data);
      saveProgress('', data, 'generating');

      // 步骤1: 生成世界观并创建项目
      setGenerationSteps(prev => ({ ...prev, worldBuilding: 'processing' }));
      setProgressMessage('正在生成世界观...');

      const worldResult = await wizardStreamApi.generateWorldBuildingStream(
        buildWorldBuildingPayload(data),
        buildTaskOptions({
          onProgress: (msg, prog) => {
            // 直接使用后端返回的进度值
            setProgress(prog);
            setProgressMessage(msg);
          },
          onResult: (result) => {
            setProjectId(result.project_id);
            setWorldBuildingResult(result);
            updateStepResearch('worldBuilding', result);
            setGenerationSteps(prev => ({ ...prev, worldBuilding: 'completed' }));
          },
          onError: (error) => {
            console.error('世界观生成失败:', error);
            setErrorDetails(`世界观生成失败: ${error}`);
            setGenerationSteps(prev => ({ ...prev, worldBuilding: 'error' }));
            setLoading(false);
            throw new Error(error);
          },
          onComplete: () => {
            console.log('世界观生成完成');
          }
        })
      );

      if (!worldResult?.project_id) {
        throw new Error('项目创建失败：未获取到项目ID');
      }

      const createdProjectId = worldResult.project_id;
      setProjectId(createdProjectId);
      setWorldBuildingResult(worldResult);
      saveProgress(createdProjectId, data, 'generating');

      // 步骤2: 生成职业体系
      setGenerationSteps(prev => ({ ...prev, careers: 'processing' }));
      setProgressMessage('正在生成职业体系...');

      await wizardStreamApi.generateCareerSystemStream(
        buildCareerPayload(createdProjectId, data),
        buildTaskOptions({
          onProgress: (msg, prog) => {
            setProgress(prog);
            setProgressMessage(msg);
          },
          onResult: (result) => {
            console.log(`成功生成职业体系：主职业${result.main_careers_count}个，副职业${result.sub_careers_count}个`);
            setGenerationSteps(prev => ({ ...prev, careers: 'completed' }));
          },
          onError: (error) => {
            console.error('职业体系生成失败:', error);
            setErrorDetails(`职业体系生成失败: ${error}`);
            setGenerationSteps(prev => ({ ...prev, careers: 'error' }));
            setLoading(false);
            throw new Error(error);
          },
          onComplete: () => {
            console.log('职业体系生成完成');
          }
        })
      );

      // 步骤3: 生成角色
      setGenerationSteps(prev => ({ ...prev, characters: 'processing' }));
      setProgressMessage('正在生成角色...');

      await wizardStreamApi.generateCharactersStream(
        buildCharactersPayload(createdProjectId, data, worldResult),
        buildTaskOptions({
          onProgress: (msg, prog) => {
            // 直接使用后端返回的进度值
            setProgress(prog);
            setProgressMessage(msg);
          },
          onResult: (result) => {
            console.log(`成功生成${result.characters?.length || 0}个角色`);
            setGenerationSteps(prev => ({ ...prev, characters: 'completed' }));
          },
          onError: (error) => {
            console.error('角色生成失败:', error);
            setErrorDetails(`角色生成失败: ${error}`);
            setGenerationSteps(prev => ({ ...prev, characters: 'error' }));
            setLoading(false);
            throw new Error(error);
          },
          onComplete: () => {
            console.log('角色生成完成');
          }
        })
      );

      // 步骤3: 生成大纲
      setGenerationSteps(prev => ({ ...prev, outline: 'processing' }));
      setProgressMessage('正在生成大纲...');

      await wizardStreamApi.generateCompleteOutlineStream(
        buildOutlinePayload(createdProjectId, data),
        buildTaskOptions({
          onProgress: (msg, prog) => {
            // 直接使用后端返回的进度值
            setProgress(prog);
            setProgressMessage(msg);
          },
          onResult: () => {
            console.log('大纲生成完成');
            setGenerationSteps(prev => ({ ...prev, outline: 'completed' }));
          },
          onError: (error) => {
            console.error('大纲生成失败:', error);
            setErrorDetails(`大纲生成失败: ${error}`);
            setGenerationSteps(prev => ({ ...prev, outline: 'error' }));
            setLoading(false);
            throw new Error(error);
          },
          onComplete: () => {
            console.log('大纲生成完成');
          }
        })
      );

      // 全部完成 - 自动跳转到项目详情页
      setProgress(100);
      setProgressMessage('项目创建完成！正在跳转...');
      message.success('项目创建成功！正在进入项目...');
      clearStorage();

      // 调用完成回调
      onComplete(createdProjectId);

      // 延迟1秒后自动跳转到项目详情页
      setTimeout(() => {
        navigate(`/project/${createdProjectId}`);
      }, 1000);

    } catch (error) {
      if (isTaskCancelledError(error)) {
        message.info('后台任务已取消');
        setLoading(false);
        setIsCancelling(false);
        setCurrentTaskId(null);
        // 【修复】释放操作锁
        operationLockRef.current = false;
        return;
      }
      const apiError = error as ApiError;
      const errorMsg = apiError.response?.data?.detail || apiError.message || '未知错误';
      console.error('创建项目失败:', errorMsg);
      setErrorDetails(errorMsg);
      message.error('创建项目失败：' + errorMsg);
      setLoading(false);
      // 【修复】释放操作锁
      operationLockRef.current = false;
    }
  };

  // 智能重试：从失败的步骤继续生成
  const handleSmartRetry = async () => {
    // 【修复】防止并发调用
    if (operationLockRef.current) {
      message.warning('操作正在进行中，请稍后重试');
      return;
    }

    if (!generationData) {
      message.warning('缺少生成数据');
      return;
    }

    // 【修复】如果正在取消中，阻止重试
    if (isCancelling) {
      message.warning('正在取消任务，请稍后重试');
      return;
    }

    // 【修复】如果有正在运行的任务，先取消
    if (currentTaskId) {
      message.info('检测到正在运行的任务，正在取消...');
      const cancelled = await handleCancelCurrentTask();
      if (!cancelled) {
        message.error('无法取消现有任务，请稍后重试');
        return;
      }
      // 【修复】等待状态稳定后再继续
      await new Promise(resolve => setTimeout(resolve, 500));
    }

    // 【修复】加锁防止重入
    operationLockRef.current = true;

    // 重置所有状态
    cancelledByUserRef.current = false;
    setCurrentTaskId(null);
    setIsCancelling(false);
    setLoading(true);
    setErrorDetails('');

    try {
      if (generationSteps.worldBuilding === 'error') {
        message.info('从世界观步骤开始重新生成...');
        await retryFromWorldBuilding();
      } else if (generationSteps.careers === 'error') {
        message.info('从职业体系步骤继续生成...');
        await retryFromCareers();
      } else if (generationSteps.characters === 'error') {
        message.info('从角色步骤继续生成...');
        await retryFromCharacters();
      } else if (generationSteps.outline === 'error') {
        message.info('从大纲步骤继续生成...');
        await retryFromOutline();
      }
    } catch (error) {
      if (isTaskCancelledError(error)) {
        message.info('后台任务已取消');
        setLoading(false);
        setIsCancelling(false);
        setCurrentTaskId(null);
        operationLockRef.current = false;
        return;
      }
      console.error('智能重试失败:', error);
      const errorMessage = error instanceof Error ? error.message : '未知错误';
      message.error('重试失败：' + errorMessage);
      setLoading(false);
      operationLockRef.current = false;
    } finally {
      // 【修复】确保锁一定会释放
      operationLockRef.current = false;
    }
  };

  // 从世界观步骤重新开始
  const retryFromWorldBuilding = async () => {
    if (!generationData) return;

    setGenerationSteps(prev => ({ ...prev, worldBuilding: 'processing' }));
    setProgressMessage('重新生成世界观...');

    const worldResult = await wizardStreamApi.generateWorldBuildingStream(
      buildWorldBuildingPayload(generationData),
      buildTaskOptions({
        onProgress: (msg, prog) => {
          // 直接使用后端返回的进度值
          setProgress(prog);
          setProgressMessage(msg);
        },
        onResult: (result) => {
          setProjectId(result.project_id);
          setWorldBuildingResult(result);
          updateStepResearch('worldBuilding', result);
          setGenerationSteps(prev => ({ ...prev, worldBuilding: 'completed' }));
        },
        onError: (error) => {
          console.error('世界观生成失败:', error);
          setErrorDetails(`世界观生成失败: ${error}`);
          setGenerationSteps(prev => ({ ...prev, worldBuilding: 'error' }));
          setLoading(false);
          throw new Error(error);
        },
        onComplete: () => {
          console.log('世界观重新生成完成');
        }
      })
    );

    if (!worldResult?.project_id) {
      throw new Error('项目创建失败：未获取到项目ID');
    }

    await continueFromCareers(worldResult);
  };

  // 从职业体系步骤继续
  const retryFromCareers = async () => {
    if (!generationData || !worldBuildingResult) {
      message.warning('缺少必要数据，无法从职业体系步骤继续');
      setLoading(false);
      return;
    }

    const pid = worldBuildingResult.project_id || projectId;
    if (!pid) {
      message.warning('缺少项目ID，无法从职业体系步骤继续');
      setLoading(false);
      return;
    }

    setGenerationSteps(prev => ({ ...prev, careers: 'processing' }));
    setProgressMessage('重新生成职业体系...');

    await wizardStreamApi.generateCareerSystemStream(
      buildCareerPayload(pid, generationData),
      buildTaskOptions({
        onProgress: (msg, prog) => {
          setProgress(prog);
          setProgressMessage(msg);
        },
        onResult: (result) => {
          console.log(`成功生成职业体系：主职业${result.main_careers_count}个，副职业${result.sub_careers_count}个`);
          setGenerationSteps(prev => ({ ...prev, careers: 'completed' }));
        },
        onError: (error) => {
          console.error('职业体系生成失败:', error);
          setErrorDetails(`职业体系生成失败: ${error}`);
          setGenerationSteps(prev => ({ ...prev, careers: 'error' }));
          setLoading(false);
          throw new Error(error);
        },
        onComplete: () => {
          console.log('职业体系重新生成完成');
        }
      })
    );

    await continueFromCharacters(worldBuildingResult);
  };

  // 从角色步骤继续
  const retryFromCharacters = async () => {
    if (!generationData || !worldBuildingResult) {
      message.warning('缺少必要数据，无法从角色步骤继续');
      setLoading(false);
      return;
    }

    // 优先使用 worldBuildingResult 中的 project_id，因为重试可能创建了新项目
    const pid = worldBuildingResult.project_id || projectId;
    if (!pid) {
      message.warning('缺少项目ID，无法从角色步骤继续');
      setLoading(false);
      return;
    }

    setGenerationSteps(prev => ({ ...prev, characters: 'processing' }));
    setProgressMessage('重新生成角色...');

    await wizardStreamApi.generateCharactersStream(
      buildCharactersPayload(pid, generationData, worldBuildingResult),
      buildTaskOptions({
        onProgress: (msg, prog) => {
          // 直接使用后端返回的进度值
          setProgress(prog);
          setProgressMessage(msg);
        },
        onResult: (result) => {
          console.log(`成功生成${result.characters?.length || 0}个角色`);
          setGenerationSteps(prev => ({ ...prev, characters: 'completed' }));
        },
        onError: (error) => {
          console.error('角色生成失败:', error);
          setErrorDetails(`角色生成失败: ${error}`);
          setGenerationSteps(prev => ({ ...prev, characters: 'error' }));
          setLoading(false);
          throw new Error(error);
        },
        onComplete: () => {
          console.log('角色重新生成完成');
        }
      })
    );

    await continueFromOutline(pid);
  };

  // 从大纲步骤继续
  const retryFromOutline = async () => {
    if (!generationData) {
      message.warning('缺少必要数据，无法从大纲步骤继续');
      setLoading(false);
      return;
    }

    // 优先使用 worldBuildingResult 中的 project_id，fallback 到状态中的 projectId
    const pid = (worldBuildingResult?.project_id) || projectId;
    if (!pid) {
      message.warning('缺少项目ID，无法从大纲步骤继续');
      setLoading(false);
      return;
    }

    setGenerationSteps(prev => ({ ...prev, outline: 'processing' }));
    setProgressMessage('重新生成大纲...');

    await wizardStreamApi.generateCompleteOutlineStream(
      buildOutlinePayload(pid, generationData),
      buildTaskOptions({
        onProgress: (msg, prog) => {
          // 直接使用后端返回的进度值
          setProgress(prog);
          setProgressMessage(msg);
        },
        onResult: () => {
          console.log('大纲生成完成');
          setGenerationSteps(prev => ({ ...prev, outline: 'completed' }));
        },
        onError: (error) => {
          console.error('大纲生成失败:', error);
          setErrorDetails(`大纲生成失败: ${error}`);
          setGenerationSteps(prev => ({ ...prev, outline: 'error' }));
          setLoading(false);
          throw new Error(error);
        },
        onComplete: () => {
          console.log('大纲重新生成完成');
        }
      })
    );

    setProgress(100);
    setProgressMessage('项目创建完成！正在跳转...');
    message.success('项目创建成功！正在进入项目...');
    setLoading(false);

    // 调用完成回调
    if (pid) {
      onComplete(pid);

      // 延迟1秒后自动跳转到项目详情页
      setTimeout(() => {
        navigate(`/project/${pid}`);
      }, 1000);
    }
  };

  // 从职业体系步骤开始的完整流程
  const continueFromCareers = async (worldResult: WorldBuildingResult) => {
    if (!generationData || !worldResult?.project_id) return;

    const pid = worldResult.project_id;

    setGenerationSteps(prev => ({ ...prev, careers: 'processing' }));
    setProgressMessage('正在生成职业体系...');

    await wizardStreamApi.generateCareerSystemStream(
      buildCareerPayload(pid, generationData),
      buildTaskOptions({
        onProgress: (msg, prog) => {
          setProgress(prog);
          setProgressMessage(msg);
        },
        onResult: (result) => {
          console.log(`成功生成职业体系：主职业${result.main_careers_count}个，副职业${result.sub_careers_count}个`);
          setGenerationSteps(prev => ({ ...prev, careers: 'completed' }));
        },
        onError: (error) => {
          console.error('职业体系生成失败:', error);
          setErrorDetails(`职业体系生成失败: ${error}`);
          setGenerationSteps(prev => ({ ...prev, careers: 'error' }));
          setLoading(false);
          throw new Error(error);
        },
        onComplete: () => {
          console.log('职业体系生成完成');
        }
      })
    );

    await continueFromCharacters(worldResult);
  };

  // 从角色步骤开始的完整流程
  const continueFromCharacters = async (worldResult: WorldBuildingResult) => {
    if (!generationData || !worldResult?.project_id) return;

    const pid = worldResult.project_id;

    setGenerationSteps(prev => ({ ...prev, characters: 'processing' }));
    setProgressMessage('正在生成角色...');

    await wizardStreamApi.generateCharactersStream(
      buildCharactersPayload(pid, generationData, worldResult),
      buildTaskOptions({
        onProgress: (msg, prog) => {
          // 直接使用后端返回的进度值
          setProgress(prog);
          setProgressMessage(msg);
        },
        onResult: (result) => {
          console.log(`成功生成${result.characters?.length || 0}个角色`);
          setGenerationSteps(prev => ({ ...prev, characters: 'completed' }));
        },
        onError: (error) => {
          console.error('角色生成失败:', error);
          setErrorDetails(`角色生成失败: ${error}`);
          setGenerationSteps(prev => ({ ...prev, characters: 'error' }));
          setLoading(false);
          throw new Error(error);
        },
        onComplete: () => {
          console.log('角色生成完成');
        }
      })
    );

    await continueFromOutline(pid);
  };

  // 从大纲步骤开始的完整流程
  const continueFromOutline = async (pid: string) => {
    if (!generationData || !pid) return;

    setGenerationSteps(prev => ({ ...prev, outline: 'processing' }));
    setProgressMessage('正在生成大纲...');

    await wizardStreamApi.generateCompleteOutlineStream(
      buildOutlinePayload(pid, generationData),
      buildTaskOptions({
        onProgress: (msg, prog) => {
          // 直接使用后端返回的进度值
          setProgress(prog);
          setProgressMessage(msg);
        },
        onResult: () => {
          console.log('大纲生成完成');
          setGenerationSteps(prev => ({ ...prev, outline: 'completed' }));
        },
        onError: (error) => {
          console.error('大纲生成失败:', error);
          setErrorDetails(`大纲生成失败: ${error}`);
          setGenerationSteps(prev => ({ ...prev, outline: 'error' }));
          setLoading(false);
          throw new Error(error);
        },
        onComplete: () => {
          console.log('大纲生成完成');
        }
      })
    );

    setProgress(100);
    setProgressMessage('项目创建完成！正在跳转...');
    message.success('项目创建成功！正在进入项目...');
    setLoading(false);

    // 调用完成回调
    if (pid) {
      onComplete(pid);

      // 延迟1秒后自动跳转到项目详情页
      setTimeout(() => {
        navigate(`/project/${pid}`);
      }, 1000);
    }
  };


  // 获取步骤状态图标和颜色
  const getStepStatus = (step: GenerationStep) => {
    if (step === 'completed') return { icon: <CheckCircleOutlined />, color: 'var(--color-success)' };
    if (step === 'processing') return { icon: <LoadingOutlined />, color: 'var(--color-primary)' };
    if (step === 'error') return { icon: '✗', color: 'var(--color-error)' };
    return { icon: '○', color: 'var(--color-text-quaternary)' };
  };

  const hasError = generationSteps.worldBuilding === 'error' ||
    generationSteps.careers === 'error' ||
    generationSteps.characters === 'error' ||
    generationSteps.outline === 'error';

  // 渲染生成进度页面
  const renderGenerating = () => (
    <div style={{
      textAlign: 'center',
      padding: isMobile ? '32px 16px' : '40px 20px',
      maxWidth: '100%',
      overflow: 'hidden'
    }}>
      <Title
        level={isMobile ? 4 : 3}
        style={{
          marginBottom: 32,
          color: 'var(--color-text-primary)',
          wordBreak: 'break-word',
          whiteSpace: 'normal',
          overflowWrap: 'break-word'
        }}
      >
        正在为《{config.title}》生成内容
      </Title>

      <Card style={{ marginBottom: 24, maxWidth: '100%' }}>
        <Progress
          percent={progress}
          status={hasError ? 'exception' : (progress === 100 ? 'success' : 'active')}
          strokeColor={{
            '0%': 'var(--color-primary)',
            '100%': 'var(--color-primary-active)',
          }}
          style={{ marginBottom: 24 }}
        />

        <Paragraph
          style={{
            fontSize: isMobile ? 14 : 16,
            marginBottom: 32,
            color: hasError ? 'var(--color-error)' : 'var(--color-text-secondary)',
            wordBreak: 'break-word',
            whiteSpace: 'normal',
            overflowWrap: 'break-word'
          }}
        >
          {progressMessage}
        </Paragraph>

        {errorDetails && (
          <Card
            size="small"
            style={{
              marginBottom: 24,
              background: 'var(--color-error-bg)',
              borderColor: 'var(--color-error-border)',
              textAlign: 'left',
              maxWidth: '100%',
              overflow: 'hidden'
            }}
          >
            <Text strong style={{ color: 'var(--color-error)' }}>错误详情：</Text>
            <br />
            <Text
              style={{
                color: 'var(--color-text-secondary)',
                fontSize: 14,
                wordBreak: 'break-word',
                whiteSpace: 'normal',
                overflowWrap: 'break-word',
                display: 'block'
              }}
            >
              {errorDetails}
            </Text>
          </Card>
        )}

        {Object.values(researchSummaries).some((item) => item && (item.query || item.assets.length > 0)) && (
          <Card
            size="small"
            title="本次检索资料摘要"
            style={{
              marginBottom: 24,
              textAlign: 'left',
              maxWidth: '100%',
              overflow: 'hidden',
            }}
          >
            <Space direction="vertical" size={12} style={{ width: '100%' }}>
              {([
                ['worldBuilding', '世界观'],
                ['careers', '职业体系'],
                ['characters', '角色'],
                ['outline', '大纲'],
              ] as Array<[ResearchStepKey, string]>).map(([stepKey, label]) => {
                const item = researchSummaries[stepKey];
                if (!item || (!item.query && item.assets.length === 0)) {
                  return null;
                }
                return (
                  <div key={stepKey} style={{ padding: '12px', border: '1px solid var(--color-border-secondary)', borderRadius: 8 }}>
                    <Text strong>{label}</Text>
                    {item.query && (
                      <div style={{ marginTop: 6, color: 'var(--color-text-secondary)', fontSize: 13 }}>
                        <strong>Query：</strong>{item.query}
                      </div>
                    )}
                    {item.assets.length > 0 && (
                      <ul style={{ margin: '8px 0 0 0', paddingLeft: 18 }}>
                        {item.assets.map((asset, index) => (
                          <li key={`${stepKey}-${index}`} style={{ marginBottom: 8 }}>
                            <div style={{ fontWeight: 500 }}>{asset.title}</div>
                            {asset.summary && <div style={{ fontSize: 13 }}>{asset.summary}</div>}
                            {asset.source && (
                              <div style={{ fontSize: 12, color: 'var(--color-text-secondary)' }}>{asset.source}</div>
                            )}
                          </li>
                        ))}
                      </ul>
                    )}
                  </div>
                );
              })}
            </Space>
          </Card>
        )}

        <Space
          direction="vertical"
          size={16}
          style={{
            width: '100%',
            maxWidth: isMobile ? '100%' : 400,
            margin: '0 auto'
          }}
        >
          {[
            { key: 'worldBuilding', label: '生成世界观', step: generationSteps.worldBuilding },
            { key: 'careers', label: '生成职业体系', step: generationSteps.careers },
            { key: 'characters', label: '生成角色', step: generationSteps.characters },
            { key: 'outline', label: '生成大纲', step: generationSteps.outline },
          ].map(({ key, label, step }) => {
            const status = getStepStatus(step);
            return (
              <div
                key={key}
                style={{
                  display: 'flex',
                  alignItems: 'center',
                  justifyContent: 'space-between',
                  padding: isMobile ? '10px 12px' : '12px 20px',
                  background: step === 'processing' ? 'var(--color-info-bg)' : (step === 'error' ? 'var(--color-error-bg)' : 'var(--color-bg-layout)'),
                  borderRadius: 8,
                  border: `1px solid ${step === 'processing' ? 'var(--color-info-border)' : (step === 'error' ? 'var(--color-error-border)' : 'var(--color-border-secondary)')}`,
                  gap: '8px',
                  maxWidth: '100%',
                  overflow: 'hidden'
                }}
              >
                <Text
                  style={{
                    fontSize: isMobile ? 14 : 16,
                    fontWeight: step === 'processing' ? 600 : 400,
                    wordBreak: 'break-word',
                    whiteSpace: 'normal',
                    overflowWrap: 'break-word',
                    flex: 1,
                    textAlign: 'left'
                  }}
                >
                  {label}
                </Text>
                <span
                  style={{
                    fontSize: 20,
                    color: status.color,
                    flexShrink: 0
                  }}
                >
                  {status.icon}
                </span>
              </div>
            );
          })}
        </Space>
      </Card>

      <Paragraph
        type="secondary"
        style={{
          color: 'var(--color-text-secondary)',
          opacity: 0.9,
          wordBreak: 'break-word',
          whiteSpace: 'normal',
          overflowWrap: 'break-word',
          fontSize: isMobile ? 14 : 16
        }}
      >
        {hasError ? '生成过程中出现错误，请点击重试按钮重新生成' : '请耐心等待，正在为您精心创作...'}
      </Paragraph>

      {!hasError && loading && (
        <Space style={{ marginTop: 16 }}>
          <Button
            danger
            size="large"
            onClick={handleCancelCurrentTask}
            loading={isCancelling}
            disabled={!currentTaskId || isCancelling}
          >
            取消当前任务
          </Button>
        </Space>
      )}

      {hasError && (
        <Space style={{ marginTop: 16 }}>
          <Button
            type="primary"
            size="large"
            onClick={handleSmartRetry}
            loading={loading || isCancelling}
            disabled={loading || isCancelling}
          >
            智能重试
          </Button>
        </Space>
      )}

    </div>
  );

  return renderGenerating();
};
