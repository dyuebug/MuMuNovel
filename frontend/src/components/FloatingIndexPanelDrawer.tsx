import type { PropsWithChildren } from 'react';
import { memo } from 'react';
import { Drawer, Space, Tag, Typography, theme } from 'antd';
import { FLOATING_INDEX_PANEL_TITLE } from '../utils/floatingIndexPanelViewHelpers';
import { designDisplayFont } from '../theme/themeConfig';

type FloatingIndexPanelDrawerProps = PropsWithChildren<{
  visible: boolean;
  onClose: () => void;
  groupCount: number;
  chapterCount: number;
  hasSearch: boolean;
}>;

function FloatingIndexPanelDrawer({
  children,
  visible,
  onClose,
  groupCount,
  chapterCount,
  hasSearch,
}: FloatingIndexPanelDrawerProps) {
  const { token } = theme.useToken();
  const alphaColor = (color: string, alpha: number) => `color-mix(in srgb, ${color} ${(alpha * 100).toFixed(0)}%, transparent)`;

  return (
    <Drawer
      title={
        <Space direction="vertical" size={4} style={{ width: '100%' }}>
          <Typography.Text style={{ fontSize: 11, letterSpacing: '0.12em', textTransform: 'uppercase', color: token.colorTextTertiary }}>
            Index Workspace
          </Typography.Text>
          <Typography.Title level={5} style={{ margin: 0, fontFamily: designDisplayFont }}>
            {FLOATING_INDEX_PANEL_TITLE}
          </Typography.Title>
          <Typography.Text type="secondary" style={{ fontSize: 12, lineHeight: 1.65 }}>
            用更轻量的抽屉入口快速切到目标章节，保持“导航”与“正文创作”分离。
          </Typography.Text>
        </Space>
      }
      placement="right"
      onClose={onClose}
      open={visible}
      width="min(320px, calc(100vw - 24px))"
      extra={
        <Space size={[6, 6]} wrap style={{ justifyContent: 'flex-end' }}>
          <Tag color="blue" style={{ margin: 0, borderRadius: 999, paddingInline: 10 }}>
            目录组 {groupCount}
          </Tag>
          <Tag color="cyan" style={{ margin: 0, borderRadius: 999, paddingInline: 10 }}>
            章节 {chapterCount}
          </Tag>
          <Tag color={hasSearch ? 'green' : 'default'} style={{ margin: 0, borderRadius: 999, paddingInline: 10 }}>
            {hasSearch ? '检索中' : '浏览中'}
          </Tag>
        </Space>
      }
      styles={{
        header: {
          padding: '18px 18px 14px',
          borderBottom: `1px solid ${alphaColor(token.colorBorderSecondary, 0.9)}`,
          background: `linear-gradient(135deg, ${alphaColor(token.colorPrimaryBg, 0.84)} 0%, ${alphaColor(token.colorBgElevated, 0.98)} 100%)`,
        },
        body: {
          padding: 0,
          maxHeight: 'calc(100dvh - 56px)',
          overflow: 'hidden',
          background: `linear-gradient(180deg, ${alphaColor(token.colorBgContainer, 0.98)} 0%, ${alphaColor(token.colorFillAlter, 0.3)} 100%)`,
        },
      }}
    >
      {children}
    </Drawer>
  );
}

export default memo(FloatingIndexPanelDrawer);
