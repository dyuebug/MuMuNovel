import { memo } from 'react';
import { List, Tag } from 'antd';
import type { Chapter } from '../types';
import FloatingIndexChapterRow from './FloatingIndexChapterRow';
import type { FloatingIndexPanelChapterClickHandler } from '../utils/floatingIndexPanelContracts';

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
  return (
    <List.Item style={{ padding: '0 16px', flexDirection: 'column', alignItems: 'flex-start' }}>
      <div style={{ padding: '12px 0', fontWeight: 'bold' }}>
        <Tag color={outlineTagColor}>
          {outlineLabel}
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
    </List.Item>
  );
}

export default memo(FloatingIndexGroupSection);