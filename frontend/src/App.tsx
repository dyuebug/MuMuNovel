import { Suspense, lazy, useEffect, useState } from 'react';
import type { ReactNode } from 'react';
import { BrowserRouter, Routes, Route, Navigate } from 'react-router-dom';
import ProtectedRoute from './components/ProtectedRoute';
import LoadingScreen from './components/LoadingScreen';
import { loadOutlinePage, loadCharactersPage, loadChaptersPage } from './routes/projectPageLoaders';
import './App.css';

const AppFooter = lazy(() => import('./components/AppFooter'));
const SpringFestival = lazy(() => import('./components/SpringFestival'));
const BackgroundTaskCenter = lazy(() => import('./components/BackgroundTaskCenter'));
const ProjectList = lazy(() => import('./pages/ProjectList'));
const ProjectWizardNew = lazy(() => import('./pages/ProjectWizardNew'));
const Inspiration = lazy(() => import('./pages/Inspiration'));
const ProjectDetail = lazy(() => import('./pages/ProjectDetail'));
const WorldSetting = lazy(() => import('./pages/WorldSetting'));
const Outline = lazy(loadOutlinePage);
const Characters = lazy(loadCharactersPage);
const Careers = lazy(() => import('./pages/Careers'));
const Relationships = lazy(() => import('./pages/Relationships'));
const RelationshipGraph = lazy(() => import('./pages/RelationshipGraph'));
const Organizations = lazy(() => import('./pages/Organizations'));
const Chapters = lazy(loadChaptersPage);
const ChapterReader = lazy(() => import('./pages/ChapterReader'));
const ChapterAnalysis = lazy(() => import('./pages/ChapterAnalysis'));
const Foreshadows = lazy(() => import('./pages/Foreshadows'));
const WritingStyles = lazy(() => import('./pages/WritingStyles'));
const PromptWorkshop = lazy(() => import('./pages/PromptWorkshop'));
const Settings = lazy(() => import('./pages/Settings'));
const MCPPlugins = lazy(() => import('./pages/MCPPlugins'));
const UserManagement = lazy(() => import('./pages/UserManagement'));
const PromptTemplates = lazy(() => import('./pages/PromptTemplates'));
const Sponsor = lazy(() => import('./pages/Sponsor'));
const Login = lazy(() => import('./pages/Login'));
const AuthCallback = lazy(() => import('./pages/AuthCallback'));

function RouteFallback() {
  return <LoadingScreen message="加载中..." minHeight="40vh" />;
}

function withSuspense(element: ReactNode) {
  return <Suspense fallback={<RouteFallback />}>{element}</Suspense>;
}

function renderFooter(sidebarWidth?: number) {
  return (
    <Suspense fallback={null}>
      <AppFooter sidebarWidth={sidebarWidth} />
    </Suspense>
  );
}

function App() {
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
      <BrowserRouter
        future={{
          v7_startTransition: true,
          v7_relativeSplatPath: true,
        }}
      >
        <Routes>
          <Route path="/login" element={withSuspense(<><Login />{renderFooter()}</>)} />
          <Route path="/auth/callback" element={withSuspense(<AuthCallback />)} />

          <Route
            path="/"
            element={<ProtectedRoute>{withSuspense(<><ProjectList />{renderFooter(220)}</>)}</ProtectedRoute>}
          />
          <Route
            path="/projects"
            element={<ProtectedRoute>{withSuspense(<><ProjectList />{renderFooter(220)}</>)}</ProtectedRoute>}
          />
          <Route path="/wizard" element={<ProtectedRoute>{withSuspense(<ProjectWizardNew />)}</ProtectedRoute>} />
          <Route path="/inspiration" element={<ProtectedRoute>{withSuspense(<Inspiration />)}</ProtectedRoute>} />
          <Route path="/settings" element={<ProtectedRoute>{withSuspense(<Settings />)}</ProtectedRoute>} />
          <Route
            path="/prompt-templates"
            element={<ProtectedRoute>{withSuspense(<><PromptTemplates />{renderFooter()}</>)}</ProtectedRoute>}
          />
          <Route path="/mcp-plugins" element={<ProtectedRoute>{withSuspense(<MCPPlugins />)}</ProtectedRoute>} />
          <Route path="/user-management" element={<ProtectedRoute>{withSuspense(<UserManagement />)}</ProtectedRoute>} />
          <Route path="/chapters/:chapterId/reader" element={<ProtectedRoute>{withSuspense(<ChapterReader />)}</ProtectedRoute>} />
          <Route path="/project/:projectId" element={<ProtectedRoute>{withSuspense(<ProjectDetail />)}</ProtectedRoute>}>
            <Route index element={<Navigate to="sponsor" replace />} />
            <Route path="world-setting" element={withSuspense(<WorldSetting />)} />
            <Route path="careers" element={withSuspense(<Careers />)} />
            <Route path="outline" element={withSuspense(<Outline />)} />
            <Route path="characters" element={withSuspense(<Characters />)} />
            <Route path="relationships" element={withSuspense(<Relationships />)} />
            <Route path="relationships-graph" element={withSuspense(<RelationshipGraph />)} />
            <Route path="organizations" element={withSuspense(<Organizations />)} />
            <Route path="chapters" element={withSuspense(<Chapters />)} />
            <Route path="chapter-analysis" element={withSuspense(<ChapterAnalysis />)} />
            <Route path="foreshadows" element={withSuspense(<Foreshadows />)} />
            <Route path="writing-styles" element={withSuspense(<WritingStyles />)} />
            <Route path="prompt-workshop" element={withSuspense(<PromptWorkshop />)} />
            <Route path="sponsor" element={withSuspense(<Sponsor />)} />
          </Route>
        </Routes>
        {deferredUiReady ? (
          <Suspense fallback={null}>
            <BackgroundTaskCenter />
          </Suspense>
        ) : null}
      </BrowserRouter>
    </>
  );
}

export default App;
