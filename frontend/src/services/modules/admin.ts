import type { User } from '../../types';
import { api } from '../core/httpClient';

export const adminApi = {
  getUsers: () =>
    api.get<unknown, { total: number; users: User[] }>('/admin/users'),

  createUser: (data: {
    username: string;
    display_name: string;
    password?: string;
    avatar_url?: string;
    trust_level?: number;
    is_admin?: boolean;
  }) =>
    api.post<unknown, {
      success: boolean;
      message: string;
      user: User;
      default_password?: string;
    }>('/admin/users', data),

  updateUser: (userId: string, data: {
    display_name?: string;
    avatar_url?: string;
    trust_level?: number;
  }) =>
    api.put<unknown, {
      success: boolean;
      message: string;
      user: User;
    }>(`/admin/users/${userId}`, data),

  toggleUserStatus: (userId: string, isActive: boolean) =>
    api.post<unknown, {
      success: boolean;
      message: string;
      is_active: boolean;
    }>(`/admin/users/${userId}/toggle-status`, { is_active: isActive }),

  resetPassword: (userId: string, newPassword?: string) =>
    api.post<unknown, {
      success: boolean;
      message: string;
      new_password: string;
    }>(`/admin/users/${userId}/reset-password`, { new_password: newPassword }),

  deleteUser: (userId: string) =>
    api.delete<unknown, {
      success: boolean;
      message: string;
    }>(`/admin/users/${userId}`),
};