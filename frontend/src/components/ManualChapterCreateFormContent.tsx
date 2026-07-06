import { Card, Form, Input, InputNumber, Select, Space, Tag, Typography, theme } from 'antd';
import type { FormInstance } from 'antd';
import type { Chapter } from '../types';
import { designDisplayFont } from '../theme/themeConfig';

const { TextArea } = Input;
const { Text, Paragraph, Title } = Typography;

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
  const { token } = theme.useToken();
  const heroBackground = `linear-gradient(135deg,
    color-mix(in srgb, ${token.colorPrimary} 80%, #704734 20%) 0%,
    color-mix(in srgb, ${token.colorInfo} 30%, #1f262e 70%) 100%)`;
  const quietPanelBackground = `linear-gradient(180deg,
    color-mix(in srgb, ${token.colorBgContainer} 96%, white 4%) 0%,
    color-mix(in srgb, ${token.colorFillAlter} 38%, ${token.colorBgContainer} 62%) 100%)`;
  const guideSteps = [
    '先确认章节序号和所属大纲，决定这章会插入到哪一段创作流程里。',
    '再补章节标题、摘要与状态，让这次手动建章更像一次正式立项，而不是临时空白条目。',
    '最后再提交创建；原有字段校验、冲突检查与创建逻辑保持不变。',
  ];
  const workspaceItems = [
    { label: '建议章节序号', value: `第 ${nextChapterNumber} 章` },
    { label: '可选大纲数', value: sortedOutlines.length > 0 ? `${sortedOutlines.length} 条` : '暂无可选大纲' },
    { label: '默认状态', value: '草稿' },
  ];

  return (
    <div style={{ marginTop: 16 }}>
      <Card
        bordered={false}
        style={{
          marginBottom: 16,
          borderRadius: 20,
          overflow: 'hidden',
          background: heroBackground,
        }}
        styles={{ body: { padding: 20 } }}
      >
        <Text style={{ color: 'rgba(255,255,255,0.68)', letterSpacing: '0.14em', textTransform: 'uppercase' }}>
          Chapter Setup
        </Text>
        <Title
          level={5}
          style={{
            margin: '8px 0 10px',
            color: '#f7f1e8',
            fontFamily: designDisplayFont,
            letterSpacing: '-0.03em',
          }}
        >
          手动创建章节前的工作区确认
        </Title>
        <Paragraph style={{ margin: 0, color: 'rgba(255,255,255,0.84)', lineHeight: 1.75 }}>
          这一步更像章节创建工作台。原有表单校验、序号冲突确认和创建动作保持不变，这里只把录入顺序和当前焦点整理得更清楚。
        </Paragraph>
      </Card>

      <Card
        bordered={false}
        style={{
          marginBottom: 16,
          borderRadius: 18,
          background: `linear-gradient(135deg, color-mix(in srgb, ${token.colorPrimary} 8%, white 92%) 0%, color-mix(in srgb, ${token.colorWarning} 8%, white 92%) 100%)`,
          border: `1px solid color-mix(in srgb, ${token.colorPrimary} 14%, white 86%)`,
        }}
        styles={{ body: { padding: 18 } }}
      >
        <Text style={{ fontSize: 12, color: token.colorTextTertiary, letterSpacing: '0.12em', textTransform: 'uppercase' }}>
          Create Guide
        </Text>
        <Paragraph style={{ margin: '8px 0 0', color: token.colorText, lineHeight: 1.75 }}>
          先定序号和归属大纲，再写标题与摘要，最后确认章节状态后提交。这里只强化阅读与录入顺序，不改变原有创建链路。
        </Paragraph>
        <div style={{ display: 'flex', flexWrap: 'wrap', gap: 8, marginTop: 12 }}>
          {guideSteps.map((item, index) => (
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
                color: token.colorTextSecondary,
                fontSize: 12,
                lineHeight: 1.5,
              }}
            >
              <span style={{ color: token.colorPrimary, fontWeight: 700 }}>{index + 1}</span>
              {item}
            </span>
          ))}
        </div>
      </Card>

      <Card
        bordered={false}
        style={{
          borderRadius: 18,
          background: quietPanelBackground,
          border: `1px solid ${token.colorBorderSecondary}`,
        }}
        styles={{ body: { padding: 18 } }}
      >
        <Text style={{ fontSize: 12, color: token.colorTextTertiary, letterSpacing: '0.12em', textTransform: 'uppercase' }}>
          Create Workspace
        </Text>
        <Title level={5} style={{ margin: '6px 0 10px', fontFamily: designDisplayFont }}>
          当前章节创建焦点
        </Title>
        <Paragraph style={{ marginTop: 0, color: token.colorTextSecondary, lineHeight: 1.75 }}>
          先把章节插入位置定准，再补充标题、摘要和状态。提交后如果序号与现有章节冲突，仍会走原来的冲突确认流程。
        </Paragraph>
        <Space wrap size={[8, 8]} style={{ marginBottom: 16 }}>
          <Tag color="blue" style={{ margin: 0, borderRadius: 999, paddingInline: 10 }}>
            {`建议从第 ${nextChapterNumber} 章开始`}
          </Tag>
          <Tag color={sortedOutlines.length > 0 ? 'cyan' : 'warning'} style={{ margin: 0, borderRadius: 999, paddingInline: 10 }}>
            {sortedOutlines.length > 0 ? `可挂接 ${sortedOutlines.length} 条大纲` : '当前没有可选大纲'}
          </Tag>
          <Tag color="default" style={{ margin: 0, borderRadius: 999, paddingInline: 10 }}>
            默认状态为草稿
          </Tag>
        </Space>

        <Space direction="vertical" size={10} style={{ width: '100%', marginBottom: 16 }}>
          {workspaceItems.map((item) => (
            <div
              key={item.label}
              style={{
                display: 'flex',
                justifyContent: 'space-between',
                gap: 12,
                padding: '10px 12px',
                borderRadius: 14,
                background: token.colorBgContainer,
                border: `1px solid ${token.colorBorderSecondary}`,
              }}
            >
              <Text type="secondary">{item.label}</Text>
              <Text strong style={{ textAlign: 'right' }}>{item.value}</Text>
            </div>
          ))}
        </Space>

        <Form
          form={form}
          layout="vertical"
          initialValues={{
            chapter_number: nextChapterNumber,
            status: 'draft',
          }}
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
      </Card>
    </div>
  );
}
