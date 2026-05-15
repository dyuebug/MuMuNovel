const CHUNK_RELOAD_GUARD_KEY = 'mumu:chunk-load-reload-guard';
const CHUNK_RELOAD_GUARD_WINDOW_MS = 15_000;

const getChunkLoadErrorMessage = (error: unknown): string => {
  if (typeof error === 'string') {
    return error;
  }
  if (error instanceof Error) {
    return `${error.name}: ${error.message}`;
  }
  if (error && typeof error === 'object' && 'message' in error) {
    const message = (error as { message?: unknown }).message;
    return typeof message === 'string' ? message : '';
  }
  return '';
};

export const isRecoverableChunkLoadError = (error: unknown): boolean => {
  const message = getChunkLoadErrorMessage(error).toLowerCase();
  return (
    message.includes('failed to fetch dynamically imported module')
    || message.includes('importing a module script failed')
    || message.includes('loading chunk')
    || message.includes('chunkloaderror')
  );
};

const shouldReloadForCurrentLocation = (): boolean => {
  if (typeof window === 'undefined') {
    return false;
  }

  const currentLocation = `${window.location.pathname}${window.location.search}`;
  const rawGuard = window.sessionStorage.getItem(CHUNK_RELOAD_GUARD_KEY);
  if (!rawGuard) {
    return true;
  }

  try {
    const parsed = JSON.parse(rawGuard) as {
      location?: string;
      ts?: number;
    };
    if (
      parsed.location === currentLocation
      && typeof parsed.ts === 'number'
      && (Date.now() - parsed.ts) < CHUNK_RELOAD_GUARD_WINDOW_MS
    ) {
      return false;
    }
  } catch {
    return true;
  }

  return true;
};

export const reloadOnceForChunkLoadError = (error: unknown): boolean => {
  if (!isRecoverableChunkLoadError(error) || typeof window === 'undefined') {
    return false;
  }

  if (!shouldReloadForCurrentLocation()) {
    return false;
  }

  const currentLocation = `${window.location.pathname}${window.location.search}`;
  window.sessionStorage.setItem(
    CHUNK_RELOAD_GUARD_KEY,
    JSON.stringify({
      location: currentLocation,
      ts: Date.now(),
    }),
  );
  window.location.reload();
  return true;
};

export const withChunkLoadRecovery = <T>(loader: () => Promise<T>) => {
  return async () => {
    try {
      return await loader();
    } catch (error) {
      if (reloadOnceForChunkLoadError(error)) {
        return await new Promise<T>(() => undefined);
      }
      throw error;
    }
  };
};

export const installChunkLoadRecovery = (): void => {
  if (typeof window === 'undefined') {
    return;
  }

  window.addEventListener('vite:preloadError', (event) => {
    const viteEvent = event as Event & {
      payload?: unknown;
      preventDefault?: () => void;
    };
    if (reloadOnceForChunkLoadError(viteEvent.payload)) {
      viteEvent.preventDefault?.();
    }
  });

  window.addEventListener('unhandledrejection', (event) => {
    if (reloadOnceForChunkLoadError(event.reason)) {
      event.preventDefault();
    }
  });
};
