import { memo } from 'react';
import { List, Typography } from 'antd';
import type { Chapter } from '../types';
import type { FloatingIndexPanelChapterClickHandler } from '../utils/floatingIndexPanelContracts';
import { formatFloatingIndexChapterLabel } from '../utils/floatingIndexPanelViewHelpers';

const { Link } = Typography;

type FloatingIndexChapterRowProps = {
  chapter: Chapter;
  onChapterClick: FloatingIndexPanelChapterClickHandler;
};

function FloatingIndexChapterRow({
  chapter,
  onChapterClick,
}: FloatingIndexChapterRowProps) {
  return (
    <List.Item style={{ paddingLeft: 16, borderBlockStart: 'none' }}>
      <Link onClick={() => onChapterClick(chapter.id)}>
        {formatFloatingIndexChapterLabel(chapter)}
      </Link>
    </List.Item>
  );
}

export default memo(FloatingIndexChapterRow);