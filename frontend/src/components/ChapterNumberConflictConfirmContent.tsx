import { Card, Space, Tag, Typography, theme } from 'antd';
import { designDisplayFont } from '../theme/themeConfig';

const { Text, Paragraph, Title } = Typography;

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
  const { token } = theme.useToken();
  const heroBackground = `linear-gradient(135deg,
    color-mix(in srgb, ${token.colorWarning} 78%, #7b5136 22%) 0%,
    color-mix(in srgb, ${token.colorError} 34%, #45242a 66%) 100%)`;
  const guideSteps = [
    '先确认冲突的是章节号，而不是仅仅标题重复。',
    '再通读现有章节的标题、状态、字数和所属大纲，判断它是否仍然需要保留。',
    '最后再决定是否删除已有章节并创建新章节，原有删除后再创建的业务流程保持不变。',
  ];

  return (
    <div style={{ marginTop: 8 }}>
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
          Chapter Conflict
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
          {`章节号 ${chapterNumber} 已被现有章节占用`}
        </Title>
        <Paragraph style={{ margin: 0, color: 'rgba(255,255,255,0.84)', lineHeight: 1.75 }}>
          这里保留原有冲突确认逻辑，只把阅读顺序和风险焦点说清楚，帮助你先审阅现有章节，再决定是否覆盖。
        </Paragraph>
      </Card>

      <Card
        bordered={false}
        style={{
          marginBottom: 16,
          borderRadius: 18,
          background: `linear-gradient(135deg, color-mix(in srgb, ${token.colorWarning} 10%, white 90%) 0%, color-mix(in srgb, ${token.colorError} 10%, white 90%) 100%)`,
          border: `1px solid color-mix(in srgb, ${token.colorWarning} 18%, white 82%)`,
        }}
        styles={{ body: { padding: 18 } }}
      >
        <Text style={{ fontSize: 12, color: token.colorTextTertiary, letterSpacing: '0.12em', textTransform: 'uppercase' }}>
          Conflict Guide
        </Text>
        <Paragraph style={{ margin: '8px 0 0', color: token.colorText, lineHeight: 1.75 }}>
          先读现有章节信息，再决定是否继续覆盖。这里只重排确认层级，不改变“先删除再创建”的原有业务动作。
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
              <span style={{ color: token.colorWarning, fontWeight: 700 }}>{index + 1}</span>
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
          Conflict Snapshot
        </Text>
        <Title level={5} style={{ margin: '6px 0 8px', fontFamily: designDisplayFont }}>
          当前占用该章节号的记录
        </Title>
        <Space wrap size={[8, 8]} style={{ marginBottom: 12 }}>
          <Tag color="gold" style={{ margin: 0, borderRadius: 999, paddingInline: 10 }}>
            章节号冲突
          </Tag>
          <Tag color="red" style={{ margin: 0, borderRadius: 999, paddingInline: 10 }}>
            继续将删除现有章节
          </Tag>
        </Space>
        <Space direction="vertical" size={10} style={{ width: '100%' }}>
          <div style={{ padding: '10px 12px', borderRadius: 14, background: token.colorFillAlter }}>
            <Text type="secondary">标题</Text>
            <div><Text strong>{conflictChapter.title}</Text></div>
          </div>
          <div style={{ padding: '10px 12px', borderRadius: 14, background: token.colorFillAlter }}>
            <Text type="secondary">状态</Text>
            <div><Text strong>{statusText}</Text></div>
          </div>
          <div style={{ padding: '10px 12px', borderRadius: 14, background: token.colorFillAlter }}>
            <Text type="secondary">字数</Text>
            <div><Text strong>{`${conflictChapter.word_count || 0} 字`}</Text></div>
          </div>
          {conflictChapter.outline_title ? (
            <div style={{ padding: '10px 12px', borderRadius: 14, background: token.colorFillAlter }}>
              <Text type="secondary">所属大纲</Text>
              <div><Text strong>{conflictChapter.outline_title}</Text></div>
            </div>
          ) : null}
        </Space>
      </Card>

      <Card
        bordered={false}
        style={{
          borderRadius: 18,
          background: `linear-gradient(180deg, color-mix(in srgb, ${token.colorErrorBg} 82%, ${token.colorBgContainer} 18%) 0%, ${token.colorBgContainer} 100%)`,
          border: `1px solid ${token.colorErrorBorder}`,
        }}
        styles={{ body: { padding: 16 } }}
      >
        <Text strong style={{ color: token.colorError }}>
          如果继续，系统会先删除现有章节，再创建新章节。
        </Text>
        <Paragraph style={{ margin: '6px 0 0', color: token.colorTextSecondary, lineHeight: 1.7 }}>
          此操作不可撤销，请确认现有章节已不再需要。
        </Paragraph>
      </Card>
    </div>
  );
}
