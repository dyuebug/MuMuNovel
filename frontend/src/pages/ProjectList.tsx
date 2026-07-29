import { Suspense, lazy, useEffect, useState, useRef, useCallback, useMemo } from 'react';
import { useNavigate, useLocation } from 'react-router-dom';
import { Button, Drawer, Menu, Modal, message, Space, Tag, theme } from 'antd';
import { EditOutlined, BookOutlined, CalendarOutlined, FileTextOutlined, TrophyOutlined, SettingOutlined, UploadOutlined, ApiOutlined, FileSearchOutlined, MenuUnfoldOutlined, MenuFoldOutlined, BulbOutlined, MoonOutlined, DesktopOutlined } from '@ant-design/icons';
import { projectApi } from '../services/modularApi';
import { useStore } from '../store';
import { useProjectSync } from '../store/hooks';
import { eventBus, EventNames } from '../store/eventBus';
import type { ReactNode } from 'react';
import type { Project } from '../types';
import InlineDeferredPanel from '../components/InlineDeferredPanel';
import WorkflowEntryFallback from '../components/WorkflowEntryFallback';
import UserMenu from '../components/UserMenu';
import ThemeSwitch from '../components/ThemeSwitch';
import { useThemeMode } from '../theme/useThemeMode';
import { designDisplayFont } from '../theme/themeConfig';
import { getStoredSidebarCollapsed, setStoredSidebarCollapsed } from '../utils/sidebarState';
import { isProjectWizardIncomplete } from '../utils/projectWizardState';
import { VERSION_INFO } from '../config/version';

const LazyChangelogFloatingButton = lazy(() => import('../components/ChangelogFloatingButton'));
const LazySettingsPage = lazy(() => import('./Settings'));
const LazyMCPPluginsPage = lazy(() => import('./MCPPlugins'));
const LazyPromptTemplates = lazy(() => import('./PromptTemplates'));
const LazyBookImport = lazy(() => import('./BookImport'));
const LazyBookshelfPage = lazy(() => import('./BookshelfPage'));
const LazyProjectImportModal = lazy(() => import('../components/ProjectImportModal'));
const LazyProjectExportModal = lazy(() => import('../components/ProjectExportModal'));


/**
 * 格式化字数显示
 * @param count 字数
 * @returns 格式化后的字符串，如 "1.2K", "3.5W", "1.2M"
 */
const formatWordCount = (count: number): string => {
  if (count < 1000) {
    return count.toString();
  } else if (count < 10000) {
    // 1K - 9.9K
    return (count / 1000).toFixed(1).replace(/\.0$/, '') + 'K';
  } else if (count < 1000000) {
    // 1W - 99.9W (万)
    return (count / 10000).toFixed(1).replace(/\.0$/, '') + 'W';
  } else {
    // 1M+ (百万)
    return (count / 1000000).toFixed(1).replace(/\.0$/, '') + 'M';
  }
};

type ProjectListView = 'projects' | 'settings' | 'mcp' | 'prompts' | 'book-import';

const parseViewFromSearch = (search: string): ProjectListView => {
  const view = new URLSearchParams(search).get('view');
  if (view === 'settings' || view === 'mcp' || view === 'prompts' || view === 'book-import' || view === 'projects') {
    return view;
  }
  return 'projects';
};

