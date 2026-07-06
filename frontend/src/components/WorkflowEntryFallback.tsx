import { Card, Modal, Spin, Typography, theme } from 'antd';
import { LoadingOutlined } from '@ant-design/icons';

const { Text } = Typography;

type WorkflowEntryFallbackProps = {
  message: string;
  title: string;
  variant?: 'dialog' | 'fullscreen' | 'floating';
  eyebrow?: string;
  tags?: Array<{ label: string; color?: string }>;
};

export default function WorkflowEntryFallback({
  title,
  variant = 'dialog',
}: WorkflowEntryFallbackProps) {
  const { token } = theme.useToken();
  const content = (
    <Card
      bordered={false}
      style={{
        borderRadius: 12,
        border: `1px solid ${token.colorBorderSecondary}`,
        background: token.colorBgElevated,
        boxShadow: `0 12px 28px color-mix(in srgb, ${token.colorText} 12%, transparent)`,
      }}
      styles={{ body: { padding: '14px 16px' } }}
    >
      <div style={{ display: 'flex', alignItems: 'center', gap: 12 }}>
        <Spin indicator={<LoadingOutlined spin style={{ fontSize: 18, color: token.colorPrimary }} />} />
        <Text style={{ color: token.colorTextSecondary, fontSize: 13 }}>{title || '加载中'}</Text>
      </div>
    </Card>
  );

  if (variant === 'fullscreen') {
    return (
      <div
        style={{
          position: 'fixed',
          inset: 0,
          zIndex: 1100,
          display: 'flex',
          alignItems: 'center',
          justifyContent: 'center',
          padding: 16,
          background: 'rgba(0, 0, 0, 0.18)',
        }}
      >
        {content}
      </div>
    );
  }

  if (variant === 'floating') {
    return (
      <div
        style={{
          position: 'fixed',
          right: 'max(16px, env(safe-area-inset-right))',
          bottom: 96,
          width: 'min(320px, calc(100vw - 32px))',
          zIndex: 9999,
          pointerEvents: 'none',
        }}
      >
        {content}
      </div>
    );
  }

  return (
    <Modal
      open
      footer={null}
      closable={false}
      maskClosable={false}
      keyboard={false}
      centered
      width={360}
      styles={{
        content: {
          padding: 0,
          background: 'transparent',
          boxShadow: 'none',
        },
        body: {
          padding: 0,
        },
      }}
    >
      {content}
    </Modal>
  );
}
