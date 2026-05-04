import axios, { type AxiosRequestConfig } from 'axios';
import { message } from 'antd';
import { buildLoginUrlFromLocation } from '../../utils/loginRedirect';

const LOCAL_HOSTNAMES = new Set(['localhost', '127.0.0.1', '0.0.0.0']);

const trimTrailingSlashes = (value: string) => value.replace(/\/+$/, '');

const ensureApiSuffix = (value: string) => {
  const normalized = trimTrailingSlashes(value.trim());
  if (!normalized) {
    return '';
  }

  return normalized.endsWith('/api') ? normalized : `${normalized}/api`;
};

const stripApiSuffix = (value: string) => value.replace(/\/api$/i, '');

const resolveApiBaseUrl = () => {
  const explicitApiBaseUrl = ensureApiSuffix(import.meta.env.VITE_API_BASE_URL || '');
  if (explicitApiBaseUrl) {
    return explicitApiBaseUrl;
  }

  const fallbackApiBaseUrl = ensureApiSuffix(import.meta.env.VITE_API_FALLBACK_ORIGIN || '');
  if (!fallbackApiBaseUrl || typeof window === 'undefined') {
    return '/api';
  }

  const { protocol, hostname, origin } = window.location;
  if (protocol === 'file:') {
    return fallbackApiBaseUrl;
  }

  if (!LOCAL_HOSTNAMES.has(hostname)) {
    return '/api';
  }

  const fallbackOrigin = stripApiSuffix(fallbackApiBaseUrl);
  return origin === fallbackOrigin ? '/api' : fallbackApiBaseUrl;
};

export interface RequestConfigWithToastControl extends AxiosRequestConfig {
  suppressErrorToast?: boolean;
  suppressErrorLog?: boolean;
  suppressAuthRedirect?: boolean;
}

/** 构建抑制 401 自动重定向的请求配置 */
export const noAuthRedirectConfig = (base?: AxiosRequestConfig): RequestConfigWithToastControl =>
  ({ ...(base || {}), suppressAuthRedirect: true });

const ERROR_TOAST_THROTTLE_MS = 3000;
const ERROR_TOAST_CACHE_RETENTION_MS = 120000;
const errorToastTimestamps = new Map<string, number>();

const pruneErrorToastCache = (now: number) => {
  for (const [messageText, timestamp] of errorToastTimestamps.entries()) {
    if (now - timestamp > ERROR_TOAST_CACHE_RETENTION_MS) {
      errorToastTimestamps.delete(messageText);
    }
  }
};

const showErrorToastWithThrottle = (errorMessage: string) => {
  const now = Date.now();
  const lastTimestamp = errorToastTimestamps.get(errorMessage);
  if (lastTimestamp && now - lastTimestamp < ERROR_TOAST_THROTTLE_MS) {
    return;
  }

  errorToastTimestamps.set(errorMessage, now);
  pruneErrorToastCache(now);
  message.error(errorMessage);
};

export const silentRequestConfig = <T extends RequestConfigWithToastControl = RequestConfigWithToastControl>(config?: T): T =>
  ({ ...(config || {}), suppressErrorToast: true, suppressErrorLog: true } as T);

// 防止 401 重定向循环的标志位
let isRedirectingToLogin = false;

export const api = axios.create({
  baseURL: resolveApiBaseUrl(),
  timeout: 120000,
  headers: {
    'Content-Type': 'application/json',
  },
  withCredentials: true,
});

