import { Card, Space, Tag, Typography, theme } from 'antd';
import type { ExpansionPlanData } from '../types';
import {
  renderCompactFactGrid,
  renderCompactListCard,
  renderCompactSelectionSummary,
  renderCompactSettingHint,
  renderCompactStoryControlHeader,
} from './storyCreationCommonUi';

const { Text } = Typography;

type ChapterExpansionPlanPreviewContentProps = {
  chapterTitle?: string | null;
  isMobile: boolean;
  planData: ExpansionPlanData;
};

export default function ChapterExpansionPlanPreviewContent({
  chapterTitle,
  isMobile,
  planData,
}: ChapterExpansionPlanPreviewContentProps) {
  const { token } = theme.useToken();
  const alphaColor = (color: string, alpha: number) => `color-mix(in srgb, ${color} ${(alpha * 100).toFixed(0)}%, transparent)`;
  const scenes = planData.scenes ?? [];
  const chapterExpansionGuideSteps = [
    '先确认这一章的目标、冲突和情绪基调，判断这份扩写规划是否真的服务于当前章节。',
    '再看关键事件、角色焦点和场景列表，优先确认哪些信息足够支撑正式写作。',
    '最后把这份规划当作写作参考，而不是固定剧本，保留后续微调空间。',
  ];
  const chapterExpansionWorkspaceFocus = scenes.length > 1
    ? {
        title: `优先检查这 ${scenes.length} 个场景之间的推进关系`,
        note: '当前场景数量较多，适合先看场景目的与角色分布是否自然衔接，再进入正式起稿。',
      }
    : {
        title: '先确认这一章的核心冲突和写作目标是否足够清晰',
        note: '当前更适合先核对目标、冲突与重点事件是否成立，再决定是否直接沿用这份扩写规划。',
      };

  return (
    <div style={{ marginTop: 16 }}>
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
              Chapter Plan Guide
            </Text>
            <Text strong style={{ display: 'block', fontSize: 17, marginBottom: 8 }}>
              章节扩写规划预览
            </Text>
            <Text type="secondary" style={{ display: 'block', lineHeight: 1.7, marginBottom: 12 }}>
              这里展示的是单章扩写的参考规划。当前只补充阅读顺序与工作重点，不改变任何章节内容、生成逻辑或后续写作状态。
            </Text>
            <div style={{ display: 'flex', flexWrap: 'wrap', gap: 8 }}>
              {chapterExpansionGuideSteps.map((item, index) => (
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
              {chapterExpansionWorkspaceFocus.title}
            </Text>
            <Text type="secondary" style={{ display: 'block', lineHeight: 1.7, marginBottom: 12 }}>
              {chapterExpansionWorkspaceFocus.note}
            </Text>
            <Space wrap>
              <Tag color="blue">章节: {chapterTitle || 'Untitled chapter'}</Tag>
              <Tag color="green">关键事件: {planData.key_events.length}</Tag>
              <Tag color="cyan">角色焦点: {planData.character_focus.length}</Tag>
              <Tag color="purple">场景数: {scenes.length}</Tag>
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
          Chapter Planning Workspace
        </Text>
        <Text strong style={{ display: 'block', marginBottom: 8 }}>
          单章扩写结构面板
        </Text>
        <Text type="secondary" style={{ display: 'block', marginBottom: 14, lineHeight: 1.7 }}>
          建议先从章节目标与冲突入手，再检查事件、人物和场景是否形成一致的推进节奏。
        </Text>
        {renderCompactFactGrid(
          [
            ['Chapter title', chapterTitle || 'Untitled chapter'],
            ['Emotional tone', planData.emotional_tone],
            ['Conflict type', planData.conflict_type],
            ['Estimated words', `${planData.estimated_words} words`],
            ['Narrative goal', planData.narrative_goal],
          ],
          {
            minColumnWidth: isMobile ? 160 : 220,
            style: { marginBottom: 12 },
          },
        )}

        {renderCompactListCard('Key events', planData.key_events, {
          numbered: true,
          tagText: `${planData.key_events.length} items`,
          tagColor: 'purple',
          style: { marginBottom: 12 },
        })}

        {planData.character_focus.length > 0 && (
          <div style={{ marginBottom: 12 }}>
            {renderCompactStoryControlHeader(
              'Focus characters',
              'These characters carry the main dramatic weight in this chapter.',
              {
                tagText: `${planData.character_focus.length} characters`,
                tagColor: 'cyan',
                style: { marginBottom: 8 },
              },
            )}
            {renderCompactSelectionSummary(
              planData.character_focus.map((character) => ({
                label: 'Character',
                value: character,
                color: 'cyan',
              })),
              { style: { marginBottom: 0 } },
            )}
          </div>
        )}

        {scenes.length > 0 && (
          <div style={{ marginBottom: 12 }}>
            {renderCompactStoryControlHeader(
              'Scene list',
              'Review each scene location, purpose, and cast before drafting.',
              {
                tagText: `${scenes.length} scenes`,
                tagColor: 'purple',
                style: { marginBottom: 8 },
              },
            )}
            <Space direction="vertical" size="small" style={{ width: '100%' }}>
              {scenes.map((scene, index) => (
                <Card
                  key={`${scene.location || 'scene'}-${index}`}
                  size="small"
                  style={{
                    backgroundColor: alphaColor(token.colorBgContainer, 0.94),
                    maxWidth: '100%',
                    overflow: 'hidden',
                    borderRadius: 16,
                    border: `1px solid ${alphaColor(token.colorBorderSecondary, 0.9)}`,
                  }}
                >
                  {renderCompactStoryControlHeader(
                    `Scene ${index + 1}`,
                    scene.location || 'Location not specified',
                    {
                      tagText: scene.characters?.length ? `${scene.characters.length} characters` : undefined,
                      tagColor: 'blue',
                      style: { marginBottom: 8 },
                    },
                  )}
                  {renderCompactFactGrid(
                    [
                      ['Scene location', scene.location || 'Not specified'],
                      ['Scene purpose', scene.purpose || 'Not specified'],
                    ],
                    {
                      minColumnWidth: isMobile ? 160 : 220,
                      style: { marginBottom: scene.characters?.length ? 8 : 0 },
                    },
                  )}
                  {scene.characters?.length > 0
                    ? renderCompactSelectionSummary(
                        scene.characters.map((character) => ({
                          label: 'Character',
                          value: character,
                          color: 'cyan',
                        })),
                        { style: { marginBottom: 0 } },
                      )
                    : null}
                </Card>
              ))}
            </Space>
          </div>
        )}

        {renderCompactSettingHint(
          'The expansion plan is a writing aid only.',
          'Before drafting, review scenes, conflicts, and character goals, then adjust the plan as needed.',
          { style: { marginTop: 16, marginBottom: 0 } },
        )}
      </div>
    </div>
  );
}
