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
      <p>将按当前设置继续生成本章内容。</p>
      <ul>
        <li>写作风格：{selectedStyleName ?? '未选择'}</li>
        <li>创作模式：{creativeModeLabel}</li>
        <li>故事焦点：{storyFocusLabel}</li>
        <li>剧情阶段：{plotStageLabel}</li>
        <li>目标字数：{targetWordCount}</li>
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
            已生成的 {previousChapters.length} 章将作为参考：
          </div>
          <div style={{ maxHeight: 150, overflowY: 'auto' }}>
            {previousChapters.map((chapter) => (
              <div key={chapter.id} style={{ padding: '4px 0', fontSize: 13 }}>
                {`第${chapter.chapter_number}章：${chapter.title}（${chapter.word_count || 0}字）`}
              </div>
            ))}
          </div>
          <div style={{ marginTop: 8, fontSize: 12, color: '#666' }}>
            继续操作将覆盖当前章节内容。
          </div>
        </div>
      ) : null}
      <p style={{ color: '#ff4d4f', marginTop: 16, marginBottom: 0 }}>
        请先确认重要内容已经保存，再继续操作。
      </p>
    </div>
  );
}
