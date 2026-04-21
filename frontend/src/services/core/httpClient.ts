import axios from 'axios';
import { message } from 'antd';

export interface RequestConfigWithToastControl {
  suppressErrorToast?: boolean;
  suppressErrorLog?: boolean;
  params?: Record<string, unknown>;
}

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

export const silentRequestConfig = <T extends RequestConfigWithToastControl>(config?: T): T =>
  ({ ...(config || {}), suppressErrorToast: true, suppressErrorLog: true } as T);

export const api = axios.create({
  baseURL: '/api',
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
    let errorMessage = 'Request failed';

    if (error.response) {
      const status = error.response.status;
      const data = error.response.data;

      switch (status) {
        case 400:
          errorMessage = data?.detail || 'Invalid request';
          break;
        case 401:
          errorMessage = 'Unauthorized, please login first';
          if (window.location.pathname !== '/login') {
            window.location.href = '/login';
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