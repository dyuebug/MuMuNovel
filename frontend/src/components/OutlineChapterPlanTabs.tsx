import { Card, Space, Tabs, Tag, Typography, theme } from 'antd';

const { Text } = Typography;

export type OutlinePlanItem = {
  sub_index: number;
  title: string;
  plot_summary: string;
  key_events: string[];
  character_focus: string[];
  emotional_tone: string;
  narrative_goal: string;
  conflict_type: string;
  estimated_words: number;
  scenes?: Array<{
    location: string;
    characters: string[];
    purpose: string;
  }> | null;
};

type OutlineChapterPlanTabsProps = {
  isMobile: boolean;
  plans: OutlinePlanItem[];
  usePlanSubIndex?: boolean;
};

const wrapTextStyle = {
  wordBreak: 'break-word' as const,
  whiteSpace: 'normal' as const,
  overflowWrap: 'break-word' as const,
};

const compactTagStyle = {
  whiteSpace: 'normal' as const,
  wordBreak: 'break-word' as const,
  height: 'auto',
  lineHeight: '1.5',
  padding: '4px 8px',
};

export default function OutlineChapterPlanTabs({
  isMobile,
  plans,
  usePlanSubIndex = false,
}: OutlineChapterPlanTabsProps) {
  const { token } = theme.useToken();
  const alphaColor = (color: string, alpha: number) => `color-mix(in srgb, ${color} ${(alpha * 100).toFixed(0)}%, transparent)`;

  const contentCardStyle = {
    borderRadius: 18,
    border: `1px solid ${alphaColor(token.colorBorderSecondary, 0.84)}`,
    background: alphaColor(token.colorBgContainer, 0.96),
  };

  return (
    <Tabs
      defaultActiveKey="0"
      type="card"
      tabBarStyle={{ marginBottom: 16 }}
      items={plans.map((plan, idx) => ({
        key: idx.toString(),
        label: (
          <div
            style={{
              maxWidth: usePlanSubIndex && isMobile ? '150px' : undefined,
              minWidth: isMobile ? 96 : 112,
            }}
          >
            <Text
              style={{
                display: 'block',
                fontWeight: 600,
                whiteSpace: usePlanSubIndex && isMobile ? 'normal' : 'nowrap',
                wordBreak: usePlanSubIndex && isMobile ? 'break-word' : 'normal',
                fontSize: isMobile ? 12 : 14,
                color: token.colorText,
              }}
            >
              {(usePlanSubIndex ? plan.sub_index : idx + 1)}. {plan.title}
            </Text>
            <Text
              type="secondary"
              style={{
                display: 'block',
                fontSize: 11,
                marginTop: 2,
                whiteSpace: 'nowrap',
                overflow: 'hidden',
                textOverflow: 'ellipsis',
              }}
            >
              {plan.emotional_tone} · {plan.conflict_type}
            </Text>
          </div>
        ),
        children: (
          <div style={{ maxHeight: '500px', overflowY: 'auto', padding: '8px 0' }}>
            <Space direction="vertical" size="middle" style={{ width: '100%' }}>
              <Card
                size="small"
                title="基本信息"
                style={{
                  ...contentCardStyle,
                  background: `linear-gradient(135deg, ${alphaColor(token.colorPrimaryBg, 0.72)} 0%, ${alphaColor(token.colorBgContainer, 0.98)} 100%)`,
                }}
              >
                <Space wrap style={usePlanSubIndex ? { maxWidth: '100%' } : undefined}>
                  <Tag color="blue" style={usePlanSubIndex ? compactTagStyle : undefined}>{plan.emotional_tone}</Tag>
                  <Tag color="orange" style={usePlanSubIndex ? compactTagStyle : undefined}>{plan.conflict_type}</Tag>
                  <Tag color="green">约{plan.estimated_words}字</Tag>
                </Space>
              </Card>

              <Card size="small" title="情节概要" style={contentCardStyle}>
                <div style={wrapTextStyle}>{plan.plot_summary}</div>
              </Card>

              <Card size="small" title="叙事目标" style={contentCardStyle}>
                <div style={wrapTextStyle}>{plan.narrative_goal}</div>
              </Card>

              <Card size="small" title="关键事件" style={contentCardStyle}>
                <Space direction="vertical" size="small" style={{ width: '100%' }}>
                  {plan.key_events.map((event, eventIdx) => (
                    <div
                      key={eventIdx}
                      style={{
                        ...wrapTextStyle,
                        padding: '10px 12px',
                        borderRadius: 14,
                        border: `1px solid ${alphaColor(token.colorBorderSecondary, 0.82)}`,
                        background: alphaColor(token.colorFillQuaternary, 0.5),
                      }}
                    >
                      • {event}
                    </div>
                  ))}
                </Space>
              </Card>

              <Card size="small" title="涉及角色" style={contentCardStyle}>
                <Space wrap style={usePlanSubIndex ? { maxWidth: '100%' } : undefined}>
                  {plan.character_focus.map((character, charIdx) => (
                    <Tag key={charIdx} color="purple" style={usePlanSubIndex ? compactTagStyle : undefined}>
                      {character}
                    </Tag>
                  ))}
                </Space>
              </Card>

              {plan.scenes && plan.scenes.length > 0 ? (
                <Card size="small" title="场景" style={contentCardStyle}>
                  <Space direction="vertical" size="small" style={{ width: '100%' }}>
                    {plan.scenes.map((scene, sceneIdx) => (
                      <Card
                        key={sceneIdx}
                        size="small"
                        style={{
                          borderRadius: 16,
                          border: `1px solid ${alphaColor(token.colorBorderSecondary, 0.8)}`,
                          background: alphaColor(token.colorFillQuaternary, 0.54),
                          maxWidth: '100%',
                          overflow: 'hidden',
                        }}
                      >
                        <div style={wrapTextStyle}>
                          <strong>地点：</strong>{scene.location}
                        </div>
                        <div style={wrapTextStyle}>
                          <strong>角色：</strong>{scene.characters.join('、')}
                        </div>
                        <div style={wrapTextStyle}>
                          <strong>目的：</strong>{scene.purpose}
                        </div>
                      </Card>
                    ))}
                  </Space>
                </Card>
              ) : null}
            </Space>
          </div>
        ),
      }))}
    />
  );
}
