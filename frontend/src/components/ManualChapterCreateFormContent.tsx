import { Form, Input, InputNumber, Select } from 'antd';
import type { FormInstance } from 'antd';

const { TextArea } = Input;

type ManualChapterCreateOutlineOption = {
  id: string;
  order_index: number;
  title: string;
};

type ManualChapterCreateFormContentProps = {
  form: FormInstance;
  nextChapterNumber: number;
  sortedOutlines: ManualChapterCreateOutlineOption[];
};

export default function ManualChapterCreateFormContent({
  form,
  nextChapterNumber,
  sortedOutlines,
}: ManualChapterCreateFormContentProps) {
  return (
    <Form
      form={form}
      layout="vertical"
      initialValues={{
        chapter_number: nextChapterNumber,
        status: 'draft',
      }}
      style={{ marginTop: 16 }}
    >
      <Form.Item
        label="章节编号"
        name="chapter_number"
        rules={[{ required: true, message: '请输入章节编号' }]}
        tooltip="用于章节排序"
      >
        <InputNumber min={1} style={{ width: '100%' }} placeholder="请输入章节编号" />
      </Form.Item>

      <Form.Item
        label="章节标题"
        name="title"
        rules={[{ required: true, message: '请输入章节标题' }]}
      >
        <Input placeholder="请输入章节标题" />
      </Form.Item>

      <Form.Item
        label="大纲"
        name="outline_id"
        rules={[{ required: true, message: '请选择大纲' }]}
        tooltip="每章必须归属到某个大纲"
      >
        <Select placeholder="请选择大纲">
          {sortedOutlines.map((outline) => (
            <Select.Option key={outline.id} value={outline.id}>
              {`#${outline.order_index} ${outline.title}`}
            </Select.Option>
          ))}
        </Select>
      </Form.Item>

      <Form.Item
        label="概要"
        name="summary"
        tooltip="章节的简要概要。"
      >
        <TextArea rows={4} placeholder="请输入简要概要" />
      </Form.Item>

      <Form.Item label="状态" name="status">
        <Select>
          <Select.Option value="draft">草稿</Select.Option>
          <Select.Option value="writing">写作中</Select.Option>
          <Select.Option value="completed">已完成</Select.Option>
        </Select>
      </Form.Item>
    </Form>
  );
}
