import React from 'react';
import { Button, Spin } from 'antd';
import { LoadingOutlined, StopOutlined } from '@ant-design/icons';

interface SSELoadingOverlayProps {
  loading: boolean;
  progress: number;
  message: string;
  blocking?: boolean;
  onCancel?: () => void;
  cancelButtonText?: string;
  cancelButtonLoading?: boolean;
  cancelButtonDisabled?: boolean;
}

export const SSELoadingOverlay: React.FC<SSELoadingOverlayProps> = ({
  loading,
  progress,
  message,
  blocking = true,
  onCancel,
  cancelButtonText = '取消任务',
  cancelButtonLoading = false,
  cancelButtonDisabled = false,
}) => {
  if (!loading) return null;

  return (
    <div style={{
      position: 'fixed',
      top: blocking ? 0 : 'auto',
      left: blocking ? 0 : 'auto',
      right: blocking ? 0 : 24,
      bottom: blocking ? 0 : 24,
      background: blocking ? 'rgba(0, 0, 0, 0.45)' : 'transparent',
      display: 'flex',
      justifyContent: 'center',
      alignItems: 'center',
      pointerEvents: blocking ? 'auto' : 'none',
      zIndex: 9999
    }}>
      <div style={{
        background: '#fff',
        borderRadius: 12,
        padding: blocking ? '40px 60px' : '20px 24px',
        minWidth: blocking ? 400 : 320,
        maxWidth: 600,
        boxShadow: '0 8px 32px rgba(0,0,0,0.2)',
        pointerEvents: 'auto'
      }}>
        <div style={{
          textAlign: 'center',
          marginBottom: 24
        }}>
          <Spin indicator={<LoadingOutlined style={{ fontSize: 48, color: 'var(--color-primary)' }} spin />} />
          <div style={{
            fontSize: blocking ? 20 : 16,
            fontWeight: 'bold',
            marginTop: 16,
            color: 'var(--color-text-primary)'
          }}>
            {blocking ? 'AI 生成中...' : 'AI 后台处理中...'}
          </div>
        </div>

        <div style={{ marginBottom: 16 }}>
          <div style={{
            height: 12,
            background: 'var(--color-bg-layout)',
            borderRadius: 6,
            overflow: 'hidden',
            marginBottom: 12
          }}>
            <div style={{
              height: '100%',
              background: progress === 100
                ? 'linear-gradient(90deg, var(--color-success) 0%, var(--color-success-active) 100%)'
                : 'linear-gradient(90deg, var(--color-primary) 0%, var(--color-primary-active) 100%)',
              width: `${progress}%`,
              transition: 'all 0.3s ease',
              borderRadius: 6,
              boxShadow: progress > 0 ? 'var(--shadow-card)' : 'none'
            }} />
          </div>

          <div style={{
            textAlign: 'center',
            fontSize: 32,
            fontWeight: 'bold',
            color: progress === 100 ? 'var(--color-success)' : 'var(--color-primary)',
            marginBottom: 8
          }}>
            {progress}%
          </div>
        </div>

        <div style={{
          textAlign: 'center',
          fontSize: 16,
          color: '#595959',
          minHeight: 24,
          padding: '0 20px'
        }}>
          {message || '准备生成...'}
        </div>

        <div style={{
          textAlign: 'center',
          fontSize: 13,
          color: '#8c8c8c',
          marginTop: 16,
          marginBottom: onCancel ? 16 : 0
        }}>
          {blocking ? '请勿关闭页面，生成过程需要一定时间' : '后台处理中，可继续其他操作'}
        </div>

        {onCancel && (
          <div style={{ textAlign: 'center' }}>
            <Button
              danger
              icon={<StopOutlined />}
              onClick={onCancel}
              loading={cancelButtonLoading}
              disabled={cancelButtonDisabled}
            >
              {cancelButtonText}
            </Button>
          </div>
        )}
      </div>
    </div>
  );
};
