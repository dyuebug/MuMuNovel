import { useMemo } from 'react';
import { useFloatingIndexSearchState } from './useFloatingIndexSearchState';
import type {
  FloatingIndexPanelGroup,
  FloatingIndexPanelViewModel,
} from '../utils/floatingIndexPanelContracts';
import {
  buildFloatingIndexChapterClickHandler,
  filterFloatingIndexGroupsByTitle,
} from '../utils/floatingIndexPanelViewHelpers';

type UseFloatingIndexPanelViewModelOptions = {
  groupedChapters: FloatingIndexPanelGroup[];
  onChapterSelect: (chapterId: string) => void;
  onClose: () => void;
};

export const useFloatingIndexPanelViewModel = ({
  groupedChapters,
  onChapterSelect,
  onClose,
}: UseFloatingIndexPanelViewModelOptions): FloatingIndexPanelViewModel => {
  const { handleSearchTermChange, normalizedSearchTerm, searchTerm } = useFloatingIndexSearchState();

  const filteredGroups = useMemo(
    () => filterFloatingIndexGroupsByTitle(groupedChapters, normalizedSearchTerm),
    [groupedChapters, normalizedSearchTerm],
  );

  const onChapterClick = useMemo(
    () => buildFloatingIndexChapterClickHandler({ onChapterSelect, onClose }),
    [onChapterSelect, onClose],
  );

  const searchModel = useMemo(
    () => ({
      onSearchTermChange: handleSearchTermChange,
      searchTerm,
    }),
    [handleSearchTermChange, searchTerm],
  );

  const resultsModel = useMemo(
    () => ({
      filteredGroups,
      onChapterClick,
    }),
    [filteredGroups, onChapterClick],
  );

  return useMemo(
    () => ({
      resultsModel,
      searchModel,
    }),
    [resultsModel, searchModel],
  );
};