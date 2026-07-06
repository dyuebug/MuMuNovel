import { Card, Form, InputNumber, Radio, Tag, Typography, theme } from 'antd';
import type { FormInstance } from 'antd';

const { Text } = Typography;

type OutlineBatchExpandConfigFormProps = {
  form: FormInstance;
  outlineCount: number;
};

export default function OutlineBatchExpandConfigForm({
  form,
  outlineCount,
}: OutlineBatchExpandConfigFormProps) {
  const { token } = theme.useToken();
  const alphaColor = (color: string, alpha: number) => `color-mix(in srgb, ${color} ${(alpha * 100).toFixed(0)}%, transparent)`;

  return (
    <div>
      <Card
        size="small"
        style={{
          marginBottom: 16,
          borderRadius: 20,
          border: `1px solid ${alphaColor(token.colorWarning, 0.18)}`,
          background: `linear-gradient(135deg, ${alphaColor(token.colorWarningBg, 0.96)} 0%, ${alphaColor(token.colorBgContainer, 0.98)} 100%)`,
        }}
        styles={{ body: { padding: 16 } }}
      >
        <Text style={{ display: 'block', fontSize: 11, letterSpacing: '0.08em', textTransform: 'uppercase', color: token.colorTextTertiary, marginBottom: 6 }}>
          Batch Expansion
        </Text>
        <Text strong style={{ display: 'block', fontSize: 17, marginBottom: 8 }}>
          批量展开控制台
        </Text>
        <Text type="secondary" style={{ display: 'block', lineHeight: 1.7, marginBottom: 12 }}>
          这一步会一次性处理当前项目下的全部大纲。先确定每条大纲的展开规模和节奏，再进入预览与确认阶段。
        </Text>
        <Tag color="warning">将对当前项目的所有 {outlineCount} 个大纲进行展开</Tag>
      </Card>

      <Card
        size="small"
        style={{
          borderRadius: 20,
          border: `1px solid ${alphaColor(token.colorBorderSecondary, 0.9)}`,
          background: `linear-gradient(180deg, ${alphaColor(token.colorBgElevated, 0.98)} 0%, ${alphaColor(token.colorFillQuaternary, 0.44)} 100%)`,
        }}
        styles={{ body: { padding: 16 } }}
      >
        <Text style={{ display: 'block', fontSize: 11, letterSpacing: '0.08em', textTransform: 'uppercase', color: token.colorTextTertiary, marginBottom: 6 }}>
          Expansion Settings
        </Text>
        <Text strong style={{ display: 'block', marginBottom: 8 }}>
          展开参数
        </Text>
        <Text type="secondary" style={{ display: 'block', lineHeight: 1.7, marginBottom: 14 }}>
          章节数决定单条大纲的切分密度，展开策略决定系统更偏重均衡推进、高潮聚焦还是细节铺陈。
        </Text>
        <Form
          form={form}
          layout="vertical"
          initialValues={{
            chapters_per_outline: 3,
            expansion_strategy: 'balanced',
          }}
        >
          <Form.Item
            label="每个大纲展开章节数"
            name="chapters_per_outline"
            rules={[{ required: true, message: '请输入章节数' }]}
            tooltip="每个大纲将被展开为几章"
          >
            <InputNumber min={2} max={10} style={{ width: '100%' }} placeholder="建议 2-5 章" />
          </Form.Item>

          <Form.Item label="展开策略" name="expansion_strategy" style={{ marginBottom: 0 }}>
            <Radio.Group>
              <Radio.Button value="balanced">均衡分配</Radio.Button>
              <Radio.Button value="climax">高潮重点</Radio.Button>
              <Radio.Button value="detail">细节丰富</Radio.Button>
            </Radio.Group>
          </Form.Item>
        </Form>
      </Card>
    </div>
  );
}
