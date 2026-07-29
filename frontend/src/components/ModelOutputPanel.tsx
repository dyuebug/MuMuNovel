import { useEffect, useRef, useState } from 'react';
import { Alert, Card, Segmented, Space, Switch, Typography, theme } from 'antd';

const SHOW_OUTPUT_STORAGE_KEY = 'mumu-model-output-visible';
const AUTO_SCROLL_STORAGE_KEY = 'mumu-model-output-auto-scroll';

type OutputChannel = 'reasoning' | 'content';
export type ModelOutputTaskStatus = 'running' | 'completed' | 'failed' | 'cancelled';

export interface ModelOutputPanelProps {
  reasoningContent: string;
  generatedContent: string;
  reasoningTruncated?: boolean;
  contentTruncated?: boolean;
  taskStatus?: ModelOutputTaskStatus;
  compact?: boolean;
}

export interface ModelOutputSectionsProps extends ModelOutputPanelProps {
  showReasoning: boolean;
  showGeneratedContent: boolean;
}

const readBooleanPreference = (key: string, fallback: boolean) => {
  if (typeof window === 'undefined') {
    return fallback;
  }
  const stored = window.localStorage.getItem(key);
  return stored === null ? fallback : stored === 'true';
};

const writeBooleanPreference = (key: string, value: boolean) => {
  if (typeof window !== 'undefined') {
    window.localStorage.setItem(key, String(value));
  }
};

function OutputViewport({
  content,
  emptyText,
  autoScroll,
  compact,
}: {
  content: string;
  emptyText: string;
  autoScroll: boolean;
  compact: boolean;
}) {
  const { token } = theme.useToken();
  const outputRef = useRef<HTMLDivElement | null>(null);
  const [followingTail, setFollowingTail] = useState(autoScroll);

  useEffect(() => {
    setFollowingTail(autoScroll);
  }, [autoScroll]);

  useEffect(() => {
    if (!autoScroll || !followingTail || !outputRef.current) {
      return;
    }
    outputRef.current.scrollTop = outputRef.current.scrollHeight;
  }, [autoScroll, content, followingTail]);

  return (
    <div
      ref={outputRef}
      onScroll={(event) => {
        const target = event.currentTarget;
        const atBottom = target.scrollHeight - target.scrollTop - target.clientHeight < 24;
        setFollowingTail(atBottom);
      }}
      style={{
        minHeight: compact ? 130 : 180,
        maxHeight: compact ? 220 : 320,
        overflow: 'auto',
        padding: 12,
        borderRadius: 10,
        background: token.colorFillQuaternary,
        border: `1px solid ${token.colorBorderSecondary}`,
        color: token.colorText,
        fontFamily: 'ui-monospace, SFMono-Regular, Consolas, monospace',
        fontSize: 12,
        lineHeight: 1.7,
        whiteSpace: 'pre-wrap',
        overflowWrap: 'anywhere',
      }}
    >
      {content || <Typography.Text type="secondary">{emptyText}</Typography.Text>}
    </div>
  );
}

function OutputChannelCard({
  title,
  description,
  content,
  emptyText,
  truncated,
  uncommitted,
  autoScroll,
  compact,
}: {
  title: string;
  description: string;
  content: string;
  emptyText: string;
  truncated: boolean;
  uncommitted: boolean;
  autoScroll: boolean;
  compact: boolean;
}) {
  const { token } = theme.useToken();
  return (
    <Card
      size="small"
      bordered={false}
      style={{
        borderRadius: compact ? 12 : 16,
        background: token.colorBgContainer,
        border: `1px solid ${token.colorBorderSecondary}`,
      }}
      styles={{ body: { padding: compact ? 12 : 16 } }}
    >
      <Typography.Text strong>{title}</Typography.Text>
      <Typography.Text type="secondary" style={{ display: 'block', fontSize: 12, marginTop: 2 }}>
        {description}
      </Typography.Text>
      <div style={{ display: 'grid', gap: 10, marginTop: 12 }}>
        {uncommitted ? (
          <Alert type="warning" showIcon message="未提交输出：任务已失败或取消，以下内容不会写入正式结果。" />
        ) : null}
        {truncated ? (
          <Alert type="info" showIcon message="输出过长，仅保留最近 50,000 个字符。" />
        ) : null}
        <OutputViewport
          content={content}
          emptyText={emptyText}
          autoScroll={autoScroll}
          compact={compact}
        />
      </div>
    </Card>
  );
}

