import { Button, Form, Input, Modal, Select, Space, Tag } from 'antd';
import type { FormInstance } from 'antd';

type ChapterBasicFormValues = {
  title?: string;
  chapter_number?: number | string;
  status?: 'draft' | 'writing' | 'completed';
};

type ChapterBasicModalProps = {
  open: boolean;
  title: string;
  isMobile: boolean;
  outlineMode: string;
  submitText: string;
  form: FormInstance<ChapterBasicFormValues>;
  onCancel: () => void;
  onFinish: (values: ChapterBasicFormValues) => void | Promise<void>;
};

export default function ChapterBasicModal({
  open,
  title,
  isMobile,
  outlineMode,
  submitText,
  form,
  onCancel,
  onFinish,
}: ChapterBasicModalProps) {
  const isOneToOne = outlineMode === 'one-to-one';

  return (
    <Modal
      title={title}
      open={open}
      onCancel={onCancel}
      footer={null}
      centered
      width={isMobile ? 'calc(100vw - 32px)' : 520}
      style={isMobile ? {
        maxWidth: 'calc(100vw - 32px)',
        margin: '0 auto',
        padding: '0 16px',
      } : undefined}
      styles={{
        body: {
          maxHeight: isMobile ? 'calc(100vh - 200px)' : 'calc(80vh - 110px)',
          overflowY: 'auto',
        },
      }}
    >
      <Form form={form} layout="vertical" onFinish={onFinish}>
        <Form.Item
          label="????"
          name="title"
          tooltip={isOneToOne ? '???????????' : '?????????????'}
          rules={isOneToOne ? undefined : [{ required: true, message: '???????' }]}
        >
          <Input placeholder="???????" disabled={isOneToOne} />
        </Form.Item>

        <Form.Item label="????" name="chapter_number" tooltip="??????">
          <Input type="number" placeholder="???????" />
        </Form.Item>

        <Form.Item label="??" name="status">
          <Select placeholder="?????">
            <Select.Option value="draft">??</Select.Option>
            <Select.Option value="writing">???</Select.Option>
            <Select.Option value="completed">???</Select.Option>
          </Select>
        </Form.Item>

        <Form.Item style={{ marginBottom: 0 }}>
          <div
            style={{
              display: 'flex',
              flexDirection: isMobile ? 'column' : 'row',
              justifyContent: 'space-between',
              alignItems: isMobile ? 'stretch' : 'center',
              gap: 12,
            }}
          >
            <Space wrap size={[8, 8]} style={{ flex: 1, minWidth: 0 }}>
              <Tag color="blue">?????{isOneToOne ? '???' : '???'}</Tag>
              <Tag color="green">???{isOneToOne ? '????' : '????'}</Tag>
            </Space>
            <Space.Compact style={{ width: isMobile ? '100%' : 'auto' }} block={isMobile}>
              <Button onClick={onCancel}>??</Button>
              <Button type="primary" htmlType="submit">{submitText}</Button>
            </Space.Compact>
          </div>
        </Form.Item>
      </Form>
    </Modal>
  );
}
