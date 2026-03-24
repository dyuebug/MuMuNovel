import { memo, useCallback, useMemo, useState } from 'react';
import { Drawer, Empty, Input, List, Tag, Typography, theme } from 'antd';
import { SearchOutlined } from '@ant-design/icons';
import type { Chapter } from '../types';

const { Link } = Typography;

type GroupedChapters = {
  outlineId: string | null;
  outlineTitle: string;
  chapters: Chapter[];
};

type FloatingIndexPanelProps = {
  visible: boolean;
  onClose: () => void;
  groupedChapters: GroupedChapters[];
  onChapterSelect: (chapterId: string) => void;
};

function FloatingIndexPanel({
  visible,
  onClose,
  groupedChapters,
  onChapterSelect,
}: FloatingIndexPanelProps) {
  const { token } = theme.useToken();
  const [searchTerm, setSearchTerm] = useState('');

  const normalizedSearchTerm = useMemo(() => searchTerm.trim().toLowerCase(), [searchTerm]);

  const filteredGroups = useMemo(() => {
    if (!normalizedSearchTerm) {
      return groupedChapters;
    }

    return groupedChapters
      .map((group) => ({
        ...group,
        chapters: group.chapters.filter((chapter) => chapter.title.toLowerCase().includes(normalizedSearchTerm)),
      }))
      .filter((group) => group.chapters.length > 0);
  }, [groupedChapters, normalizedSearchTerm]);

  const handleChapterClick = useCallback((chapterId: string) => {
    onChapterSelect(chapterId);
    onClose();
  }, [onChapterSelect, onClose]);

  return (
    <Drawer
      title="????"
      placement="right"
      onClose={onClose}
      open={visible}
      width={320}
      styles={{
        body: { padding: 0 },
      }}
    >
      <div style={{ padding: '16px', borderBottom: `1px solid ${token.colorBorderSecondary}` }}>
        <Input
          placeholder="??????"
          prefix={<SearchOutlined />}
          value={searchTerm}
          onChange={(event) => setSearchTerm(event.target.value)}
          allowClear
        />
      </div>

      {filteredGroups.length > 0 ? (
        <List
          rowKey={(group) => group.outlineId ?? 'uncategorized'}
          dataSource={filteredGroups}
          renderItem={(group) => (
            <List.Item style={{ padding: '0 16px', flexDirection: 'column', alignItems: 'flex-start' }}>
              <div style={{ padding: '12px 0', fontWeight: 'bold' }}>
                <Tag color={group.outlineId ? 'blue' : 'default'}>
                  {group.outlineTitle}
                </Tag>
              </div>
              <List
                rowKey="id"
                size="small"
                dataSource={group.chapters}
                renderItem={(chapter) => (
                  <List.Item style={{ paddingLeft: 16, borderBlockStart: 'none' }}>
                    <Link onClick={() => handleChapterClick(chapter.id)}>
                      {`?${chapter.chapter_number}? ${chapter.title}`}
                    </Link>
                  </List.Item>
                )}
                split={false}
              />
            </List.Item>
          )}
          style={{ height: 'calc(100vh - 120px)', overflowY: 'auto' }}
        />
      ) : (
        <Empty description="?????????" style={{ marginTop: 48 }} />
      )}
    </Drawer>
  );
}

export default memo(FloatingIndexPanel);
