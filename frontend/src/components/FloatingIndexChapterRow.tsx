import { memo } from 'react';
import { List, Typography, theme } from 'antd';
import type { Chapter } from '../types';
import type { FloatingIndexPanelChapterClickHandler } from '../utils/floatingIndexPanelContracts';
import { formatFloatingIndexChapterLabel } from '../utils/floatingIndexPanelViewHelpers';

const { Link, Text } = Typography;

type FloatingIndexChapterRowProps = {
  chapter: Chapter;
  onChapterClick: FloatingIndexPanelChapterClickHandler;
};

function FloatingIndexChapterRow({
  chapter,
  onChapterClick,
}: FloatingIndexChapterRowProps) {
  const { token } = theme.useToken();
  const alphaColor = (color: string, alpha: number) =>
    `color-mix(in srgb, ${color} ${(alpha * 100).toFixed(0)}%, transparent)`;

  return (
    <List.Item style={{ padding: '0 0 8px', borderBlockStart: 'none' }}>
      <Link
        onClick={() => onChapterClick(chapter.id)}
        style={{
          display: 'block',
          width: '100%',
          padding: '10px 12px',
          borderRadius: 14,
          background: alphaColor(token.colorBgElevated, 0.96),
          border: `1px solid ${alphaColor(token.colorPrimary, 0.08)}`,
          color: token.colorText,
        }}
      >
        <Text style={{ color: token.colorText, lineHeight: 1.7 }}>
          {formatFloatingIndexChapterLabel(chapter)}
        </Text>
      </Link>
    </List.Item>
  );
}

export default memo(FloatingIndexChapterRow);
