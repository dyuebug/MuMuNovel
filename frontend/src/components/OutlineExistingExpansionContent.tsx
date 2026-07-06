import { Card, Empty, Space, Tag, Typography, theme } from 'antd';
import OutlineChapterPlanTabs, { type OutlinePlanItem } from './OutlineChapterPlanTabs';

const { Text } = Typography;

type ExistingExpansionData = {
  chapter_count: number;
  expansion_plans: OutlinePlanItem[] | null;
};

type OutlineExistingExpansionContentProps = {
  data: ExistingExpansionData;
  isMobile: boolean;
  outlineTitle: string;
};

export default function OutlineExistingExpansionContent({
  data,
  isMobile,
  outlineTitle,
}: OutlineExistingExpansionContentProps) {
  const { token } = theme.useToken();
  const alphaColor = (color: string, alpha: number) => `color-mix(in srgb, ${color} ${(alpha * 100).toFixed(0)}%, transparent)`;
  const plans = data.expansion_plans ?? [];
  const existingExpansionGuideSteps = [
    '先确认这份已保存规划对应的是哪条大纲，避免把旧版本章节安排误当成当前最新方案。',
    '再逐章回看标签页里的目标、事件和场景，重点检查哪些段落仍然适合直接进入写作。',
    '最后把这份已落库规划当作校准依据，而不是重新生成逻辑的入口。',
  ];
  const existingExpansionWorkspaceFocus = plans.length <= 1
    ? {
        title: '先确认这一章保存下来的展开逻辑是否仍可复用',
        note: '当前只需要聚焦单章规划，适合逐项核对目标、事件与角色安排，再决定是否继续沿用这份章节结构。',
      }
    : {
        title: `把这 ${plans.length} 章已保存规划当作回看与校准的基线`,
        note: '当前已有多章数据落库，更适合顺着章节顺序复核推进节奏、角色分布和结构连贯性，而不是直接重做规划。',
      };

  return (
    <div>
      <Card
        size="small"
        style={{
          marginBottom: 16,
          borderRadius: 20,
          border: `1px solid ${alphaColor(token.colorPrimary, 0.12)}`,
          background: `linear-gradient(135deg, ${alphaColor(token.colorPrimaryBg, 0.82)} 0%, ${alphaColor(token.colorBgContainer, 0.98)} 100%)`,
        }}
        styles={{ body: { padding: 16 } }}
      >
        <div
          style={{
            display: 'grid',
            gridTemplateColumns: 'repeat(auto-fit, minmax(240px, 1fr))',
            gap: 16,
          }}
        >
          <div>
            <Text style={{ display: 'block', fontSize: 11, letterSpacing: '0.08em', textTransform: 'uppercase', color: token.colorTextTertiary, marginBottom: 6 }}>
              Existing Expansion Guide
            </Text>
            <Text strong style={{ display: 'block', fontSize: 17, marginBottom: 8 }}>
              已创建章节概览
            </Text>
            <Text type="secondary" style={{ display: 'block', lineHeight: 1.7, marginBottom: 12 }}>
              这里展示的是已经保存过的展开规划。当前只重排回看顺序和判断重点，不会改动任何已落库的章节数据或规划逻辑。
            </Text>
            <div style={{ display: 'flex', flexWrap: 'wrap', gap: 8 }}>
              {existingExpansionGuideSteps.map((item, index) => (
                <span
                  key={item}
                  style={{
                    display: 'inline-flex',
                    alignItems: 'center',
                    gap: 8,
                    padding: '6px 12px',
                    borderRadius: 999,
                    background: token.colorBgContainer,
                    border: `1px solid ${alphaColor(token.colorPrimary, 0.12)}`,
                    color: token.colorText,
                    fontSize: 12,
                  }}
                >
                  <span style={{ color: token.colorPrimary, fontWeight: 700 }}>{index + 1}</span>
                  {item}
                </span>
              ))}
            </div>
          </div>
          <div
            style={{
              borderRadius: 18,
              padding: '16px 18px 14px',
              background: `linear-gradient(180deg, ${alphaColor(token.colorBgContainer, 0.98)} 0%, ${alphaColor(token.colorFillQuaternary, 0.5)} 100%)`,
              border: `1px solid ${alphaColor(token.colorPrimary, 0.12)}`,
            }}
          >
            <Text style={{ display: 'block', fontSize: 11, letterSpacing: '0.08em', textTransform: 'uppercase', color: token.colorTextTertiary, marginBottom: 6 }}>
              当前工作焦点
            </Text>
            <Text strong style={{ display: 'block', fontSize: 16, marginBottom: 8 }}>
              {existingExpansionWorkspaceFocus.title}
            </Text>
            <Text type="secondary" style={{ display: 'block', lineHeight: 1.7, marginBottom: 12 }}>
              {existingExpansionWorkspaceFocus.note}
            </Text>
            <Space wrap style={{ maxWidth: '100%' }}>
              <Tag
                color="blue"
                style={{
                  whiteSpace: 'normal',
                  wordBreak: 'break-word',
                  height: 'auto',
                  lineHeight: '1.5',
                  padding: '4px 8px',
                }}
              >
                大纲: {outlineTitle}
              </Tag>
              <Tag color="green">章节数: {data.chapter_count}</Tag>
              <Tag color="purple">规划页签: {plans.length}</Tag>
              <Tag color="orange">已创建章节</Tag>
            </Space>
          </div>
        </div>
      </Card>

      {plans.length > 0 ? (
        <div
          style={{
            padding: isMobile ? 12 : 16,
            borderRadius: 22,
            border: `1px solid ${alphaColor(token.colorBorderSecondary, 0.9)}`,
            background: `linear-gradient(180deg, ${alphaColor(token.colorBgElevated, 0.98)} 0%, ${alphaColor(token.colorFillQuaternary, 0.46)} 100%)`,
          }}
        >
          <Text style={{ display: 'block', fontSize: 11, letterSpacing: '0.08em', textTransform: 'uppercase', color: token.colorTextTertiary, marginBottom: 6 }}>
            Saved Chapter Workspace
          </Text>
          <Text strong style={{ display: 'block', marginBottom: 8 }}>
            已落库章节规划
          </Text>
          <Text type="secondary" style={{ display: 'block', marginBottom: 14, lineHeight: 1.7 }}>
            这些内容已经存在于项目里，因此当前面板更适合做回看和校准，而不是重新推导章节逻辑。
          </Text>
          <OutlineChapterPlanTabs plans={plans} isMobile={isMobile} usePlanSubIndex />
        </div>
      ) : (
        <div
          style={{
            minHeight: 240,
            borderRadius: 22,
            border: `1px dashed ${alphaColor(token.colorBorder, 0.92)}`,
            background: alphaColor(token.colorFillAlter, 0.7),
            display: 'flex',
            alignItems: 'center',
            justifyContent: 'center',
          }}
        >
          <Empty description="未找到展开规划数据" image={Empty.PRESENTED_IMAGE_SIMPLE} />
        </div>
      )}
    </div>
  );
}
