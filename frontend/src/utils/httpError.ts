import axios from 'axios';

export const getHttpStatus = (error: unknown): number | null => {
  if (!axios.isAxiosError(error)) {
    return null;
  }

  return error.response?.status ?? null;
};

export const isServiceUnavailableError = (error: unknown): boolean =>
  getHttpStatus(error) === 503;
