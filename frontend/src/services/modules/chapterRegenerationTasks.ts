import { api } from '../core/httpClient';

export const chapterRegenerationTaskApi = {
  getRegenerationTasks: (chapterId: string, limit?: number) =>
    api.get<unknown, {
      chapter_id: string;
      total: number;
      tasks: Array<{
        task_id: string;
        status: string;
        version_number: number | null;
        version_note: string | null;
        original_word_count: number | null;
        regenerated_word_count: number | null;
        created_at: string | null;
        completed_at: string | null;
      }>;
    }>(`/chapters/${chapterId}/regeneration/tasks`, { params: { limit } }),
};