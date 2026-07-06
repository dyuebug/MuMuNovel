import { Suspense, lazy } from 'react';
import AmbientDeferredFallback from './AmbientDeferredFallback';

const AppFooter = lazy(() => import('../../components/AppFooter'));

export default function AppFooterSlot({ sidebarWidth }: { sidebarWidth?: number }) {
  const floating = Boolean(sidebarWidth);

  return (
    <Suspense
      fallback={(
        <AmbientDeferredFallback
          variant={floating ? 'floating' : 'footer'}
          eyebrow="Workspace Footer"
          title="正在整理页脚工作区"
          message="页脚正在接入导航补充信息与帮助入口。这里只补一层轻量过渡提示，不改变页面主体和原有懒加载链路。"
          tags={[
            { label: '页脚信息补位中', color: 'blue' },
            { label: '页面主体已可继续阅读', color: 'green' },
          ]}
        />
      )}
    >
      <AppFooter sidebarWidth={sidebarWidth} floating={floating} />
    </Suspense>
  );
}
