import { memo } from 'react';
import { List, Tag, Typography, theme } from 'antd';
import type { Chapter } from '../types';
import FloatingIndexChapterRow from './FloatingIndexChapterRow';
import type { FloatingIndexPanelChapterClickHandler } from '../utils/floatingIndexPanelContracts';
import { designDisplayFont } from '../theme/themeConfig';

const { Text } = Typography;

type FloatingIndexGroupSectionProps = {
  chapters: Chapter[];
  onChapterClick: FloatingIndexPanelChapterClickHandler;
  outlineLabel: string;
  outlineTagColor: 'blue' | 'default';
};

function FloatingIndexGroupSection({
  chapters,
  onChapterClick,
  outlineLabel,
  outlineTagColor,
}: FloatingIndexGroupSectionProps) {
  const { token } = theme.useToken();
  const alphaColor = (color: string, alpha: number) =>
    `color-mix(in srgb, ${color} ${(alpha * 100).toFixed(0)}%, transparent)`;

  return (
    <List.Item style={{ padding: '0 0 12px', flexDirection: 'column', alignItems: 'stretch', borderBlockStart: 'none' }}>
      <div
        style={{
          borderRadius: 18,
          padding: '14px 16px',
          background: `linear-gradient(180deg, ${alphaColor(token.colorBgContainer, 0.98)} 0%, ${alphaColor(token.colorFillQuaternary, 0.46)} 100%)`,
          border: `1px solid ${alphaColor(token.colorPrimary, 0.1)}`,
        }}
      >
        <div
          style={{
            display: 'grid',
            gridTemplateColumns: 'minmax(0, 1fr) auto',
            gap: 12,
            alignItems: 'start',
            marginBottom: 10,
          }}
        >
          <div style={{ minWidth: 0 }}>
            <Text style={{ display: 'block', fontSize: 11, letterSpacing: '0.08em', textTransform: 'uppercase', color: token.colorTextTertiary, marginBottom: 6 }}>
              Outline Group
            </Text>
            <Text
              strong
              style={{
                display: 'block',
                fontSize: 15,
                fontFamily: designDisplayFont,
                letterSpacing: '-0.02em',
              }}
            >
              {outlineLabel}
            </Text>
          </div>
          <Tag color={outlineTagColor} style={{ margin: 0, borderRadius: 999, paddingInline: 10 }}>
            章节 {chapters.length}
          </Tag>
        </div>
        <List
          rowKey="id"
          size="small"
          dataSource={chapters}
          renderItem={(chapter) => (
            <FloatingIndexChapterRow
              chapter={chapter}
              onChapterClick={onChapterClick}
            />
          )}
          split={false}
        />
      </div>
    </List.Item>
  );
}

export default memo(FloatingIndexGroupSection);
