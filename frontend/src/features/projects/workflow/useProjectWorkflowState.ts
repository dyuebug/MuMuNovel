import { useCallback, useEffect, useRef, useState } from 'react';

import { projectApi } from '../../../services/modularApi';
import {
  getAxiosErrorStatus,
  isRequestCancelledError,
  silentRequestConfig,
} from '../../../services/core/httpClient';
import { useStore } from '../../../store';
import type {
  NovelWorkflowPhase,
  NovelWorkflowStateView,
  NovelWorkflowTransitionReceipt,
} from '../../../types';

type ApiLikeError = {
  response?: {
    data?: {
      detail?: string;
    };
  };
  message?: string;
};

export type ProjectWorkflowTransitionOutcome =
  | {
      status: 'success';
      receipt: NovelWorkflowTransitionReceipt;
    }
  | {
      status: 'conflict';
      state: NovelWorkflowStateView | null;
      message: string;
    }
  | {
      status: 'error';
      message: string;
    };

export interface ProjectWorkflowTransitionOptions {
  reason?: string;
  relatedTaskId?: string;
}

const getApiErrorMessage = (error: unknown, fallbackMessage: string): string => {
  const apiError = error as ApiLikeError;
  return apiError.response?.data?.detail || apiError.message || fallbackMessage;
};

export const useProjectWorkflowState = (projectId: string) => {
  const updateProject = useStore((store) => store.updateProject);
  const [state, setState] = useState<NovelWorkflowStateView | null>(null);
  const [loading, setLoading] = useState(true);
  const [transitioning, setTransitioning] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const loadAbortRef = useRef<AbortController | null>(null);
  const requestSequenceRef = useRef(0);

  const refresh = useCallback(async (): Promise<NovelWorkflowStateView | null> => {
    loadAbortRef.current?.abort();
    const controller = new AbortController();
    const requestSequence = ++requestSequenceRef.current;
    loadAbortRef.current = controller;
    setLoading(true);
    setError(null);

    try {
      const nextState = await projectApi.getWorkflowState(
        projectId,
        silentRequestConfig({ signal: controller.signal }),
      );
      if (requestSequence !== requestSequenceRef.current) {
        return null;
      }

      setState(nextState);
      updateProject(projectId, { status: nextState.phase });
      return nextState;
    } catch (requestError) {
      if (isRequestCancelledError(requestError) || requestSequence !== requestSequenceRef.current) {
        return null;
      }

      const message = getApiErrorMessage(requestError, '获取创作阶段失败');
      setError(message);
      return null;
    } finally {
      if (requestSequence === requestSequenceRef.current) {
        setLoading(false);
      }
      if (loadAbortRef.current === controller) {
        loadAbortRef.current = null;
      }
    }
  }, [projectId, updateProject]);

  useEffect(() => {
    setState(null);
    setError(null);
    void refresh();

    return () => {
      requestSequenceRef.current += 1;
      loadAbortRef.current?.abort();
      loadAbortRef.current = null;
    };
  }, [refresh]);

  const transition = useCallback(async (
    targetPhase: NovelWorkflowPhase,
    options: ProjectWorkflowTransitionOptions = {},
  ): Promise<ProjectWorkflowTransitionOutcome> => {
    if (!state) {
      return { status: 'error', message: '创作阶段尚未加载，请稍后重试' };
    }

    setTransitioning(true);
    try {
      const receipt = await projectApi.transitionWorkflowState(
        projectId,
        {
          target_phase: targetPhase,
          expected_phase: state.phase,
          ...(options.reason?.trim() ? { reason: options.reason.trim() } : {}),
          ...(options.relatedTaskId?.trim()
            ? { related_task_id: options.relatedTaskId.trim() }
            : {}),
        },
        silentRequestConfig(),
      );
      setState(receipt.state);
      setError(null);
      updateProject(projectId, { status: receipt.state.phase });
      return { status: 'success', receipt };
    } catch (transitionError) {
      if (getAxiosErrorStatus(transitionError) === 409) {
        const latestState = await refresh();
        return {
          status: 'conflict',
          state: latestState,
          message: '创作阶段已被其他操作更新，已刷新最新状态',
        };
      }

      return {
        status: 'error',
        message: getApiErrorMessage(transitionError, '切换创作阶段失败'),
      };
    } finally {
      setTransitioning(false);
    }
  }, [projectId, refresh, state, updateProject]);

  return {
    state,
    loading,
    transitioning,
    error,
    refresh,
    transition,
  };
};
