import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { useParams, useNavigate, Outlet, Link, useLocation } from 'react-router-dom';
import { Layout, Menu, Spin, Button, Drawer, theme } from 'antd';
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
  HeartOutlined,
  TrophyOutlined,
  BulbOutlined,
  CloudOutlined,
  MoonOutlined,
} from '@ant-design/icons';
import { useStore } from '../store';
import { useCharacterSync, useOutlineSync, useChapterSync, loadProjectCharacters, loadProjectOutlines, loadProjectChapters, isProjectCollectionFresh } from '../store/hooks';
import { preloadProjectPage } from '../routes/projectPageLoaders';
import type { ProjectNavigationPageKey } from '../routes/projectPageLoaders';
import { projectApi } from '../services/api';
import { preloadProjectCareers } from '../services/projectCareers';
import ThemeSwitch from '../components/ThemeSwitch';
import { useThemeMode } from '../theme/useThemeMode';
import { getStoredSidebarCollapsed, setStoredSidebarCollapsed } from '../utils/sidebarState';
import { VERSION_INFO } from '../config/version';

const { Header, Sider, Content } = Layout;

// 判断是否为移动端
const isMobile = () => window.innerWidth <= 768;

const projectLoadPromises = new Map<string, Promise<void>>();
const PROJECT_COLLECTION_HYDRATION_DELAY_MS = 1000;

const shouldHydrateProjectCollectionsForPath = (pathname: string) => {
  return !(
    pathname.includes('/outline')
    || pathname.includes('/characters')
    || pathname.includes('/chapters')
    || pathname.includes('/organizations')
    || pathname.includes('/careers')
    || pathname.includes('/relationships')
  );
};

