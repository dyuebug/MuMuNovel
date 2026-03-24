import { Card, Space } from 'antd';
import type { ExpansionPlanData } from '../types';
import {
  renderCompactFactGrid,
  renderCompactListCard,
  renderCompactSelectionSummary,
  renderCompactSettingHint,
  renderCompactStoryControlHeader,
} from './storyCreationCommonUi';

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
  return (
    <div style={{ marginTop: 16 }}>
      {renderCompactFactGrid(
        [
          ['章节标题', chapterTitle || '未命名章节'],
          ['情感基调', planData.emotional_tone],
          ['冲突类型', planData.conflict_type],
          ['预计字数', `${planData.estimated_words} 字`],
          ['叙事目标', planData.narrative_goal],
        ],
        {
          minColumnWidth: isMobile ? 160 : 220,
          style: { marginBottom: 12 },
        },
      )}

      {renderCompactListCard('关键事件', planData.key_events, {
        numbered: true,
        tagText: `${planData.key_events.length} 项`,
        tagColor: 'purple',
        style: { marginBottom: 12 },
      })}

      {planData.character_focus.length > 0 && (
        <div style={{ marginBottom: 12 }}>
          {renderCompactStoryControlHeader(
            '关注角色',
            '这些角色会在本章承担主要戏份。',
            {
              tagText: `${planData.character_focus.length} 人`,
              tagColor: 'cyan',
              style: { marginBottom: 8 },
            },
          )}
          {renderCompactSelectionSummary(
            planData.character_focus.map((char) => ({ label: '角色', value: char, color: 'cyan' })),
            { style: { marginBottom: 0 } },
          )}
        </div>
      )}

      {planData.scenes && planData.scenes.length > 0 && (
        <div style={{ marginBottom: 12 }}>
          {renderCompactStoryControlHeader(
            '场景列表',
            '按场景查看地点、角色与目的，落笔时更容易照着走。',
            {
              tagText: `${planData.scenes.length} 段`,
              tagColor: 'purple',
              style: { marginBottom: 8 },
            },
          )}
          <Space direction="vertical" size="small" style={{ width: '100%' }}>
            {planData.scenes.map((scene, index) => (
              <Card
                key={`${scene.location || 'scene'}-${index}`}
                size="small"
                style={{
                  backgroundColor: '#fafafa',
                  maxWidth: '100%',
                  overflow: 'hidden',
                }}
              >
                {renderCompactStoryControlHeader(
                  `场景 ${index + 1}`,
                  scene.location || '未填写地点',
                  {
                    tagText: scene.characters?.length ? `${scene.characters.length} 人` : undefined,
                    tagColor: 'blue',
                    style: { marginBottom: 8 },
                  },
                )}
                {renderCompactFactGrid(
                  [
                    ['场景地点', scene.location || '未填写'],
                    ['场景目的', scene.purpose || '未填写'],
                  ],
                  {
                    minColumnWidth: isMobile ? 160 : 220,
                    style: { marginBottom: scene.characters?.length ? 8 : 0 },
                  },
                )}
                {scene.characters?.length > 0
                  ? renderCompactSelectionSummary(
                      scene.characters.map((char) => ({ label: '角色', value: char, color: 'cyan' })),
                      { style: { marginBottom: 0 } },
                    )
                  : null}
              </Card>
            ))}
          </Space>
        </div>
      )}

      {renderCompactSettingHint(
        '扩写计划只作为写作辅助。',
        '落笔前建议再核对场景、冲突与角色目标，再按实际需要微调。',
        { style: { marginTop: 16, marginBottom: 0 } },
      )}
    </div>
  );
}
