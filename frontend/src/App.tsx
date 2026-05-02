import { BrowserRouter } from 'react-router-dom';
import AppDeferredFeatures from './app/providers/AppDeferredFeatures';
import AppRouter from './app/router/AppRouter';
import './App.css';


type RenderDebugGlobal = typeof globalThis & {
  __NOVEL_RENDER_DEBUG__?: boolean;
  __NOVEL_RENDER_DEBUG_FILTER__?: string[];
  enableNovelRenderDebug?: (filters?: string[]) => void;
  disableNovelRenderDebug?: () => void;
};

if (import.meta.env.DEV) {
  const renderDebugGlobal = globalThis as RenderDebugGlobal;
  renderDebugGlobal.enableNovelRenderDebug = (filters?: string[]) => {
    renderDebugGlobal.__NOVEL_RENDER_DEBUG__ = true;
    renderDebugGlobal.__NOVEL_RENDER_DEBUG_FILTER__ = Array.isArray(filters) && filters.length > 0
      ? [...filters]
      : undefined;
  };
  renderDebugGlobal.disableNovelRenderDebug = () => {
    renderDebugGlobal.__NOVEL_RENDER_DEBUG__ = false;
    renderDebugGlobal.__NOVEL_RENDER_DEBUG_FILTER__ = undefined;
  };
}

function App() {
  return (
    <BrowserRouter
      future={{
        v7_startTransition: true,
        v7_relativeSplatPath: true,
      }}
    >
      <AppDeferredFeatures />
      <AppRouter />
    </BrowserRouter>
  );
}

export default App;
