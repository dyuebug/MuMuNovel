import { memo } from 'react';
import { Input, Space, Tag, Typography, theme } from 'antd';
import { SearchOutlined } from '@ant-design/icons';
import type { FloatingIndexPanelSearchModel } from '../utils/floatingIndexPanelContracts';
import { FLOATING_INDEX_PANEL_SEARCH_PLACEHOLDER } from '../utils/floatingIndexPanelViewHelpers';
import { designDisplayFont } from '../theme/themeConfig';

const { Text } = Typography;

type FloatingIndexPanelSearchHeaderProps = {
  searchModel: FloatingIndexPanelSearchModel;
};

function FloatingIndexPanelSearchHeader({ searchModel }: FloatingIndexPanelSearchHeaderProps) {
  const { onSearchTermChange, searchTerm } = searchModel;
  const { token } = theme.useToken();
  const alphaColor = (color: string, alpha: number) =>
    `color-mix(in srgb, ${color} ${(alpha * 100).toFixed(0)}%, transparent)`;
  const hasSearch = searchTerm.trim().length > 0;

  return (
    <div
      style={{
        margin: '14px 16px 0',
        padding: '14px 16px 16px',
        borderRadius: 18,
        border: `1px solid ${alphaColor(token.colorPrimary, 0.12)}`,
        background: `linear-gradient(180deg, ${alphaColor(token.colorBgContainer, 0.98)} 0%, ${alphaColor(token.colorFillQuaternary, 0.56)} 100%)`,
      }}
    >
      <div style={{ display: 'grid', gap: 10 }}>
        <div
          style={{
            display: 'grid',
            gridTemplateColumns: 'minmax(0, 1fr) auto',
            gap: 12,
            alignItems: 'start',
          }}
        >
          <div style={{ minWidth: 0 }}>
            <Text style={{ display: 'block', fontSize: 11, letterSpacing: '0.08em', textTransform: 'uppercase', color: token.colorTextTertiary, marginBottom: 6 }}>
              Search Focus
            </Text>
            <Text
              strong
              style={{
                display: 'block',
                fontSize: 16,
                marginBottom: 6,
                fontFamily: designDisplayFont,
                letterSpacing: '-0.02em',
              }}
            >
              先用关键词把目录范围缩到足够小
            </Text>
            <Text type="secondary" style={{ display: 'block', lineHeight: 1.7 }}>
              搜索只改变目录聚焦范围，不改变章节本身。更适合先按关键词缩小到一组候选章节，再执行跳转。
            </Text>
          </div>
          <Space wrap size={[8, 8]} style={{ justifyContent: 'flex-end' }}>
            <Tag color={hasSearch ? 'green' : 'default'} style={{ margin: 0, borderRadius: 999, paddingInline: 10 }}>
              {hasSearch ? '检索中' : '浏览全部'}
            </Tag>
            {hasSearch ? (
              <Tag color="blue" style={{ margin: 0, borderRadius: 999, paddingInline: 10 }}>
                {searchTerm}
              </Tag>
            ) : null}
          </Space>
        </div>
      <Input
        placeholder={FLOATING_INDEX_PANEL_SEARCH_PLACEHOLDER}
        prefix={<SearchOutlined />}
        value={searchTerm}
        onChange={onSearchTermChange}
        allowClear
        size="large"
        style={{ borderRadius: 14 }}
      />
    </div>
    </div>
  );
}

export default memo(FloatingIndexPanelSearchHeader);
