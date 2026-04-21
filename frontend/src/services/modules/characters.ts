import axios from 'axios';

import type {
  Character,
  CharacterUpdate,
  GenerateCharacterRequest,
} from '../../types';
import { api } from '../core/httpClient';

export const characterApi = {
  getCharacters: (projectId: string) =>
    api.get<unknown, { total: number; items: Character[] }>(`/characters/project/${projectId}`).then(res => res.items),

  getCharacter: (id: string) => api.get<unknown, Character>(`/characters/${id}`),

  createCharacter: (data: {
    project_id: string;
    name: string;
    age?: string;
    gender?: string;
    is_organization?: boolean;
    role_type?: string;
    personality?: string;
    background?: string;
    appearance?: string;
    relationships?: string;
    organization_type?: string;
    organization_purpose?: string;
    organization_members?: string;
    traits?: string;
    avatar_url?: string;
    power_level?: number;
    location?: string;
    motto?: string;
    color?: string;
  }) =>
    api.post<unknown, Character>('/characters', data),

  updateCharacter: (id: string, data: CharacterUpdate) =>
    api.put<unknown, Character>(`/characters/${id}`, data),

  deleteCharacter: (id: string) => api.delete(`/characters/${id}`),

  generateCharacter: (data: GenerateCharacterRequest) =>
    api.post<unknown, Character>('/characters/generate', data),

  exportCharacters: async (characterIds: string[]) => {
    const response = await axios.post(
      '/api/characters/export',
      { character_ids: characterIds },
      {
        responseType: 'blob',
        headers: {
          'Content-Type': 'application/json',
        },
      }
    );

    const contentDisposition = response.headers['content-disposition'];
    let filename = 'characters_export.json';
    if (contentDisposition) {
      const matches = /filename=(.+)/.exec(contentDisposition);
      if (matches && matches[1]) {
        filename = matches[1];
      }
    }

    const url = window.URL.createObjectURL(new Blob([response.data]));
    const link = document.createElement('a');
    link.href = url;
    link.setAttribute('download', filename);
    document.body.appendChild(link);
    link.click();
    link.remove();
    window.URL.revokeObjectURL(url);
  },

  validateImportCharacters: (file: File) => {
    const formData = new FormData();
    formData.append('file', file);
    return api.post<unknown, {
      valid: boolean;
      version: string;
      statistics: { characters: number; organizations: number };
      errors: string[];
      warnings: string[];
    }>('/characters/validate-import', formData, {
      headers: { 'Content-Type': 'multipart/form-data' },
    });
  },

  importCharacters: (projectId: string, file: File) => {
    const formData = new FormData();
    formData.append('file', file);
    return api.post<unknown, {
      success: boolean;
      message: string;
      statistics: {
        total: number;
        imported: number;
        skipped: number;
        errors: number;
      };
      details: {
        imported_characters: string[];
        imported_organizations: string[];
        skipped: string[];
        errors: string[];
      };
      warnings: string[];
    }>(`/characters/import?project_id=${projectId}`, formData, {
      headers: { 'Content-Type': 'multipart/form-data' },
    });
  },
};