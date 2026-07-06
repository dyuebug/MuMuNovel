import { Card, Space, Tag, Typography, theme } from 'antd';
import type { OutlineExpansionResponse } from '../types';
import OutlineChapterPlanTabs from './OutlineChapterPlanTabs';

const { Text } = Typography;

type OutlineExpansionPreviewContentProps = {
  isMobile: boolean;
  response: OutlineExpansionResponse;
};

export default function OutlineExpansionPreviewContent({
  isMobile,
  response,
}: OutlineExpansionPreviewContentProps) {
  const { token } = theme.useToken();
  const alphaColor = (color: string, alpha: number) => `color-mix(in srgb, ${color} ${(alpha * 100).toFixed(0)}%, transparent)`;
  const outlineExpansionGuideSteps = [
    '先看这轮展开策略和章节数量，确认这份预览是否还在你预期的篇幅与节奏区间内。',
    '再逐章检查每个标签页里的目标、事件和场景，不必一开始就急着决定是否创建章节。',
    '最后再根据整组章节的连贯性做判断，把“是否落库”建立在完整浏览之后。',
  ];
  const outlineExpansionWorkspaceFocus = response.actual_chapter_count <= 1
    ? {
        title: '先确认这一章的展开方向是否成立',
        note: '当前预览只有单章结果，更适合仔细确认目标、事件和场景是否足以支撑后续章节创建。',
      }
    : {
        title: `逐章复核这 ${response.actual_chapter_count} 章的展开节奏`,
        note: '当前已经得到多章规划，适合先看章节之间的推进关系和角色分布，再决定是否正式创建全部章节。',
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
              Expansion Preview Guide
            </Text>
            <Text strong style={{ display: 'block', fontSize: 17, marginBottom: 8 }}>
              章节展开预览
            </Text>
            <Text type="secondary" style={{ display: 'block', lineHeight: 1.7, marginBottom: 12 }}>
              当前结果还只是章节规划草稿。这里不会改变任何章节数据，只把预览顺序和判断重点提前说明，帮助你先看清整组章节，再决定是否落库。
            </Text>
            <div style={{ display: 'flex', flexWrap: 'wrap', gap: 8 }}>
              {outlineExpansionGuideSteps.map((item, index) => (
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
              {outlineExpansionWorkspaceFocus.title}
            </Text>
            <Text type="secondary" style={{ display: 'block', lineHeight: 1.7, marginBottom: 12 }}>
              {outlineExpansionWorkspaceFocus.note}
            </Text>
            <Space wrap>
              <Tag color="blue">策略: {response.expansion_strategy}</Tag>
              <Tag color="green">章节数: {response.actual_chapter_count}</Tag>
              <Tag color="orange">预览模式（未创建章节）</Tag>
            </Space>
          </div>
        </div>
      </Card>
      <div
        style={{
          padding: isMobile ? 12 : 16,
          borderRadius: 22,
          border: `1px solid ${alphaColor(token.colorBorderSecondary, 0.9)}`,
          background: `linear-gradient(180deg, ${alphaColor(token.colorBgElevated, 0.98)} 0%, ${alphaColor(token.colorFillQuaternary, 0.46)} 100%)`,
        }}
      >
        <Text style={{ display: 'block', fontSize: 11, letterSpacing: '0.08em', textTransform: 'uppercase', color: token.colorTextTertiary, marginBottom: 6 }}>
          Chapter Workspace
        </Text>
        <Text strong style={{ display: 'block', marginBottom: 8 }}>
          逐章校对面板
        </Text>
        <Text type="secondary" style={{ display: 'block', marginBottom: 14, lineHeight: 1.7 }}>
          每个标签页对应一章规划，重点确认节奏、冲突和人物关注点是否符合你想要的展开方向。
        </Text>
        <OutlineChapterPlanTabs plans={response.chapter_plans} isMobile={isMobile} />
      </div>
    </div>
  );
}
