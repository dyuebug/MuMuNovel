import type { ChangeEvent } from 'react';
import { useCallback, useMemo, useState } from 'react';
import { normalizeFloatingIndexSearchTerm } from '../utils/floatingIndexPanelViewHelpers';

export const useFloatingIndexSearchState = () => {
  const [searchTerm, setSearchTerm] = useState('');

  const normalizedSearchTerm = useMemo(
    () => normalizeFloatingIndexSearchTerm(searchTerm),
    [searchTerm],
  );

  const handleSearchTermChange = useCallback((event: ChangeEvent<HTMLInputElement>) => {
    setSearchTerm(event.target.value);
  }, []);

  return {
    handleSearchTermChange,
    normalizedSearchTerm,
    searchTerm,
  };
};