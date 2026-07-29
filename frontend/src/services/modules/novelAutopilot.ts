import { api, silentRequestConfig } from '../core/httpClient';
import type {
  CreateNovelAutopilotRunRequest,
  CreateNovelAutopilotRunResponse,
  NovelAutopilotDecisionRequest,
  NovelAutopilotGuidanceRequest,
  NovelAutopilotRunListResponse,
  NovelAutopilotRunMutationResponse,
  NovelAutopilotRunResponse,
  NovelAutopilotStepListResponse,
  NovelAutopilotVersionedRequest,
} from '../../features/novel-autopilot/types';

const segment = (value: string) => encodeURIComponent(value);
const runBase = (projectId: string) => `/projects/${segment(projectId)}/novel-autopilot-runs`;
const runPath = (projectId: string, runId: string) => `${runBase(projectId)}/${segment(runId)}`;

export const novelAutopilotApi = {
  createRun: (projectId: string, request: CreateNovelAutopilotRunRequest) =>
    api.post<unknown, CreateNovelAutopilotRunResponse>(runBase(projectId), request),

  listRuns: (projectId: string) =>
    api.get<unknown, NovelAutopilotRunListResponse>(runBase(projectId), silentRequestConfig()),

  getRun: (projectId: string, runId: string) =>
    api.get<unknown, NovelAutopilotRunResponse>(runPath(projectId, runId), silentRequestConfig()),

  listSteps: (projectId: string, runId: string) =>
    api.get<unknown, NovelAutopilotStepListResponse>(
      `${runPath(projectId, runId)}/steps`,
      silentRequestConfig(),
    ),

  pauseRun: (projectId: string, runId: string, request: NovelAutopilotVersionedRequest) =>
    api.post<unknown, NovelAutopilotRunMutationResponse>(`${runPath(projectId, runId)}/pause`, request),

  resumeRun: (projectId: string, runId: string, request: NovelAutopilotVersionedRequest) =>
    api.post<unknown, NovelAutopilotRunMutationResponse>(`${runPath(projectId, runId)}/resume`, request),

  cancelRun: (projectId: string, runId: string, request: NovelAutopilotVersionedRequest) =>
    api.post<unknown, NovelAutopilotRunMutationResponse>(`${runPath(projectId, runId)}/cancel`, request),

  updateGuidance: (projectId: string, runId: string, request: NovelAutopilotGuidanceRequest) =>
    api.post<unknown, NovelAutopilotRunMutationResponse>(`${runPath(projectId, runId)}/guidance`, request),

  submitDecision: (projectId: string, runId: string, request: NovelAutopilotDecisionRequest) =>
    api.post<unknown, NovelAutopilotRunMutationResponse>(`${runPath(projectId, runId)}/decision`, request),
};
