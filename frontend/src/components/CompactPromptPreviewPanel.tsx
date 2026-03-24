import { Button, Input, Space, Tag } from 'antd';
import type { CSSProperties } from 'react';

import { renderCompactSelectionSummary, renderCompactStoryControlHeader } from './storyCreationCommonUi';

const { TextArea } = Input;

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
  return (
    <div style={{ padding: '10px 12px', border: '1px solid #f0f0f0', borderRadius: 8, ...style }}>
      {renderCompactStoryControlHeader(
        '提示词',
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
  );
}
