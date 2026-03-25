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
          label="章节标题"
          name="title"
          tooltip={isOneToOne ? "一章一纲模式下会自动沿用对应大纲标题" : "可手动填写章节标题，便于后续检索与排序"}
          rules={isOneToOne ? undefined : [{ required: true, message: "请输入章节标题" }]}
        >
          <Input placeholder="请输入章节标题" disabled={isOneToOne} />
        </Form.Item>

        <Form.Item label="章节序号" name="chapter_number" tooltip="用于排序与显示">
          <Input type="number" placeholder="请输入章节序号" />
        </Form.Item>

        <Form.Item label="章节状态" name="status">
          <Select placeholder="请选择状态">
            <Select.Option value="draft">草稿</Select.Option>
            <Select.Option value="writing">创作中</Select.Option>
            <Select.Option value="completed">已完成</Select.Option>
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
              <Tag color="blue">标题模式：{isOneToOne ? "沿用大纲" : "手动填写"}</Tag>
              <Tag color="green">结构模式：{isOneToOne ? "一章一纲" : "自由章节"}</Tag>
            </Space>
            <Space.Compact style={{ width: isMobile ? '100%' : 'auto' }} block={isMobile}>
              <Button onClick={onCancel}>取消</Button>
              <Button type="primary" htmlType="submit">{submitText}</Button>
            </Space.Compact>
          </div>
        </Form.Item>
      </Form>
    </Modal>
  );
}
