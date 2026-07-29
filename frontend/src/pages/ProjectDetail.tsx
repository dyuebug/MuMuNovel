import { memo, useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { useParams, useNavigate, Outlet, Link, useLocation } from 'react-router-dom';
import { Layout, Menu, Button, Drawer, Typography, theme } from 'antd';
import {
  ArrowLeftOutlined,
  FileTextOutlined,
  TeamOutlined,
  BookOutlined,
  // ToolOutlined,
  GlobalOutlined,
  MenuFoldOutlined,
  MenuUnfoldOutlined,
  ApartmentOutlined,
  BankOutlined,
  EditOutlined,
  FundOutlined,
  BarChartOutlined,
  TrophyOutlined,
  BulbOutlined,
  CloudOutlined,
  MoonOutlined,
  RobotOutlined,
} from '@ant-design/icons';
import { useStore } from '../store';
import type { Project } from '../types';
import { invalidateAllProjectCollectionFreshness } from '../store/projectCollectionRefresh';
import { preloadProjectPage } from '../routes/projectPageLoaders';
import { preloadProjectNavigationPages, shouldSkipProjectNavigationPreload } from '../routes/projectPageLoaders';
import type { ProjectNavigationPageKey } from '../routes/projectPageLoaders';
import { projectApi } from '../services/modularApi';
import { isRequestCancelledError } from '../services/core/httpClient';
import InlineDeferredPanel from '../components/InlineDeferredPanel';
import { ProjectWorkflowStatePanel } from '../features/projects/workflow';
import { ProjectRuntimeMetricsPanel } from '../features/projects/metrics';
import ThemeSwitch from '../components/ThemeSwitch';
import { useThemeMode } from '../theme/useThemeMode';
import { designDisplayFont } from '../theme/themeConfig';
import { getStoredSidebarCollapsed, setStoredSidebarCollapsed } from '../utils/sidebarState';
import { VERSION_INFO } from '../config/version';
import { useShallow } from 'zustand/react/shallow';

const { Header, Sider, Content } = Layout;
const { Title, Text } = Typography;

// 判断是否为移动端
const isMobile = () => window.innerWidth <= 768;

const projectLoadPromises = new Map<string, Promise<Project>>();

type ProjectStatsItem = {
  label: string;
  value: number;
  unit: string;
};

type WorkspaceFocusItem = {
  title: string;
};

const OUTLET_CONTAINER_STYLE = {
  flex: 1,
  minHeight: 0,
  overflowY: 'auto',
  overflowX: 'hidden',
  display: 'flex',
  flexDirection: 'column',
} as const;

const WORKSPACE_FOCUS: Record<string, WorkspaceFocusItem> = {
  autopilot: {
    title: '自动创作',
  },
  'world-setting': {
    title: '世界设定',
  },
  characters: {
    title: '角色管理',
  },
  organizations: {
    title: '组织管理',
  },
  careers: {
    title: '职业管理',
  },
  relationships: {
    title: '关系管理',
  },
  outline: {
    title: '大纲管理',
  },
  chapters: {
    title: '章节管理',
  },
  'chapter-analysis': {
    title: '剧情分析',
  },
  foreshadows: {
    title: '伏笔管理',
  },
  'writing-styles': {
    title: '写作风格',
  },
  'prompt-workshop': {
    title: '提示词工坊',
  },
};

const ProjectStatsBar = memo(function ProjectStatsBar({
  projectStats,
  token,
  alphaColor,
}: {
  projectStats: ProjectStatsItem[];
  token: ReturnType<typeof theme.useToken>['token'];
  alphaColor: (color: string, alpha: number) => string;
}) {
  return (
    <div style={{ display: 'flex', alignItems: 'center', gap: '12px', zIndex: 1 }}>
      <div style={{ display: 'flex', gap: '16px' }}>
        {projectStats.map((item) => (
          <div
            key={item.label}
            style={{
              display: 'flex',
              flexDirection: 'column',
              alignItems: 'center',
              justifyContent: 'center',
              background: alphaColor('#ffffff', 0.08),
              border: `1px solid ${alphaColor('#f7f1e8', 0.1)}`,
              backdropFilter: 'blur(10px)',
              borderRadius: '22px',
              minWidth: '64px',
              height: '58px',
              padding: '0 14px',
              boxShadow: `0 16px 30px ${alphaColor('#000000', 0.12)}`,
              cursor: 'default',
              transition: 'all 0.3s ease',
            }}
            onMouseEnter={(e) => {
              e.currentTarget.style.transform = 'translateY(-3px) scale(1.02)';
              e.currentTarget.style.boxShadow = `0 20px 36px ${alphaColor('#000000', 0.18)}`;
              e.currentTarget.style.border = `1px solid ${alphaColor('#f7f1e8', 0.18)}`;
            }}
            onMouseLeave={(e) => {
              e.currentTarget.style.transform = 'translateY(0) scale(1)';
              e.currentTarget.style.boxShadow = `0 16px 30px ${alphaColor('#000000', 0.12)}`;
            }}
          >
            <span
              style={{
                fontSize: '11px',
                color: alphaColor('#f7f1e8', 0.66),
                marginBottom: '4px',
                lineHeight: 1,
                letterSpacing: '0.12em',
                textTransform: 'uppercase',
              }}
            >
              {item.label}
            </span>
            <span
              style={{
                fontSize: '15px',
                fontWeight: '600',
                color: '#f7f1e8',
                lineHeight: 1,
                fontFamily: token.fontFamilyCode,
              }}
            >
              {item.value > 10000 ? `${(item.value / 10000).toFixed(1)}w` : item.value}
              <span style={{ fontSize: '10px', marginLeft: '2px', opacity: 0.8 }}>{item.unit}</span>
            </span>
          </div>
        ))}
      </div>
    </div>
  );
});

const ProjectPageOutletContainer = memo(function ProjectPageOutletContainer() {
  return (
    <div style={OUTLET_CONTAINER_STYLE}>
      <Outlet />
    </div>
  );
});

export default function ProjectDetail() {
  const { projectId } = useParams<{ projectId: string }>();
  const navigate = useNavigate();
  const location = useLocation();
  const [collapsed, setCollapsed] = useState<boolean>(() => getStoredSidebarCollapsed());
  const [drawerVisible, setDrawerVisible] = useState(false);
  const [runtimeMetricsVisible, setRuntimeMetricsVisible] = useState(false);
  const [mobile, setMobile] = useState(isMobile());
  const [projectLoadError, setProjectLoadError] = useState<string | null>(null);
  const prefetchedNavigationTargetsRef = useRef<Set<string>>(new Set());
  const projectLoadAbortRef = useRef<AbortController | null>(null);
  const activeProjectIdRef = useRef<string | null>(projectId ?? null);
  const { token } = theme.useToken();
  const alphaColor = (color: string, alpha: number) => `color-mix(in srgb, ${color} ${(alpha * 100).toFixed(0)}%, transparent)`;
  const { mode, resolvedMode, setMode } = useThemeMode();
  const editorialInk = '#f7f1e8';
  const editorialShellBackground = resolvedMode === 'dark'
    ? 'radial-gradient(circle at top, rgba(204, 120, 92, 0.16) 0%, transparent 26%), linear-gradient(180deg, #0f0e0d 0%, #151311 100%)'
    : 'radial-gradient(circle at top, rgba(204, 120, 92, 0.12) 0%, transparent 28%), linear-gradient(180deg, #f8f2e9 0%, #efe6da 100%)';
  const editorialSidebarBackground = 'linear-gradient(180deg, #151311 0%, #1d1916 100%)';
  const editorialHeaderBackground = 'linear-gradient(135deg, #171411 0%, color-mix(in srgb, #171411 64%, #cc785c 36%) 100%)';
  const editorialPanelBackground = resolvedMode === 'dark'
    ? 'linear-gradient(180deg, #171513 0%, #1d1917 100%)'
    : 'linear-gradient(180deg, #fffaf3 0%, #f7f0e7 100%)';
  const editorialPanelBorder = resolvedMode === 'dark'
    ? alphaColor('#f7f1e8', 0.1)
    : alphaColor(token.colorText, 0.1);
  const editorialMenuFooterBackground = alphaColor('#ffffff', resolvedMode === 'dark' ? 0.04 : 0.06);
  const editorialMutedInk = alphaColor(editorialInk, 0.66);
  const cycleThemeMode = () => {
    const nextMode = mode === 'light' ? 'dark' : mode === 'dark' ? 'system' : 'light';
    setMode(nextMode);
  };
  const collapsedThemeIcon = mode === 'light' ? <BulbOutlined /> : mode === 'dark' ? <MoonOutlined /> : <CloudOutlined />;
  const navigateHome = useCallback(() => {
    navigate('/');
  }, [navigate]);

  // 监听窗口大小变化
  useEffect(() => {
    activeProjectIdRef.current = projectId ?? null;
    setRuntimeMetricsVisible(false);
  }, [projectId]);

  useEffect(() => {
    const handleResize = () => {
      setMobile(isMobile());
      if (!isMobile()) {
        setDrawerVisible(false);
      }
    };
    window.addEventListener('resize', handleResize);
    return () => window.removeEventListener('resize', handleResize);
  }, []);

  useEffect(() => {
    setStoredSidebarCollapsed(collapsed);
  }, [collapsed]);

  useEffect(() => {
    if (!projectId || shouldSkipProjectNavigationPreload()) {
      return;
    }

    const windowWithIdleCallback = window as Window & typeof globalThis & {
      requestIdleCallback?: (callback: IdleRequestCallback, options?: IdleRequestOptions) => number;
      cancelIdleCallback?: (handle: number) => void;
    };

    let cancelled = false;
    let timeoutId: number | null = null;
    let idleHandle: number | null = null;

    const runPreload = () => {
      if (cancelled) {
        return;
      }

      void preloadProjectNavigationPages({
        delayMs: 120,
        pages: ['characters', 'chapters', 'careers', 'chapter-analysis', 'foreshadows'],
      });
    };

    if (typeof windowWithIdleCallback.requestIdleCallback === 'function') {
      idleHandle = windowWithIdleCallback.requestIdleCallback(runPreload, { timeout: 1800 });
    } else {
      timeoutId = window.setTimeout(runPreload, 600);
    }

    return () => {
      cancelled = true;
      if (timeoutId !== null) {
        window.clearTimeout(timeoutId);
      }
      if (idleHandle !== null && typeof windowWithIdleCallback.cancelIdleCallback === 'function') {
        windowWithIdleCallback.cancelIdleCallback(idleHandle);
      }
    };
  }, [projectId]);

  const prefetchProjectNavigationTarget = useCallback((
    pageKey?: ProjectNavigationPageKey,
    path?: string,
  ) => {
    const targetKey = pageKey ?? path;
    if (!targetKey) {
      return;
    }

    const chunkPrefetchKey = `chunk:${projectId ?? 'unknown-project'}:${targetKey}`;

    if (pageKey && !prefetchedNavigationTargetsRef.current.has(chunkPrefetchKey)) {
      prefetchedNavigationTargetsRef.current.add(chunkPrefetchKey);
      void preloadProjectPage(pageKey);
    }
  }, [projectId]);

  const {
    currentProjectId,
    currentProjectTitle,
    currentProjectCharacterCount,
    currentProjectChapterCount,
    currentProjectCurrentWords,
    setCurrentProject,
    clearProjectData,
    outlineCount,
  } = useStore(useShallow((state) => ({
    currentProjectId: state.currentProject?.id ?? null,
    currentProjectTitle: state.currentProject?.title ?? '',
    currentProjectCharacterCount: state.currentProject?.character_count ?? 0,
    currentProjectChapterCount: state.currentProject?.chapter_count ?? 0,
    currentProjectCurrentWords: state.currentProject?.current_words ?? 0,
    setCurrentProject: state.setCurrentProject,
    clearProjectData: state.clearProjectData,
    outlineCount: state.outlines.length,
  })));
  const createMenuLink = useCallback((
    path: string,
    label: string,
    pageKey?: ProjectNavigationPageKey,
  ) => {
    const handleHoverPrefetch = () => {
      prefetchProjectNavigationTarget(pageKey, path);
    };

    const handlePressPrefetch = () => {
      prefetchProjectNavigationTarget(pageKey, path);
    };

    return (
      <Link
        to={path}
        onMouseEnter={handleHoverPrefetch}
        onFocus={handleHoverPrefetch}
        onPointerDown={handlePressPrefetch}
        onTouchStart={handlePressPrefetch}
      >
        {label}
      </Link>
    );
  }, [prefetchProjectNavigationTarget]);

  useEffect(() => {
    let cancelled = false;
    const effectProjectId = projectId ?? null;

    const loadProjectData = async (id: string) => {
      setProjectLoadError(null);

      projectLoadAbortRef.current?.abort();
      const abortController = new AbortController();
      projectLoadAbortRef.current = abortController;

      let loadPromise = projectLoadPromises.get(id);
      if (!loadPromise) {
        loadPromise = (async () => {
          try {
            return await projectApi.getProject(id, { signal: abortController.signal });
          } finally {
            projectLoadPromises.delete(id);
          }
        })();

        projectLoadPromises.set(id, loadPromise);
      }

      try {
        const project = await loadPromise;
        if (
          cancelled
          || abortController.signal.aborted
          || activeProjectIdRef.current !== id
        ) {
          return;
        }

        setCurrentProject(project);
      } catch (error) {
        if (isRequestCancelledError(error) || abortController.signal.aborted) {
          return;
        }
        console.error('加载项目数据失败:', error);
        setProjectLoadError(error instanceof Error ? error.message : '加载项目数据失败，请稍后重试');
      } finally {
        if (projectLoadAbortRef.current === abortController) {
          projectLoadAbortRef.current = null;
        }
      }
    };

    if (projectId) {
      void loadProjectData(projectId);
    }

    return () => {
      cancelled = true;
      projectLoadAbortRef.current?.abort();
      if (effectProjectId) {
        projectLoadPromises.delete(effectProjectId);
      }
      if (effectProjectId && activeProjectIdRef.current === effectProjectId) {
        invalidateAllProjectCollectionFreshness(effectProjectId);
        clearProjectData();
      }
    };
  }, [projectId, clearProjectData, setCurrentProject]);

  // 移除事件监听，避免无限循环
  // Hook 内部已经更新了 store，不需要再次刷新

  const projectStats = useMemo(() => {
    if (!currentProjectId) {
      return [];
    }

    return [
      {
        label: '大纲',
        value: outlineCount,
        unit: '条',
      },
      {
        label: '角色',
        value: currentProjectCharacterCount,
        unit: '个',
      },
      {
        label: '章节',
        value: currentProjectChapterCount,
        unit: '章',
      },
      {
        label: '已写',
        value: currentProjectCurrentWords,
        unit: '字',
      },
    ];
  }, [
    currentProjectChapterCount,
    currentProjectCharacterCount,
    currentProjectCurrentWords,
    currentProjectId,
    outlineCount,
  ]);

  const autopilotPath = `/project/${projectId}/autopilot`;
  const worldSettingPath = `/project/${projectId}/world-setting`;
  const charactersPath = `/project/${projectId}/characters`;
  const organizationsPath = `/project/${projectId}/organizations`;
  const careersPath = `/project/${projectId}/careers`;
  const relationshipsPath = `/project/${projectId}/relationships`;
  const outlinePath = `/project/${projectId}/outline`;
  const chaptersPath = `/project/${projectId}/chapters`;
  const chapterAnalysisPath = `/project/${projectId}/chapter-analysis`;
  const foreshadowsPath = `/project/${projectId}/foreshadows`;
  const writingStylesPath = `/project/${projectId}/writing-styles`;
  const promptWorkshopPath = `/project/${projectId}/prompt-workshop`;

  const autopilotLink = useMemo(() => createMenuLink(autopilotPath, '自动创作', 'autopilot'), [autopilotPath, createMenuLink]);
  const worldSettingLink = useMemo(() => <Link to={worldSettingPath}>世界设定</Link>, [worldSettingPath]);
  const writingStylesLink = useMemo(() => <Link to={writingStylesPath}>写作风格</Link>, [writingStylesPath]);
  const promptWorkshopLink = useMemo(() => <Link to={promptWorkshopPath}>提示词工坊</Link>, [promptWorkshopPath]);
  const charactersLink = useMemo(() => createMenuLink(charactersPath, '角色管理', 'characters'), [charactersPath, createMenuLink]);
  const organizationsLink = useMemo(() => createMenuLink(organizationsPath, '组织管理', 'organizations'), [organizationsPath, createMenuLink]);
  const careersLink = useMemo(() => createMenuLink(careersPath, '职业管理', 'careers'), [careersPath, createMenuLink]);
  const relationshipsLink = useMemo(() => createMenuLink(relationshipsPath, '关系管理', 'relationships'), [relationshipsPath, createMenuLink]);
  const outlineLink = useMemo(() => createMenuLink(outlinePath, '大纲管理', 'outline'), [outlinePath, createMenuLink]);
  const chaptersLink = useMemo(() => createMenuLink(chaptersPath, '章节管理', 'chapters'), [chaptersPath, createMenuLink]);
  const chapterAnalysisLink = useMemo(() => createMenuLink(chapterAnalysisPath, '剧情分析', 'chapter-analysis'), [chapterAnalysisPath, createMenuLink]);
  const foreshadowsLink = useMemo(() => createMenuLink(foreshadowsPath, '伏笔管理', 'foreshadows'), [foreshadowsPath, createMenuLink]);

  const menuItems = useMemo(() => [
    {
      type: 'group' as const,
      label: '创作管理',
      children: [
        {
          key: 'autopilot',
          icon: <RobotOutlined />,
          label: autopilotLink,
        },
        {
          key: 'world-setting',
          icon: <GlobalOutlined />,
          label: worldSettingLink,
        },
        {
          key: 'characters',
          icon: <TeamOutlined />,
          label: charactersLink,
        },
        {
          key: 'organizations',
          icon: <BankOutlined />,
          label: organizationsLink,
        },
        {
          key: 'careers',
          icon: <TrophyOutlined />,
          label: careersLink,
        },
        {
          key: 'relationships',
          icon: <ApartmentOutlined />,
          label: relationshipsLink,
        },
        {
          key: 'outline',
          icon: <FileTextOutlined />,
          label: outlineLink,
        },
        {
          key: 'chapters',
          icon: <BookOutlined />,
          label: chaptersLink,
        },
        {
          key: 'chapter-analysis',
          icon: <FundOutlined />,
          label: chapterAnalysisLink,
        },
        {
          key: 'foreshadows',
          icon: <BulbOutlined />,
          label: foreshadowsLink,
        },
      ],
    },
    {
      type: 'group' as const,
      label: '创作工具',
      children: [
        {
          key: 'writing-styles',
          icon: <EditOutlined />,
          label: writingStylesLink,
        },
        {
          key: 'prompt-workshop',
          icon: <CloudOutlined />,
          label: promptWorkshopLink,
        },
      ],
    },
  ], [
    autopilotLink,
    careersLink,
    chapterAnalysisLink,
    chaptersLink,
    charactersLink,
    foreshadowsLink,
    organizationsLink,
    outlineLink,
    promptWorkshopLink,
    relationshipsLink,
    worldSettingLink,
    writingStylesLink,
  ]);

  const menuItemsCollapsed = useMemo(() => [
    {
      key: 'autopilot',
      icon: <RobotOutlined />,
      label: autopilotLink,
    },
    {
      key: 'world-setting',
      icon: <GlobalOutlined />,
      label: worldSettingLink,
    },
    {
      key: 'careers',
      icon: <TrophyOutlined />,
      label: careersLink,
    },
    {
      key: 'characters',
      icon: <TeamOutlined />,
      label: charactersLink,
    },
    {
      key: 'relationships',
      icon: <ApartmentOutlined />,
      label: relationshipsLink,
    },
    {
      key: 'organizations',
      icon: <BankOutlined />,
      label: organizationsLink,
    },
    {
      key: 'outline',
      icon: <FileTextOutlined />,
      label: outlineLink,
    },
    {
      key: 'chapters',
      icon: <BookOutlined />,
      label: chaptersLink,
    },
    {
      key: 'chapter-analysis',
      icon: <FundOutlined />,
      label: chapterAnalysisLink,
    },
    {
      key: 'foreshadows',
      icon: <BulbOutlined />,
      label: foreshadowsLink,
    },
    {
      key: 'writing-styles',
      icon: <EditOutlined />,
      label: writingStylesLink,
    },
    {
      key: 'prompt-workshop',
      icon: <CloudOutlined />,
      label: promptWorkshopLink,
    },
  ], [
    autopilotLink,
    careersLink,
    chapterAnalysisLink,
    chaptersLink,
    charactersLink,
    foreshadowsLink,
    organizationsLink,
    outlineLink,
    promptWorkshopLink,
    relationshipsLink,
    worldSettingLink,
    writingStylesLink,
  ]);

  // 根据当前路径动态确定选中的菜单项
  const selectedKey = useMemo(() => {
    const path = location.pathname;
    if (path.includes('/autopilot')) return 'autopilot';
    if (path.includes('/world-setting')) return 'world-setting';
    if (path.includes('/careers')) return 'careers';
    if (path.includes('/relationships')) return 'relationships';
    if (path.includes('/organizations')) return 'organizations';
    if (path.includes('/outline')) return 'outline';
    if (path.includes('/characters')) return 'characters';
    if (path.includes('/chapter-analysis')) return 'chapter-analysis';
    if (path.includes('/foreshadows')) return 'foreshadows';
    if (path.includes('/chapters')) return 'chapters';
    if (path.includes('/writing-styles')) return 'writing-styles';
    if (path.includes('/prompt-workshop')) return 'prompt-workshop';
    // if (path.includes('/polish')) return 'polish';
    return 'world-setting';
  }, [location.pathname]);

  const currentWorkspaceFocus = useMemo(
    () => WORKSPACE_FOCUS[selectedKey] ?? WORKSPACE_FOCUS['world-setting'],
    [selectedKey],
  );

  const menuNode = useMemo(() => (
    <div style={{
      flex: 1,
      overflowY: 'auto',
      overflowX: 'hidden'
    }}>
      <Menu
        theme="dark"
        mode="inline"
        inlineCollapsed={collapsed}
        selectedKeys={[selectedKey]}
        style={{
          borderRight: 0,
          padding: '12px 10px 0',
          background: 'transparent',
          color: editorialInk,
        }}
        items={collapsed ? menuItemsCollapsed : menuItems}
        onClick={() => mobile && setDrawerVisible(false)}
      />
    </div>
  ), [collapsed, menuItems, menuItemsCollapsed, mobile, selectedKey]);

  if (!currentProjectId || currentProjectId !== projectId) {
    if (projectLoadError) {
      return (
        <div style={{ display: 'flex', justifyContent: 'center', alignItems: 'center', height: '100vh', padding: 24 }}>
          <div style={{ textAlign: 'center', maxWidth: 420 }}>
            <Title level={4}>项目数据加载失败</Title>
            <Text type="secondary">{projectLoadError}</Text>
            <div style={{ marginTop: 24 }}>
              <Button type="primary" onClick={() => navigate('/projects')}>
                返回项目列表
              </Button>
            </div>
          </div>
        </div>
      );
    }

    return (
      <div
        style={{
          display: 'flex',
          justifyContent: 'center',
          alignItems: 'center',
          minHeight: '100vh',
          padding: 24,
          background: editorialShellBackground,
        }}
      >
        <div style={{ width: 'min(560px, 100%)' }}>
          <InlineDeferredPanel
            eyebrow="Project Workspace"
            title="恢复项目导航与工作台骨架"
            message="当前正在读取项目元信息、侧边导航与页面预加载状态。原有项目切换、路由恢复和工作区预热逻辑保持不变。"
            minHeight={300}
            tags={[
              { label: '项目元信息恢复中', color: 'processing' },
              { label: '导航骨架预热', color: 'blue' },
              { label: '页面路由同步', color: 'default' },
            ]}
          />
        </div>
      </div>
    );
  }

  return (
    <Layout style={{ minHeight: '100vh', height: '100vh', overflow: 'hidden', background: editorialShellBackground }}>
      <Header style={{
        background: editorialHeaderBackground,
        padding: mobile ? '0 12px' : '0 24px',
        display: 'flex',
        alignItems: 'center',
        justifyContent: 'space-between',
        position: 'fixed',
        top: 0,
        left: mobile ? 0 : (collapsed ? 60 : 220),
        right: 0,
        zIndex: 1000,
        boxShadow: `0 18px 32px ${alphaColor('#000000', 0.12)}`,
        borderBottom: `1px solid ${alphaColor(editorialInk, 0.08)}`,
        height: mobile ? 56 : 70,
        transition: 'left 0.3s cubic-bezier(0.4, 0, 0.2, 1)',
        overflow: 'hidden'
      }}>
        <div style={{ display: 'flex', alignItems: 'center', gap: '8px', zIndex: 1 }}>
          {mobile && (
            <Button
              type="text"
              icon={<MenuUnfoldOutlined />}
              onClick={() => setDrawerVisible(true)}
              style={{
                fontSize: '18px',
                color: editorialInk,
                width: '36px',
                height: '36px'
              }}
            />
          )}
          {projectId ? (
            <Button
              type="text"
              icon={<BarChartOutlined />}
              onClick={() => setRuntimeMetricsVisible(true)}
              aria-label="打开运行指标"
              style={{ color: editorialInk, height: 36 }}
            >
              {mobile ? null : '运行指标'}
            </Button>
          ) : null}
        </div>

        <h2 style={{
          margin: 0,
          color: editorialInk,
          fontSize: mobile ? '16px' : '28px',
          fontWeight: 600,
          fontFamily: designDisplayFont,
          letterSpacing: '-0.03em',
          position: mobile ? 'static' : 'absolute',
          left: mobile ? 'auto' : '50%',
          transform: mobile ? 'none' : 'translateX(-50%)',
          whiteSpace: 'nowrap',
          overflow: 'hidden',
          textOverflow: 'ellipsis',
          flex: mobile ? 1 : 'none',
          textAlign: mobile ? 'center' : 'left',
          paddingLeft: mobile ? '8px' : '0',
          paddingRight: mobile ? '8px' : '0'
        }}>
          {currentProjectTitle}
        </h2>

        {mobile && (
          <Button
            type="text"
            icon={<ArrowLeftOutlined />}
            onClick={navigateHome}
            style={{
              fontSize: '14px',
              color: editorialInk,
              height: '36px',
              padding: '0 8px',
              zIndex: 1
            }}
          >
            主页
          </Button>
        )}

        {!mobile && (
          <ProjectStatsBar
            projectStats={projectStats}
            token={token}
            alphaColor={alphaColor}
          />
        )}
      </Header>

      <Layout style={{ marginTop: mobile ? 56 : 70 }}>
        {mobile ? (
          <Drawer
            title={
              <div style={{ display: 'flex', alignItems: 'center', gap: 10 }}>
                <div style={{
                  width: 30,
                  height: 30,
                  background: editorialHeaderBackground,
                  borderRadius: 8,
                  display: 'flex',
                  alignItems: 'center',
                  justifyContent: 'center',
                  color: editorialInk,
                  fontSize: 16,
                }}>
                  <BookOutlined />
                </div>
                <span style={{ fontWeight: 600, fontSize: 18, fontFamily: designDisplayFont, color: editorialInk }}>{VERSION_INFO.projectName}</span>
              </div>
            }
            placement="left"
            onClose={() => setDrawerVisible(false)}
            open={drawerVisible}
            width={280}
            styles={{
              header: {
                background: editorialSidebarBackground,
                borderBottom: `1px solid ${alphaColor(editorialInk, 0.08)}`,
              },
              body: {
                padding: 0,
                display: 'flex',
                flexDirection: 'column',
                background: editorialSidebarBackground,
              },
            }}
          >
            {menuNode}
            <div style={{ padding: 16, borderTop: `1px solid ${alphaColor(editorialInk, 0.08)}`, background: editorialMenuFooterBackground }}>
              <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', fontSize: 12, color: editorialMutedInk, marginBottom: 8 }}>
                <span>主题模式</span>
                <span>{mode === 'system' ? `跟随系统 · ${resolvedMode === 'dark' ? '深色' : '浅色'}` : resolvedMode === 'dark' ? '深色' : '浅色'}</span>
              </div>
              <ThemeSwitch block />
            </div>
          </Drawer>
        ) : (
          <Sider
            collapsible
            collapsed={collapsed}
            onCollapse={setCollapsed}
            trigger={null}
            width={220}
            collapsedWidth={60}
            style={{
              position: 'fixed',
              left: 0,
              top: 0,
              bottom: 0,
              overflow: 'hidden',
              transition: 'all 0.3s cubic-bezier(0.4, 0, 0.2, 1)',
              height: '100vh',
              background: editorialSidebarBackground,
              borderRight: `1px solid ${alphaColor(editorialInk, 0.08)}`,
              boxShadow: `18px 0 36px ${alphaColor('#000000', 0.16)}`,
              zIndex: 1000
            }}
          >
            <div style={{
              height: '100%',
              display: 'flex',
              flexDirection: 'column'
            }}>
              <div style={{
                height: 82,
                display: 'flex',
                alignItems: 'center',
                padding: collapsed ? 0 : '0 12px',
                background: editorialHeaderBackground,
                flexShrink: 0,
                justifyContent: collapsed ? 'center' : 'space-between',
                gap: 8
              }}>
                {collapsed ? (
                  <Button
                    type="text"
                    icon={<MenuUnfoldOutlined />}
                    onClick={() => setCollapsed(false)}
                    style={{
                      color: editorialInk,
                      width: '100%',
                      height: '100%',
                      padding: 0,
                      borderRadius: 0,
                      display: 'flex',
                      alignItems: 'center',
                      justifyContent: 'center'
                    }}
                  />
                ) : (
                  <>
                    <div style={{ display: 'flex', alignItems: 'center', gap: 10, minWidth: 0, overflow: 'hidden' }}>
                      <div style={{
                        width: 30,
                        height: 30,
                        background: alphaColor(editorialInk, 0.16),
                        borderRadius: 8,
                        display: 'flex',
                        alignItems: 'center',
                        justifyContent: 'center',
                        color: editorialInk,
                        fontSize: 16,
                        backdropFilter: 'blur(4px)'
                      }}>
                        <BookOutlined />
                      </div>
                      <div style={{ display: 'flex', flexDirection: 'column', minWidth: 0 }}>
                        <span style={{
                          color: editorialMutedInk,
                          fontSize: 10,
                          letterSpacing: '0.18em',
                          textTransform: 'uppercase',
                        }}>
                          Project Workspace
                        </span>
                        <span style={{
                          color: editorialInk,
                          fontWeight: 600,
                          fontSize: 18,
                          fontFamily: designDisplayFont,
                          whiteSpace: 'nowrap',
                          overflow: 'hidden',
                          textOverflow: 'ellipsis'
                        }}>
                          {VERSION_INFO.projectName}
                        </span>
                      </div>
                    </div>
                    <Button
                      type="text"
                      icon={<MenuFoldOutlined />}
                      onClick={() => setCollapsed(true)}
                      style={{
                        color: editorialInk,
                        width: 32,
                        height: 32,
                        padding: 0,
                        flexShrink: 0
                      }}
                    />
                  </>
                )}
              </div>
              {menuNode}
              <div style={{
                padding: collapsed ? '12px 8px' : '12px',
                borderTop: `1px solid ${alphaColor(editorialInk, 0.08)}`,
                flexShrink: 0,
                background: editorialMenuFooterBackground,
              }}>
                {collapsed ? (
                  <div style={{ display: 'flex', flexDirection: 'column', alignItems: 'center', gap: 10 }}>
                    <Button
                      type="text"
                      icon={collapsedThemeIcon}
                      onClick={cycleThemeMode}
                      title={`主题模式：${mode === 'light' ? '浅色' : mode === 'dark' ? '深色' : '跟随系统'}（点击切换）`}
                      style={{
                        width: 40,
                        height: 40,
                        borderRadius: 20,
                        background: alphaColor('#ffffff', 0.08),
                        border: `1px solid ${alphaColor(editorialInk, 0.12)}`,
                        color: editorialInk,
                        padding: 0,
                      }}
                    />
                    <Button
                      type="text"
                      icon={<ArrowLeftOutlined />}
                      onClick={navigateHome}
                      style={{
                        width: 40,
                        height: 40,
                        borderRadius: 20,
                        background: alphaColor('#ffffff', 0.08),
                        border: `1px solid ${alphaColor(editorialInk, 0.12)}`,
                        color: editorialInk,
                        padding: 0,
                      }}
                    />
                  </div>
                ) : (
                  <div style={{ display: 'flex', flexDirection: 'column', gap: 10 }}>
                    <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', fontSize: 12, color: editorialMutedInk }}>
                      <span>主题模式</span>
                      <span>{mode === 'system' ? `跟随系统 · ${resolvedMode === 'dark' ? '深色' : '浅色'}` : resolvedMode === 'dark' ? '深色' : '浅色'}</span>
                    </div>
                    <ThemeSwitch block />
                    <Button
                      type="text"
                      icon={<ArrowLeftOutlined />}
                      onClick={navigateHome}
                      block
                      style={{
                        color: editorialInk,
                        height: 40,
                        justifyContent: 'flex-start',
                        padding: '0 12px',
                        background: alphaColor('#ffffff', 0.05),
                        border: `1px solid ${alphaColor(editorialInk, 0.08)}`,
                      }}
                    >
                      返回主页
                    </Button>
                  </div>
                )}
              </div>
            </div>
          </Sider>
        )}

        <Layout style={{
          marginLeft: mobile ? 0 : (collapsed ? 60 : 220),
          transition: 'margin-left 0.3s cubic-bezier(0.4, 0, 0.2, 1)',
          background: editorialShellBackground,
        }}>
          <Content
            style={{
              background: editorialShellBackground,
              padding: mobile ? 12 : 28,
              height: mobile ? 'calc(100vh - 56px)' : 'calc(100vh - 70px)',
              overflow: 'hidden',
              display: 'flex',
              flexDirection: 'column'
            }}
          >
            <div style={{ display: 'flex', flexDirection: 'column', gap: mobile ? 10 : 16, height: '100%' }}>
              <div
                style={{
                  display: 'flex',
                  alignItems: mobile ? 'flex-start' : 'center',
                  justifyContent: 'space-between',
                  gap: 12,
                  flexDirection: mobile ? 'column' : 'row',
                  borderRadius: mobile ? 14 : 18,
                  padding: mobile ? '12px 14px' : '14px 18px',
                  border: `1px solid ${alphaColor(editorialInk, resolvedMode === 'dark' ? 0.1 : 0.08)}`,
                  background: `linear-gradient(135deg, #171411 0%, color-mix(in srgb, #171411 78%, #cc785c 22%) 100%)`,
                  boxShadow: `0 14px 28px ${alphaColor('#000000', 0.14)}`,
                }}
              >
                <div style={{ minWidth: 0 }}>
                  <Text style={{ color: alphaColor(editorialInk, 0.64), letterSpacing: '0.1em', textTransform: 'uppercase', fontSize: 11 }}>
                    Project
                  </Text>
                  <Title
                    level={mobile ? 5 : 4}
                    style={{
                      margin: '4px 0 0',
                      color: editorialInk,
                      fontFamily: designDisplayFont,
                      letterSpacing: 0,
                    }}
                  >
                    {currentProjectTitle}
                  </Title>
                </div>
                <div
                  style={{
                    display: 'flex',
                    alignItems: mobile ? 'flex-start' : 'center',
                    flexDirection: mobile ? 'column' : 'row',
                    justifyContent: mobile ? 'flex-start' : 'flex-end',
                    gap: 10,
                    minWidth: 0,
                    flexWrap: 'wrap',
                  }}
                >
                  <Text style={{ color: alphaColor(editorialInk, 0.82), fontWeight: 600 }}>
                    {currentWorkspaceFocus.title}
                  </Text>
                  {projectId ? (
                    <ProjectWorkflowStatePanel projectId={projectId} compact={mobile} />
                  ) : null}
                </div>
              </div>

              <div
                data-testid="project-page-content"
                style={{
                  background: editorialPanelBackground,
                  padding: mobile ? 12 : 16,
                  borderRadius: mobile ? '16px' : '24px',
                  border: `1px solid ${editorialPanelBorder}`,
                  boxShadow: `0 22px 48px ${alphaColor(token.colorText, resolvedMode === 'dark' ? 0.18 : 0.08)}`,
                  flex: 1,
                  minHeight: 0,
                  overflow: 'hidden',
                  display: 'flex',
                  flexDirection: 'column'
                }}
              >
                <ProjectPageOutletContainer />
              </div>
            </div>
          </Content>
        </Layout>
      </Layout>

      <Drawer
        title="运行指标"
        placement="right"
        open={runtimeMetricsVisible}
        onClose={() => setRuntimeMetricsVisible(false)}
        width={mobile ? 'calc(100vw - 24px)' : 560}
        destroyOnHidden
      >
        {runtimeMetricsVisible && projectId ? (
          <ProjectRuntimeMetricsPanel projectId={projectId} />
        ) : null}
      </Drawer>
    </Layout>
  );
}
