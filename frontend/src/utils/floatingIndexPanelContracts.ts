import type { ChangeEvent } from 'react';
import type { Chapter } from '../types';

export type FloatingIndexPanelSourceGroup = {
  key: string;
  outlineId: string | null;
  outlineTitle: string;
  chapters: Chapter[];
};

export type FloatingIndexPanelGroup = {
  chapters: Chapter[];
  key: string;
  outlineLabel: string;
  outlineTagColor: 'blue' | 'default';
};

export type FloatingIndexPanelChapterClickHandler = (chapterId: string) => void;

export type FloatingIndexPanelSearchChangeHandler = (
  event: ChangeEvent<HTMLInputElement>,
) => void;

export type FloatingIndexPanelSearchModel = {
  onSearchTermChange: FloatingIndexPanelSearchChangeHandler;
  searchTerm: string;
};

export type FloatingIndexPanelResultsModel = {
  filteredGroups: FloatingIndexPanelGroup[];
  onChapterClick: FloatingIndexPanelChapterClickHandler;
};

export type FloatingIndexPanelViewModel = {
  resultsModel: FloatingIndexPanelResultsModel;
  searchModel: FloatingIndexPanelSearchModel;
};