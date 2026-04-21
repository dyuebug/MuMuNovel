type PreviousChapterPreview = {
  id: string;
  chapter_number: number;
  title: string;
  word_count?: number | null;
};

type ContinueGenerateConfirmContentProps = {
  selectedStyleName?: string;
  creativeModeLabel: string;
  storyFocusLabel: string;
  plotStageLabel: string;
  targetWordCount: number;
  previousChapters: PreviousChapterPreview[];
};

export default function ContinueGenerateConfirmContent({
  selectedStyleName,
  creativeModeLabel,
  storyFocusLabel,
  plotStageLabel,
  targetWordCount,
  previousChapters,
}: ContinueGenerateConfirmContentProps) {
  return (
    <div style={{ marginTop: 16 }}>
      <p>Continue generating this chapter with the current settings.</p>
      <ul>
        <li>{`Writing style: ${selectedStyleName ?? 'Not selected'}`}</li>
        <li>{`Creative mode: ${creativeModeLabel}`}</li>
        <li>{`Story focus: ${storyFocusLabel}`}</li>
        <li>{`Plot stage: ${plotStageLabel}`}</li>
        <li>{`Target word count: ${targetWordCount}`}</li>
      </ul>
      {previousChapters.length > 0 ? (
        <div
          style={{
            marginTop: 16,
            padding: 12,
            background: 'var(--color-info-bg)',
            borderRadius: 4,
            border: '1px solid var(--color-info-border)',
          }}
        >
          <div style={{ marginBottom: 8, fontWeight: 500, color: 'var(--color-primary)' }}>
            {`${previousChapters.length} earlier chapters will be used as context:`}
          </div>
          <div style={{ maxHeight: 150, overflowY: 'auto' }}>
            {previousChapters.map((chapter) => (
              <div key={chapter.id} style={{ padding: '4px 0', fontSize: 13 }}>
                {`Chapter ${chapter.chapter_number}: ${chapter.title} (${chapter.word_count || 0} words)`}
              </div>
            ))}
          </div>
          <div style={{ marginTop: 8, fontSize: 12, color: '#666' }}>
            Continuing will overwrite the current chapter content.
          </div>
        </div>
      ) : null}
      <p style={{ color: '#ff4d4f', marginTop: 16, marginBottom: 0 }}>
        Please make sure important content is already saved before continuing.
      </p>
    </div>
  );
}