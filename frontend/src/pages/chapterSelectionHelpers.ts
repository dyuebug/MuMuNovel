export function selectChapterListItem({
  chapterId,
  highlightColor = '#e6f7ff',
  highlightDurationMs = 1500,
}: {
  chapterId: string;
  highlightColor?: string;
  highlightDurationMs?: number;
}): void {
  const element = document.getElementById(`chapter-item-${chapterId}`);

  if (!element) {
    return;
  }

  element.scrollIntoView({ behavior: 'smooth', block: 'center' });
  element.style.transition = 'background-color 0.5s ease';
  element.style.backgroundColor = highlightColor;

  window.setTimeout(() => {
    element.style.backgroundColor = '';
  }, highlightDurationMs);
}