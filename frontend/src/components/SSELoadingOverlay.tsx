import React from 'react';
import { Button, Spin } from 'antd';
import { DownOutlined, LoadingOutlined, StopOutlined, UnorderedListOutlined, UpOutlined } from '@ant-design/icons';
import { useFloatingTaskCard } from './useFloatingTaskCard';
import { OPEN_BACKGROUND_TASK_CENTER_EVENT } from '../constants/backgroundTaskEvents';
import { isActiveBackgroundTask, useBackgroundTaskStore } from '../store/backgroundTasks';

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
  const { collapsed, floatingBottom, toggleCollapsed } = useFloatingTaskCard({
    active: loading,
    blocking,
  });
  const activeTaskCount = useBackgroundTaskStore((state) =>
    Object.values(state.tasks).filter(isActiveBackgroundTask).length
  );
  const queueSummary = !blocking && activeTaskCount > 1
    ? `当前共 ${activeTaskCount} 个后台任务正在运行`
    : null;

  const openTaskCenter = () => {
    window.dispatchEvent(new Event(OPEN_BACKGROUND_TASK_CENTER_EVENT));
  };

  if (!loading) return null;

  const progressBar = (
    <div
      style={{
        height: 12,
        background: 'var(--color-bg-layout)',
        borderRadius: 6,
        overflow: 'hidden',
        marginBottom: 12,
      }}
    >
      <div
        style={{
          height: '100%',
          background: progress === 100
            ? 'linear-gradient(90deg, var(--color-success) 0%, var(--color-success-active) 100%)'
            : 'linear-gradient(90deg, var(--color-primary) 0%, var(--color-primary-active) 100%)',
          width: `${progress}%`,
          transition: 'all 0.3s ease',
          borderRadius: 6,
          boxShadow: progress > 0 ? 'var(--shadow-card)' : 'none',
        }}
      />
    </div>
  );

  const content = (
    <div>
      <div
        style={{
          textAlign: 'center',
          marginBottom: 24,
        }}
      >
        <Spin indicator={<LoadingOutlined style={{ fontSize: blocking ? 48 : 32, color: 'var(--color-primary)' }} spin />} />
        <div
          style={{
            fontSize: blocking ? 20 : 16,
            fontWeight: 'bold',
            marginTop: 16,
            color: 'var(--color-text-primary)',
          }}
        >
          {blocking ? 'AI 生成中...' : 'AI 后台处理中...'}
        </div>
      </div>

      <div style={{ marginBottom: 16 }}>
        {progressBar}
        <div
          style={{
            textAlign: 'center',
            fontSize: 32,
            fontWeight: 'bold',
            color: progress === 100 ? 'var(--color-success)' : 'var(--color-primary)',
            marginBottom: 8,
          }}
        >
          {progress}%
        </div>
      </div>

      <div
        style={{
          textAlign: 'center',
          fontSize: 16,
          color: '#595959',
          minHeight: 24,
          padding: '0 20px',
        }}
      >
        {message || '准备生成...'}
      </div>

      <div
        style={{
          textAlign: 'center',
          fontSize: 13,
          color: '#8c8c8c',
          marginTop: 16,
          marginBottom: onCancel ? 16 : 0,
        }}
      >
        {blocking ? '请勿关闭页面，生成过程需要一定时间' : '后台处理中，可继续其他操作'}
      </div>

      {queueSummary ? (
        <div
          style={{
            textAlign: 'center',
            fontSize: 12,
            color: '#8c8c8c',
            marginBottom: onCancel ? 12 : 0,
          }}
        >
          {queueSummary}
        </div>
      ) : null}

      {!blocking ? (
        <div style={{ textAlign: 'center', marginBottom: onCancel ? 12 : 0 }}>
          <Button type="link" size="small" icon={<UnorderedListOutlined />} onClick={openTaskCenter}>
            查看全部任务
          </Button>
        </div>
      ) : null}

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
  );

  if (!blocking) {
    return (
      <div
        style={{
          position: 'fixed',
          right: 'max(16px, env(safe-area-inset-right))',
          bottom: floatingBottom,
          zIndex: 9999,
          pointerEvents: 'none',
        }}
      >
        <div
          style={{
            width: collapsed ? 'min(280px, calc(100vw - 32px))' : 'min(340px, calc(100vw - 32px))',
            background: '#fff',
            borderRadius: 12,
            padding: collapsed ? '12px 14px' : '20px 24px',
            boxShadow: '0 8px 32px rgba(0,0,0,0.2)',
            boxSizing: 'border-box',
            pointerEvents: 'auto',
            transition: 'width 0.2s ease, padding 0.2s ease',
          }}
        >
          {collapsed ? (
            <div>
              <div style={{ display: 'flex', alignItems: 'center', gap: 10 }}>
                <Spin indicator={<LoadingOutlined style={{ fontSize: 18, color: 'var(--color-primary)' }} spin />} />
                <div style={{ flex: 1, minWidth: 0 }}>
                  <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', gap: 8 }}>
                    <div
                      style={{
                        fontSize: 14,
                        fontWeight: 600,
                        color: 'var(--color-text-primary)',
                        whiteSpace: 'nowrap',
                        overflow: 'hidden',
                        textOverflow: 'ellipsis',
                      }}
                    >
                      AI 后台处理中
                    </div>
                    <div style={{ fontSize: 13, fontWeight: 600, color: 'var(--color-primary)' }}>
                      {progress}%
                    </div>
                  </div>
                  <div
                    style={{
                      marginTop: 2,
                      fontSize: 12,
                      color: 'var(--color-text-secondary)',
                      whiteSpace: 'nowrap',
                      overflow: 'hidden',
                      textOverflow: 'ellipsis',
                    }}
                  >
                    {queueSummary
                      ? `${message || '后台处理中，可继续其他操作'} · 共 ${activeTaskCount} 项`
                      : message || '后台处理中，可继续其他操作'}
                  </div>
                </div>
                {onCancel ? (
                  <Button
                    type="text"
                    danger
                    size="small"
                    icon={<StopOutlined />}
                    onClick={onCancel}
                    loading={cancelButtonLoading}
                    disabled={cancelButtonDisabled}
                  />
                ) : null}
                <Button
                  type="text"
                  size="small"
                  icon={<UnorderedListOutlined />}
                  onClick={openTaskCenter}
                  style={{ color: 'var(--color-text-secondary)' }}
                />
                <Button
                  type="text"
                  size="small"
                  icon={<UpOutlined />}
                  onClick={toggleCollapsed}
                  style={{ color: 'var(--color-text-secondary)' }}
                />
              </div>
              <div style={{ marginTop: 10 }}>{progressBar}</div>
            </div>
          ) : (
            <>
              <div style={{ display: 'flex', justifyContent: 'flex-end', marginBottom: 8 }}>
                <Button
                  type="text"
                  size="small"
                  icon={<DownOutlined />}
                  onClick={toggleCollapsed}
                  style={{ color: 'var(--color-text-secondary)' }}
                />
              </div>
              {content}
            </>
          )}
        </div>
      </div>
    );
  }

  return (
    <div
      style={{
        position: 'fixed',
        top: 0,
        left: 0,
        right: 0,
        bottom: 0,
        background: 'rgba(0, 0, 0, 0.45)',
        display: 'flex',
        justifyContent: 'center',
        alignItems: 'center',
        pointerEvents: 'auto',
        zIndex: 9999,
      }}
    >
      <div
        style={{
          background: '#fff',
          borderRadius: 12,
          padding: '40px 60px',
          minWidth: 400,
          maxWidth: 600,
          boxShadow: '0 8px 32px rgba(0,0,0,0.2)',
          pointerEvents: 'auto',
        }}
      >
        {content}
      </div>
    </div>
  );
};
