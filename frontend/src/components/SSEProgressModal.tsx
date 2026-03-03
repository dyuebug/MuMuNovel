import React from 'react';
import { Modal, Spin, Button } from 'antd';
import { LoadingOutlined, StopOutlined } from '@ant-design/icons';

interface SSEProgressModalProps {
  visible: boolean;
  progress: number;
  message: string;
  title?: string;
  showPercentage?: boolean;
  showIcon?: boolean;
  onCancel?: () => void;
  cancelButtonText?: string;
  blocking?: boolean;
}

export const SSEProgressModal: React.FC<SSEProgressModalProps> = ({
  visible,
  progress,
  message,
  title = 'AI鐢熸垚涓?..',
  showPercentage = true,
  showIcon = true,
  onCancel,
  cancelButtonText = '鍙栨秷浠诲姟',
  blocking = true,
}) => {
  const [floatButtonOffset, setFloatButtonOffset] = React.useState(0);

  React.useEffect(() => {
    if (blocking || !visible || typeof window === 'undefined') {
      setFloatButtonOffset(0);
      return;
    }

    let rafId: number | null = null;

    const recalculateOffset = () => {
      const nodes = Array.from(document.querySelectorAll<HTMLElement>('.ant-float-btn, .ant-float-btn-group'));
      let nextOffset = 0;

      for (const node of nodes) {
        const style = window.getComputedStyle(node);
        if (style.display === 'none' || style.visibility === 'hidden' || style.opacity === '0') {
          continue;
        }

        const rect = node.getBoundingClientRect();
        if (rect.width === 0 || rect.height === 0) {
          continue;
        }

        // 浠呴伩璁╀綅浜庡彸渚х殑娴姩鎸夐挳锛岄伩鍏嶈鍒ら〉闈腑闂村厓绱犮€?
        const rightDistance = window.innerWidth - rect.right;
        if (rightDistance > 180) {
          continue;
        }

        const clearance = Math.max(0, window.innerHeight - rect.top + 12);
        if (clearance > nextOffset) {
          nextOffset = clearance;
        }
      }

      setFloatButtonOffset((prev) => (Math.abs(prev - nextOffset) < 1 ? prev : nextOffset));
    };

    const scheduleRecalculate = () => {
      if (rafId !== null) {
        window.cancelAnimationFrame(rafId);
      }
      rafId = window.requestAnimationFrame(recalculateOffset);
    };

    scheduleRecalculate();

    window.addEventListener('resize', scheduleRecalculate);
    window.addEventListener('scroll', scheduleRecalculate, true);

    const observer = new MutationObserver(scheduleRecalculate);
    observer.observe(document.body, {
      childList: true,
      subtree: true,
      attributes: true,
      attributeFilter: ['class', 'style']
    });

    return () => {
      window.removeEventListener('resize', scheduleRecalculate);
      window.removeEventListener('scroll', scheduleRecalculate, true);
      observer.disconnect();
      if (rafId !== null) {
        window.cancelAnimationFrame(rafId);
      }
    };
  }, [blocking, visible]);

  const floatingBottom = floatButtonOffset > 0
    ? `calc(max(16px, env(safe-area-inset-bottom)) + ${Math.round(floatButtonOffset)}px)`
    : 'max(16px, env(safe-area-inset-bottom))';

  if (!visible) return null;

  const content = (
    <div>
      {showIcon && (
        <div
          style={{
            textAlign: 'center',
            marginBottom: 24
          }}
        >
          <Spin
            indicator={
              <LoadingOutlined
                style={{ fontSize: blocking ? 48 : 32, color: 'var(--color-primary)' }}
                spin
              />
            }
          />
          <div
            style={{
              fontSize: blocking ? 20 : 16,
              fontWeight: 'bold',
              marginTop: 16,
              color: 'var(--color-text-primary)'
            }}
          >
            {title}
          </div>
        </div>
      )}

      <div style={{ marginBottom: showPercentage ? 16 : 24 }}>
        <div
          style={{
            height: 12,
            background: 'var(--color-bg-layout)',
            borderRadius: 6,
            overflow: 'hidden',
            marginBottom: showPercentage ? 12 : 0
          }}
        >
          <div
            style={{
              height: '100%',
              background:
                progress === 100
                  ? 'linear-gradient(90deg, var(--color-success) 0%, var(--color-success-active) 100%)'
                  : 'linear-gradient(90deg, var(--color-primary) 0%, var(--color-primary-active) 100%)',
              width: `${progress}%`,
              transition: 'all 0.3s ease',
              borderRadius: 6,
              boxShadow: progress > 0 ? 'var(--shadow-card)' : 'none'
            }}
          />
        </div>

        {showPercentage && (
          <div
            style={{
              textAlign: 'center',
              fontSize: blocking ? 32 : 24,
              fontWeight: 'bold',
              color: progress === 100 ? 'var(--color-success)' : 'var(--color-primary)',
              marginBottom: 8
            }}
          >
            {progress}%
          </div>
        )}
      </div>

      <div
        style={{
          textAlign: 'center',
          fontSize: blocking ? 16 : 14,
          color: 'var(--color-text-secondary)',
          minHeight: 24,
          padding: '0 20px',
          marginBottom: 16
        }}
      >
        {message || '鍑嗗鐢熸垚...'}
      </div>

      <div
        style={{
          textAlign: 'center',
          fontSize: 13,
          color: 'var(--color-text-tertiary)',
          marginBottom: onCancel ? 16 : 0
        }}
      >
        {blocking ? '请勿关闭页面，生成过程需要一定时间' : '后台处理中，可继续其他操作'}
      </div>

      {onCancel && (
        <div
          style={{
            textAlign: 'center',
            marginTop: 16
          }}
        >
          <Button
            danger
            size="large"
            icon={<StopOutlined />}
            onClick={onCancel}
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
          pointerEvents: 'none'
        }}
      >
        <div
          style={{
            width: 'min(360px, calc(100vw - 32px))',
            background: '#fff',
            borderRadius: 12,
            padding: '20px 24px',
            boxShadow: '0 8px 32px rgba(0,0,0,0.2)',
            boxSizing: 'border-box',
            pointerEvents: 'auto'
          }}
        >
          {content}
        </div>
      </div>
    );
  }

  return (
    <Modal
      title={null}
      open={visible}
      footer={null}
      closable={false}
      centered
      width={500}
      mask
      maskClosable={false}
      keyboard={false}
      styles={{
        body: {
          padding: '40px 40px 32px',
        }
      }}
    >
      {content}
    </Modal>
  );
};

export default SSEProgressModal;
