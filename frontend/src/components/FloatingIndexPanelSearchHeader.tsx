import { memo } from 'react';
import { Input, theme } from 'antd';
import { SearchOutlined } from '@ant-design/icons';
import type { FloatingIndexPanelSearchModel } from '../utils/floatingIndexPanelContracts';
import { FLOATING_INDEX_PANEL_SEARCH_PLACEHOLDER } from '../utils/floatingIndexPanelViewHelpers';

type FloatingIndexPanelSearchHeaderProps = {
  searchModel: FloatingIndexPanelSearchModel;
};

function FloatingIndexPanelSearchHeader({ searchModel }: FloatingIndexPanelSearchHeaderProps) {
  const { onSearchTermChange, searchTerm } = searchModel;
  const { token } = theme.useToken();

  return (
    <div style={{ padding: '16px', borderBottom: `1px solid ${token.colorBorderSecondary}` }}>
      <Input
        placeholder={FLOATING_INDEX_PANEL_SEARCH_PLACEHOLDER}
        prefix={<SearchOutlined />}
        value={searchTerm}
        onChange={onSearchTermChange}
        allowClear
      />
    </div>
  );
}

export default memo(FloatingIndexPanelSearchHeader);