export default function ProjectDetail() {
  const { projectId } = useParams<{ projectId: string }>();
  const navigate = useNavigate();
  const location = useLocation();
  const [collapsed, setCollapsed] = useState<boolean>(() => getStoredSidebarCollapsed());
  const [drawerVisible, setDrawerVisible] = useState(false);
  const [mobile, setMobile] = useState(isMobile());
  const [projectReady, setProjectReady] = useState(false);
  const [isProjectDataHydrating, setIsProjectDataHydrating] = useState(false);
  const shouldHydrateCollectionsRef = useRef(shouldHydrateProjectCollectionsForPath(location.pathname));
  const hydrateTimerRef = useRef<number | null>(null);
  const idleHydrationHandleRef = useRef<number | null>(null);
  const prefetchedNavigationTargetsRef = useRef<Set<string>>(new Set());
  const { token } = theme.useToken();
  const alphaColor = (color: string, alpha: number) => `color-mix(in srgb, ${color} ${(alpha * 100).toFixed(0)}%, transparent)`;
  const { mode, resolvedMode, setMode } = useThemeMode();
  const cycleThemeMode = () => {
    const nextMode = mode === 'light' ? 'dark' : mode === 'dark' ? 'system' : 'light';
    setMode(nextMode);
  };
  const collapsedThemeIcon = mode === 'light' ? <BulbOutlined /> : mode === 'dark' ? <MoonOutlined /> : <CloudOutlined />;
  const cancelScheduledProjectHydration = useCallback(() => {
    const windowWithIdleCallback = window as Window & typeof globalThis & {
      cancelIdleCallback?: (handle: number) => void;
    };

    if (idleHydrationHandleRef.current !== null && typeof windowWithIdleCallback.cancelIdleCallback === 'function') {
      windowWithIdleCallback.cancelIdleCallback(idleHydrationHandleRef.current);
    }
    idleHydrationHandleRef.current = null;

    if (hydrateTimerRef.current !== null) {
      window.clearTimeout(hydrateTimerRef.current);
    }
    hydrateTimerRef.current = null;
  }, []);

  // 监听窗口大小变化
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
    shouldHydrateCollectionsRef.current = shouldHydrateProjectCollectionsForPath(location.pathname);
    if (!shouldHydrateCollectionsRef.current) {
      cancelScheduledProjectHydration();
      setIsProjectDataHydrating(false);
    }
  }, [location.pathname, cancelScheduledProjectHydration]);

  const prefetchProjectNavigationTarget = useCallback((
    pageKey?: ProjectNavigationPageKey,
    path?: string,
  ) => {
    const targetKey = pageKey ?? path;
    if (!targetKey) {
      return;
    }

    const chunkPrefetchKey = `chunk:${projectId ?? 'unknown-project'}:${targetKey}`;
    const dataPrefetchKey = `data:${projectId ?? 'unknown-project'}:${targetKey}`;

    if (pageKey && !prefetchedNavigationTargetsRef.current.has(chunkPrefetchKey)) {
      prefetchedNavigationTargetsRef.current.add(chunkPrefetchKey);
      void preloadProjectPage(pageKey);
    }

    if (!projectId || prefetchedNavigationTargetsRef.current.has(dataPrefetchKey)) {
      return;
    }

    if (pageKey === 'outline') {
      if (!isProjectCollectionFresh('outlines', projectId)) {
        prefetchedNavigationTargetsRef.current.add(dataPrefetchKey);
        void loadProjectOutlines(projectId, { silent: true });
      }
      return;
    }

    if (pageKey === 'characters' || pageKey === 'organizations' || pageKey === 'relationships') {
      if (!isProjectCollectionFresh('characters', projectId)) {
        prefetchedNavigationTargetsRef.current.add(dataPrefetchKey);
        void loadProjectCharacters(projectId, { silent: true });
      }
      return;
    }

    if (pageKey === 'chapters') {
      if (!isProjectCollectionFresh('chapters', projectId)) {
        prefetchedNavigationTargetsRef.current.add(dataPrefetchKey);
        void loadProjectChapters(projectId, { silent: true });
      }
      return;
    }

    prefetchedNavigationTargetsRef.current.add(dataPrefetchKey);

    if (pageKey === 'careers') {
      void preloadProjectCareers(projectId);
    }
  }, [projectId]);

  const currentProject = useStore((state) => state.currentProject);
  const setCurrentProject = useStore((state) => state.setCurrentProject);
  const clearProjectData = useStore((state) => state.clearProjectData);
  const outlineCount = useStore((state) => state.outlines.length);
  const characterCount = useStore((state) => state.characters.length);
  const chapterCount = useStore((state) => state.chapters.length);

  const createMenuLink = useCallback((
    path: string,
    label: string,
    pageKey?: ProjectNavigationPageKey,
  ) => {
    const handleIntentPrefetch = () => {
      prefetchProjectNavigationTarget(pageKey, path);
    };

    const handleNavigate = () => {
      if (!shouldHydrateProjectCollectionsForPath(path)) {
        cancelScheduledProjectHydration();
      }
    };

    return (
      <Link
        to={path}
        onMouseEnter={handleIntentPrefetch}
        onFocus={handleIntentPrefetch}
        onPointerDown={handleIntentPrefetch}
        onTouchStart={handleIntentPrefetch}
        onClick={handleNavigate}
      >
        {label}
      </Link>
    );
  }, [cancelScheduledProjectHydration, prefetchProjectNavigationTarget]);

  // 使用同步 hooks
  const { refreshCharacters } = useCharacterSync();
  const { refreshOutlines } = useOutlineSync();
  const { refreshChapters } = useChapterSync();

  useEffect(() => {
    let cancelled = false;
    const windowWithIdleCallback = window as Window & typeof globalThis & {
      requestIdleCallback?: (callback: IdleRequestCallback, options?: IdleRequestOptions) => number;
    };

    const hydrateProjectCollections = async (id: string) => {
      if (cancelled || !shouldHydrateCollectionsRef.current) {
        return;
      }

      const pendingLoads: Promise<unknown>[] = [];
      if (!isProjectCollectionFresh('outlines', id)) {
        pendingLoads.push(refreshOutlines(id));
      }
      if (!isProjectCollectionFresh('characters', id)) {
        pendingLoads.push(refreshCharacters(id));
      }
      if (!isProjectCollectionFresh('chapters', id)) {
        pendingLoads.push(refreshChapters(id));
      }

      if (pendingLoads.length === 0) {
        return;
      }

      setIsProjectDataHydrating(true);
      try {
        await Promise.all(pendingLoads);
      } catch (error) {
        console.error('Background project detail hydration failed:', error);
      } finally {
        if (!cancelled) {
          setIsProjectDataHydrating(false);
        }
      }
    };

    const scheduleProjectCollectionHydration = (id: string) => {
      cancelScheduledProjectHydration();
      hydrateTimerRef.current = window.setTimeout(() => {
        hydrateTimerRef.current = null;

        if (!shouldHydrateCollectionsRef.current) {
          return;
        }

        if (typeof windowWithIdleCallback.requestIdleCallback === 'function') {
          idleHydrationHandleRef.current = windowWithIdleCallback.requestIdleCallback(() => {
            idleHydrationHandleRef.current = null;
            if (!shouldHydrateCollectionsRef.current) {
              return;
            }
            void hydrateProjectCollections(id);
          }, { timeout: 1500 });
          return;
        }

        void hydrateProjectCollections(id);
      }, PROJECT_COLLECTION_HYDRATION_DELAY_MS);
    };

    const loadProjectData = async (id: string) => {
      const existingLoad = projectLoadPromises.get(id);
      if (existingLoad) {
        await existingLoad;
        if (!cancelled && useStore.getState().currentProject?.id === id) {
          setProjectReady(true);
        }
        return;
      }

      cancelScheduledProjectHydration();
      setProjectReady(false);
      setIsProjectDataHydrating(false);

      const loadPromise = (async () => {
        try {
          const project = await projectApi.getProject(id);
          if (cancelled) {
            return;
          }

          setCurrentProject(project);
          setProjectReady(true);
          scheduleProjectCollectionHydration(id);
        } catch (error) {
          console.error('加载项目数据失败:', error);
        } finally {
          projectLoadPromises.delete(id);
        }
      })();

      projectLoadPromises.set(id, loadPromise);
      await loadPromise;
    };

    if (projectId) {
      void loadProjectData(projectId);
    }

    return () => {
      cancelled = true;
      cancelScheduledProjectHydration();
      setProjectReady(false);
      setIsProjectDataHydrating(false);
      clearProjectData();
    };
  }, [projectId, clearProjectData, setCurrentProject, refreshOutlines, refreshCharacters, refreshChapters, cancelScheduledProjectHydration]);

  // 移除事件监听，避免无限循环
  // Hook 内部已经更新了 store，不需要再次刷新

  const projectStats = useMemo(() => {
    if (!currentProject) {
      return [];
    }

    return [
      {
        label: '大纲',
        value: isProjectDataHydrating && outlineCount === 0 ? '—' : outlineCount,
        unit: '条',
      },
      {
        label: '角色',
        value: characterCount > 0 ? characterCount : (currentProject.character_count ?? (isProjectDataHydrating ? '—' : 0)),
        unit: '个',
      },
      {
        label: '章节',
        value: chapterCount > 0 ? chapterCount : (currentProject.chapter_count ?? (isProjectDataHydrating ? '—' : 0)),
        unit: '章',
      },
      {
        label: '已写',
        value: currentProject.current_words,
        unit: '字',
      },
    ];
  }, [currentProject, isProjectDataHydrating, outlineCount, characterCount, chapterCount]);

  const menuItems = useMemo(() => [
    {
      key: 'sponsor',
      icon: <HeartOutlined />,
      label: <Link to={`/project/${projectId}/sponsor`}>赞助支持</Link>,
    },
    {
      type: 'group' as const,
      label: '创作管理',
      children: [
        {
          key: 'world-setting',
          icon: <GlobalOutlined />,
          label: <Link to={`/project/${projectId}/world-setting`}>世界设定</Link>,
        },
        {
          key: 'characters',
          icon: <TeamOutlined />,
          label: createMenuLink(`/project/${projectId}/characters`, '角色管理', 'characters'),
        },
        {
          key: 'organizations',
          icon: <BankOutlined />,
          label: createMenuLink(`/project/${projectId}/organizations`, '组织管理', 'organizations'),
        },
        {
          key: 'careers',
          icon: <TrophyOutlined />,
          label: createMenuLink(`/project/${projectId}/careers`, '职业管理', 'careers'),
        },
        {
          key: 'relationships',
          icon: <ApartmentOutlined />,
          label: createMenuLink(`/project/${projectId}/relationships`, '关系管理', 'relationships'),
        },
        {
          key: 'outline',
          icon: <FileTextOutlined />,
          label: createMenuLink(`/project/${projectId}/outline`, '大纲管理', 'outline'),
        },
        {
          key: 'chapters',
          icon: <BookOutlined />,
          label: createMenuLink(`/project/${projectId}/chapters`, '章节管理', 'chapters'),
        },
        {
          key: 'chapter-analysis',
          icon: <FundOutlined />,
          label: <Link to={`/project/${projectId}/chapter-analysis`}>剧情分析</Link>,
        },
        {
          key: 'foreshadows',
          icon: <BulbOutlined />,
          label: <Link to={`/project/${projectId}/foreshadows`}>伏笔管理</Link>,
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
          label: <Link to={`/project/${projectId}/writing-styles`}>写作风格</Link>,
        },
        {
          key: 'prompt-workshop',
          icon: <CloudOutlined />,
          label: <Link to={`/project/${projectId}/prompt-workshop`}>提示词工坊</Link>,
        },
      ],
    },
  ], [projectId, createMenuLink]);

  const menuItemsCollapsed = useMemo(() => [
    {
      key: 'sponsor',
      icon: <HeartOutlined />,
      label: <Link to={`/project/${projectId}/sponsor`}>赞助支持</Link>,
    },
    {
      key: 'world-setting',
      icon: <GlobalOutlined />,
      label: <Link to={`/project/${projectId}/world-setting`}>世界设定</Link>,
    },
    {
      key: 'careers',
      icon: <TrophyOutlined />,
      label: createMenuLink(`/project/${projectId}/careers`, '职业管理', 'careers'),
    },
    {
      key: 'characters',
      icon: <TeamOutlined />,
      label: createMenuLink(`/project/${projectId}/characters`, '角色管理', 'characters'),
    },
    {
      key: 'relationships',
      icon: <ApartmentOutlined />,
      label: createMenuLink(`/project/${projectId}/relationships`, '关系管理', 'relationships'),
    },
    {
      key: 'organizations',
      icon: <BankOutlined />,
      label: createMenuLink(`/project/${projectId}/organizations`, '组织管理', 'organizations'),
    },
    {
      key: 'outline',
      icon: <FileTextOutlined />,
      label: createMenuLink(`/project/${projectId}/outline`, '大纲管理', 'outline'),
    },
    {
      key: 'chapters',
      icon: <BookOutlined />,
      label: createMenuLink(`/project/${projectId}/chapters`, '章节管理', 'chapters'),
    },
    {
      key: 'chapter-analysis',
      icon: <FundOutlined />,
      label: <Link to={`/project/${projectId}/chapter-analysis`}>剧情分析</Link>,
    },
    {
      key: 'foreshadows',
      icon: <BulbOutlined />,
      label: <Link to={`/project/${projectId}/foreshadows`}>伏笔管理</Link>,
    },
    {
      key: 'writing-styles',
      icon: <EditOutlined />,
      label: <Link to={`/project/${projectId}/writing-styles`}>写作风格</Link>,
    },
    {
      key: 'prompt-workshop',
      icon: <CloudOutlined />,
      label: <Link to={`/project/${projectId}/prompt-workshop`}>提示词工坊</Link>,
    },
  ], [projectId, createMenuLink]);

  // 根据当前路径动态确定选中的菜单项
  const selectedKey = useMemo(() => {
    const path = location.pathname;
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
    if (path.includes('/sponsor')) return 'sponsor';
    // if (path.includes('/polish')) return 'polish';
    return 'sponsor'; // 默认选中赞助支持
  }, [location.pathname]);

  if (!projectReady || !currentProject || currentProject.id !== projectId) {
    return (
      <div style={{ display: 'flex', justifyContent: 'center', alignItems: 'center', height: '100vh' }}>
        <Spin size="large" />
      </div>
    );
  }

  // 渲染菜单内容
  const renderMenu = () => (
    <div style={{
      flex: 1,
      overflowY: 'auto',
      overflowX: 'hidden'
    }}>
      <Menu
        mode="inline"
        inlineCollapsed={collapsed}
        selectedKeys={[selectedKey]}
        style={{
          borderRight: 0,
          paddingTop: '12px'
        }}
        items={collapsed ? menuItemsCollapsed : menuItems}
        onClick={() => mobile && setDrawerVisible(false)}
      />
    </div>
  );

  return (
    <Layout style={{ minHeight: '100vh', height: '100vh', overflow: 'hidden' }}>
      <Header style={{
        background: token.colorPrimary,
        padding: mobile ? '0 12px' : '0 24px',
        display: 'flex',
        alignItems: 'center',
        justifyContent: 'space-between',
        position: 'fixed',
        top: 0,
        left: mobile ? 0 : (collapsed ? 60 : 220),
        right: 0,
        zIndex: 1000,
        boxShadow: `0 2px 10px ${alphaColor(token.colorText, 0.16)}`,
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
                color: token.colorWhite,
                width: '36px',
                height: '36px'
              }}
            />
          )}
        </div>

        <h2 style={{
          margin: 0,
          color: token.colorWhite,
          fontSize: mobile ? '16px' : '24px',
          fontWeight: 600,
          textShadow: `0 2px 4px ${alphaColor(token.colorText, 0.2)}`,
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
          {currentProject.title}
        </h2>

        {mobile && (
          <Button
            type="text"
            icon={<ArrowLeftOutlined />}
            onClick={() => navigate('/')}
            style={{
              fontSize: '14px',
              color: token.colorWhite,
              height: '36px',
              padding: '0 8px',
              zIndex: 1
            }}
          >
            主页
          </Button>
        )}

        {!mobile && (
          <div style={{ display: 'flex', alignItems: 'center', gap: '12px', zIndex: 1 }}>
            <div style={{ display: 'flex', gap: '16px' }}>
              {projectStats.map((item, index) => (
                <div
                  key={index}
                  style={{
                    display: 'flex',
                    flexDirection: 'column',
                    alignItems: 'center',
                    justifyContent: 'center',
                    backdropFilter: 'blur(4px)',
                    borderRadius: '28px',
                    minWidth: '56px',
                    height: '56px',
                    padding: '0 12px',
                    boxShadow: `inset 0 0 15px ${alphaColor(token.colorWhite, 0.15)}, 0 4px 10px ${alphaColor(token.colorText, 0.1)}`,
                    cursor: 'default',
                    transition: 'all 0.3s ease',
                  }}
                  onMouseEnter={(e) => {
                    e.currentTarget.style.transform = 'translateY(-3px) scale(1.02)';
                    e.currentTarget.style.boxShadow = `inset 0 0 20px ${alphaColor(token.colorWhite, 0.25)}, 0 8px 16px ${alphaColor(token.colorText, 0.15)}`;
                    e.currentTarget.style.border = `1px solid ${alphaColor(token.colorWhite, 0.1)}`;
                  }}
                  onMouseLeave={(e) => {
                    e.currentTarget.style.transform = 'translateY(0) scale(1)';
                    e.currentTarget.style.boxShadow = `inset 0 0 15px ${alphaColor(token.colorWhite, 0.15)}, 0 4px 10px ${alphaColor(token.colorText, 0.1)}`;
                  }}
                >
                  <span style={{
                    fontSize: '11px',
                    color: alphaColor(token.colorWhite, 0.9),
                    marginBottom: '2px',
                    lineHeight: 1
                  }}>
                    {item.label}
                  </span>
                  <span style={{
                    fontSize: '15px',
                    fontWeight: '600',
                    color: token.colorWhite,
                    lineHeight: 1,
                    fontFamily: 'Monaco, monospace'
                  }}>
                    {typeof item.value === 'number' && item.value > 10000 ? (item.value / 10000).toFixed(1) + 'w' : item.value}
                    <span style={{ fontSize: '10px', marginLeft: '2px', opacity: 0.8 }}>{item.unit}</span>
                  </span>
                </div>
              ))}
            </div>
          </div>
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
                  background: token.colorPrimary,
                  borderRadius: 8,
                  display: 'flex',
                  alignItems: 'center',
                  justifyContent: 'center',
                  color: token.colorWhite,
                  fontSize: 16,
                }}>
                  <BookOutlined />
                </div>
                <span style={{ fontWeight: 600, fontSize: 16 }}>{VERSION_INFO.projectName}</span>
              </div>
            }
            placement="left"
            onClose={() => setDrawerVisible(false)}
            open={drawerVisible}
            width={280}
            styles={{ body: { padding: 0, display: 'flex', flexDirection: 'column' } }}
          >
            {renderMenu()}
            <div style={{ padding: 16, borderTop: `1px solid ${token.colorBorderSecondary}` }}>
              <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', fontSize: 12, color: token.colorTextTertiary, marginBottom: 8 }}>
                <span>主题模式</span>
                <span>{resolvedMode === 'dark' ? '深色' : '浅色'}</span>
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
              background: token.colorBgContainer,
              borderRight: `1px solid ${token.colorBorderSecondary}`,
              boxShadow: `4px 0 16px ${alphaColor(token.colorText, 0.06)}`,
              zIndex: 1000
            }}
          >
            <div style={{
              height: '100%',
              display: 'flex',
              flexDirection: 'column'
            }}>
              <div style={{
                height: 70,
                display: 'flex',
                alignItems: 'center',
                padding: collapsed ? 0 : '0 12px',
                background: token.colorPrimary,
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
                      color: token.colorWhite,
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
                        background: alphaColor(token.colorWhite, 0.2),
                        borderRadius: 8,
                        display: 'flex',
                        alignItems: 'center',
                        justifyContent: 'center',
                        color: token.colorWhite,
                        fontSize: 16,
                        backdropFilter: 'blur(4px)'
                      }}>
                        <BookOutlined />
                      </div>
                      <span style={{
                        color: token.colorWhite,
                        fontWeight: 600,
                        fontSize: 15,
                        whiteSpace: 'nowrap',
                        overflow: 'hidden',
                        textOverflow: 'ellipsis'
                      }}>
                        {VERSION_INFO.projectName}
                      </span>
                    </div>
                    <Button
                      type="text"
                      icon={<MenuFoldOutlined />}
                      onClick={() => setCollapsed(true)}
                      style={{
                        color: token.colorWhite,
                        width: 32,
                        height: 32,
                        padding: 0,
                        flexShrink: 0
                      }}
                    />
                  </>
                )}
              </div>
              {renderMenu()}
              <div style={{
                padding: collapsed ? '12px 8px' : '12px',
                borderTop: `1px solid ${token.colorBorderSecondary}`,
                flexShrink: 0
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
                        background: alphaColor(token.colorBgContainer, 0.65),
                        border: `1px solid ${token.colorBorder}`,
                        color: token.colorText,
                        padding: 0,
                      }}
                    />
                    <Button
                      type="text"
                      icon={<ArrowLeftOutlined />}
                      onClick={() => navigate('/')}
                      style={{
                        width: 40,
                        height: 40,
                        borderRadius: 20,
                        background: alphaColor(token.colorBgContainer, 0.65),
                        border: `1px solid ${token.colorBorder}`,
                        color: token.colorText,
                        padding: 0,
                      }}
                    />
                  </div>
                ) : (
                  <div style={{ display: 'flex', flexDirection: 'column', gap: 10 }}>
                    <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', fontSize: 12, color: token.colorTextTertiary }}>
                      <span>主题模式</span>
                      <span>{resolvedMode === 'dark' ? '深色' : '浅色'}</span>
                    </div>
                    <ThemeSwitch block />
                    <Button
                      type="text"
                      icon={<ArrowLeftOutlined />}
                      onClick={() => navigate('/')}
                      block
                      style={{
                        color: token.colorText,
                        height: 40,
                        justifyContent: 'flex-start',
                        padding: '0 12px'
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
          transition: 'margin-left 0.3s cubic-bezier(0.4, 0, 0.2, 1)'
        }}>
          <Content
            style={{
              background: token.colorBgLayout,
              padding: mobile ? 12 : 24,
              height: mobile ? 'calc(100vh - 56px)' : 'calc(100vh - 70px)',
              overflow: 'hidden',
              display: 'flex',
              flexDirection: 'column'
            }}
          >
            <div style={{
              background: token.colorBgContainer,
              padding: mobile ? 12 : 24,
              borderRadius: mobile ? '8px' : '12px',
              boxShadow: `0 8px 24px ${alphaColor(token.colorText, 0.08)}`,
              height: '100%',
              overflow: 'hidden',
              display: 'flex',
              flexDirection: 'column'
            }}>
              <Outlet />
            </div>
          </Content>
        </Layout>
      </Layout>
    </Layout>
  );
}
