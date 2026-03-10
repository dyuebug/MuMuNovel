import type { CSSProperties } from 'react';

interface LoadingScreenProps {
  message?: string;
  minHeight?: string;
}

const wrapperBaseStyle: CSSProperties = {
  display: 'flex',
  flexDirection: 'column',
  justifyContent: 'center',
  alignItems: 'center',
  gap: 12,
  width: '100%',
  padding: '24px 16px',
  color: 'var(--ant-color-text, #2b2b2b)',
};

const spinnerStyle: CSSProperties = {
  width: 28,
  height: 28,
  borderRadius: '50%',
  border: '3px solid color-mix(in srgb, var(--ant-color-primary, #4D8088) 24%, transparent)',
  borderTopColor: 'var(--ant-color-primary, #4D8088)',
  animation: 'app-loading-spin 0.8s linear infinite',
};

const messageStyle: CSSProperties = {
  fontSize: 14,
  fontWeight: 500,
  letterSpacing: '0.02em',
};

export default function LoadingScreen({
  message = '加载中...',
  minHeight = '40vh',
}: LoadingScreenProps) {
  return (
    <>
      <style>{'@keyframes app-loading-spin { from { transform: rotate(0deg); } to { transform: rotate(360deg); } }'}</style>
      <div style={{ ...wrapperBaseStyle, minHeight }} role="status" aria-live="polite">
        <div style={spinnerStyle} />
        <div style={messageStyle}>{message}</div>
      </div>
    </>
  );
}
