import React from 'react';
import { Button, Tooltip, Typography, theme } from 'antd';
import { EditOutlined } from '@ant-design/icons';

const { Text } = Typography;

interface PartialRegenerateToolbarProps {
  visible: boolean;
  position: { top: number; left: number };
  onRegenerate: () => void;
  selectedText: string;
}

/**
 * 局部重写浮动工具栏
 * 当用户在章节内容编辑器中选中文本时显示
 */
export const PartialRegenerateToolbar: React.FC<PartialRegenerateToolbarProps> = ({
  visible,
  position,
  onRegenerate,
  selectedText
}) => {
  const { token } = theme.useToken();
  const alphaColor = (color: string, alpha: number) => `color-mix(in srgb, ${color} ${(alpha * 100).toFixed(0)}%, transparent)`;

  if (!visible || !selectedText) return null;

  // 限制显示的选中文本长度
  const displayText = selectedText.length > 20 
    ? selectedText.substring(0, 20) + '...' 
    : selectedText;

  return (
    <div
      style={{
        position: 'fixed',
        top: position.top,
        left: position.left,
        zIndex: 10000,
        background: `linear-gradient(135deg, ${alphaColor(token.colorPrimaryBg, 0.94)} 0%, ${alphaColor(token.colorBgElevated, 0.98)} 100%)`,
        borderRadius: 14,
        boxShadow: token.boxShadowSecondary,
        padding: '8px 10px',
        display: 'flex',
        alignItems: 'center',
        gap: 8,
        animation: 'fadeIn 0.2s ease-out',
        border: `1px solid ${alphaColor(token.colorPrimary, 0.14)}`,
        maxWidth: 'calc(100vw - 24px)',
        flexWrap: 'wrap',
      }}
    >
      <div style={{ minWidth: 0 }}>
        <Text
          style={{
            display: 'block',
            fontSize: 10,
            letterSpacing: '0.08em',
            textTransform: 'uppercase',
            color: token.colorTextTertiary,
            marginBottom: 2,
          }}
        >
          Selected Passage
        </Text>
        <Text
          style={{
            fontSize: 12,
            color: token.colorTextSecondary,
            maxWidth: 150,
            overflow: 'hidden',
            textOverflow: 'ellipsis',
            whiteSpace: 'nowrap',
            display: 'block',
          }}
        >
          已选 {selectedText.length} 字
        </Text>
      </div>
      <Tooltip
        title={`智能重写选中内容: "${displayText}"`}
        placement="top"
      >
        <Button
          type="primary"
          size="small"
          icon={<EditOutlined />}
          onClick={(e) => {
            e.preventDefault();
            e.stopPropagation();
            onRegenerate();
          }}
          style={{
            background: 'linear-gradient(135deg, var(--color-primary) 0%, var(--color-primary-hover) 100%)',
            border: 'none',
            fontWeight: 500,
            boxShadow: token.boxShadowSecondary,
          }}
        >
          智能重写
        </Button>
      </Tooltip>
    </div>
  );
};

// 添加动画样式
const style = document.createElement('style');
style.textContent = `
  @keyframes fadeIn {
    from {
      opacity: 0;
      transform: translateY(-4px);
    }
    to {
      opacity: 1;
      transform: translateY(0);
    }
  }
`;
if (!document.head.querySelector('style[data-partial-regenerate-toolbar]')) {
  style.setAttribute('data-partial-regenerate-toolbar', 'true');
  document.head.appendChild(style);
}

export default PartialRegenerateToolbar;
