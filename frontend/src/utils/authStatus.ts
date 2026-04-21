import { authApi } from '../services/modularApi';
import { sessionManager } from './sessionManager';
import { isServiceUnavailableError } from './httpError';

export interface AuthResolution {
  authenticated: boolean;
  serviceUnavailable: boolean;
}

const AUTH_STATUS_CACHE_MS = 10000;
const FAILURE_CACHE_MS = 2000;

let cachedAuthStatus: { value: AuthResolution; expiresAt: number } | null = null;
let authStatusPromise: Promise<AuthResolution> | null = null;

export const clearAuthStatusCache = () => {
  cachedAuthStatus = null;
  authStatusPromise = null;
};

export const resolveAuthStatus = async (): Promise<AuthResolution> => {
  const now = Date.now();
  if (cachedAuthStatus && cachedAuthStatus.expiresAt > now) {
    return cachedAuthStatus.value;
  }

  if (!authStatusPromise) {
    authStatusPromise = (async () => {
      try {
        await authApi.getCurrentUser();
        sessionManager.start();
        const nextState: AuthResolution = {
          authenticated: true,
          serviceUnavailable: false,
        };
        cachedAuthStatus = {
          value: nextState,
          expiresAt: Date.now() + AUTH_STATUS_CACHE_MS,
        };
        return nextState;
      } catch (error) {
        sessionManager.stop();
        const nextState: AuthResolution = {
          authenticated: false,
          serviceUnavailable: isServiceUnavailableError(error),
        };
        cachedAuthStatus = {
          value: nextState,
          expiresAt: Date.now() + FAILURE_CACHE_MS,
        };
        return nextState;
      } finally {
        authStatusPromise = null;
      }
    })();
  }

  return authStatusPromise;
};