export const getAxiosErrorStatus = (error: unknown): number | null => {
  if (!axios.isAxiosError(error)) {
    return null;
  }

  return error.response?.status ?? null;
};
export const isRequestCancelledError = (error: unknown): boolean => {
  if (axios.isCancel(error)) {
    return true;
  }

  if (typeof error !== 'object' || error === null) {
    return false;
  }

  const candidate = error as {
    code?: unknown;
    message?: unknown;
    name?: unknown;
    config?: { signal?: AbortSignal | null };
  };

  const errorCode = typeof candidate.code === 'string' ? candidate.code.toUpperCase() : '';
  const errorMessage = typeof candidate.message === 'string' ? candidate.message.toLowerCase() : '';
  const errorName = typeof candidate.name === 'string' ? candidate.name : '';
  const isSignalAborted = Boolean(candidate.config?.signal?.aborted);

  return (
    errorCode === 'ERR_CANCELED'
    || errorName === 'AbortError'
    || isSignalAborted
    || /\babort(ed)?\b/.test(errorMessage)
    || /\bcancel(l)?ed\b/.test(errorMessage)
  );
};

api.interceptors.request.use(
  (config) => {
    return config;
  },
  (error) => {
    return Promise.reject(error);
  }
);

api.interceptors.response.use(
  (response) => {
    return response.data;
  },
  (error) => {
    const requestConfig = (error?.config || {}) as RequestConfigWithToastControl;
    const suppressErrorToast = Boolean(requestConfig.suppressErrorToast);
    const suppressErrorLog = Boolean(requestConfig.suppressErrorLog);

    if (isRequestCancelledError(error)) {
      return Promise.reject(error);
    }

    let errorMessage = 'Request failed';

    if (error.response) {
      const status = error.response.status;
      const data = error.response.data;

      const requestUrl = typeof error.config?.url === 'string' ? error.config.url : '';
      const pathname = typeof window !== 'undefined' ? window.location.pathname : '';
      const isAuthRequest = requestUrl.includes('/auth/');
      const isAuthPage = pathname === '/login' || pathname.startsWith('/auth/callback');
      const suppressAuthRedirect = Boolean(requestConfig.suppressAuthRedirect);

      switch (status) {
        case 400:
          errorMessage = data?.detail || 'Invalid request';
          break;
        case 401:
          errorMessage = data?.detail || data?.message || 'Unauthorized, please login first';
          if (!isAuthRequest && !isAuthPage && !suppressAuthRedirect && !isRedirectingToLogin && typeof window !== 'undefined') {
            const loginUrl = buildLoginUrlFromLocation(window.location);
            if (window.location.href !== loginUrl && window.location.pathname + window.location.search + window.location.hash !== loginUrl) {
              isRedirectingToLogin = true;
              window.location.assign(loginUrl);
            }
          }
          break;
        case 403:
          errorMessage = 'Permission denied';
          break;
        case 404:
          errorMessage = data?.detail || 'Resource not found';
          break;
        case 422:
          errorMessage = data?.detail || 'Validation failed';
          if (data?.errors) {
            console.error('Validation errors:', data.errors);
          }
          break;
        case 429:
          errorMessage = data?.detail || 'Too many requests';
          break;
        case 500:
          errorMessage = data?.detail || 'Internal server error';
          break;
        case 503:
          errorMessage = data?.detail || 'Service temporarily unavailable';
          break;
        default:
          errorMessage = data?.detail || data?.message || `Request failed (${status})`;
      }
    } else if (error.request) {
      const errorCode = typeof error.code === 'string' ? error.code : '';
      const rawMessage = typeof error.message === 'string' ? error.message : '';
      const isOffline = typeof navigator !== 'undefined' && navigator.onLine === false;
      const isTimeout = errorCode === 'ECONNABORTED' || /timeout/i.test(rawMessage);

      if (isOffline) {
        errorMessage = 'You are offline';
      } else if (isTimeout) {
        errorMessage = 'Request timed out';
      } else {
        errorMessage = 'Network request failed';
      }
    } else {
      errorMessage = error.message || 'Request failed';
    }

    if (typeof error === 'object' && error !== null) {
      error.message = errorMessage;
    }

    if (!suppressErrorToast) {
      showErrorToastWithThrottle(errorMessage);
    }
    if (!suppressErrorLog) {
      console.error('API Error:', errorMessage, error);
    }

    return Promise.reject(error);
  }
);

export default api;
