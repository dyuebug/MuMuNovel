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
        章节编号 <strong>{chapterNumber}</strong> 已被现有章节占用。
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
          <strong>章节标题：</strong>
          {conflictChapter.title}
        </div>
        <div>
          <strong>当前状态：</strong>
          {statusText}
        </div>
        <div>
          <strong>当前字数：</strong>
          {conflictChapter.word_count || 0} 字
        </div>
        {conflictChapter.outline_title ? (
          <div>
            <strong>关联大纲：</strong>
            {conflictChapter.outline_title}
          </div>
        ) : null}
      </div>

      <p style={{ color: '#ff4d4f', marginBottom: 8 }}>
        如果继续创建，系统会先删除当前章节，再使用该编号创建新章节。
      </p>

      <p style={{ fontSize: 12, color: '#666', marginBottom: 0 }}>
        此操作不可撤销，请确认原章节内容已经不再需要。
      </p>
    </div>
  );
}
