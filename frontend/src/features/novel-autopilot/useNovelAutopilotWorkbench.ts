import { useCallback, useEffect, useReducer, useRef } from 'react';

import { novelAutopilotApi } from '../../services/modules/novelAutopilot';
import { isRequestCancelledError } from '../../services/core/httpClient';
import {
  initialNovelAutopilotWorkbenchState,
  novelAutopilotWorkbenchReducer,
} from './reducer';
import type {
  CreateNovelAutopilotRunRequest,
  NovelAutopilotHumanDecision,
  NovelAutopilotRun,
} from './types';
import { isNovelAutopilotRunTerminal } from './types';

const ACTIVE_POLL_INTERVAL_MS = 2_000;
const IDLE_POLL_INTERVAL_MS = 10_000;

const errorMessage = (error: unknown, fallback: string) => {
  if (typeof error !== 'object' || error === null) {
    return fallback;
  }
  const candidate = error as {
    response?: { data?: { detail?: unknown } };
    message?: unknown;
  };
  const detail = candidate.response?.data?.detail;
  if (typeof detail === 'string' && detail.trim()) {
    return detail;
  }
  return typeof candidate.message === 'string' && candidate.message.trim()
    ? candidate.message
    : fallback;
};

const selectLatestRun = (runs: NovelAutopilotRun[]) => (
  runs.find((run) => !isNovelAutopilotRunTerminal(run.status)) ?? runs[0] ?? null
);

export const useNovelAutopilotWorkbench = (projectId: string) => {
  const [state, dispatch] = useReducer(
    novelAutopilotWorkbenchReducer,
    initialNovelAutopilotWorkbenchState,
  );
  const mountedRef = useRef(true);
  const refreshInFlightRef = useRef(false);

  useEffect(() => {
    mountedRef.current = true;
    return () => {
      mountedRef.current = false;
    };
  }, []);

  const load = useCallback(async () => {
    dispatch({ type: 'load_started' });
    try {
      const response = await novelAutopilotApi.listRuns(projectId);
      const run = selectLatestRun(response.items);
      const steps = run
        ? (await novelAutopilotApi.listSteps(projectId, run.id)).items
        : [];
      if (mountedRef.current) {
        dispatch({ type: 'load_succeeded', run, steps });
      }
    } catch (error: unknown) {
      if (mountedRef.current && !isRequestCancelledError(error)) {
        dispatch({ type: 'load_failed', error: errorMessage(error, '加载自动创作运行状态失败') });
      }
    }
  }, [projectId]);

  const refresh = useCallback(async (showRefreshing = true) => {
    const runId = state.run?.id;
    if (!runId || refreshInFlightRef.current) {
      return;
    }
    refreshInFlightRef.current = true;
    if (showRefreshing) {
      dispatch({ type: 'refresh_started' });
    }
    try {
      const [runResponse, stepResponse] = await Promise.all([
        novelAutopilotApi.getRun(projectId, runId),
        novelAutopilotApi.listSteps(projectId, runId),
      ]);
      if (mountedRef.current) {
        dispatch({
          type: 'refresh_succeeded',
          run: runResponse.run,
          steps: stepResponse.items,
        });
      }
    } catch (error: unknown) {
      if (mountedRef.current && !isRequestCancelledError(error)) {
        dispatch({ type: 'refresh_failed', error: errorMessage(error, '刷新自动创作状态失败') });
      }
    } finally {
      refreshInFlightRef.current = false;
    }
  }, [projectId, state.run?.id]);

  useEffect(() => {
    dispatch({ type: 'reset' });
    void load();
  }, [load]);

  useEffect(() => {
    const run = state.run;
    if (!run) {
      return;
    }
    const interval = isNovelAutopilotRunTerminal(run.status)
      ? IDLE_POLL_INTERVAL_MS
      : ACTIVE_POLL_INTERVAL_MS;
    const timer = window.setInterval(() => {
      void refresh(false);
    }, interval);
    return () => window.clearInterval(timer);
  }, [refresh, state.run]);

  const commitMutation = useCallback(async (
    operation: () => Promise<{ run: NovelAutopilotRun }>,
    fallback: string,
  ) => {
    dispatch({ type: 'mutation_started' });
    try {
      const response = await operation();
      if (mountedRef.current) {
        dispatch({ type: 'mutation_succeeded', run: response.run });
      }
      await refresh(false);
      return response.run;
    } catch (error: unknown) {
      const message = errorMessage(error, fallback);
      if (mountedRef.current) {
        dispatch({ type: 'mutation_failed', error: message });
      }
      throw error;
    }
  }, [refresh]);

  const createRun = useCallback(async (request: CreateNovelAutopilotRunRequest) => {
    dispatch({ type: 'mutation_started' });
    try {
      const response = await novelAutopilotApi.createRun(projectId, request);
      const steps = (await novelAutopilotApi.listSteps(projectId, response.run.id)).items;
      if (mountedRef.current) {
        dispatch({ type: 'load_succeeded', run: response.run, steps });
      }
      return response;
    } catch (error: unknown) {
      const message = errorMessage(error, '创建自动创作任务失败');
      if (mountedRef.current) {
        dispatch({ type: 'mutation_failed', error: message });
      }
      throw error;
    }
  }, [projectId]);

  const requireRun = useCallback(() => {
    if (!state.run) {
      throw new Error('当前没有可控制的自动创作 Run');
    }
    return state.run;
  }, [state.run]);

  const pause = useCallback(() => {
    const run = requireRun();
    return commitMutation(
      () => novelAutopilotApi.pauseRun(projectId, run.id, { expected_version: run.version }),
      '暂停自动创作失败',
    );
  }, [commitMutation, projectId, requireRun]);

  const resume = useCallback(() => {
    const run = requireRun();
    return commitMutation(
      () => novelAutopilotApi.resumeRun(projectId, run.id, { expected_version: run.version }),
      '恢复自动创作失败',
    );
  }, [commitMutation, projectId, requireRun]);

  const cancel = useCallback(() => {
    const run = requireRun();
    return commitMutation(
      () => novelAutopilotApi.cancelRun(projectId, run.id, { expected_version: run.version }),
      '取消自动创作失败',
    );
  }, [commitMutation, projectId, requireRun]);

  const updateGuidance = useCallback((guidance: string) => {
    const run = requireRun();
    return commitMutation(
      () => novelAutopilotApi.updateGuidance(projectId, run.id, {
        expected_version: run.version,
        guidance,
      }),
      '更新后续指导失败',
    );
  }, [commitMutation, projectId, requireRun]);

  const submitDecision = useCallback((decision: NovelAutopilotHumanDecision, guidance?: string) => {
    const run = requireRun();
    return commitMutation(
      () => novelAutopilotApi.submitDecision(projectId, run.id, {
        expected_version: run.version,
        decision,
        ...(guidance?.trim() ? { guidance: guidance.trim() } : {}),
      }),
      '提交人工决定失败',
    );
  }, [commitMutation, projectId, requireRun]);

  return {
    state,
    load,
    refresh: () => refresh(true),
    createRun,
    pause,
    resume,
    cancel,
    updateGuidance,
    submitDecision,
  };
};
