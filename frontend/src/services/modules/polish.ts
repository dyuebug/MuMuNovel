import { api } from '../core/httpClient';
import type {
  PolishBatchRequest,
  PolishTextRequest,
  PolishTextResponse,
} from '../../types';

export const polishApi = {
  polishText: (data: PolishTextRequest) =>
    api.post<unknown, PolishTextResponse>('/polish', data),

  polishBatch: (data: PolishBatchRequest | string[]) =>
    api.post<unknown, {
      total: number;
      results: Array<{
        index: number;
        original: string;
        polished: string;
        word_count_before: number;
        word_count_after: number;
      }>;
    }>('/polish/batch', Array.isArray(data) ? { texts: data } : data),
};