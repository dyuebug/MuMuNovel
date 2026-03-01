/**
 * Store Hooks - 提供数据获取和自动同步功能
 * 这些 hooks 封装了数据获取逻辑，并自动更新 store
 */

import { useCallback } from 'react';
import { message } from 'antd';
import { useStore } from './index';
import { projectApi, outlineApi, characterApi, chapterApi } from '../services/api';
import type {
  PaginationResponse,
  Outline,
  Character,
  Chapter,
  Project,
  ProjectCreate,
  ProjectUpdate,
  OutlineCreate,
  OutlineUpdate,
  ChapterCreate,
  ChapterUpdate,
  GenerateOutlineRequest,
  GenerateCharacterRequest
} from '../types';

/**
 * 项目数据同步 Hook
 */
export function useProjectSync() {
  const { setProjects, setLoading, addProject, updateProject, removeProject } = useStore();

  // 刷新项目列表
  const refreshProjects = useCallback(async () => {
    try {
      setLoading(true);
      const data = await projectApi.getProjects();
      const projects = Array.isArray(data) ? data : (data as PaginationResponse<Project>).items || [];
      setProjects(projects);
      return projects;
    } catch (error) {
      console.error('刷新项目列表失败:', error);
      message.error('刷新项目列表失败');
      return [];
    } finally {
      setLoading(false);
    }
  }, [setProjects, setLoading]);

  // 创建项目（带同步）
  const createProject = useCallback(async (data: ProjectCreate) => {
    try {
      const created = await projectApi.createProject(data);
      addProject(created);
      return created;
    } catch (error) {
      console.error('创建项目失败:', error);
      throw error;
    }
  }, [addProject]);

  // 更新项目（带同步）
  const updateProjectSync = useCallback(async (id: string, data: ProjectUpdate) => {
    try {
      const updated = await projectApi.updateProject(id, data);
      updateProject(id, updated);
      return updated;
    } catch (error) {
      console.error('更新项目失败:', error);
      throw error;
    }
  }, [updateProject]);

  // 删除项目（带同步）
  const deleteProject = useCallback(async (id: string) => {
    try {
      await projectApi.deleteProject(id);
      removeProject(id);
    } catch (error) {
      console.error('删除项目失败:', error);
      throw error;
    }
  }, [removeProject]);

  return {
    refreshProjects,
    createProject,
    updateProject: updateProjectSync,
    deleteProject,
  };
}

/**
 * 角色数据同步 Hook
 */
export function useCharacterSync() {
  const { currentProject, setCharacters, addCharacter, removeCharacter } = useStore();

  // 刷新角色列表
  const refreshCharacters = useCallback(async (projectId?: string) => {
    const id = projectId || currentProject?.id;
    if (!id) return [];

    try {
      const data = await characterApi.getCharacters(id);
      const characters = Array.isArray(data) ? data : (data as PaginationResponse<Character>).items || [];
      setCharacters(characters);
      return characters;
    } catch (error) {
      console.error('刷新角色列表失败:', error);
      message.error('刷新角色列表失败');
      return [];
    }
  }, [currentProject?.id, setCharacters]);

  // 删除角色（带同步）
  const deleteCharacter = useCallback(async (id: string) => {
    try {
      await characterApi.deleteCharacter(id);
      removeCharacter(id);
    } catch (error) {
      console.error('删除角色失败:', error);
      throw error;
    }
  }, [removeCharacter]);

  // AI生成角色（带同步）
  const generateCharacter = useCallback(async (data: GenerateCharacterRequest) => {
    try {
      const generated = await characterApi.generateCharacter(data);
      addCharacter(generated);
      return generated;
    } catch (error) {
      console.error('AI生成角色失败:', error);
      throw error;
    }
  }, [addCharacter]);

  return {
    refreshCharacters,
    deleteCharacter,
    generateCharacter,
  };
}

/**
 * 大纲数据同步 Hook
 */
