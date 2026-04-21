import type { User } from '../../types';
import { api } from '../core/httpClient';

export const userApi = {
  getCurrentUser: () => api.get<unknown, User>('/users/current'),

  listUsers: () => api.get<unknown, User[]>('/users'),

  setAdmin: (userId: string, isAdmin: boolean) =>
    api.post('/users/set-admin', { user_id: userId, is_admin: isAdmin }),

  deleteUser: (userId: string) => api.delete(`/users/${userId}`),

  getUser: (userId: string) => api.get<unknown, User>(`/users/${userId}`),

  resetPassword: (userId: string, newPassword?: string) =>
    api.post<unknown, {
      message: string;
      user_id: string;
      username: string;
      default_password?: string;
    }>('/users/reset-password', { user_id: userId, new_password: newPassword }),
};
