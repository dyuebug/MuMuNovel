import { Button, Input, Space, Tag, Typography, theme } from 'antd';
import type { CSSProperties } from 'react';

import { renderCompactSelectionSummary, renderCompactStoryControlHeader } from './storyCreationCommonUi';

const { TextArea } = Input;
const { Text } = Typography;

type CompactPromptPreviewPanelProps = {
  prompt?: string;
  promptLayerLabels: string[];
  promptCharCount: number;
  isVerbose: boolean;
  onCopy: () => void;
  placeholder: string;
  style?: CSSProperties;
};

export default function CompactPromptPreviewPanel({
  prompt,
  promptLayerLabels,
  promptCharCount,
  isVerbose,
  onCopy,
  placeholder,
  style,
}: CompactPromptPreviewPanelProps) {
  const { token } = theme.useToken();
  const alphaColor = (color: string, alpha: number) => `color-mix(in srgb, ${color} ${(alpha * 100).toFixed(0)}%, transparent)`;
  const promptGuideSteps = [
    '先确认当前提示词是标准模式还是详细模式，再决定是否需要直接复制给生成链路。',
    '再看字符数和层级标签，判断这份提示词的复杂度是否符合当前创作任务。',
    '最后再通读文本内容，把它当作生成前的最后一道校准，而不是现场编辑器。',
  ];
  const promptWorkspaceFocus = prompt
    ? {
        title: isVerbose ? '先确认这份详细提示词是否真的需要完整上下文' : '先确认这份标准提示词是否足够直接可用',
        note: isVerbose
          ? '当前提示词信息更全、字符也更长，适合先检查层级覆盖和细节密度，再复制进入生成流程。'
          : '当前提示词更适合快速生成，建议先核对关键层级和字符数，再直接复用到生成链路。',
      }
    : {
        title: '先补齐可预览的提示词内容',
        note: '当前还没有可复制文本，建议先完成上游选择或生成步骤，再回到这里做最终校准。',
      };

  return (
    <div
      style={{
        padding: '10px 12px',
        border: `1px solid ${alphaColor(token.colorBorderSecondary, 0.92)}`,
        borderRadius: 16,
        background: `linear-gradient(180deg, ${alphaColor(token.colorBgContainer, 0.98)} 0%, ${alphaColor(token.colorFillQuaternary, 0.42)} 100%)`,
        ...style,
      }}
    >
      <div
        style={{
          display: 'grid',
          gridTemplateColumns: 'repeat(auto-fit, minmax(240px, 1fr))',
          gap: 16,
          marginBottom: 12,
          padding: 4,
        }}
      >
        <div>
          <Text style={{ display: 'block', fontSize: 11, letterSpacing: '0.08em', textTransform: 'uppercase', color: token.colorTextTertiary, marginBottom: 6 }}>
            Prompt Guide
          </Text>
          <Text strong style={{ display: 'block', fontSize: 17, marginBottom: 8 }}>
            提示词预览工作区
          </Text>
          <Text type="secondary" style={{ display: 'block', lineHeight: 1.7, marginBottom: 12 }}>
            这里负责在生成前复核当前提示词。当前只增强阅读顺序和焦点说明，不改变提示词拼装、复制或上游生成逻辑。
          </Text>
          <div style={{ display: 'flex', flexWrap: 'wrap', gap: 8 }}>
            {promptGuideSteps.map((item, index) => (
              <span
                key={item}
                style={{
                  display: 'inline-flex',
                  alignItems: 'center',
                  gap: 8,
                  padding: '6px 12px',
                  borderRadius: 999,
                  background: token.colorBgContainer,
                  border: `1px solid ${alphaColor(token.colorPrimary, 0.12)}`,
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
            background: `linear-gradient(180deg, ${alphaColor(token.colorBgContainer, 0.98)} 0%, ${alphaColor(token.colorFillQuaternary, 0.5)} 100%)`,
            border: `1px solid ${alphaColor(token.colorPrimary, 0.12)}`,
          }}
        >
          <Text style={{ display: 'block', fontSize: 11, letterSpacing: '0.08em', textTransform: 'uppercase', color: token.colorTextTertiary, marginBottom: 6 }}>
            当前工作焦点
          </Text>
          <Text strong style={{ display: 'block', fontSize: 16, marginBottom: 8 }}>
            {promptWorkspaceFocus.title}
          </Text>
          <Text type="secondary" style={{ display: 'block', lineHeight: 1.7, marginBottom: 12 }}>
            {promptWorkspaceFocus.note}
          </Text>
          <Space wrap>
            <Tag color={isVerbose ? 'gold' : 'blue'}>{isVerbose ? '详细提示' : '标准提示'}</Tag>
            <Tag color={isVerbose ? 'gold' : 'blue'}>字符: {promptCharCount}</Tag>
            <Tag color="processing">层级: {promptLayerLabels.length} 项</Tag>
            <Tag color={prompt ? 'green' : 'default'}>{prompt ? '可复制文本' : '暂无文本'}</Tag>
          </Space>
        </div>
      </div>

      <div
        style={{
          padding: '12px 12px 10px',
          borderRadius: 14,
          border: `1px solid ${alphaColor(token.colorBorderSecondary, 0.92)}`,
          background: alphaColor(token.colorBgContainer, 0.9),
        }}
      >
        {renderCompactStoryControlHeader(
          '提示词文本',
          isVerbose
            ? '当前属于详细提示词，信息更全，文本也会更长。'
            : '按当前选择自动拼装，可直接复制给生成链路使用。',
          {
            tagText: isVerbose ? '详细提示' : '标准提示',
            tagColor: isVerbose ? 'gold' : 'blue',
            action: (
              <Button size="small" onClick={onCopy} disabled={!prompt}>
                复制提示词
              </Button>
            ),
            style: { marginBottom: 8 },
          },
        )}
        {renderCompactSelectionSummary(
          [
            { label: '字符', value: `${promptCharCount}`, color: isVerbose ? 'gold' : 'blue' },
            { label: '层级', value: `${promptLayerLabels.length} 项`, color: 'processing' },
          ],
          { style: { marginBottom: promptLayerLabels.length > 0 ? 8 : 10 } },
        )}
        {promptLayerLabels.length > 0 ? (
          <Space wrap size={[8, 8]} style={{ marginBottom: 8 }}>
            {promptLayerLabels.map((item) => (
              <Tag key={item} color="processing">{item}</Tag>
            ))}
          </Space>
        ) : null}
        <TextArea
          value={prompt ?? ''}
          autoSize={{ minRows: 6, maxRows: 12 }}
          readOnly
          placeholder={placeholder}
        />
      </div>
    </div>
  );
}
