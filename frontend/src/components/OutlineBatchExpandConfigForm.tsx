import { Form, InputNumber, Radio } from 'antd';
import type { FormInstance } from 'antd';

type OutlineBatchExpandConfigFormProps = {
  form: FormInstance;
  outlineCount: number;
};

export default function OutlineBatchExpandConfigForm({
  form,
  outlineCount,
}: OutlineBatchExpandConfigFormProps) {
  return (
    <div>
      <div style={{ marginBottom: 16, padding: 12, background: 'var(--color-warning-bg)', borderRadius: 4 }}>
        <div style={{ color: '#856404' }}>
          ⚠️ 将对当前项目的所有 {outlineCount} 个大纲进行展开
        </div>
      </div>

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

        <Form.Item label="展开策略" name="expansion_strategy">
          <Radio.Group>
            <Radio.Button value="balanced">均衡分配</Radio.Button>
            <Radio.Button value="climax">高潮重点</Radio.Button>
            <Radio.Button value="detail">细节丰富</Radio.Button>
          </Radio.Group>
        </Form.Item>
      </Form>
    </div>
  );
}
