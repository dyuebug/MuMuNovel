import { Suspense, lazy, useEffect, useState } from 'react';
import AmbientDeferredFallback from '../layout/AmbientDeferredFallback';

const SpringFestival = lazy(() => import('../../components/SpringFestival'));
const BackgroundTaskCenter = lazy(() => import('../../components/BackgroundTaskCenter'));

export default function AppDeferredFeatures() {
  const [deferredUiReady, setDeferredUiReady] = useState(false);

  useEffect(() => {
    const timer = window.setTimeout(() => {
      setDeferredUiReady(true);
    }, 1000);

    return () => {
      window.clearTimeout(timer);
    };
  }, []);

  return (
    <>
      {deferredUiReady ? (
        <Suspense
          fallback={(
            <AmbientDeferredFallback
              variant="floating"
              bottomOffset={24}
              eyebrow="Ambient Feature"
              title="正在接入节庆氛围层"
              message="装饰层正在懒加载接入。这里只补充轻量说明，不改变页面主工作区与交互链路。"
              tags={[
                { label: '氛围层接入中', color: 'gold' },
                { label: '主工作区保持原样', color: 'green' },
              ]}
            />
          )}
        >
          <SpringFestival />
        </Suspense>
      ) : null}
      {deferredUiReady ? (
        <Suspense
          fallback={(
            <AmbientDeferredFallback
              variant="floating"
              bottomOffset={126}
              eyebrow="Deferred Tools"
              title="正在连接后台任务中心"
              message="全局任务面板正在恢复挂载。这里只提示工作区工具接入状态，不改变任务恢复、轮询或跳转逻辑。"
              tags={[
                { label: '后台任务中心', color: 'purple' },
                { label: '任务逻辑保持原样', color: 'green' },
              ]}
            />
          )}
        >
          <BackgroundTaskCenter />
        </Suspense>
      ) : null}
    </>
  );
}
