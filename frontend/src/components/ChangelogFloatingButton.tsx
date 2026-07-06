import { Suspense, lazy, useState } from 'react';
import { FloatButton, Grid, theme } from 'antd';
import { FileTextOutlined } from '@ant-design/icons';
import WorkflowEntryFallback from './WorkflowEntryFallback';
const LazyChangelogModal = lazy(() => import('./ChangelogModal'));

const { useBreakpoint } = Grid;

export default function ChangelogFloatingButton() {
  const [showChangelog, setShowChangelog] = useState(false);
  const screens = useBreakpoint();
  const isMobile = !screens.md;
  const { token } = theme.useToken();
  const alphaColor = (color: string, alpha: number) => `color-mix(in srgb, ${color} ${(alpha * 100).toFixed(0)}%, transparent)`;

  return (
    <>
      <FloatButton
        icon={<FileTextOutlined />}
        type="primary"
        shape={isMobile ? 'circle' : 'square'}
        description={isMobile ? undefined : '更新日志'}
        tooltip={showChangelog ? '更新日志窗口已打开' : '打开更新日志'}
        style={{
          right: 24,
          bottom: 100,
          background: `linear-gradient(135deg, ${token.colorPrimary} 0%, ${token.colorPrimaryHover} 100%)`,
          boxShadow: `0 18px 36px ${alphaColor(token.colorPrimary, 0.28)}`,
          border: `1px solid ${alphaColor(token.colorWhite, 0.28)}`,
          ...(isMobile ? {} : {
            width: 132,
            height: 60,
            borderRadius: 18,
          }),
          ...(isMobile ? {} : {
            zIndex: 999,
          }),
        }}
        onClick={() => setShowChangelog(true)}
      />

      {showChangelog ? (
        <Suspense
          fallback={(
            <WorkflowEntryFallback
              eyebrow="Release Notes"
              title="正在展开更新日志窗口"
              message="系统正在整理版本记录、重点变更与阅读面板，原有打开与关闭逻辑保持不变。"
              tags={[
                { label: '更新日志', color: 'blue' },
                { label: '说明面板恢复中', color: 'processing' },
                { label: '交互逻辑保持原样', color: 'green' },
              ]}
            />
          )}
        >
          <LazyChangelogModal
            visible={showChangelog}
            onClose={() => setShowChangelog(false)}
          />
        </Suspense>
      ) : null}
    </>
  );
}
