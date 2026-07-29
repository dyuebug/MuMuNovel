import { useBackgroundTaskStore } from './backgroundTasks';
import { chapterApi, chapterSingleTaskApi } from '../services/modularApi';
import type {
  CreativeMode,
  PlotStage,
  QualityPreset,
  StoryFocus,
} from '../types';
import { formatActiveStoryRepairLabel } from '../utils/activeStoryRepair';
import { MAX_CONSECUTIVE_TASK_POLL_ERRORS } from '../utils/taskPolling';

export interface GenerateChapterContentStreamOptions {
  chapterId: string;
  projectId?: string;
  refreshChapters: () => Promise<unknown>;
  onProgress?: (content: string) => void;
  onChunk?: (content: string) => void;
  onReasoningChunk?: (content: string) => void;
  styleId?: number;
  targetWordCount?: number;
  onProgressUpdate?: (message: string, progress: number) => void;
  model?: string;
  narrativePerspective?: string;
  creativeMode?: CreativeMode;
  storyFocus?: StoryFocus;
  plotStage?: PlotStage;
  storyCreationBrief?: string;
  qualityPreset?: QualityPreset;
  qualityNotes?: string;
  storyRepairSummary?: string;
  storyRepairTargets?: string[];
  storyPreserveStrengths?: string[];
}

export interface GenerateChapterContentCompletionResult {
  content: string;
  content_source: 'chapter' | 'candidate_draft';
  analysis_task_id?: string;
  generation_task_id: string;
}

export interface GenerateChapterContentStreamResult {
  generation_task_id: string;
  analysis_task_id: undefined;
  completion: Promise<GenerateChapterContentCompletionResult>;
}

type IdleCallbackWindow = Window & typeof globalThis & {
  requestIdleCallback?: (callback: IdleRequestCallback, options?: IdleRequestOptions) => number;
};

const NON_URGENT_CHAPTER_REFRESH_DELAY_MS = 96;

const scheduleNonUrgentChapterRefresh = (callback: () => void) => {
  const windowWithIdleCallback = window as IdleCallbackWindow;
  if (typeof windowWithIdleCallback.requestIdleCallback === 'function') {
    windowWithIdleCallback.requestIdleCallback(() => {
      callback();
    }, { timeout: 400 });
    return;
  }

  window.setTimeout(callback, NON_URGENT_CHAPTER_REFRESH_DELAY_MS);
};

const formatQualityMessage = (metrics: {
  overall_score?: unknown;
  conflict_chain_hit_rate?: unknown;
  rule_grounding_hit_rate?: unknown;
} | null | undefined): string | null => {
  if (!metrics) return null;
  const overall = Number(metrics.overall_score ?? 0).toFixed(1);
  const conflict = Number(metrics.conflict_chain_hit_rate ?? 0).toFixed(1);
  const rule = Number(metrics.rule_grounding_hit_rate ?? 0).toFixed(1);
  return `Score ${overall} | Conflict ${conflict}% | Grounding ${rule}%`;
};

const resolveCandidateDraftContent = async (
  chapterId: string,
  latestCandidateDraftSummary: Record<string, unknown> | null,
): Promise<{ content: string; source: 'chapter' | 'candidate_draft' }> => {
  const latestChapter = await chapterApi.getChapter(chapterId);
  const latestContent = typeof latestChapter.content === 'string' ? latestChapter.content : '';

  if (latestContent.trim()) {
    return {
      content: latestContent,
      source: 'chapter',
    };
  }

  const rawCandidateFullContent = latestCandidateDraftSummary?.content;
  const rawCandidatePreviewContent = latestCandidateDraftSummary?.content_preview;
  const candidateFullContent = typeof rawCandidateFullContent === 'string'
    ? rawCandidateFullContent.trim()
    : '';
  const candidatePreviewContent = typeof rawCandidatePreviewContent === 'string'
    ? rawCandidatePreviewContent.trim()
    : '';
  const sseFallbackContent = candidateFullContent || candidatePreviewContent;

  if (sseFallbackContent) {
    return {
      content: sseFallbackContent,
      source: 'candidate_draft',
    };
  }

  try {
    const candidateDraftResponse = await chapterApi.getCandidateDraft(chapterId);
    const candidateDraft = candidateDraftResponse?.candidate_draft;
    const candidateDraftFullContent = typeof candidateDraft?.content === 'string'
      ? candidateDraft.content.trim()
      : '';
    const candidateDraftPreviewContent = typeof candidateDraft?.content_preview === 'string'
      ? candidateDraft.content_preview.trim()
      : '';
    const fallbackContent = candidateDraftFullContent || candidateDraftPreviewContent;

    if (fallbackContent) {
      return {
        content: fallbackContent,
        source: 'candidate_draft',
      };
    }
  } catch (candidateDraftError) {
    console.warn('Failed to load candidate draft fallback:', candidateDraftError);
  }

  return {
    content: latestContent,
    source: 'chapter',
  };
};