export function useOutlineSync() {
  const { currentProject, setOutlines, addOutline, updateOutline, removeOutline } = useStore();

  // 刷新大纲列表
  const refreshOutlines = useCallback(async (projectId?: string) => {
    const id = projectId || currentProject?.id;
    if (!id) return [];

    try {
      const data = await outlineApi.getOutlines(id);
      const outlines = Array.isArray(data) ? data : (data as PaginationResponse<Outline>).items || [];
      setOutlines(outlines);
      return outlines;
    } catch (error) {
      console.error('刷新大纲列表失败:', error);
      message.error('刷新大纲列表失败');
      return [];
    }
  }, [currentProject?.id, setOutlines]); // 添加 currentProject?.id 到依赖数组

  // 创建大纲（带同步）
  const createOutline = useCallback(async (data: OutlineCreate) => {
    try {
      const created = await outlineApi.createOutline(data);
      addOutline(created);
      return created;
    } catch (error) {
      console.error('创建大纲失败:', error);
      throw error;
    }
  }, [addOutline]);

  // 更新大纲（带同步）
  const updateOutlineSync = useCallback(async (id: string, data: OutlineUpdate) => {
    try {
      const updated = await outlineApi.updateOutline(id, data);
      updateOutline(id, updated);
      return updated;
    } catch (error) {
      console.error('更新大纲失败:', error);
      throw error;
    }
  }, [updateOutline]);

  // 删除大纲（带同步）
  const deleteOutline = useCallback(async (id: string) => {
    try {
      await outlineApi.deleteOutline(id);
      removeOutline(id);
    } catch (error) {
      console.error('删除大纲失败:', error);
      throw error;
    }
  }, [removeOutline]);

  // AI生成大纲（带同步）
  const generateOutlines = useCallback(async (data: GenerateOutlineRequest) => {
    try {
      const result = await outlineApi.generateOutline(data);
      const outlines = Array.isArray(result) ? result : (result as PaginationResponse<Outline>).items || [];
      outlines.forEach((outline: Outline) => addOutline(outline));
      return outlines;
    } catch (error) {
      console.error('AI生成大纲失败:', error);
      throw error;
    }
  }, [addOutline]);

  return {
    refreshOutlines,
    createOutline,
    updateOutline: updateOutlineSync,
    deleteOutline,
    generateOutlines,
  };
}

/**
 * 章节数据同步 Hook
 */
