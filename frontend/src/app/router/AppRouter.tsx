import { Suspense, lazy } from 'react';
import type { ReactNode } from 'react';
import { Navigate, Route, Routes } from 'react-router-dom';

import ProtectedRoute from '../../components/ProtectedRoute';
import {
  loadCharactersPage,
  loadChapterAnalysisPage,
  loadChaptersPage,
  loadForeshadowsPage,
  loadNovelAutopilotPage,
  loadOutlinePage,
} from '../../routes/projectPageLoaders';
import AppFooterSlot from '../layout/AppFooterSlot';
import RouteFallback from './RouteFallback';
import { withChunkLoadRecovery } from '../../utils/chunkLoadRecovery';

const ProjectList = lazy(withChunkLoadRecovery(() => import('../../pages/ProjectList')));
const ProjectWizardNew = lazy(withChunkLoadRecovery(() => import('../../pages/ProjectWizardNew')));
const Inspiration = lazy(withChunkLoadRecovery(() => import('../../pages/Inspiration')));
const ProjectDetail = lazy(withChunkLoadRecovery(() => import('../../pages/ProjectDetail')));
const WorldSetting = lazy(withChunkLoadRecovery(() => import('../../pages/WorldSetting')));
const NovelAutopilot = lazy(loadNovelAutopilotPage);
const Outline = lazy(loadOutlinePage);
const Characters = lazy(loadCharactersPage);
const Careers = lazy(withChunkLoadRecovery(() => import('../../pages/Careers')));
const Relationships = lazy(withChunkLoadRecovery(() => import('../../pages/Relationships')));
const RelationshipGraph = lazy(withChunkLoadRecovery(() => import('../../pages/RelationshipGraph')));
const Organizations = lazy(withChunkLoadRecovery(() => import('../../pages/Organizations')));
const Chapters = lazy(loadChaptersPage);
const ChapterReader = lazy(withChunkLoadRecovery(() => import('../../pages/ChapterReader')));
const ChapterAnalysis = lazy(loadChapterAnalysisPage);
const Foreshadows = lazy(loadForeshadowsPage);
const WritingStyles = lazy(withChunkLoadRecovery(() => import('../../pages/WritingStyles')));
const PromptWorkshop = lazy(withChunkLoadRecovery(() => import('../../pages/PromptWorkshop')));
const Settings = lazy(withChunkLoadRecovery(() => import('../../pages/Settings')));
const MCPPlugins = lazy(withChunkLoadRecovery(() => import('../../pages/MCPPlugins')));
const UserManagement = lazy(withChunkLoadRecovery(() => import('../../pages/UserManagement')));
const PromptTemplates = lazy(withChunkLoadRecovery(() => import('../../pages/PromptTemplates')));
const Login = lazy(withChunkLoadRecovery(() => import('../../pages/Login')));
const AuthCallback = lazy(withChunkLoadRecovery(() => import('../../pages/AuthCallback')));

function withSuspense(element: ReactNode) {
  return <Suspense fallback={<RouteFallback />}>{element}</Suspense>;
}

export default function AppRouter() {
  return (
    <Routes>
      <Route
        path="/login"
        element={withSuspense(
          <>
            <Login />
            <AppFooterSlot />
          </>
        )}
      />
      <Route path="/auth/callback" element={withSuspense(<AuthCallback />)} />

      <Route
        path="/"
        element={
          <ProtectedRoute>
            {withSuspense(
              <>
                <ProjectList />
                <AppFooterSlot sidebarWidth={220} />
              </>
            )}
          </ProtectedRoute>
        }
      />
      <Route
        path="/projects"
        element={
          <ProtectedRoute>
            {withSuspense(
              <>
                <ProjectList />
                <AppFooterSlot sidebarWidth={220} />
              </>
            )}
          </ProtectedRoute>
        }
      />
      <Route path="/wizard" element={<ProtectedRoute>{withSuspense(<ProjectWizardNew />)}</ProtectedRoute>} />
      <Route path="/inspiration" element={<ProtectedRoute>{withSuspense(<Inspiration />)}</ProtectedRoute>} />
      <Route path="/settings" element={<ProtectedRoute>{withSuspense(<Settings />)}</ProtectedRoute>} />
      <Route
        path="/prompt-templates"
        element={
          <ProtectedRoute>
            {withSuspense(
              <>
                <PromptTemplates />
                <AppFooterSlot />
              </>
            )}
          </ProtectedRoute>
        }
      />
      <Route path="/mcp-plugins" element={<ProtectedRoute>{withSuspense(<MCPPlugins />)}</ProtectedRoute>} />
      <Route path="/user-management" element={<ProtectedRoute>{withSuspense(<UserManagement />)}</ProtectedRoute>} />
      <Route
        path="/chapters/:chapterId/reader"
        element={<ProtectedRoute>{withSuspense(<ChapterReader />)}</ProtectedRoute>}
      />

      <Route path="/project/:projectId" element={<ProtectedRoute>{withSuspense(<ProjectDetail />)}</ProtectedRoute>}>
        <Route index element={<Navigate to="world-setting" replace />} />
        <Route path="world-setting" element={withSuspense(<WorldSetting />)} />
        <Route path="autopilot" element={withSuspense(<NovelAutopilot />)} />
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
      </Route>
    </Routes>
  );
}
