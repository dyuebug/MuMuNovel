import { Suspense, lazy, useEffect, useState } from 'react';

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
        <Suspense fallback={null}>
          <SpringFestival />
        </Suspense>
      ) : null}
      {deferredUiReady ? (
        <Suspense fallback={null}>
          <BackgroundTaskCenter />
        </Suspense>
      ) : null}
    </>
  );
}
