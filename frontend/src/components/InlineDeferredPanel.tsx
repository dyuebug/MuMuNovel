import { Card, Spin, Typography, theme } from 'antd';
import { LoadingOutlined } from '@ant-design/icons';

const { Text } = Typography;

type InlineDeferredPanelProps = {
  title: string;
  message: string;
  eyebrow?: string;
  minHeight?: number | string;
  tags?: Array<{ label: string; color?: string }>;
};

export default function InlineDeferredPanel({
  title,
  minHeight = 96,
}: InlineDeferredPanelProps) {
  const { token } = theme.useToken();

  return (
    <Card
      bordered={false}
      style={{
        minHeight,
        borderRadius: 10,
        background: token.colorBgContainer,
        border: `1px solid ${token.colorBorderSecondary}`,
      }}
      styles={{ body: { padding: 16, height: '100%' } }}
    >
      <div style={{ display: 'flex', alignItems: 'center', gap: 12, minHeight: 44 }}>
        <Spin indicator={<LoadingOutlined spin style={{ fontSize: 18, color: token.colorPrimary }} />} />
        <Text style={{ color: token.colorTextSecondary, fontSize: 13 }}>{title || '加载中'}</Text>
      </div>
    </Card>
  );
}
