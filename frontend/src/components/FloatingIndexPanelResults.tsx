import { memo } from 'react';
import { Empty, List } from 'antd';
import FloatingIndexGroupSection from './FloatingIndexGroupSection';
import type { FloatingIndexPanelResultsModel } from '../utils/floatingIndexPanelContracts';
import { FLOATING_INDEX_PANEL_EMPTY_DESCRIPTION } from '../utils/floatingIndexPanelViewHelpers';

type FloatingIndexPanelResultsProps = {
  resultsModel: FloatingIndexPanelResultsModel;
};

function FloatingIndexPanelResults({ resultsModel }: FloatingIndexPanelResultsProps) {
  const { filteredGroups, onChapterClick } = resultsModel;

  return filteredGroups.length > 0 ? (
    <List
      rowKey={(group) => group.key}
      dataSource={filteredGroups}
      renderItem={(group) => (
        <FloatingIndexGroupSection
          chapters={group.chapters}
          onChapterClick={onChapterClick}
          outlineLabel={group.outlineLabel}
          outlineTagColor={group.outlineTagColor}
        />
      )}
      style={{
        height: 'calc(100dvh - 120px)',
        maxHeight: 'calc(100dvh - 120px)',
        overflowY: 'auto',
        overflowX: 'hidden',
      }}
    />
  ) : (
    <Empty description={FLOATING_INDEX_PANEL_EMPTY_DESCRIPTION} style={{ marginTop: 48 }} />
  );
}

export default memo(FloatingIndexPanelResults);