export const ModelOutputPanel = ({
  reasoningContent,
  generatedContent,
  reasoningTruncated = false,
  contentTruncated = false,
  taskStatus = 'running',
  compact = false,
}: ModelOutputPanelProps) => {
  const { token } = theme.useToken();
  const [visible, setVisible] = useState(() => readBooleanPreference(SHOW_OUTPUT_STORAGE_KEY, false));
  const [autoScroll, setAutoScroll] = useState(() => readBooleanPreference(AUTO_SCROLL_STORAGE_KEY, true));
  const [channel, setChannel] = useState<OutputChannel>('content');

  const output = channel === 'reasoning' ? reasoningContent : generatedContent;
  const truncated = channel === 'reasoning' ? reasoningTruncated : contentTruncated;

  const handleVisibleChange = (nextVisible: boolean) => {
    setVisible(nextVisible);
    writeBooleanPreference(SHOW_OUTPUT_STORAGE_KEY, nextVisible);
  };

  const handleAutoScrollChange = (nextAutoScroll: boolean) => {
    setAutoScroll(nextAutoScroll);
    writeBooleanPreference(AUTO_SCROLL_STORAGE_KEY, nextAutoScroll);
  };

  const emptyText = channel === 'reasoning'
    ? '当前模型未返回可展示的推理内容'
    : '模型尚未返回可展示的生成内容';
  const uncommitted = taskStatus === 'failed' || taskStatus === 'cancelled';

  return (
    <Card
      size="small"
      bordered={false}
      style={{
        borderRadius: compact ? 12 : 16,
        background: token.colorBgContainer,
        border: `1px solid ${token.colorBorderSecondary}`,
      }}
      styles={{ body: { padding: compact ? 12 : 16 } }}
    >
      <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', gap: 12 }}>
        <div>
          <Typography.Text strong>显示模型输出</Typography.Text>
          <Typography.Text type="secondary" style={{ display: 'block', fontSize: 12 }}>
            仅显示模型接口明确返回的推理与生成内容
          </Typography.Text>
        </div>
        <Switch checked={visible} onChange={handleVisibleChange} />
      </div>

      {visible ? (
        <div style={{ display: 'grid', gap: 10, marginTop: 12 }}>
          <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', gap: 10, flexWrap: 'wrap' }}>
            <Segmented<OutputChannel>
              size="small"
              value={channel}
              options={[
                { label: '生成内容', value: 'content' },
                { label: '思考过程', value: 'reasoning' },
              ]}
              onChange={setChannel}
            />
            <Space size={6}>
              <Typography.Text type="secondary" style={{ fontSize: 12 }}>自动滚动</Typography.Text>
              <Switch size="small" checked={autoScroll} onChange={handleAutoScrollChange} />
            </Space>
          </div>

          {uncommitted ? (
            <Alert type="warning" showIcon message="未提交输出：任务已失败或取消，以下内容不会写入正式结果。" />
          ) : null}
          {truncated ? (
            <Alert type="info" showIcon message="输出过长，仅保留最近 50,000 个字符。" />
          ) : null}
          <OutputViewport
            content={output}
            emptyText={emptyText}
            autoScroll={autoScroll}
            compact={compact}
          />
        </div>
      ) : null}
    </Card>
  );
};

export const ModelOutputSections = ({
  reasoningContent,
  generatedContent,
  reasoningTruncated = false,
  contentTruncated = false,
  taskStatus = 'running',
  compact = false,
  showReasoning,
  showGeneratedContent,
}: ModelOutputSectionsProps) => {
  const [autoScroll, setAutoScroll] = useState(() => readBooleanPreference(AUTO_SCROLL_STORAGE_KEY, true));
  const uncommitted = taskStatus === 'failed' || taskStatus === 'cancelled';

  if (!showReasoning && !showGeneratedContent) {
    return null;
  }

  const handleAutoScrollChange = (nextAutoScroll: boolean) => {
    setAutoScroll(nextAutoScroll);
    writeBooleanPreference(AUTO_SCROLL_STORAGE_KEY, nextAutoScroll);
  };

  return (
    <div style={{ display: 'grid', gap: 12 }}>
      <div style={{ display: 'flex', justifyContent: 'flex-end' }}>
        <Space size={6}>
          <Typography.Text type="secondary" style={{ fontSize: 12 }}>输出自动滚动</Typography.Text>
          <Switch size="small" checked={autoScroll} onChange={handleAutoScrollChange} />
        </Space>
      </div>
      {showReasoning ? (
        <OutputChannelCard
          title="Provider 思考过程（临时）"
          description="仅展示 Provider 明确返回的 reasoning/thinking；刷新后不会恢复，也不会写入项目或 Run。"
          content={reasoningContent}
          emptyText="当前模型未返回可展示的推理内容"
          truncated={reasoningTruncated}
          uncommitted={uncommitted}
          autoScroll={autoScroll}
          compact={compact}
        />
      ) : null}
      {showGeneratedContent ? (
        <OutputChannelCard
          title="模型生成内容（临时预览）"
          description="这是当前模型调用的实时预览；正式内容以 Durable Step 完成后的业务数据为准。"
          content={generatedContent}
          emptyText="模型尚未返回可展示的生成内容"
          truncated={contentTruncated}
          uncommitted={uncommitted}
          autoScroll={autoScroll}
          compact={compact}
        />
      ) : null}
    </div>
  );
};

export default ModelOutputPanel;
