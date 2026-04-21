import type { PaginationResponse } from '../types';

export const normalizeStoreItems = <T>(data: T[] | PaginationResponse<T>): T[] =>
  Array.isArray(data) ? data : data.items || [];

export async function runStoreMutation<T>({
  request,
  onSuccess,
  errorLogLabel,
}: {
  request: () => Promise<T>;
  onSuccess?: (result: T) => void;
  errorLogLabel: string;
}): Promise<T> {
  try {
    const result = await request();
    onSuccess?.(result);
    return result;
  } catch (error) {
    console.error(errorLogLabel, error);
    throw error;
  }
}
