import { Card, Spin, Typography, theme } from 'antd';
import { LoadingOutlined } from '@ant-design/icons';

const { Text } = Typography;

type AmbientDeferredFallbackProps = {
  title: string;
  message: string;
  eyebrow?: string;
  tags?: Array<{ label: string; color?: string }>;
  variant?: 'footer' | 'floating';
  bottomOffset?: number;
};

export default function AmbientDeferredFallback({
  title,
  variant = 'footer',
  bottomOffset = 24,
}: AmbientDeferredFallbackProps) {
  const { token } = theme.useToken();
  const content = (
    <Card
      bordered={false}
      style={{
        borderRadius: 10,
        background: token.colorBgElevated,
        border: `1px solid ${token.colorBorderSecondary}`,
        boxShadow: `0 10px 24px color-mix(in srgb, ${token.colorText} 8%, transparent)`,
      }}
      styles={{ body: { padding: '10px 12px' } }}
    >
      <div style={{ display: 'flex', alignItems: 'center', gap: 10 }}>
        <Spin indicator={<LoadingOutlined spin style={{ fontSize: 16, color: token.colorPrimary }} />} />
        <Text style={{ color: token.colorTextSecondary, fontSize: 12 }}>{title || '加载中'}</Text>
      </div>
    </Card>
  );

  if (variant === 'floating') {
    return (
      <div
        style={{
          position: 'fixed',
          left: 24,
          bottom: bottomOffset,
          width: 'min(320px, calc(100vw - 32px))',
          zIndex: 920,
          pointerEvents: 'none',
        }}
      >
        {content}
      </div>
    );
  }

  return (
    <div
      style={{
        width: 'min(720px, calc(100% - 32px))',
        margin: '0 auto',
        padding: '8px 0 0',
      }}
    >
      {content}
    </div>
  );
}
