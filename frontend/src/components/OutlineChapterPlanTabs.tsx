import { Card, Space, Tabs, Tag } from 'antd';
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
  return (
    <Tabs
      defaultActiveKey="0"
      type="card"
      items={plans.map((plan, idx) => ({
        key: idx.toString(),
        label: (
          <Space size="small" style={usePlanSubIndex ? { maxWidth: isMobile ? '150px' : 'none' } : undefined}>
            <span
              style={{
                fontWeight: 500,
                whiteSpace: usePlanSubIndex && isMobile ? 'normal' : 'nowrap',
                wordBreak: usePlanSubIndex && isMobile ? 'break-word' : 'normal',
                fontSize: isMobile ? 12 : 14,
              }}
            >
              {(usePlanSubIndex ? plan.sub_index : idx + 1)}. {plan.title}
            </span>
          </Space>
        ),
        children: (
          <div style={{ maxHeight: '500px', overflowY: 'auto', padding: '8px 0' }}>
            <Space direction="vertical" size="middle" style={{ width: '100%' }}>
              <Card size="small" title="基本信息">
                <Space wrap style={usePlanSubIndex ? { maxWidth: '100%' } : undefined}>
                  <Tag color="blue" style={usePlanSubIndex ? compactTagStyle : undefined}>{plan.emotional_tone}</Tag>
                  <Tag color="orange" style={usePlanSubIndex ? compactTagStyle : undefined}>{plan.conflict_type}</Tag>
                  <Tag color="green">约{plan.estimated_words}字</Tag>
                </Space>
              </Card>

              <Card size="small" title="情节概要">
                <div style={wrapTextStyle}>{plan.plot_summary}</div>
              </Card>

              <Card size="small" title="叙事目标">
                <div style={wrapTextStyle}>{plan.narrative_goal}</div>
              </Card>

              <Card size="small" title="关键事件">
                <Space direction="vertical" size="small" style={{ width: '100%' }}>
                  {plan.key_events.map((event, eventIdx) => (
                    <div key={eventIdx} style={wrapTextStyle}>
                      • {event}
                    </div>
                  ))}
                </Space>
              </Card>

              <Card size="small" title="涉及角色">
                <Space wrap style={usePlanSubIndex ? { maxWidth: '100%' } : undefined}>
                  {plan.character_focus.map((character, charIdx) => (
                    <Tag key={charIdx} color="purple" style={usePlanSubIndex ? compactTagStyle : undefined}>
                      {character}
                    </Tag>
                  ))}
                </Space>
              </Card>

              {plan.scenes && plan.scenes.length > 0 ? (
                <Card size="small" title="场景">
                  <Space direction="vertical" size="small" style={{ width: '100%' }}>
                    {plan.scenes.map((scene, sceneIdx) => (
                      <Card
                        key={sceneIdx}
                        size="small"
                        style={{
                          backgroundColor: '#fafafa',
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