export async function startChapterGenerationWorkflow({
  chapterId,
  projectId,
  refreshChapters,
  onProgress,
  onChunk,
  onReasoningChunk,
  styleId,
  targetWordCount,
  onProgressUpdate,
  model,
  narrativePerspective,
  creativeMode,
  storyFocus,
  plotStage,
  storyCreationBrief,
  qualityPreset,
  qualityNotes,
  storyRepairSummary,
  storyRepairTargets,
  storyPreserveStrengths,
}: GenerateChapterContentStreamOptions): Promise<GenerateChapterContentStreamResult> {
  const resolveTaskProgress = (
    taskStatus: Awaited<ReturnType<typeof chapterSingleTaskApi.getSingleGenerateTaskStatus>>,
    fallback: number,
  ): number => {
    const checkpointProgress = taskStatus.checkpoint?.progress;
    if (typeof checkpointProgress === 'number' && Number.isFinite(checkpointProgress)) {
      return Math.max(0, Math.min(Math.round(checkpointProgress), 100));
    }
    return fallback;
  };

  const startResult = await chapterSingleTaskApi.createSingleGenerateTask(
    chapterId,
    {
      style_id: styleId,
      target_word_count: targetWordCount,
      model,
      narrative_perspective: narrativePerspective,
      creative_mode: creativeMode,
      story_focus: storyFocus,
      plot_stage: plotStage,
      story_creation_brief: storyCreationBrief,
      quality_preset: qualityPreset,
      quality_notes: qualityNotes,
      story_repair_summary: storyRepairSummary,
      story_repair_targets: storyRepairTargets,
      story_preserve_strengths: storyPreserveStrengths,
    },
    projectId,
  );

  const taskId = startResult.task_id;
  if (!taskId) {
    throw new Error('Missing generation task_id in response');
  }

  onProgressUpdate?.(startResult.message || 'Generation task started.', 5);

  const completion = (async (): Promise<GenerateChapterContentCompletionResult> => {
    let fullContent = '';
    let streamFailure: string | null = null;
    let latestAnalysisTaskId: string | undefined;
    let latestCandidateDraftSummary: Record<string, unknown> | null = null;
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

              if (message.type === 'chunk' && typeof message.content === 'string' && message.content) {
                onChunk?.(message.content);
                fullContent += message.content;
                onProgress?.(fullContent);
              } else if (message.type === 'reasoning_chunk' && typeof message.content === 'string' && message.content) {
                onReasoningChunk?.(message.content);
              } else if (message.type === 'progress') {
                onProgressUpdate?.(message.message || 'Generating chapter...', message.progress || 0);
              } else if (message.type === 'chapter_start') {
                onProgressUpdate?.(
                  `Generating chapter ${message.chapter_number || ''}...`,
                  message.progress || 15,
                );
              } else if (message.type === 'analysis_started') {
                if (typeof message.task_id === 'string' && message.task_id.trim()) {
                  latestAnalysisTaskId = message.task_id.trim();
                }
                onProgressUpdate?.(message.message || 'Starting quality analysis...', message.progress || 85);
              } else if (message.type === 'quality_metrics') {
                const qualityMessage = formatQualityMessage(message);
                if (qualityMessage) {
                  onProgressUpdate?.(qualityMessage, 92);
                }
              } else if (message.type === 'quality_gate_retry' || message.type === 'quality_gate_blocked') {
                onProgressUpdate?.(
                  message.message
                    || (message.type === 'quality_gate_blocked'
                      ? 'Quality gate blocked the current draft.'
                      : 'Retrying generation after quality gate feedback.'),
                  message.progress || (message.type === 'quality_gate_blocked' ? 95 : 74),
                );
              } else if (message.type === 'result') {
                const resultPayload = typeof message.data === 'object' && message.data !== null
                  ? message.data as Record<string, unknown>
                  : message as Record<string, unknown>;
                if (typeof resultPayload.analysis_task_id === 'string' && resultPayload.analysis_task_id.trim()) {
                  latestAnalysisTaskId = resultPayload.analysis_task_id.trim();
                }
                if (typeof resultPayload.candidate_draft === 'object' && resultPayload.candidate_draft !== null) {
                  latestCandidateDraftSummary = resultPayload.candidate_draft as Record<string, unknown>;
                }
              } else if (message.type === 'error') {
                streamFailure = message.error || 'Generation stream failed.';
              }
            } catch (parseError) {
              console.error('Failed to parse chapter generation SSE message:', parseError);
            }
          }
        }
      } catch (streamError) {
        const error = streamError as Error;
        if (error.name !== 'AbortError') {
          console.warn('Chapter generation SSE stream ended unexpectedly:', error.message);
        }
      }
    })();

    try {
      const maxPollCount = 900;
      let pollCount = 0;
      let consecutivePollErrors = 0;

      while (pollCount < maxPollCount) {
        await new Promise((resolve) => setTimeout(resolve, 2000));
        pollCount += 1;

        if (streamFailure) {
          throw new Error(streamFailure);
        }

        const trackedTask = useBackgroundTaskStore.getState().tasks[taskId];
        if (trackedTask?.taskType === 'chapter_single_generate' && trackedTask.status === 'cancelled') {
          throw new Error(trackedTask.error || trackedTask.message || 'Generation task was cancelled.');
        }

        let taskStatus: Awaited<ReturnType<typeof chapterSingleTaskApi.getSingleGenerateTaskStatus>>;
        try {
          taskStatus = await chapterSingleTaskApi.getSingleGenerateTaskStatus(taskId, projectId);
          consecutivePollErrors = 0;
        } catch (pollError) {
          consecutivePollErrors += 1;
          if (consecutivePollErrors < MAX_CONSECUTIVE_TASK_POLL_ERRORS) {
            console.warn('Polling chapter generation task failed; retrying.', pollError);
            continue;
          }
          throw pollError;
        }

        const activeRepairStrategyLabel = formatActiveStoryRepairLabel(taskStatus.active_story_repair_payload);

        if (taskStatus.status === 'pending') {
          onProgressUpdate?.(
            `Waiting for chapter generation to start...${activeRepairStrategyLabel ? ` | ${activeRepairStrategyLabel}` : ''}`,
            resolveTaskProgress(taskStatus, 15),
          );
          continue;
        }

        if (taskStatus.status === 'running') {
          const retrySuffix = taskStatus.current_retry_count
            ? ` | retry ${taskStatus.current_retry_count}`
            : '';
          const qualityMessage = formatQualityMessage(taskStatus.latest_quality_metrics);
          if (qualityMessage) {
            onProgressUpdate?.(
              `${qualityMessage} | Quality review in progress${retrySuffix}${activeRepairStrategyLabel ? ` | ${activeRepairStrategyLabel}` : ''}`,
              resolveTaskProgress(taskStatus, 70),
            );
          } else {
            onProgressUpdate?.(
              `Generating content...${retrySuffix}${activeRepairStrategyLabel ? ` | ${activeRepairStrategyLabel}` : ''}`,
              resolveTaskProgress(taskStatus, 65),
            );
          }
          continue;
        }

        if (taskStatus.status === 'failed') {
          throw new Error(taskStatus.error_message || 'Generation task failed.');
        }

        if (taskStatus.status === 'cancelled') {
          throw new Error('Generation task was cancelled.');
        }

        if (taskStatus.status === 'completed') {
          onProgressUpdate?.('Finalizing chapter content...', 95);

          streamAbortController.abort();
          await streamPromise;

          scheduleNonUrgentChapterRefresh(() => {
            void refreshChapters().catch((refreshError) => {
              console.error('Failed to refresh chapters after single chapter generation.', refreshError);
            });
          });
          const { content: finalContent, source: contentSource } = await resolveCandidateDraftContent(
            chapterId,
            latestCandidateDraftSummary,
          );

          if (onProgress && finalContent !== fullContent) {
            onProgress(finalContent);
          }

          onProgressUpdate?.(
            contentSource === 'candidate_draft'
              ? 'Generation completed. Candidate draft is ready for review.'
              : 'Generation completed.',
            100,
          );

          return {
            content: finalContent,
            content_source: contentSource,
            analysis_task_id: latestAnalysisTaskId,
            generation_task_id: taskId,
          };
        }
      }

      throw new Error('Chapter generation timed out while waiting for completion.');
    } finally {
      streamAbortController.abort();
      await streamPromise.catch(() => undefined);
    }
  })();

  return {
    generation_task_id: taskId,
    analysis_task_id: undefined,
    completion,
  };
}
