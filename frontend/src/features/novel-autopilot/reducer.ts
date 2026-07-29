import type { NovelAutopilotRun, NovelAutopilotStepRun } from './types';

export interface NovelAutopilotWorkbenchState {
  run: NovelAutopilotRun | null;
  steps: NovelAutopilotStepRun[];
  loading: boolean;
  refreshing: boolean;
  mutating: boolean;
  error: string | null;
}

export type NovelAutopilotWorkbenchAction =
  | { type: 'load_started' }
  | { type: 'load_succeeded'; run: NovelAutopilotRun | null; steps: NovelAutopilotStepRun[] }
  | { type: 'load_failed'; error: string }
  | { type: 'refresh_started' }
  | { type: 'refresh_succeeded'; run: NovelAutopilotRun; steps: NovelAutopilotStepRun[] }
  | { type: 'refresh_failed'; error: string }
  | { type: 'mutation_started' }
  | { type: 'mutation_succeeded'; run: NovelAutopilotRun }
  | { type: 'mutation_failed'; error: string }
  | { type: 'reset' };

export const initialNovelAutopilotWorkbenchState: NovelAutopilotWorkbenchState = {
  run: null,
  steps: [],
  loading: true,
  refreshing: false,
  mutating: false,
  error: null,
};

const assertNever = (value: never): never => {
  throw new Error(`Unhandled novel autopilot action: ${JSON.stringify(value)}`);
};

export const novelAutopilotWorkbenchReducer = (
  state: NovelAutopilotWorkbenchState,
  action: NovelAutopilotWorkbenchAction,
): NovelAutopilotWorkbenchState => {
  switch (action.type) {
    case 'load_started':
      return { ...state, loading: true, error: null };
    case 'load_succeeded':
      return {
        ...state,
        run: action.run,
        steps: action.steps,
        loading: false,
        refreshing: false,
        error: null,
      };
    case 'load_failed':
      return { ...state, loading: false, refreshing: false, error: action.error };
    case 'refresh_started':
      return { ...state, refreshing: true };
    case 'refresh_succeeded':
      return {
        ...state,
        run: action.run,
        steps: action.steps,
        refreshing: false,
        error: null,
      };
    case 'refresh_failed':
      return { ...state, refreshing: false, error: action.error };
    case 'mutation_started':
      return { ...state, mutating: true, error: null };
    case 'mutation_succeeded':
      return { ...state, run: action.run, mutating: false, error: null };
    case 'mutation_failed':
      return { ...state, mutating: false, error: action.error };
    case 'reset':
      return initialNovelAutopilotWorkbenchState;
    default:
      return assertNever(action);
  }
};
