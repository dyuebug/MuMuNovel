import { Button, Form, Input, Modal, Select, Space, Tag, Typography, theme } from 'antd';
import type { FormInstance } from 'antd';

const { Text } = Typography;

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
  const { token } = theme.useToken();
  const isOneToOne = outlineMode === 'one-to-one';
  const chapterBasicGuideSteps = [
    '先确认这一章是沿用大纲标题还是手动命名，再决定标题字段是否需要主动填写。',
    '再补齐章节序号和状态，把这条记录的基础排序与工作阶段说明清楚。',
    '最后再提交保存，避免先创建章节、后回头补基础信息。',
  ];
  const chapterBasicWorkspaceFocus = isOneToOne
    ? {
        title: '确认一章一纲模式下的章节基础信息',
        note: '当前标题会自动沿用对应大纲，更适合把注意力放在序号和状态上，确保后续章节列表排序清晰。',
      }
    : {
        title: '先定义这章的命名与状态',
        note: '当前支持手动填写章节标题，适合先把章节定位和当前创作阶段说清楚，再提交这条基础记录。',
      };

  return (
    <Modal
      title={(
        <div>
          <Text style={{ display: 'block', fontSize: 11, letterSpacing: '0.08em', textTransform: 'uppercase', color: token.colorTextTertiary, marginBottom: 6 }}>
            Chapter Basics
          </Text>
          <Text strong style={{ display: 'block', fontSize: 18, marginBottom: 4 }}>
            {title}
          </Text>
          <Text type="secondary">
            先确认章节标题、序号与状态，再进入更复杂的生成或编辑流程。
          </Text>
        </div>
      )}
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
      <div
        style={{
          marginBottom: 16,
          padding: 16,
          borderRadius: 20,
          border: `1px solid ${token.colorBorderSecondary}`,
          background: `linear-gradient(135deg, ${token.colorPrimaryBg} 0%, ${token.colorBgContainer} 100%)`,
          display: 'grid',
          gridTemplateColumns: 'repeat(auto-fit, minmax(220px, 1fr))',
          gap: 16,
        }}
      >
        <div>
          <Text style={{ display: 'block', fontSize: 11, letterSpacing: '0.08em', textTransform: 'uppercase', color: token.colorTextTertiary, marginBottom: 6 }}>
            Basic Guide
          </Text>
          <Text strong style={{ display: 'block', fontSize: 16, marginBottom: 8 }}>
            章节基础信息工作台
          </Text>
          <Text type="secondary" style={{ display: 'block', lineHeight: 1.7, marginBottom: 12 }}>
            这里不改变原有表单提交逻辑，只把填写顺序和判断重点前置，帮助你更快完成这一章的基础建档。
          </Text>
          <div style={{ display: 'flex', flexWrap: 'wrap', gap: 8 }}>
            {chapterBasicGuideSteps.map((item, index) => (
              <span
                key={item}
                style={{
                  display: 'inline-flex',
                  alignItems: 'center',
                  gap: 8,
                  padding: '6px 12px',
                  borderRadius: 999,
                  background: token.colorBgContainer,
                  border: `1px solid ${token.colorBorderSecondary}`,
                  color: token.colorText,
                  fontSize: 12,
                }}
              >
                <span style={{ color: token.colorPrimary, fontWeight: 700 }}>{index + 1}</span>
                {item}
              </span>
            ))}
          </div>
        </div>
        <div
          style={{
            borderRadius: 18,
            padding: '16px 18px 14px',
            background: `linear-gradient(180deg, ${token.colorBgContainer} 0%, ${token.colorFillAlter} 100%)`,
            border: `1px solid ${token.colorBorderSecondary}`,
          }}
        >
          <Text style={{ display: 'block', fontSize: 11, letterSpacing: '0.08em', textTransform: 'uppercase', color: token.colorTextTertiary, marginBottom: 6 }}>
            当前工作焦点
          </Text>
          <Text strong style={{ display: 'block', fontSize: 15, marginBottom: 8 }}>
            {chapterBasicWorkspaceFocus.title}
          </Text>
          <Text type="secondary" style={{ display: 'block', lineHeight: 1.7, marginBottom: 12 }}>
            {chapterBasicWorkspaceFocus.note}
          </Text>
          <Space wrap size={[8, 8]}>
            <Tag color="blue">标题模式：{isOneToOne ? '沿用大纲' : '手动填写'}</Tag>
            <Tag color="green">结构模式：{isOneToOne ? '一章一纲' : '自由章节'}</Tag>
          </Space>
        </div>
      </div>

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
