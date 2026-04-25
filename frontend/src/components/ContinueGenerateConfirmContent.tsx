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
      <p>将基于当前配置继续生成本章内容。</p>
      <ul>
        <li>{`写作风格：${selectedStyleName ?? '未选择'}`}</li>
        <li>{`创作模式：${creativeModeLabel}`}</li>
        <li>{`故事聚焦：${storyFocusLabel}`}</li>
        <li>{`剧情阶段：${plotStageLabel}`}</li>
        <li>{`目标字数：${targetWordCount}`}</li>
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
            {`将使用前 ${previousChapters.length} 章作为上下文：`}
          </div>
          <div style={{ maxHeight: 150, overflowY: 'auto' }}>
            {previousChapters.map((chapter) => (
              <div key={chapter.id} style={{ padding: '4px 0', fontSize: 13 }}>
                {`第 ${chapter.chapter_number} 章：${chapter.title}（${chapter.word_count || 0} 字）`}
              </div>
            ))}
          </div>
          <div style={{ marginTop: 8, fontSize: 12, color: '#666' }}>
            继续生成将覆盖当前章节正文。
          </div>
        </div>
      ) : null}
      <p style={{ color: '#ff4d4f', marginTop: 16, marginBottom: 0 }}>
        继续前请确认重要内容已经保存。
      </p>
    </div>
  );
}