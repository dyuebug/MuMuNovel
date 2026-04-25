import type { PropsWithChildren } from 'react';
import { memo } from 'react';
import { Drawer } from 'antd';
import { FLOATING_INDEX_PANEL_TITLE } from '../utils/floatingIndexPanelViewHelpers';

type FloatingIndexPanelDrawerProps = PropsWithChildren<{
  visible: boolean;
  onClose: () => void;
}>;

function FloatingIndexPanelDrawer({ children, visible, onClose }: FloatingIndexPanelDrawerProps) {
  return (
    <Drawer
      title={FLOATING_INDEX_PANEL_TITLE}
      placement="right"
      onClose={onClose}
      open={visible}
      width="min(320px, calc(100vw - 24px))"
      styles={{
        body: {
          padding: 0,
          maxHeight: 'calc(100dvh - 56px)',
          overflow: 'hidden',
        },
      }}
    >
      {children}
    </Drawer>
  );
}

export default memo(FloatingIndexPanelDrawer);