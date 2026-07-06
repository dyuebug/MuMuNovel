import { Card, Space, Tag, Typography, theme } from 'antd';
import { designDisplayFont } from '../theme/themeConfig';

const { Text, Paragraph, Title } = Typography;

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
  const { token } = theme.useToken();
  const heroBackground = `linear-gradient(135deg,
    color-mix(in srgb, ${token.colorPrimary} 80%, #704734 20%) 0%,
    color-mix(in srgb, ${token.colorInfo} 32%, #1f262e 68%) 100%)`;
  const guideSteps = [
    '先确认继续生成会沿用当前写作配置，而不是重新初始化本章。',
    '再检查前文上下文和目标字数，确保本次续写方向没有偏离当前创作节奏。',
    '最后再决定是否继续，原有生成与覆盖逻辑保持不变。',
  ];
  const workflowItems = [
    { label: '写作风格', value: selectedStyleName ?? '未选择' },
    { label: '创作模式', value: creativeModeLabel },
    { label: '故事聚焦', value: storyFocusLabel },
    { label: '剧情阶段', value: plotStageLabel },
    { label: '目标字数', value: `${targetWordCount} 字` },
  ];

  return (
    <div style={{ marginTop: 16 }}>
      <Card
        bordered={false}
        style={{
          marginBottom: 16,
          borderRadius: 20,
          overflow: 'hidden',
          background: heroBackground,
        }}
        styles={{ body: { padding: 20 } }}
      >
        <Text style={{ color: 'rgba(255,255,255,0.68)', letterSpacing: '0.14em', textTransform: 'uppercase' }}>
          Continue Draft
        </Text>
        <Title
          level={5}
          style={{
            margin: '8px 0 10px',
            color: '#f7f1e8',
            fontFamily: designDisplayFont,
            letterSpacing: '-0.03em',
          }}
        >
          继续生成当前章节前的最终确认
        </Title>
        <Paragraph style={{ margin: 0, color: 'rgba(255,255,255,0.84)', lineHeight: 1.75 }}>
          这一步更像续写前的工作流确认台。原有继续生成、覆盖正文和上下文拼接逻辑保持不变，这里只把确认顺序和风险焦点说清楚。
        </Paragraph>
      </Card>

      <Card
        bordered={false}
        style={{
          marginBottom: 16,
          borderRadius: 18,
          background: `linear-gradient(135deg, color-mix(in srgb, ${token.colorPrimary} 8%, white 92%) 0%, color-mix(in srgb, ${token.colorWarning} 8%, white 92%) 100%)`,
          border: `1px solid color-mix(in srgb, ${token.colorPrimary} 14%, white 86%)`,
        }}
        styles={{ body: { padding: 18 } }}
      >
        <Text style={{ fontSize: 12, color: token.colorTextTertiary, letterSpacing: '0.12em', textTransform: 'uppercase' }}>
          Continue Guide
        </Text>
        <Paragraph style={{ margin: '8px 0 0', color: token.colorText, lineHeight: 1.75 }}>
          先确认当前配置，再确认会带入哪些前文章节，最后再决定是否继续生成。这里只重排阅读顺序，不改变原有生成动作。
        </Paragraph>
        <div style={{ display: 'flex', flexWrap: 'wrap', gap: 8, marginTop: 12 }}>
          {guideSteps.map((item, index) => (
            <span
              key={item}
              style={{
                display: 'inline-flex',
                alignItems: 'center',
                gap: 8,
                padding: '6px 12px',
                borderRadius: 999,
                background: token.colorBgContainer,
                border: `1px solid ${token.colorBorderSecondary}`,
                color: token.colorTextSecondary,
                fontSize: 12,
              }}
            >
              <span style={{ color: token.colorPrimary, fontWeight: 700 }}>{index + 1}</span>
              {item}
            </span>
          ))}
        </div>
      </Card>

      <Card
        bordered={false}
        style={{
          marginBottom: 16,
          borderRadius: 18,
          background: token.colorBgContainer,
          border: `1px solid ${token.colorBorderSecondary}`,
        }}
        styles={{ body: { padding: 18 } }}
      >
        <Text style={{ fontSize: 12, color: token.colorTextTertiary, letterSpacing: '0.12em', textTransform: 'uppercase' }}>
          Continue Workspace
        </Text>
        <Title level={5} style={{ margin: '6px 0 10px', fontFamily: designDisplayFont }}>
          当前续写配置
        </Title>
        <Space direction="vertical" size={10} style={{ width: '100%' }}>
          {workflowItems.map((item) => (
            <div
              key={item.label}
              style={{
                display: 'flex',
                justifyContent: 'space-between',
                gap: 12,
                padding: '10px 12px',
                borderRadius: 14,
                background: token.colorFillAlter,
              }}
            >
              <Text type="secondary">{item.label}</Text>
              <Text strong style={{ textAlign: 'right' }}>{item.value}</Text>
            </div>
          ))}
        </Space>
      </Card>

      {previousChapters.length > 0 ? (
        <Card
          bordered={false}
          style={{
            marginTop: 16,
            borderRadius: 18,
            background: `linear-gradient(180deg, color-mix(in srgb, ${token.colorInfoBg} 72%, ${token.colorBgContainer} 28%) 0%, ${token.colorBgContainer} 100%)`,
            border: `1px solid ${token.colorInfoBorder}`,
          }}
          styles={{ body: { padding: 18 } }}
        >
          <Text style={{ fontSize: 12, color: token.colorTextTertiary, letterSpacing: '0.12em', textTransform: 'uppercase' }}>
            Context Snapshot
          </Text>
          <Title level={5} style={{ margin: '6px 0 8px', fontFamily: designDisplayFont }}>
            {`将带入前 ${previousChapters.length} 章作为上下文`}
          </Title>
          <Space wrap size={[8, 8]} style={{ marginBottom: 12 }}>
            <Tag color="blue" style={{ margin: 0, borderRadius: 999, paddingInline: 10 }}>
              前文章节上下文
            </Tag>
            <Tag color="gold" style={{ margin: 0, borderRadius: 999, paddingInline: 10 }}>
              将覆盖当前章节正文
            </Tag>
          </Space>
          <div style={{ maxHeight: 150, overflowY: 'auto' }}>
            {previousChapters.map((chapter) => (
              <div
                key={chapter.id}
                style={{
                  padding: '8px 10px',
                  fontSize: 13,
                  borderRadius: 12,
                  background: token.colorBgContainer,
                  marginBottom: 8,
                }}
              >
                {`第 ${chapter.chapter_number} 章：${chapter.title}（${chapter.word_count || 0} 字）`}
              </div>
            ))}
          </div>
          <div style={{ marginTop: 8, fontSize: 12, color: token.colorTextSecondary }}>
            继续生成将覆盖当前章节正文。
          </div>
        </Card>
      ) : null}

      <Card
        bordered={false}
        style={{
          marginTop: 16,
          borderRadius: 18,
          background: `linear-gradient(180deg, color-mix(in srgb, ${token.colorErrorBg} 82%, ${token.colorBgContainer} 18%) 0%, ${token.colorBgContainer} 100%)`,
          border: `1px solid ${token.colorErrorBorder}`,
        }}
        styles={{ body: { padding: 16 } }}
      >
        <Text strong style={{ color: token.colorError }}>
          继续前请确认重要内容已经保存。
        </Text>
        <Paragraph style={{ margin: '6px 0 0', color: token.colorTextSecondary, lineHeight: 1.7 }}>
          这次确认之后，系统会按当前配置继续生成本章，并覆盖当前章节正文内容。
        </Paragraph>
      </Card>
    </div>
  );
}
