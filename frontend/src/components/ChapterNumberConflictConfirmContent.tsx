type ConflictChapterPreview = {
  title: string;
  word_count?: number | null;
  outline_title?: string | null;
};

type ChapterNumberConflictConfirmContentProps = {
  chapterNumber: number;
  conflictChapter: ConflictChapterPreview;
  statusText: string;
};

export default function ChapterNumberConflictConfirmContent({
  chapterNumber,
  conflictChapter,
  statusText,
}: ChapterNumberConflictConfirmContentProps) {
  return (
    <div>
      <p style={{ marginBottom: 12 }}>
        {`Chapter number `}
        <strong>{chapterNumber}</strong>
        {' is already in use by an existing chapter.'}
      </p>

      <div
        style={{
          padding: 12,
          background: '#fff7e6',
          borderRadius: 4,
          border: '1px solid #ffd591',
          marginBottom: 12,
        }}
      >
        <div>
          <strong>Title: </strong>
          {conflictChapter.title}
        </div>
        <div>
          <strong>Status: </strong>
          {statusText}
        </div>
        <div>
          <strong>Word count: </strong>
          {`${conflictChapter.word_count || 0} words`}
        </div>
        {conflictChapter.outline_title ? (
          <div>
            <strong>Outline: </strong>
            {conflictChapter.outline_title}
          </div>
        ) : null}
      </div>

      <p style={{ color: '#ff4d4f', marginBottom: 8 }}>
        If you continue, the existing chapter will be deleted before the new one is created.
      </p>

      <p style={{ fontSize: 12, color: '#666', marginBottom: 0 }}>
        This action cannot be undone. Please confirm the existing chapter is no longer needed.
      </p>
    </div>
  );
}