export default function ProjectList() {
  const navigate = useNavigate();
  const location = useLocation();
  const { projects, loading } = useStore();
  const updateProjectInStore = useStore((state) => state.updateProject);
  const [drawerVisible, setDrawerVisible] = useState(false);
  const [collapsed, setCollapsed] = useState<boolean>(() => getStoredSidebarCollapsed());
  const [modal, contextHolder] = Modal.useModal();
  const [showApiTip, setShowApiTip] = useState(true);
  const [importModalVisible, setImportModalVisible] = useState(false);
  const [exportModalVisible, setExportModalVisible] = useState(false);
  const [selectedFile, setSelectedFile] = useState<File | null>(null);
  const [validationResult, setValidationResult] = useState<any>(null); // eslint-disable-line @typescript-eslint/no-explicit-any
  const [importing, setImporting] = useState(false);
  const [validating, setValidating] = useState(false);
  const [exporting, setExporting] = useState(false);
  const [selectedProjectIds, setSelectedProjectIds] = useState<string[]>([]);
  const [exportOptions, setExportOptions] = useState({
    includeWritingStyles: true,
    includeGenerationHistory: false,
    includeCareers: true,
    includeMemories: false,
    includePlotAnalysis: false,
  });
  const { refreshProjects, deleteProject } = useProjectSync();
  const { mode, resolvedMode, setMode } = useThemeMode();
  const { token } = theme.useToken();
  const alphaColor = (color: string, alpha: number) => `color-mix(in srgb, ${color} ${(alpha * 100).toFixed(0)}%, transparent)`;

  const activeView = useMemo<ProjectListView>(() => parseViewFromSearch(location.search), [location.search]);
  const cycleThemeMode = () => {
    const nextMode = mode === 'light' ? 'dark' : mode === 'dark' ? 'system' : 'light';
    setMode(nextMode);
  };
  const collapsedThemeIcon = mode === 'light' ? <BulbOutlined /> : mode === 'dark' ? <MoonOutlined /> : <DesktopOutlined />;

  const changeView = useCallback((view: ProjectListView) => {
    const searchParams = new URLSearchParams(location.search);
    if (view === 'projects') {
      searchParams.delete('view');
    } else {
      searchParams.set('view', view);
    }

    const search = searchParams.toString();
    navigate(
      {
        pathname: location.pathname,
        search: search ? `?${search}` : '',
      },
      { replace: false }
    );
  }, [location.pathname, location.search, navigate]);

  const scrollContainerRef = useRef<HTMLDivElement>(null);
  const mountedRef = useRef(true);
  const importRequestIdRef = useRef(0);
  const enterProjectRequestIdRef = useRef(0);
  const exportRequestIdRef = useRef(0);

  useEffect(() => {
    mountedRef.current = true;
    return () => {
      mountedRef.current = false;
      importRequestIdRef.current += 1;
      enterProjectRequestIdRef.current += 1;
      exportRequestIdRef.current += 1;
    };
  }, []);

  const beginImportRequest = useCallback(() => {
    importRequestIdRef.current += 1;
    return importRequestIdRef.current;
  }, []);

  const invalidateImportRequest = useCallback(() => {
    importRequestIdRef.current += 1;
  }, []);

  const isImportRequestActive = useCallback((requestId: number) => {
    return mountedRef.current && importRequestIdRef.current === requestId;
  }, []);

  const beginEnterProjectRequest = useCallback(() => {
    enterProjectRequestIdRef.current += 1;
    return enterProjectRequestIdRef.current;
  }, []);

  const isEnterProjectRequestActive = useCallback((requestId: number) => {
    return mountedRef.current && enterProjectRequestIdRef.current === requestId;
  }, []);

  const beginExportRequest = useCallback(() => {
    exportRequestIdRef.current += 1;
    return exportRequestIdRef.current;
  }, []);

  const invalidateExportRequest = useCallback(() => {
    exportRequestIdRef.current += 1;
  }, []);

  const isExportRequestActive = useCallback((requestId: number) => {
    return mountedRef.current && exportRequestIdRef.current === requestId;
  }, []);

  // 处理切换到 MCP 视图的事件
  const handleSwitchToMcp = useCallback(() => {
    changeView('mcp');
  }, [changeView]);

  useEffect(() => {
    refreshProjects();
    
    // 监听切换到 MCP 视图的事件
    eventBus.on(EventNames.SWITCH_TO_MCP_VIEW, handleSwitchToMcp);
    
    return () => {
      eventBus.off(EventNames.SWITCH_TO_MCP_VIEW, handleSwitchToMcp);
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [handleSwitchToMcp]);

  useEffect(() => {
    const handleVisibilityChange = () => {
      if (!document.hidden) {
        refreshProjects();
      }
    };

    document.addEventListener('visibilitychange', handleVisibilityChange);
    return () => {
      document.removeEventListener('visibilitychange', handleVisibilityChange);
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  useEffect(() => {
    setStoredSidebarCollapsed(collapsed);
  }, [collapsed]);

  const handleDelete = (id: string) => {
    const isMobile = window.innerWidth <= 768;
    modal.confirm({
      title: '确认删除',
      content: '删除项目将同时删除所有相关数据，此操作不可恢复。确定要删除吗？',
      okText: '确定',
      cancelText: '取消',
      okType: 'danger',
      centered: true,
      ...(isMobile && {
        style: { top: 'auto' }
      }),
      onOk: async () => {
        try {
          await deleteProject(id);
          message.success('项目删除成功');
        } catch {
          message.error('删除项目失败');
        }
      },
    });
  };

  const handleEnterProject = async (project: Project) => {
    const localWizardIncomplete = isProjectWizardIncomplete(project);
    if (!localWizardIncomplete) {
      navigate(`/project/${project.id}`);
      return;
    }

    const requestId = beginEnterProjectRequest();

    try {
      const latestProject = await projectApi.getProject(project.id);
      if (!isEnterProjectRequestActive(requestId)) {
        return;
      }
      updateProjectInStore(project.id, latestProject);

      const latestWizardIncomplete = isProjectWizardIncomplete(latestProject);
      if (latestWizardIncomplete) {
        navigate(`/wizard?project_id=${project.id}`);
        return;
      }
    } catch (error) {
      console.error('检查项目向导状态失败:', error);
    }

    if (!isEnterProjectRequestActive(requestId)) {
      return;
    }
    navigate(`/project/${project.id}`);
  };

  const getStatusTag = (status: string) => {
    const statusConfig: Record<string, { color: string; text: string; icon: ReactNode }> = {
      inspiration: { color: 'cyan', text: '灵感', icon: <BulbOutlined /> },
      foundation: { color: 'blue', text: '基础设定', icon: <CalendarOutlined /> },
      world_building: { color: 'geekblue', text: '世界构建', icon: <BookOutlined /> },
      character_design: { color: 'gold', text: '角色设计', icon: <FileTextOutlined /> },
      outline: { color: 'processing', text: '大纲', icon: <FileSearchOutlined /> },
      writing: { color: 'green', text: '创作', icon: <EditOutlined /> },
      reviewing: { color: 'orange', text: '审校', icon: <FileSearchOutlined /> },
      polishing: { color: 'magenta', text: '润色', icon: <FileTextOutlined /> },
      completed: { color: 'purple', text: '已完结', icon: <TrophyOutlined /> },
      planning: { color: 'blue', text: '基础设定', icon: <CalendarOutlined /> },
      draft: { color: 'blue', text: '基础设定', icon: <CalendarOutlined /> },
      active: { color: 'green', text: '创作', icon: <EditOutlined /> },
      revising: { color: 'orange', text: '审校', icon: <FileSearchOutlined /> },
    };
    const config = statusConfig[status] || statusConfig.foundation;
    return (
      <Tag color={config.color} icon={config.icon} style={{ margin: 0, borderRadius: 4, flexShrink: 0 }}>
        {config.text}
      </Tag>
    );
  };

  // 项目阶段只来自服务端 workflow 状态；字数进度仅作为独立指标展示。
  const getDisplayStatus = (status: string): string => status;

  const getProgress = (current: number, target: number) => {
    if (!target) return 0;
    return Math.min(Math.round((current / target) * 100), 100);
  };

  const getProgressColor = (progress: number) => {
    if (progress >= 80) return token.colorSuccess;
    if (progress >= 50) return token.colorPrimary;
    if (progress >= 20) return token.colorWarning;
    return token.colorError;
  };

  const formatDate = (dateString: string) => {
    const date = new Date(dateString);
    const now = new Date();
    const diff = now.getTime() - date.getTime();
    const days = Math.floor(diff / (1000 * 60 * 60 * 24));

    if (days === 0) return '今天';
    if (days === 1) return '昨天';
    if (days < 7) return `${days}天前`;
    return date.toLocaleDateString('zh-CN');
  };

  const totalWords = projects.reduce((sum, p) => sum + (p.current_words || 0), 0);
  const activeProjects = projects.filter(p => p.status === 'writing').length;
  // 已完结项目数只读取服务端权威 workflow 阶段。
  const completedProjects = projects.filter(p => p.status === 'completed').length;

  const renderWorkspaceFallback = (
    view: ProjectListView,
    options: {
      eyebrow: string;
      title: string;
      message: string;
      tags: Array<{ label: string; color?: string }>;
    },
  ) => (
    <div
      style={{
        padding: view === 'projects'
          ? 0
          : (isMobile ? '16px 16px 28px' : '24px 24px 36px'),
      }}
    >
      <InlineDeferredPanel
        eyebrow={options.eyebrow}
        title={options.title}
        message={options.message}
        minHeight={view === 'projects' ? (isMobile ? 320 : 360) : 'calc(100vh - 220px)'}
        tags={options.tags}
      />
    </div>
  );

  const handleFileSelect = async (file: File) => {
    const requestId = beginImportRequest();
    setSelectedFile(file);
    setValidationResult(null);
    try {
      setValidating(true);
      const result = await projectApi.validateImportFile(file);
      if (!isImportRequestActive(requestId)) {
        return false;
      }
      setValidationResult(result);
      if (!result.valid) {
        message.error('文件验证失败');
      }
    } catch (error) {
      if (!isImportRequestActive(requestId)) {
        return false;
      }
      console.error('验证失败:', error);
      message.error('文件验证失败');
    } finally {
      if (isImportRequestActive(requestId)) {
        setValidating(false);
      }
    }
    return false;
  };

  const handleImport = async () => {
    if (!selectedFile || !validationResult?.valid) {
      message.warning('请选择有效的导入文件');
      return;
    }
    const requestId = beginImportRequest();
    try {
      setImporting(true);
      const result = await projectApi.importProject(selectedFile);
      if (!isImportRequestActive(requestId)) {
        return;
      }
      if (result.success) {
        message.success(`项目导入成功！${result.message}`);
        setImportModalVisible(false);
        setSelectedFile(null);
        setValidationResult(null);
        await refreshProjects();
        if (!isImportRequestActive(requestId)) {
          return;
        }
        if (result.project_id) {
          navigate(`/project/${result.project_id}`);
        }
      } else {
        message.error(result.message || '导入失败');
      }
    } catch (error) {
      if (!isImportRequestActive(requestId)) {
        return;
      }
      console.error('导入失败:', error);
      message.error('导入失败，请重试');
    } finally {
      if (isImportRequestActive(requestId)) {
        setImporting(false);
      }
    }
  };

  const handleCloseImportModal = () => {
    invalidateImportRequest();
    setImportModalVisible(false);
    setSelectedFile(null);
    setValidationResult(null);
    setValidating(false);
    setImporting(false);
  };

  const handleOpenExportModal = () => {
    invalidateExportRequest();
    setExportModalVisible(true);
    setSelectedProjectIds([]);
  };

  const exportableProjects = projects;

  const handleCloseExportModal = () => {
    invalidateExportRequest();
    setExportModalVisible(false);
    setSelectedProjectIds([]);
    setExporting(false);
  };

  const handleToggleProject = (projectId: string) => {
    setSelectedProjectIds(prev =>
      prev.includes(projectId)
        ? prev.filter(id => id !== projectId)
        : [...prev, projectId]
    );
  };

  const handleToggleAll = () => {
    if (selectedProjectIds.length === exportableProjects.length) {
      setSelectedProjectIds([]);
    } else {
      setSelectedProjectIds(exportableProjects.map(p => p.id));
    }
  };

  const handleExport = async () => {
    if (selectedProjectIds.length === 0) {
      message.warning('请至少选择一个项目');
      return;
    }
    const requestId = beginExportRequest();
    try {
      setExporting(true);
      if (selectedProjectIds.length === 1) {
        const projectId = selectedProjectIds[0];
        const project = projects.find(p => p.id === projectId);
        await projectApi.exportProjectData(projectId, {
          include_generation_history: exportOptions.includeGenerationHistory,
          include_writing_styles: exportOptions.includeWritingStyles,
          include_careers: exportOptions.includeCareers,
          include_memories: exportOptions.includeMemories,
          include_plot_analysis: exportOptions.includePlotAnalysis
        });
        if (!isExportRequestActive(requestId)) {
          return;
        }
        message.success(`项目 "${project?.title}" 导出成功`);
      } else {
        let successCount = 0;
        let failCount = 0;
        for (const projectId of selectedProjectIds) {
          if (!isExportRequestActive(requestId)) {
            return;
          }
          try {
            await projectApi.exportProjectData(projectId, {
              include_generation_history: exportOptions.includeGenerationHistory,
              include_writing_styles: exportOptions.includeWritingStyles,
              include_careers: exportOptions.includeCareers,
              include_memories: exportOptions.includeMemories,
              include_plot_analysis: exportOptions.includePlotAnalysis
            });
            if (!isExportRequestActive(requestId)) {
              return;
            }
            successCount++;
            await new Promise(resolve => setTimeout(resolve, 500));
            if (!isExportRequestActive(requestId)) {
              return;
            }
          } catch (error) {
            if (!isExportRequestActive(requestId)) {
              return;
            }
            console.error(`导出项目 ${projectId} 失败:`, error);
            failCount++;
          }
        }
        if (!isExportRequestActive(requestId)) {
          return;
        }
        if (failCount === 0) {
          message.success(`成功导出 ${successCount} 个项目`);
        } else {
          message.warning(`导出完成：成功 ${successCount} 个，失败 ${failCount} 个`);
        }
      }
      if (!isExportRequestActive(requestId)) {
        return;
      }
      handleCloseExportModal();
    } catch (error) {
      if (!isExportRequestActive(requestId)) {
        return;
      }
      console.error('导出失败:', error);
      message.error('导出失败，请重试');
    } finally {
      if (isExportRequestActive(requestId)) {
        setExporting(false);
      }
    }
  };

  const isMobile = window.innerWidth <= 768;
  const headerHeight = isMobile ? 56 : 70;
  const expandedSiderWidth = 220;
  const collapsedSiderWidth = 60;
  const desktopSiderWidth = collapsed ? collapsedSiderWidth : expandedSiderWidth;
  const editorialInk = '#f7f1e8';
  const editorialShellBackground = resolvedMode === 'dark'
    ? 'radial-gradient(circle at top, rgba(204, 120, 92, 0.16) 0%, transparent 26%), linear-gradient(180deg, #0f0e0d 0%, #151311 100%)'
    : 'radial-gradient(circle at top, rgba(204, 120, 92, 0.12) 0%, transparent 28%), linear-gradient(180deg, #f8f2e9 0%, #efe6da 100%)';
  const editorialSidebarBackground = 'linear-gradient(180deg, #151311 0%, #1d1916 100%)';
  const editorialHeaderBackground = 'linear-gradient(135deg, #171411 0%, color-mix(in srgb, #171411 64%, #cc785c 36%) 100%)';
  const editorialMenuFooterBackground = alphaColor('#ffffff', resolvedMode === 'dark' ? 0.04 : 0.06);
  const editorialMutedInk = alphaColor(editorialInk, 0.66);

  const currentViewTitle = activeView === 'projects'
    ? '我的书架'
    : activeView === 'prompts'
      ? '提示词模板'
      : activeView === 'book-import'
        ? '拆书导入'
        : activeView === 'mcp'
          ? 'MCP 插件'
          : 'API 设置';

  const sideMenuItems = [
    {
      key: 'projects',
      icon: <BookOutlined />,
      label: '我的书架',
    },
    {
      type: 'group' as const,
      label: '创作工具',
      children: [
        {
          key: 'book-import',
          icon: <UploadOutlined />,
          label: '拆书导入',
        },
        {
          key: 'mcp',
          icon: <ApiOutlined />,
          label: 'MCP 插件',
        },
        {
          key: 'prompts',
          icon: <FileSearchOutlined />,
          label: '提示词管理',
        },
      ],
    },
    {
      type: 'group' as const,
      label: '系统设置',
      children: [
        {
          key: 'settings',
          icon: <SettingOutlined />,
          label: 'API 设置',
        },
      ],
    },
  ];

  const sideMenuItemsCollapsed = [
    {
      key: 'projects',
      icon: <BookOutlined />,
      label: '我的书架',
    },
    {
      key: 'book-import',
      icon: <UploadOutlined />,
      label: '拆书导入',
    },
    {
      key: 'mcp',
      icon: <ApiOutlined />,
      label: 'MCP 插件',
    },
    {
      key: 'prompts',
      icon: <FileSearchOutlined />,
      label: '提示词管理',
    },
    {
      key: 'settings',
      icon: <SettingOutlined />,
      label: 'API 设置',
    },
  ];

  return (
    <div style={{
      height: '100vh',
      display: 'flex',
      flexDirection: 'column',
      background: editorialShellBackground,
      overflow: 'hidden',
    }}>
      {contextHolder}

      {!isMobile && (
        <div
          style={{
          width: desktopSiderWidth,
          background: editorialSidebarBackground,
          borderRight: `1px solid ${alphaColor(editorialInk, 0.08)}`,
          display: 'flex',
          flexDirection: 'column',
          position: 'fixed',
          left: 0,
          top: 0,
          bottom: 0,
          height: '100vh',
          overflow: 'hidden',
          transition: 'width 0.3s cubic-bezier(0.4, 0, 0.2, 1)',
          boxShadow: `18px 0 36px ${alphaColor('#000000', 0.16)}`,
          zIndex: 1000
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
                      Editorial Workspace
                    </span>
                    <span style={{
                      color: editorialInk,
                      fontWeight: 600,
                      fontSize: 18,
                      fontFamily: designDisplayFont,
                      whiteSpace: 'nowrap',
                      overflow: 'hidden',
                      textOverflow: 'ellipsis',
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

          <div style={{ flex: 1, overflowY: 'auto', overflowX: 'hidden', padding: '12px 10px 10px' }}>
            <Menu
              theme="dark"
              mode="inline"
              inlineCollapsed={collapsed}
              selectedKeys={[activeView]}
              style={{
                borderRight: 0,
                width: '100%',
                background: 'transparent',
                color: editorialInk,
                fontSize: 14,
              }}
              onClick={({ key }) => {
                changeView(key as ProjectListView);
              }}
              items={collapsed ? sideMenuItemsCollapsed : sideMenuItems}
            />
          </div>

          <div style={{
            padding: collapsed ? '12px 8px' : 16,
            borderTop: `1px solid ${alphaColor(editorialInk, 0.08)}`,
            flexShrink: 0,
            background: editorialMenuFooterBackground,
          }}>
            {collapsed ? (
              <Space direction="vertical" style={{ width: '100%', alignItems: 'center' }} size={10}>
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
                  }}
                />
                <UserMenu compact />
              </Space>
            ) : (
              <Space direction="vertical" style={{ width: '100%' }} size={12}>
                <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', fontSize: 12, color: editorialMutedInk }}>
                  <span>主题模式</span>
                  <span>{mode === 'system' ? `跟随系统 · ${resolvedMode === 'dark' ? '深色' : '浅色'}` : resolvedMode === 'dark' ? '深色' : '浅色'}</span>
                </div>
                <ThemeSwitch block />
                <UserMenu />
              </Space>
            )}
          </div>
        </div>
      )}

      <div style={{
        background: editorialHeaderBackground,
        padding: isMobile ? '0 12px' : '0 24px',
        display: 'flex',
        alignItems: 'center',
        justifyContent: 'space-between',
        position: 'fixed',
        top: 0,
        left: isMobile ? 0 : desktopSiderWidth,
        right: 0,
        zIndex: 1000,
        boxShadow: `0 18px 32px ${alphaColor('#000000', 0.12)}`,
        borderBottom: `1px solid ${alphaColor(editorialInk, 0.08)}`,
        height: headerHeight,
        flexShrink: 0,
        transition: 'left 0.3s cubic-bezier(0.4, 0, 0.2, 1)',
        overflow: 'hidden'
      }}>
        {isMobile ? (
          <>
            <div style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
              <Button
                type="text"
                icon={<MenuUnfoldOutlined />}
                onClick={() => setDrawerVisible(true)}
                style={{
                  fontSize: 18,
                  color: editorialInk,
                  width: 36,
                  height: 36
                }}
              />
            </div>

            <h2 style={{
              margin: 0,
              color: editorialInk,
              fontSize: 16,
              fontWeight: 600,
              fontFamily: designDisplayFont,
              letterSpacing: '-0.02em',
              flex: 1,
              textAlign: 'center',
              whiteSpace: 'nowrap',
              overflow: 'hidden',
              textOverflow: 'ellipsis',
              paddingRight: 36
            }}>
              {currentViewTitle}
            </h2>

            <div style={{ width: 36, height: 36 }} />
          </>
        ) : (
          <>
            <div style={{ width: 160, zIndex: 1, display: 'flex', flexDirection: 'column', gap: 2 }}>
              <span style={{ color: editorialMutedInk, fontSize: 11, letterSpacing: '0.18em', textTransform: 'uppercase' }}>
                Workspace
              </span>
              <span style={{ color: editorialInk, fontSize: 13 }}>
                Long-form writing hub
              </span>
            </div>

            <div style={{
              position: 'absolute',
              left: '50%',
              transform: 'translateX(-50%)',
              maxWidth: '45%',
              textAlign: 'center',
            }}>
              <div style={{ color: editorialMutedInk, fontSize: 11, letterSpacing: '0.2em', textTransform: 'uppercase', marginBottom: 6 }}>
                {activeView === 'projects' ? 'Library Overview' : 'Creative Toolkit'}
              </div>
              <h2 style={{
                margin: 0,
                color: editorialInk,
                fontSize: '30px',
                fontWeight: 600,
                fontFamily: designDisplayFont,
                letterSpacing: '-0.03em',
                whiteSpace: 'nowrap',
                overflow: 'hidden',
                textOverflow: 'ellipsis',
              }}>
                {currentViewTitle}
              </h2>
            </div>

            <div style={{ display: 'flex', alignItems: 'center', gap: 16, zIndex: 1 }}>
              {activeView === 'projects' && (
                <div style={{ display: 'flex', alignItems: 'center', gap: 24 }}>
                  {projects.length > 0 && (
                    <div style={{ display: 'flex', gap: '16px' }}>
                      {[
                        { label: '创作中', value: activeProjects, unit: '本' },
                        { label: '已完结', value: completedProjects, unit: '本' },
                        { label: '总字数', value: totalWords, unit: '字' },
                      ].map((item, index) => (
                        <div
                          key={index}
                          style={{
                            display: 'flex',
                            flexDirection: 'column',
                            alignItems: 'center',
                            justifyContent: 'center',
                            background: alphaColor('#ffffff', 0.08),
                            border: `1px solid ${alphaColor(editorialInk, 0.1)}`,
                            backdropFilter: 'blur(10px)',
                            borderRadius: '22px',
                            minWidth: '64px',
                            height: '58px',
                            padding: '0 14px',
                            boxShadow: `0 16px 30px ${alphaColor('#000000', 0.12)}`,
                            cursor: 'default',
                            transition: 'transform 0.3s ease, box-shadow 0.3s ease',
                          }}
                          onMouseEnter={(e) => {
                            e.currentTarget.style.transform = 'translateY(-3px) scale(1.02)';
                            e.currentTarget.style.boxShadow = `0 20px 36px ${alphaColor('#000000', 0.18)}`;
                            e.currentTarget.style.border = `1px solid ${alphaColor(editorialInk, 0.18)}`;
                          }}
                          onMouseLeave={(e) => {
                            e.currentTarget.style.transform = 'translateY(0) scale(1)';
                            e.currentTarget.style.boxShadow = `0 16px 30px ${alphaColor('#000000', 0.12)}`;
                          }}
                        >
                          <span style={{ fontSize: '11px', color: editorialMutedInk, marginBottom: '4px', lineHeight: 1, letterSpacing: '0.12em', textTransform: 'uppercase' }}>
                            {item.label}
                          </span>
                          <span style={{ fontSize: '16px', fontWeight: '600', color: editorialInk, lineHeight: 1, fontFamily: token.fontFamilyCode }}>
                            {item.label === '总字数' ? formatWordCount(item.value) : item.value}
                            {item.unit && <span style={{ fontSize: '10px', marginLeft: '2px', opacity: 0.8 }}>{item.unit}</span>}
                          </span>
                        </div>
                      ))}
                    </div>
                  )}
                </div>
              )}
            </div>
          </>
        )}
      </div>

      {isMobile && (
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
          <div style={{ flex: 1, overflowY: 'auto' }}>
            <Menu
              theme="dark"
              mode="inline"
              selectedKeys={[activeView]}
              style={{ borderRight: 0, padding: '8px 10px 0', background: 'transparent' }}
              onClick={({ key }) => {
                changeView(key as ProjectListView);
                setDrawerVisible(false);
              }}
              items={sideMenuItems}
            />

          </div>

          <div style={{ padding: 16, borderTop: `1px solid ${alphaColor(editorialInk, 0.08)}`, background: editorialMenuFooterBackground }}>
            <Space direction="vertical" style={{ width: '100%' }} size={12}>
              <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', fontSize: 12, color: editorialMutedInk }}>
                <span>主题模式</span>
                <span>{mode === 'system' ? `跟随系统 · ${resolvedMode === 'dark' ? '深色' : '浅色'}` : resolvedMode === 'dark' ? '深色' : '浅色'}</span>
              </div>
              <ThemeSwitch block />
              <UserMenu showFullInfo />
            </Space>
          </div>
        </Drawer>
      )}

      <div style={{
        flex: 1,
        display: 'flex',
        flexDirection: 'column',
        height: '100%',
        overflow: 'hidden',
        marginLeft: isMobile ? 0 : desktopSiderWidth,
        marginTop: headerHeight,
        transition: 'margin-left 0.3s cubic-bezier(0.4, 0, 0.2, 1)'
      }}>

        {/* 内容显示区 */}
        <div
          ref={scrollContainerRef}
          style={{
            flex: 1,
            overflowY: 'auto',
            padding: activeView === 'projects'
              ? (isMobile ? '20px 16px 70px' : '28px 28px 78px')
              : 0,
            background: activeView === 'projects'
              ? editorialShellBackground
              : token.colorBgLayout,
          }}
        >
          {activeView === 'settings' ? (
            <Suspense
              fallback={renderWorkspaceFallback('settings', {
                eyebrow: 'Workspace Settings',
                title: '正在展开设置工作区',
                message: '系统正在恢复模型提供商、研究参数与保存入口，原有设置读取、测试和提交流程保持不变。',
                tags: [
                  { label: '设置中心', color: 'processing' },
                  { label: '模型与研究配置', color: 'gold' },
                  { label: '保存逻辑保持原样', color: 'green' },
                ],
              })}
            >
              <LazySettingsPage />
            </Suspense>
          ) : null}
          {activeView === 'mcp' ? (
            <Suspense
              fallback={renderWorkspaceFallback('mcp', {
                eyebrow: 'MCP Plugins',
                title: '正在展开 MCP 插件工作区',
                message: '系统正在恢复插件列表、连接说明与管理入口，原有插件配置和安装逻辑保持不变。',
                tags: [
                  { label: 'MCP 插件中心', color: 'cyan' },
                  { label: '插件工作区恢复中', color: 'processing' },
                  { label: '管理逻辑保持原样', color: 'green' },
                ],
              })}
            >
              <LazyMCPPluginsPage />
            </Suspense>
          ) : null}
          {activeView === 'prompts' ? (
            <Suspense
              fallback={renderWorkspaceFallback('prompts', {
                eyebrow: 'Prompt Workshop',
                title: '正在展开提示词工坊',
                message: '系统正在恢复模板列表、编辑入口与发布视图，原有提示词配置与提交逻辑保持不变。',
                tags: [
                  { label: '提示词工坊', color: 'purple' },
                  { label: '模板工作区恢复中', color: 'processing' },
                  { label: '发布逻辑保持原样', color: 'green' },
                ],
              })}
            >
              <LazyPromptTemplates />
            </Suspense>
          ) : null}
          
          {activeView === 'book-import' ? (
            <Suspense
              fallback={renderWorkspaceFallback('book-import', {
                eyebrow: 'Book Import',
                title: '正在展开拆书导入工作台',
                message: '系统正在恢复上传、任务进度与预览入口，原有导入步骤和任务状态流保持不变。',
                tags: [
                  { label: '拆书导入', color: 'volcano' },
                  { label: '导入工作台恢复中', color: 'processing' },
                  { label: '任务状态流保持原样', color: 'green' },
                ],
              })}
            >
              <LazyBookImport />
            </Suspense>
          ) : null}

          {activeView === 'projects' ? (
            <Suspense
              fallback={renderWorkspaceFallback('projects', {
                eyebrow: 'Project Library',
                title: '正在整理项目书架与创作入口',
                message: '系统正在恢复项目书架、快捷操作与导航入口，原有项目列表数据和进入项目逻辑保持不变。',
                tags: [
                  { label: '项目书架', color: 'blue' },
                  { label: '创作入口恢复中', color: 'processing' },
                  { label: '导航逻辑保持原样', color: 'green' },
                ],
              })}
            >
              <>
                <LazyBookshelfPage
                  isMobile={isMobile}
                  loading={loading}
                  projects={projects}
                  showApiTip={showApiTip}
                  setShowApiTip={setShowApiTip}
                  exportableProjectsCount={exportableProjects.length}
                  onOpenImportModal={() => setImportModalVisible(true)}
                  onOpenExportModal={handleOpenExportModal}
                  onGoSettings={() => changeView('settings')}
                  onStartWizard={() => navigate('/wizard')}
                  onOpenInspiration={() => navigate('/inspiration')}
                  onEnterProject={handleEnterProject}
                  onDeleteProject={handleDelete}
                  formatWordCount={formatWordCount}
                  getProgress={getProgress}
                  getProgressColor={getProgressColor}
                  getDisplayStatus={getDisplayStatus}
                  getStatusTag={getStatusTag}
                  formatDate={formatDate}
                />
              </>
            </Suspense>
          ) : null}
        
          <Suspense
            fallback={(
              <WorkflowEntryFallback
                eyebrow="Release Notes"
                title="正在接入更新日志入口"
                message="系统正在恢复更新日志浮动入口与说明面板触发器，原有打开逻辑和页面工作区保持不变。"
                tags={[
                  { label: '更新日志入口', color: 'blue' },
                  { label: '入口恢复中', color: 'processing' },
                  { label: '交互逻辑保持原样', color: 'green' },
                ]}
              />
            )}
          >
            <LazyChangelogFloatingButton />
          </Suspense>
        </div>
      </div>

      {importModalVisible ? (
        <Suspense
          fallback={(
            <WorkflowEntryFallback
              eyebrow="Project Import"
              title="正在整理项目导入工作台"
              message="系统正在恢复文件校验、导入配置与确认入口，原有导入链路和校验状态保持不变。"
              tags={[
                { label: '项目导入', color: 'blue' },
                { label: '文件校验恢复中', color: 'processing' },
                { label: '导入逻辑保持原样', color: 'green' },
              ]}
            />
          )}
        >
          <LazyProjectImportModal
            open={importModalVisible}
            isMobile={isMobile}
            importing={importing}
            validating={validating}
            selectedFile={selectedFile}
            validationResult={validationResult}
            token={token}
            onOk={handleImport}
            onCancel={handleCloseImportModal}
            onFileSelect={handleFileSelect}
            onRemoveFile={() => {
              invalidateImportRequest();
              setSelectedFile(null);
              setValidationResult(null);
              setValidating(false);
            }}
          />
        </Suspense>
      ) : null}

      {exportModalVisible ? (
        <Suspense
          fallback={(
            <WorkflowEntryFallback
              eyebrow="Project Export"
              title="正在整理项目导出工作台"
              message="系统正在恢复项目选择、导出选项与确认入口，原有导出逻辑和选择状态保持不变。"
              tags={[
                { label: '项目导出', color: 'purple' },
                { label: '导出面板恢复中', color: 'processing' },
                { label: '选择逻辑保持原样', color: 'green' },
              ]}
            />
          )}
        >
          <LazyProjectExportModal
            open={exportModalVisible}
            isMobile={isMobile}
            exporting={exporting}
            exportableProjects={exportableProjects}
            selectedProjectIds={selectedProjectIds}
            exportOptions={exportOptions}
            setExportOptions={setExportOptions}
            token={token}
            formatWordCount={formatWordCount}
            renderProjectStatus={(project) =>
              getStatusTag(getDisplayStatus(project.status))
            }
            onOk={handleExport}
            onCancel={handleCloseExportModal}
            onToggleAll={handleToggleAll}
            onToggleProject={handleToggleProject}
          />
        </Suspense>
      ) : null}

    </div>
  );
}
