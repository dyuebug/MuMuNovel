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
        label="Chapter number"
        name="chapter_number"
        rules={[{ required: true, message: 'Please enter the chapter number' }]}
        tooltip="Used for chapter ordering"
      >
        <InputNumber min={1} style={{ width: '100%' }} placeholder="Enter the chapter number" />
      </Form.Item>

      <Form.Item
        label="Chapter title"
        name="title"
        rules={[{ required: true, message: 'Please enter the chapter title' }]}
      >
        <Input placeholder="Enter the chapter title" />
      </Form.Item>

      <Form.Item
        label="Outline"
        name="outline_id"
        rules={[{ required: true, message: 'Please select an outline' }]}
        tooltip="Each chapter must belong to an outline"
      >
        <Select placeholder="Select an outline">
          {sortedOutlines.map((outline) => (
            <Select.Option key={outline.id} value={outline.id}>
              {`#${outline.order_index} ${outline.title}`}
            </Select.Option>
          ))}
        </Select>
      </Form.Item>

      <Form.Item
        label="Summary"
        name="summary"
        tooltip="A short summary of this chapter"
      >
        <TextArea rows={4} placeholder="Enter a short summary" />
      </Form.Item>

      <Form.Item label="Status" name="status">
        <Select>
          <Select.Option value="draft">Draft</Select.Option>
          <Select.Option value="writing">Writing</Select.Option>
          <Select.Option value="completed">Completed</Select.Option>
        </Select>
      </Form.Item>
    </Form>
  );
}