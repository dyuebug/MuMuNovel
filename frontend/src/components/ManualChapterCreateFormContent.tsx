import { Form, Input, InputNumber, Select } from 'antd';
import type { FormInstance } from 'antd';
import type { Chapter } from '../types';

const { TextArea } = Input;

export type ManualChapterCreateOutlineOption = {
  id: string;
  order_index: number;
  title: string;
};

export type ManualChapterCreateFormValues = {
  chapter_number: number;
  title: string;
  outline_id: string;
  summary?: string;
  status: Chapter['status'];
};

type ManualChapterCreateFormContentProps = {
  form: FormInstance<ManualChapterCreateFormValues>;
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
        label="章节序号"
        name="chapter_number"
        rules={[{ required: true, message: '请输入章节序号' }]}
        tooltip="用于章节排序"
      >
        <InputNumber min={1} style={{ width: '100%' }} placeholder="请输入章节序号" />
      </Form.Item>

      <Form.Item
        label="章节标题"
        name="title"
        rules={[{ required: true, message: '请输入章节标题' }]}
      >
        <Input placeholder="请输入章节标题" />
      </Form.Item>

      <Form.Item
        label="所属大纲"
        name="outline_id"
        rules={[{ required: true, message: '请选择所属大纲' }]}
        tooltip="每个章节都需要归属一个大纲"
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
        label="章节摘要"
        name="summary"
        tooltip="用于记录本章内容摘要"
      >
        <TextArea rows={4} placeholder="请输入章节摘要" />
      </Form.Item>

      <Form.Item label="章节状态" name="status">
        <Select>
          <Select.Option value="draft">草稿</Select.Option>
          <Select.Option value="writing">写作中</Select.Option>
          <Select.Option value="completed">已完成</Select.Option>
        </Select>
      </Form.Item>
    </Form>
  );
}