export function useChapterSync() {
  const { currentProject, setChapters, addChapter, updateChapter, removeChapter } = useStore();

  // 刷新章节列表
  const refreshChapters = useCallback(async (projectId?: string) => {
    const id = projectId || currentProject?.id;
    if (!id) return [];

    try {
      const data = await chapterApi.getChapters(id);
      const chapters = Array.isArray(data) ? data : (data as PaginationResponse<Chapter>).items || [];
      setChapters(chapters);
      return chapters;
    } catch (error) {
      console.error('刷新章节列表失败:', error);
      message.error('刷新章节列表失败');
      return [];
    }
  }, [currentProject?.id, setChapters]); // 添加 currentProject?.id 到依赖数组

  // 创建章节（带同步）
  const createChapter = useCallback(async (data: ChapterCreate) => {
    try {
      const created = await chapterApi.createChapter(data);
      addChapter(created);
      return created;
    } catch (error) {
      console.error('创建章节失败:', error);
      throw error;
    }
  }, [addChapter]);

  // 更新章节（带同步）
  const updateChapterSync = useCallback(async (id: string, data: ChapterUpdate) => {
    try {
      const updated = await chapterApi.updateChapter(id, data);
      updateChapter(id, updated);
      return updated;
    } catch (error) {
      console.error('更新章节失败:', error);
      throw error;
    }
  }, [updateChapter]);

  // 删除章节（带同步）
  const deleteChapter = useCallback(async (id: string) => {
    try {
      await chapterApi.deleteChapter(id);
      removeChapter(id);
    } catch (error) {
      console.error('删除章节失败:', error);
      throw error;
    }
  }, [removeChapter]);

  // AI后台生成章节内容（创建任务后立即返回，完成过程在后台异步跟踪）
  const generateChapterContentStream = useCallback(async (
    chapterId: string,
    onProgress?: (content: string) => void,
    styleId?: number,
    targetWordCount?: number,
    onProgressUpdate?: (message: string, progress: number) => void,
    model?: string,
    narrativePerspective?: string
  ) => {
    const formatQualityMessage = (metrics: any): string | null => {
      if (!metrics || typeof metrics !== 'object') return null;
      const overall = Number(metrics.overall_score ?? 0).toFixed(1);
      const conflict = Number(metrics.conflict_chain_hit_rate ?? 0).toFixed(1);
      const rule = Number(metrics.rule_grounding_hit_rate ?? 0).toFixed(1);
      return `剧情评分：综合${overall}｜冲突链${conflict}%｜规则落地${rule}%`;
    };

    // 1) 创建后台任务（立即返回 task_id）
    const response = await fetch(`/api/chapters/${chapterId}/generate-background`, {
      method: 'POST',
      headers: {
        'Content-Type': 'application/json',
      },
      body: JSON.stringify({
        style_id: styleId,
        target_word_count: targetWordCount,
        model: model,
        narrative_perspective: narrativePerspective
      }),
    });

    if (!response.ok) {
      const errorData = await response.json().catch(() => ({}));
      throw new Error(errorData.detail || `HTTP error! status: ${response.status}`);
    }

    const startResult = await response.json();
    const taskId: string | undefined = startResult.task_id;
    if (!taskId) {
      throw new Error('后台任务创建成功但未返回 task_id');
    }

    if (onProgressUpdate) {
      onProgressUpdate(startResult.message || '后台任务已创建', 5);
    }

    // 2) 在后台异步跟踪任务，保留实时chunk和轮询兜底
    const completion = (async () => {
      let fullContent = '';
      let streamFailure: string | null = null;
      const streamAbortController = new AbortController();

      const streamPromise = (async () => {
        try {
          const streamResponse = await fetch(`/api/chapters/batch-generate/${taskId}/stream`, {
            method: 'GET',
            signal: streamAbortController.signal,
          });

          if (!streamResponse.ok || !streamResponse.body) {
            return;
          }

          const reader = streamResponse.body.getReader();
          const decoder = new TextDecoder();
          let buffer = '';

          while (true) {
            const { done, value } = await reader.read();
            if (done) break;

            buffer += decoder.decode(value, { stream: true });
            const lines = buffer.split('\n\n');
            buffer = lines.pop() || '';

            for (const line of lines) {
              if (line.trim() === '' || line.startsWith(':')) continue;
              const dataMatch = line.match(/^data: (.+)$/m);
              if (!dataMatch) continue;

              try {
                const message = JSON.parse(dataMatch[1]);

                if (message.type === 'chunk' && message.content) {
                  fullContent += message.content;
                  if (onProgress) {
                    onProgress(fullContent);
                  }
                } else if (message.type === 'progress' && onProgressUpdate) {
                  onProgressUpdate(message.message || '后台生成中...', message.progress || 0);
                } else if (message.type === 'chapter_start' && onProgressUpdate) {
                  onProgressUpdate(
                    `开始生成第${message.chapter_number || ''}章...`,
                    message.progress || 15
                  );
                } else if (message.type === 'analysis_started' && onProgressUpdate) {
                  onProgressUpdate(message.message || '章节分析中...', message.progress || 85);
                } else if (message.type === 'quality_metrics' && onProgressUpdate) {
                  const qualityMessage = formatQualityMessage(message);
                  if (qualityMessage) {
                    onProgressUpdate(qualityMessage, 92);
                  }
                } else if (message.type === 'error') {
                  streamFailure = message.error || '后台生成失败';
                }
              } catch (parseError) {
                console.error('解析任务SSE消息失败:', parseError);
              }
            }
          }
        } catch (streamError) {
          const err = streamError as Error;
          // 主动abort时会抛错，忽略
          if (err.name !== 'AbortError') {
            console.warn('任务SSE流异常，降级为轮询模式:', err.message);
          }
        }
      })();

      try {
        const maxPollCount = 900; // 最多轮询约30分钟（2秒一次）
        let pollCount = 0;

        while (pollCount < maxPollCount) {
          await new Promise((resolve) => setTimeout(resolve, 2000));
          pollCount += 1;

          if (streamFailure) {
            throw new Error(streamFailure);
          }

          const statusResponse = await fetch(`/api/chapters/batch-generate/${taskId}/status`);
          if (!statusResponse.ok) {
            throw new Error(`查询任务状态失败: ${statusResponse.status}`);
          }
          const taskStatus = await statusResponse.json();

          if (taskStatus.status === 'pending') {
            if (onProgressUpdate) {
              onProgressUpdate('后台排队中，可继续其他操作...', 15);
            }
            continue;
          }

          if (taskStatus.status === 'running') {
            if (onProgressUpdate) {
              const retrySuffix = taskStatus.current_retry_count ? `（重试${taskStatus.current_retry_count}）` : '';
              const qualityMessage = formatQualityMessage(taskStatus.latest_quality_metrics);
              if (qualityMessage) {
                onProgressUpdate(`${qualityMessage}｜后台生成中${retrySuffix}`, 70);
              } else {
                onProgressUpdate(`后台生成中${retrySuffix}，可并行执行其他任务...`, 65);
              }
            }
            continue;
          }

          if (taskStatus.status === 'failed') {
            throw new Error(taskStatus.error_message || '后台生成失败');
          }

          if (taskStatus.status === 'cancelled') {
            throw new Error('后台生成已取消');
          }

          if (taskStatus.status === 'completed') {
            if (onProgressUpdate) {
              onProgressUpdate('后台生成完成，正在同步内容...', 95);
            }

            streamAbortController.abort();
            await streamPromise;

            await refreshChapters();
            const latestChapter = await chapterApi.getChapter(chapterId);
            const finalContent = latestChapter.content || '';

            if (onProgress && finalContent !== fullContent) {
              onProgress(finalContent);
            }
            if (onProgressUpdate) {
              onProgressUpdate('生成完成', 100);
            }

            return {
              content: finalContent,
              analysis_task_id: undefined,
              generation_task_id: taskId
            };
          }
        }

        throw new Error('后台生成超时，请稍后查看章节内容');
      } finally {
        // 兜底清理，避免异常分支遗留流式连接
        streamAbortController.abort();
        await streamPromise.catch(() => undefined);
      }
    })();

    return {
      generation_task_id: taskId,
      analysis_task_id: undefined,
      completion
    };
  }, [refreshChapters]);

  return {
    refreshChapters,
    createChapter,
    updateChapter: updateChapterSync,
    deleteChapter,
    generateChapterContentStream,
  };
}
