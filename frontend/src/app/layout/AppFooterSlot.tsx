import { Suspense, lazy } from 'react';

const AppFooter = lazy(() => import('../../components/AppFooter'));

export default function AppFooterSlot({ sidebarWidth }: { sidebarWidth?: number }) {
  return (
    <Suspense fallback={null}>
      <AppFooter sidebarWidth={sidebarWidth} />
    </Suspense>
  );
}
