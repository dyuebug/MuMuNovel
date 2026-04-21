import type { AuthUrlResponse, User } from '../../types';
import { api } from '../core/httpClient';

export const authApi = {
  getAuthConfig: () => api.get<unknown, { local_auth_enabled: boolean; linuxdo_enabled: boolean }>('/auth/config'),

  localLogin: (username: string, password: string) =>
    api.post<unknown, { success: boolean; message: string; user: User }>('/auth/local/login', { username, password }),

  bindAccountLogin: (username: string, password: string) =>
    api.post<unknown, { success: boolean; message: string; user: User }>('/auth/bind/login', { username, password }),

  getLinuxDOAuthUrl: () => api.get<unknown, AuthUrlResponse>('/auth/linuxdo/url'),

  getCurrentUser: () => api.get<unknown, User>('/auth/user'),

  getPasswordStatus: () => api.get<unknown, {
    has_password: boolean;
    has_custom_password: boolean;
    username: string | null;
    default_password: string | null;
  }>('/auth/password/status'),

  setPassword: (password: string) =>
    api.post<unknown, { success: boolean; message: string }>('/auth/password/set', { password }),

  initializePassword: (password: string) =>
    api.post<unknown, { success: boolean; message: string }>('/auth/password/initialize', { password }),

  refreshSession: () => api.post<unknown, { message: string; expire_at: number; remaining_minutes: number }>('/auth/refresh'),

  logout: () => api.post('/auth/logout'),
};
