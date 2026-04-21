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

      {planData.scenes && planData.scenes.length > 0 && (
        <div style={{ marginBottom: 12 }}>
          {renderCompactStoryControlHeader(
            'Scene list',
            'Review each scene location, purpose, and cast before drafting.',
            {
              tagText: `${planData.scenes.length} scenes`,
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
  );
}