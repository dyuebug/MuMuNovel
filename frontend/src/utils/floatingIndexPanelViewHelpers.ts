import type { Chapter } from '../types';

export const FLOATING_INDEX_PANEL_TITLE = '\u7AE0\u8282\u7D22\u5F15';
export const FLOATING_INDEX_PANEL_SEARCH_PLACEHOLDER = '\u641C\u7D22\u7AE0\u8282\u6807\u9898';
export const FLOATING_INDEX_PANEL_EMPTY_DESCRIPTION = '\u6682\u65E0\u5339\u914D\u7AE0\u8282';
export const FLOATING_INDEX_PANEL_TRIGGER_TOOLTIP = '\u6253\u5F00\u7AE0\u8282\u7D22\u5F15';
export const FLOATING_INDEX_PANEL_UNCATEGORIZED_LABEL = '\u672A\u5206\u7EC4\u5927\u7EB2';

export type FloatingIndexSearchableChapter = Pick<Chapter, 'title'>;
export type FloatingIndexSearchableGroup<TChapter extends FloatingIndexSearchableChapter = FloatingIndexSearchableChapter> = {
  chapters: TChapter[];
};

export function formatFloatingIndexChapterLabel(
  chapter: Pick<Chapter, 'chapter_number' | 'title'>,
): string {
  const chapterNumberLabel = chapter.chapter_number != null
    ? `\u7B2C${chapter.chapter_number}\u7AE0`
    : '\u672A\u7F16\u53F7\u7AE0\u8282';

  return chapter.title ? `${chapterNumberLabel} ${chapter.title}` : chapterNumberLabel;
}

export function formatFloatingIndexOutlineLabel(outlineTitle: string | null | undefined): string {
  return outlineTitle?.trim() || FLOATING_INDEX_PANEL_UNCATEGORIZED_LABEL;
}

export function resolveFloatingIndexOutlineTagColor(outlineId: string | null): 'blue' | 'default' {
  return outlineId ? 'blue' : 'default';
}

export function normalizeFloatingIndexSearchTerm(searchTerm: string): string {
  return searchTerm.trim().toLowerCase();
}

export function filterFloatingIndexGroupsByTitle<
  TChapter extends FloatingIndexSearchableChapter,
  TGroup extends FloatingIndexSearchableGroup<TChapter>,
>(
  groupedChapters: TGroup[],
  normalizedSearchTerm: string,
): TGroup[] {
  if (!normalizedSearchTerm) {
    return groupedChapters;
  }

  return groupedChapters
    .map((group) => ({
      ...group,
      chapters: group.chapters.filter((chapter) => chapter.title.toLowerCase().includes(normalizedSearchTerm)),
    }))
    .filter((group) => group.chapters.length > 0) as TGroup[];
}

export function buildFloatingIndexChapterClickHandler({
  onChapterSelect,
  onClose,
}: {
  onChapterSelect: (chapterId: string) => void;
  onClose: () => void;
}): (chapterId: string) => void {
  return (chapterId: string) => {
    onChapterSelect(chapterId);
    onClose();
  };
}