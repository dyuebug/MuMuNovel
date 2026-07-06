import { Suspense, lazy, useCallback, useEffect, useRef, useState } from 'react';
import { Alert, Button, Card, Col, Divider, Form, Grid, Input, Layout, Row, Space, Typography, message, theme } from 'antd';
import { BookOutlined, LockOutlined, RobotOutlined, SafetyCertificateOutlined, TeamOutlined, ThunderboltOutlined, UserOutlined } from '@ant-design/icons';
import { useNavigate, useSearchParams } from 'react-router-dom';
import { authApi } from '../services/modularApi';
import InlineDeferredPanel from '../components/InlineDeferredPanel';
import WorkflowEntryFallback from '../components/WorkflowEntryFallback';
import ThemeSwitch from '../components/ThemeSwitch';
import { VERSION_INFO } from '../config/version';
import { getHttpStatus } from '../utils/httpError';
import { clearAuthStatusCache } from '../utils/authStatus';
import { getRedirectFromSearchParams, saveLoginRedirect } from '../utils/loginRedirect';
import { useThemeMode } from '../theme/useThemeMode';

const LazyAnnouncementModal = lazy(() => import('../components/AnnouncementModal'));

const { Title, Paragraph, Text } = Typography;
const { useBreakpoint } = Grid;

export default function Login() {
  const navigate = useNavigate();
  const [searchParams] = useSearchParams();
  const [loading, setLoading] = useState(false);
  const [checking, setChecking] = useState(true);
  const [localAuthEnabled, setLocalAuthEnabled] = useState(false);
  const [linuxdoEnabled, setLinuxdoEnabled] = useState(false);
  const [form] = Form.useForm();
  const { token } = theme.useToken();
  const { resolvedMode } = useThemeMode();
  const screens = useBreakpoint();
  const isDesktop = Boolean(screens.lg);
  const isDark = resolvedMode === 'dark';
  const alphaColor = (color: string, alpha: number) => `color-mix(in srgb, ${color} ${(alpha * 100).toFixed(0)}%, transparent)`;
  const [showAnnouncement, setShowAnnouncement] = useState(false);
  const [serviceUnavailableMessage, setServiceUnavailableMessage] = useState('');
  const [loginErrorMessage, setLoginErrorMessage] = useState('');
  const mountedRef = useRef(true);
  const requestIdRef = useRef(0);

  const serifFontFamily = '"Tiempos Headline", "Cormorant Garamond", "Times New Roman", serif';
  const sansFontFamily = 'Inter, -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif';

  const designColors = {
    coral: '#cc785c',
    coralActive: '#a9583e',
    cream: '#faf9f5',
    creamSoft: '#f5f0e8',
    creamCard: '#efe9de',
    creamStrong: '#e8e0d2',
    hairline: '#e6dfd8',
    ink: '#141413',
    body: '#3d3d3a',
    muted: '#6c6a64',
    mutedSoft: '#8e8b82',
    dark: '#181715',
    darkElevated: '#252320',
    darkSoft: '#1f1e1b',
    onDark: '#faf9f5',
    onDarkSoft: '#a09d96',
    teal: '#5db8a6',
    amber: '#e8a55a',
  };

  const pageBackground = isDark
    ? `
      radial-gradient(circle at 18% 18%, ${alphaColor(designColors.coral, 0.16)} 0%, transparent 34%),
      radial-gradient(circle at 82% 12%, ${alphaColor(designColors.teal, 0.1)} 0%, transparent 26%),
      linear-gradient(180deg, #100f0e 0%, #141311 45%, #0f0e0d 100%)
    `
    : `
      radial-gradient(circle at 18% 18%, ${alphaColor(designColors.coral, 0.14)} 0%, transparent 34%),
      radial-gradient(circle at 82% 12%, ${alphaColor(designColors.teal, 0.08)} 0%, transparent 26%),
      linear-gradient(180deg, #f7f1e8 0%, ${designColors.cream} 48%, #f3ece2 100%)
    `;

  const themeSwitchShellStyle = {
    position: 'fixed' as const,
    top: isDesktop ? 24 : 16,
    right: isDesktop ? 24 : 16,
    zIndex: 20,
    padding: isDesktop ? '10px 12px' : '8px 10px',
    borderRadius: 999,
    background: isDark
      ? alphaColor(designColors.darkElevated, 0.94)
      : alphaColor(designColors.cream, 0.88),
    border: `1px solid ${isDark ? alphaColor(designColors.onDark, 0.1) : designColors.hairline}`,
    boxShadow: isDark
      ? `0 18px 40px ${alphaColor('#000000', 0.32)}`
      : `0 12px 32px ${alphaColor(designColors.ink, 0.08)}`,
    backdropFilter: 'blur(12px)',
  };

  const heroSurfaceStyle = {
    height: '100%',
    padding: isDesktop ? '48px 56px 56px' : '0',
    display: 'flex',
    flexDirection: 'column' as const,
    justifyContent: 'space-between',
    position: 'relative' as const,
    overflow: 'hidden',
    borderRight: isDark ? `1px solid ${alphaColor(designColors.onDark, 0.06)}` : `1px solid ${alphaColor(designColors.ink, 0.04)}`,
    background: isDark
      ? `
        linear-gradient(180deg, ${alphaColor(designColors.dark, 0.94)} 0%, ${alphaColor(designColors.darkSoft, 0.96)} 100%)
      `
      : `
        linear-gradient(180deg, ${alphaColor(designColors.cream, 0.95)} 0%, ${alphaColor(designColors.creamSoft, 0.94)} 100%)
      `,
  };

  const heroGridOverlayStyle = {
    position: 'absolute' as const,
    inset: 0,
    backgroundImage: `
      linear-gradient(${isDark ? alphaColor(designColors.onDark, 0.04) : alphaColor(designColors.ink, 0.05)} 1px, transparent 1px),
      linear-gradient(90deg, ${isDark ? alphaColor(designColors.onDark, 0.04) : alphaColor(designColors.ink, 0.05)} 1px, transparent 1px)
    `,
    backgroundSize: '72px 72px',
    maskImage: 'linear-gradient(180deg, rgba(0,0,0,0.94), rgba(0,0,0,0.22))',
    pointerEvents: 'none' as const,
  };

  const heroGlowStyle = {
    position: 'absolute' as const,
    inset: 0,
    background: isDark
      ? `
        radial-gradient(circle at 22% 18%, ${alphaColor(designColors.coral, 0.22)} 0%, transparent 36%),
        radial-gradient(circle at 78% 76%, ${alphaColor(designColors.teal, 0.14)} 0%, transparent 28%)
      `
      : `
        radial-gradient(circle at 22% 18%, ${alphaColor(designColors.coral, 0.18)} 0%, transparent 36%),
        radial-gradient(circle at 78% 76%, ${alphaColor(designColors.teal, 0.1)} 0%, transparent 28%)
      `,
    pointerEvents: 'none' as const,
  };

  const logoShellStyle = {
    width: 52,
    height: 52,
    borderRadius: 18,
    display: 'flex',
    alignItems: 'center',
    justifyContent: 'center',
    background: isDark
      ? `linear-gradient(180deg, ${designColors.darkElevated} 0%, ${designColors.dark} 100%)`
      : `linear-gradient(180deg, ${designColors.dark} 0%, ${alphaColor(designColors.dark, 0.92)} 100%)`,
    border: `1px solid ${isDark ? alphaColor(designColors.onDark, 0.08) : alphaColor(designColors.ink, 0.08)}`,
    boxShadow: `0 16px 36px ${alphaColor(isDark ? '#000000' : designColors.ink, isDark ? 0.34 : 0.12)}`,
  };

  const badgeStyle = (tone: 'coral' | 'neutral' = 'neutral') => ({
    display: 'inline-flex',
    alignItems: 'center',
    gap: 8,
    padding: '7px 12px',
    borderRadius: 999,
    fontSize: 12,
    letterSpacing: '0.14em',
    textTransform: 'uppercase' as const,
    fontWeight: 600,
    fontFamily: sansFontFamily,
    color: tone === 'coral'
      ? designColors.onDark
      : (isDark ? designColors.onDarkSoft : designColors.muted),
    background: tone === 'coral'
      ? designColors.coral
      : (isDark ? alphaColor(designColors.onDark, 0.06) : alphaColor(designColors.creamStrong, 0.86)),
    border: `1px solid ${tone === 'coral'
      ? alphaColor(designColors.coralActive, 0.9)
      : (isDark ? alphaColor(designColors.onDark, 0.08) : alphaColor(designColors.ink, 0.06))}`,
  });

  const featureCardStyle = {
    height: '100%',
    minHeight: isDesktop ? 154 : 136,
    borderRadius: 20,
    border: `1px solid ${isDark ? alphaColor(designColors.onDark, 0.08) : alphaColor(designColors.ink, 0.06)}`,
    background: isDark
      ? `linear-gradient(180deg, ${alphaColor(designColors.darkElevated, 0.94)} 0%, ${alphaColor(designColors.darkSoft, 0.96)} 100%)`
      : `linear-gradient(180deg, ${alphaColor(designColors.creamCard, 0.98)} 0%, ${alphaColor(designColors.creamStrong, 0.94)} 100%)`,
    boxShadow: isDark
      ? `0 18px 40px ${alphaColor('#000000', 0.28)}`
      : `0 18px 42px ${alphaColor(designColors.ink, 0.08)}`,
  };

  const productCardStyle = {
    borderRadius: 28,
    border: `1px solid ${alphaColor(designColors.onDark, 0.08)}`,
    background: `
      radial-gradient(circle at top right, ${alphaColor(designColors.coral, 0.16)} 0%, transparent 32%),
      linear-gradient(180deg, ${designColors.dark} 0%, ${designColors.darkSoft} 100%)
    `,
    boxShadow: `0 26px 56px ${alphaColor('#000000', 0.32)}`,
    overflow: 'hidden' as const,
  };

  const productLineStyle = {
    padding: '14px 0',
    borderBottom: `1px solid ${alphaColor(designColors.onDark, 0.08)}`,
  };

  const platformPillStyle = {
    display: 'inline-flex',
    alignItems: 'center',
    padding: '6px 12px',
    borderRadius: 999,
    background: isDark
      ? alphaColor(designColors.onDark, 0.06)
      : alphaColor(designColors.cream, 0.84),
    border: `1px solid ${isDark ? alphaColor(designColors.onDark, 0.08) : alphaColor(designColors.ink, 0.06)}`,
    color: isDark ? designColors.onDark : designColors.ink,
    fontSize: 12,
    letterSpacing: '0.04em',
  };

  const rightStageStyle = {
    minHeight: '100vh',
    display: 'flex',
    alignItems: 'center',
    justifyContent: 'center',
    padding: isDesktop ? '48px min(7vw, 76px)' : '96px 20px 32px',
    position: 'relative' as const,
  };

  const loginShellStyle = {
    width: '100%',
    maxWidth: 560,
    borderRadius: isDesktop ? 30 : 24,
    border: `1px solid ${alphaColor(designColors.onDark, 0.08)}`,
    background: `
      radial-gradient(circle at top right, ${alphaColor(designColors.coral, 0.14)} 0%, transparent 34%),
      linear-gradient(180deg, ${designColors.dark} 0%, ${designColors.darkSoft} 100%)
    `,
    boxShadow: `0 32px 72px ${alphaColor('#000000', 0.28)}`,
    overflow: 'hidden' as const,
  };

  const formSurfaceStyle = {
    borderRadius: 24,
    padding: isDesktop ? 28 : 22,
    background: isDark
      ? `linear-gradient(180deg, ${alphaColor(designColors.darkElevated, 0.92)} 0%, ${alphaColor(designColors.darkSoft, 0.98)} 100%)`
      : `linear-gradient(180deg, ${alphaColor(designColors.cream, 0.98)} 0%, ${alphaColor(designColors.creamSoft, 0.98)} 100%)`,
    border: `1px solid ${isDark ? alphaColor(designColors.onDark, 0.08) : designColors.hairline}`,
    boxShadow: isDark
      ? `inset 0 1px 0 ${alphaColor(designColors.onDark, 0.04)}`
      : `0 18px 42px ${alphaColor(designColors.ink, 0.06)}`,
  };

  const fieldStyle = {
    height: 50,
    borderRadius: 14,
    borderColor: isDark ? alphaColor(designColors.onDark, 0.12) : designColors.hairline,
    background: isDark ? alphaColor(designColors.dark, 0.8) : alphaColor('#ffffff', 0.8),
    color: isDark ? designColors.onDark : designColors.ink,
    boxShadow: 'none',
  };

  const primaryButtonStyle = {
    height: 50,
    fontSize: 15,
    fontWeight: 600,
    fontFamily: sansFontFamily,
    background: designColors.coral,
    color: '#ffffff',
    border: 'none',
    borderRadius: 14,
    boxShadow: `0 18px 32px ${alphaColor(designColors.coral, 0.34)}`,
  };

  const secondaryButtonStyle = {
    height: 50,
    fontSize: 15,
    fontWeight: 600,
    fontFamily: sansFontFamily,
    background: isDark ? designColors.darkElevated : alphaColor('#ffffff', 0.84),
    color: isDark ? designColors.onDark : designColors.ink,
    border: `1px solid ${isDark ? alphaColor(designColors.onDark, 0.1) : designColors.hairline}`,
    borderRadius: 14,
    boxShadow: isDark
      ? `0 14px 30px ${alphaColor('#000000', 0.2)}`
      : `0 12px 26px ${alphaColor(designColors.ink, 0.06)}`,
  };

  const infoCardStyle = {
    marginTop: 18,
    borderRadius: 18,
    padding: '18px 18px 4px',
    background: isDark
      ? alphaColor(designColors.onDark, 0.04)
      : alphaColor(designColors.creamCard, 0.76),
    border: `1px solid ${isDark ? alphaColor(designColors.onDark, 0.08) : designColors.hairline}`,
  };

  const mobileHeroStyle = {
    marginBottom: 18,
    borderRadius: 24,
    border: `1px solid ${alphaColor(designColors.onDark, 0.08)}`,
    background: `
      radial-gradient(circle at top right, ${alphaColor(designColors.coral, 0.18)} 0%, transparent 38%),
      linear-gradient(180deg, ${designColors.dark} 0%, ${designColors.darkSoft} 100%)
    `,
    boxShadow: `0 24px 56px ${alphaColor('#000000', 0.24)}`,
    overflow: 'hidden' as const,
  };

  useEffect(() => {
    mountedRef.current = true;
    return () => {
      mountedRef.current = false;
      requestIdRef.current += 1;
    };
  }, []);

  const beginRequest = useCallback(() => {
    requestIdRef.current += 1;
    return requestIdRef.current;
  }, []);

  const isRequestActive = useCallback((requestId: number) => {
    return mountedRef.current && requestIdRef.current === requestId;
  }, []);

  const resolveServiceUnavailableMessage = (error: unknown): string =>
    getHttpStatus(error) === 503
      ? '数据库服务暂时不可用，请先启动 PostgreSQL 或 Docker Desktop 后重试。'
      : '';

  const resolveLoginErrorMessage = (error: unknown): string => {
    if (error instanceof Error && error.message.trim()) {
      return error.message.trim();
    }

    return '登录失败，请检查账号和密码后重试。';
  };

  useEffect(() => {
    const checkAuth = async () => {
      const requestId = beginRequest();
      try {
        await authApi.getCurrentUser();
        if (!isRequestActive(requestId)) {
          return;
        }
        setServiceUnavailableMessage('');
        const redirect = getRedirectFromSearchParams(searchParams);
        navigate(redirect);
      } catch (error) {
        if (!isRequestActive(requestId)) {
          return;
        }
        const currentUserServiceMessage = resolveServiceUnavailableMessage(error);
        setServiceUnavailableMessage(currentUserServiceMessage);

        try {
          const config = await authApi.getAuthConfig();
          if (!isRequestActive(requestId)) {
            return;
          }
          setLocalAuthEnabled(config.local_auth_enabled);
          setLinuxdoEnabled(config.linuxdo_enabled);
        } catch (configError) {
          if (!isRequestActive(requestId)) {
            return;
          }
          console.error('获取认证配置失败:', configError);
          setLocalAuthEnabled(true);
          setLinuxdoEnabled(true);

          const configServiceMessage = resolveServiceUnavailableMessage(configError);
          if (configServiceMessage) {
            setServiceUnavailableMessage(configServiceMessage);
          }
        }

        if (isRequestActive(requestId)) {
          setChecking(false);
        }
      }
    };

    void checkAuth();
  }, [beginRequest, isRequestActive, navigate, searchParams]);

  const handleLocalLogin = async (values: { username: string; password: string }) => {
    const requestId = beginRequest();
    try {
      setServiceUnavailableMessage('');
      setLoginErrorMessage('');
      setLoading(true);
      const response = await authApi.localLogin(values.username, values.password);
      if (!isRequestActive(requestId)) {
        return;
      }

      if (response.success) {
        message.success('登录成功！');

        const hideForever = localStorage.getItem('announcement_hide_forever');
        const hideToday = localStorage.getItem('announcement_hide_today');
        const today = new Date().toDateString();

        if (hideForever === 'true' || hideToday === today) {
          const redirect = getRedirectFromSearchParams(searchParams);
          clearAuthStatusCache();
          navigate(redirect);
        } else {
          setShowAnnouncement(true);
        }
      }
    } catch (error) {
      if (!isRequestActive(requestId)) {
        return;
      }
      console.error('本地登录失败:', error);
      const serviceMessage = resolveServiceUnavailableMessage(error);
      setServiceUnavailableMessage(serviceMessage);
      setLoginErrorMessage(serviceMessage ? '' : resolveLoginErrorMessage(error));
    } finally {
      if (isRequestActive(requestId)) {
        setLoading(false);
      }
    }
  };

  const handleLinuxDOLogin = async () => {
    const requestId = beginRequest();
    try {
      setServiceUnavailableMessage('');
      setLoginErrorMessage('');
      setLoading(true);
      const response = await authApi.getLinuxDOAuthUrl();
      if (!isRequestActive(requestId)) {
        return;
      }

      const redirect = getRedirectFromSearchParams(searchParams, '');
      saveLoginRedirect(redirect);

      window.location.href = response.auth_url;
    } catch (error) {
      if (!isRequestActive(requestId)) {
        return;
      }
      console.error('获取授权地址失败:', error);
      setServiceUnavailableMessage(resolveServiceUnavailableMessage(error));
      message.error('获取授权地址失败，请稍后重试');
    } finally {
      if (isRequestActive(requestId)) {
        setLoading(false);
      }
    }
  };

  const handleAnnouncementClose = () => {
    setShowAnnouncement(false);
    const redirect = getRedirectFromSearchParams(searchParams);
    clearAuthStatusCache();
    navigate(redirect);
  };

  const handleDoNotShowToday = () => {
    const today = new Date().toDateString();
    localStorage.setItem('announcement_hide_today', today);
  };

  const handleNeverShow = () => {
    localStorage.setItem('announcement_hide_forever', 'true');
  };

  const loginTips = [
    '本地登录默认账号：admin / admin123',
    '首次 LinuxDO 登录会自动创建账号',
    '每位用户拥有独立的创作空间、模型配置与项目数据。',
  ];

  const featureItems = [
    {
      icon: <RobotOutlined />,
      title: '多模型协同',
      description: '在 OpenAI、Gemini 与 Claude 之间灵活切换，让不同创作环节各用所长。',
    },
    {
      icon: <ThunderboltOutlined />,
      title: '灵感到成稿',
      description: '从主题发想、角色建模到章节精修，所有关键节点都被串成连续工作流。',
    },
    {
      icon: <TeamOutlined />,
      title: '关系与设定',
      description: '角色、组织与世界观结构化沉淀，复杂设定也能持续演进而不失控。',
    },
    {
      icon: <BookOutlined />,
      title: '章节质检闭环',
      description: '生成、重写、润色与分析形成闭环，帮助长篇内容始终保持节奏和质量。',
    },
  ];

  const workflowHighlights = [
    {
      title: '创作起点',
      description: '灵感、题材与人物动机先行，快速搭出故事框架。',
    },
    {
      title: '工作流编排',
      description: '多模型协同处理大纲、角色卡、章节草稿与重写任务。',
    },
    {
      title: '一致性维护',
      description: '通过关系、组织和章节分析维持设定统一与情绪连贯。',
    },
  ];

  const platformPills = ['OpenAI', 'Gemini', 'Claude', 'LinuxDO OAuth', 'Docker Compose', 'PostgreSQL'];

  const renderLocalLogin = () => (
    <Form
      form={form}
      layout="vertical"
      onFinish={handleLocalLogin}
      onValuesChange={() => setLoginErrorMessage('')}
      size="large"
      style={{ marginTop: 18 }}
    >
      <Form.Item
        name="username"
        label={<span style={{ color: isDark ? designColors.onDarkSoft : designColors.muted, fontFamily: sansFontFamily }}>管理账号</span>}
        rules={[{ required: true, message: '请输入管理账号' }]}
      >
        <Input
          prefix={<UserOutlined style={{ color: isDark ? designColors.onDarkSoft : token.colorTextTertiary }} />}
          placeholder="请输入管理账号"
          autoComplete="username"
          style={fieldStyle}
        />
      </Form.Item>
      <Form.Item
        name="password"
        label={<span style={{ color: isDark ? designColors.onDarkSoft : designColors.muted, fontFamily: sansFontFamily }}>访问密钥</span>}
        rules={[{ required: true, message: '请输入访问密钥' }]}
      >
        <Input.Password
          prefix={<LockOutlined style={{ color: isDark ? designColors.onDarkSoft : token.colorTextTertiary }} />}
          placeholder="请输入访问密钥"
          autoComplete="current-password"
          style={fieldStyle}
        />
      </Form.Item>
      <Form.Item style={{ marginBottom: 0, marginTop: 6 }}>
        <Button
          htmlType="submit"
          loading={loading}
          block
          style={primaryButtonStyle}
        >
          登录创作工作台
        </Button>
      </Form.Item>
    </Form>
  );

  const renderLinuxDOLogin = () => (
    <Button
      size="large"
      icon={(
        <img
          src="/favicon.ico"
          alt="LinuxDO"
          style={{
            width: 18,
            height: 18,
            objectFit: 'contain',
          }}
        />
      )}
      loading={loading}
      onClick={handleLinuxDOLogin}
      block
      style={secondaryButtonStyle}
    >
      使用 LinuxDO OAuth 登录
    </Button>
  );

  if (checking) {
    return (
      <div
        style={{
          display: 'flex',
          justifyContent: 'center',
          alignItems: 'center',
          minHeight: '100vh',
          background: pageBackground,
          padding: '24px 16px',
        }}
      >
        <div style={{ width: 'min(560px, 100%)' }}>
          <InlineDeferredPanel
            eyebrow="Entry Check"
            title="确认登录入口与回跳目标"
            message="当前正在检查登录配置、恢复访问目标，并决定应该展示本地登录还是 LinuxDO OAuth 入口。原有鉴权与回跳逻辑保持不变。"
            minHeight={280}
            tags={[
              { label: '登录配置检测中', color: 'processing' },
              { label: '回跳目标恢复', color: 'blue' },
              { label: '入口状态校验', color: 'default' },
            ]}
          />
        </div>
      </div>
    );
  }

  const loginGuideSteps = [
    '先确认当前可用的登录方式，再决定是直接进入本地账号登录，还是走 LinuxDO OAuth。',
    '如果页面出现错误或服务不可用提示，先处理当前阻塞信息，再继续提交凭据或发起授权跳转。',
    '登录成功后会回到原本要访问的创作流程，所以更适合在这里先快速完成入口确认，不做额外停留。',
  ];
  const loginWorkspaceFocus = serviceUnavailableMessage
    ? {
        title: '等待登录服务恢复',
        note: '当前登录入口已经给出服务不可用提示，适合先确认服务状态或稍后重试，不要重复触发授权流程。',
      }
    : loginErrorMessage
      ? {
          title: '修正当前登录阻塞',
          note: '页面已经返回明确的登录失败信息，先处理账号、密码或授权状态，再继续下一次登录尝试。',
        }
      : loading
        ? {
            title: '等待登录校验完成',
            note: '当前正在提交登录或拉起授权地址，适合先等待结果回流，避免重复点击触发并发登录请求。',
          }
        : localAuthEnabled && linuxdoEnabled
          ? {
              title: '选择本轮登录入口',
              note: '本地账号与 LinuxDO OAuth 都可用，适合先判断你这次是要快速进入工作台，还是沿用社区账号授权。',
            }
          : linuxdoEnabled
            ? {
                title: '通过 LinuxDO OAuth 进入',
                note: '当前只开放社区授权登录，适合直接走 OAuth 流程，把账号创建和回跳交给现有链路完成。',
              }
            : localAuthEnabled
              ? {
                  title: '使用本地账号进入工作台',
                  note: '当前本地登录可用，适合先完成账号密码校验，再继续进入原本的创作项目或工作流页面。',
                }
              : {
                  title: '等待管理员启用登录方式',
                  note: '页面当前没有可用登录入口，适合先联系管理员确认系统配置，而不是继续反复刷新页面。',
                };

  return (
    <>
      {showAnnouncement ? (
        <Suspense
          fallback={(
            <WorkflowEntryFallback
              eyebrow="Workspace Notice"
              title="正在展开工作台公告"
              message="系统正在接入登录前公告内容与确认入口，原有关闭、今日不再提示与永久忽略逻辑保持不变。"
              tags={[
                { label: '登录前公告', color: 'blue' },
                { label: '说明层恢复中', color: 'processing' },
                { label: '行为逻辑保持原样', color: 'green' },
              ]}
            />
          )}
        >
          <LazyAnnouncementModal
            visible={showAnnouncement}
            onClose={handleAnnouncementClose}
            onDoNotShowToday={handleDoNotShowToday}
            onNeverShow={handleNeverShow}
          />
        </Suspense>
      ) : null}
      <Layout style={{ minHeight: '100vh', background: pageBackground, fontFamily: sansFontFamily }}>
        <div style={themeSwitchShellStyle}>
          <ThemeSwitch size="small" />
        </div>

        <Row style={{ minHeight: '100vh' }}>
          <Col xs={0} lg={11}>
            <section style={heroSurfaceStyle}>
              <div style={heroGridOverlayStyle} />
              <div style={heroGlowStyle} />

              <div
                style={{
                  position: 'relative',
                  zIndex: 1,
                  display: 'flex',
                  flexDirection: 'column',
                  gap: 28,
                  width: '100%',
                }}
              >
                <Space align="center" size={16}>
                  <div style={logoShellStyle}>
                    <img
                      src="/logo.svg"
                      alt={VERSION_INFO.projectName}
                      style={{ width: 26, height: 26, filter: 'brightness(0) invert(1)' }}
                    />
                  </div>
                  <div>
                    <Text style={{ display: 'block', color: isDark ? designColors.onDarkSoft : designColors.muted, letterSpacing: '0.12em', textTransform: 'uppercase', fontSize: 12 }}>
                      Creative Operating System
                    </Text>
                    <Title level={3} style={{ margin: 0, color: isDark ? designColors.onDark : designColors.ink, fontFamily: serifFontFamily, fontWeight: 400 }}>
                      {VERSION_INFO.projectName}
                    </Title>
                  </div>
                </Space>

                <Space direction="vertical" size={22} style={{ width: '100%', maxWidth: 760 }}>
                  <div style={badgeStyle('coral')}>
                    <SafetyCertificateOutlined />
                    Claude DESIGN.md Driven
                  </div>

                  <div>
                    <Title
                      level={1}
                      style={{
                        marginBottom: 18,
                        color: isDark ? designColors.onDark : designColors.ink,
                        fontFamily: serifFontFamily,
                        fontWeight: 400,
                        letterSpacing: '-0.04em',
                        lineHeight: 1.02,
                        fontSize: 'clamp(54px, 5vw, 86px)',
                      }}
                    >
                      为长篇创作
                      <br />
                      打造一间有节奏的
                      <br />
                      智能工作室
                    </Title>
                    <Paragraph
                      style={{
                        marginBottom: 0,
                        maxWidth: 720,
                        color: isDark ? designColors.onDarkSoft : designColors.body,
                        fontSize: 'clamp(17px, 1.3vw, 21px)',
                        lineHeight: 1.9,
                      }}
                    >
                      从灵感萌发、角色关系搭建，到章节生成、重写和润色，MuMuNovel 把 AI 协作、设定管理与长文本迭代整合进同一条创作流水线。
                    </Paragraph>
                  </div>

                  <Card variant="borderless" style={productCardStyle} styles={{ body: { padding: isDesktop ? 28 : 22 } }}>
                    <Space direction="vertical" size={18} style={{ width: '100%' }}>
                      <Space align="center" style={{ justifyContent: 'space-between', width: '100%' }}>
                        <div style={badgeStyle()}>
                          <RobotOutlined />
                          Product Surface
                        </div>
                        <Text style={{ color: designColors.onDarkSoft, fontSize: 12, letterSpacing: '0.08em', textTransform: 'uppercase' }}>
                          AI Writing Workflow
                        </Text>
                      </Space>

                      <div
                        style={{
                          borderRadius: 20,
                          background: alphaColor(designColors.onDark, 0.04),
                          border: `1px solid ${alphaColor(designColors.onDark, 0.08)}`,
                          padding: '10px 18px',
                        }}
                      >
                        {workflowHighlights.map((item, index) => (
                          <div
                            key={item.title}
                            style={{
                              ...productLineStyle,
                              borderBottom: index === workflowHighlights.length - 1 ? 'none' : productLineStyle.borderBottom,
                            }}
                          >
                            <Space align="start" size={14} style={{ width: '100%' }}>
                              <div
                                style={{
                                  width: 28,
                                  height: 28,
                                  flex: '0 0 auto',
                                  borderRadius: 999,
                                  display: 'flex',
                                  alignItems: 'center',
                                  justifyContent: 'center',
                                  background: alphaColor(designColors.coral, 0.18),
                                  color: designColors.onDark,
                                  fontSize: 12,
                                  fontWeight: 700,
                                }}
                              >
                                0{index + 1}
                              </div>
                              <div>
                                <Text style={{ display: 'block', color: designColors.onDark, fontSize: 15, fontWeight: 600 }}>
                                  {item.title}
                                </Text>
                                <Paragraph style={{ margin: '6px 0 0', color: designColors.onDarkSoft, fontSize: 13, lineHeight: 1.75 }}>
                                  {item.description}
                                </Paragraph>
                              </div>
                            </Space>
                          </div>
                        ))}
                      </div>

                      <Row gutter={[12, 12]}>
                        <Col span={12}>
                          <div
                            style={{
                              borderRadius: 18,
                              padding: '16px 18px',
                              background: alphaColor(designColors.onDark, 0.04),
                              border: `1px solid ${alphaColor(designColors.onDark, 0.08)}`,
                            }}
                          >
                            <Text style={{ display: 'block', color: designColors.onDarkSoft, fontSize: 12, letterSpacing: '0.08em', textTransform: 'uppercase' }}>
                              模型协作
                            </Text>
                            <Paragraph style={{ margin: '8px 0 0', color: designColors.onDark, fontSize: 15, lineHeight: 1.7 }}>
                              把不同模型分配给灵感、设定、正文与润色的最合适阶段。
                            </Paragraph>
                          </div>
                        </Col>
                        <Col span={12}>
                          <div
                            style={{
                              borderRadius: 18,
                              padding: '16px 18px',
                              background: alphaColor(designColors.onDark, 0.04),
                              border: `1px solid ${alphaColor(designColors.onDark, 0.08)}`,
                            }}
                          >
                            <Text style={{ display: 'block', color: designColors.onDarkSoft, fontSize: 12, letterSpacing: '0.08em', textTransform: 'uppercase' }}>
                              长篇一致性
                            </Text>
                            <Paragraph style={{ margin: '8px 0 0', color: designColors.onDark, fontSize: 15, lineHeight: 1.7 }}>
                              角色设定、组织结构和章节分析共同守住故事世界的统一感。
                            </Paragraph>
                          </div>
                        </Col>
                      </Row>
                    </Space>
                  </Card>

                  <Row gutter={[18, 18]} style={{ width: '100%' }}>
                    {featureItems.map((item) => (
                      <Col span={12} key={item.title}>
                        <Card size="small" variant="borderless" style={featureCardStyle} styles={{ body: { padding: 22 } }}>
                          <Space direction="vertical" size={12}>
                            <div style={badgeStyle()}>
                              {item.icon}
                              Capability
                            </div>
                            <Title
                              level={4}
                              style={{
                                margin: 0,
                                fontFamily: serifFontFamily,
                                fontWeight: 400,
                                fontSize: 24,
                                color: isDark ? designColors.onDark : designColors.ink,
                              }}
                            >
                              {item.title}
                            </Title>
                            <Paragraph
                              style={{
                                marginBottom: 0,
                                color: isDark ? designColors.onDarkSoft : designColors.body,
                                fontSize: 14,
                                lineHeight: 1.8,
                              }}
                            >
                              {item.description}
                            </Paragraph>
                          </Space>
                        </Card>
                      </Col>
                    ))}
                  </Row>
                </Space>

                <Space size={[10, 12]} wrap>
                  {platformPills.map((item) => (
                    <span key={item} style={platformPillStyle}>
                      {item}
                    </span>
                  ))}
                </Space>
              </div>

              <Paragraph
                style={{
                  marginBottom: 0,
                  fontSize: 12,
                  color: isDark ? designColors.onDarkSoft : designColors.mutedSoft,
                  position: 'relative',
                  zIndex: 1,
                  letterSpacing: '0.08em',
                  textTransform: 'uppercase',
                }}
              >
                © 2026 {VERSION_INFO.projectName} · GPLv3 License
              </Paragraph>
            </section>
          </Col>

          <Col xs={24} lg={13}>
            <section style={rightStageStyle}>
              <div
                style={{
                  position: 'absolute',
                  inset: 0,
                  background: isDark
                    ? `
                      radial-gradient(circle at 12% 18%, ${alphaColor(designColors.coral, 0.08)} 0%, transparent 24%),
                      radial-gradient(circle at 88% 74%, ${alphaColor(designColors.teal, 0.07)} 0%, transparent 22%)
                    `
                    : `
                      radial-gradient(circle at 12% 18%, ${alphaColor(designColors.coral, 0.08)} 0%, transparent 24%),
                      radial-gradient(circle at 88% 74%, ${alphaColor(designColors.teal, 0.06)} 0%, transparent 22%)
                    `,
                  pointerEvents: 'none',
                }}
              />

              <div style={{ width: '100%', maxWidth: 560, position: 'relative', zIndex: 1 }}>
                {!isDesktop ? (
                  <Card variant="borderless" style={mobileHeroStyle} styles={{ body: { padding: 22 } }}>
                    <Space direction="vertical" size={14}>
                      <div style={badgeStyle('coral')}>
                        <RobotOutlined />
                        AI Writing Workbench
                      </div>
                      <Title
                        level={2}
                        style={{
                          margin: 0,
                          color: designColors.onDark,
                          fontFamily: serifFontFamily,
                          fontWeight: 400,
                          lineHeight: 1.08,
                        }}
                      >
                        把你的创作流程
                        <br />
                        带回同一张工作台
                      </Title>
                      <Paragraph style={{ marginBottom: 0, color: designColors.onDarkSoft, lineHeight: 1.8 }}>
                        灵感、角色、世界观和章节迭代都在一个持续演进的 AI 创作环境中完成。
                      </Paragraph>
                    </Space>
                  </Card>
                ) : null}

                <Card
                  variant="borderless"
                  style={{
                    marginBottom: 18,
                    borderRadius: 24,
                    border: `1px solid ${alphaColor(designColors.onDark, 0.08)}`,
                    background: `
                      radial-gradient(circle at top right, ${alphaColor(designColors.coral, 0.16)} 0%, transparent 36%),
                      linear-gradient(180deg, ${alphaColor(designColors.darkElevated, 0.96)} 0%, ${alphaColor(designColors.darkSoft, 0.98)} 100%)
                    `,
                    boxShadow: `0 24px 56px ${alphaColor('#000000', 0.22)}`,
                    overflow: 'hidden',
                  }}
                  styles={{ body: { padding: isDesktop ? 22 : 18 } }}
                >
                  <Row gutter={[16, 16]}>
                    <Col xs={24} lg={15}>
                      <Space direction="vertical" size={8} style={{ width: '100%' }}>
                        <Text style={{ color: designColors.onDarkSoft, fontSize: 12, letterSpacing: '0.12em', textTransform: 'uppercase' }}>
                          Login Guide
                        </Text>
                        <Paragraph style={{ marginBottom: 0, color: designColors.onDark, lineHeight: 1.75 }}>
                          这个页面更像创作工作台的安全入口。原有的本地登录、LinuxDO OAuth、失败提示和回跳逻辑都保持不变，这里只把登录顺序和当前阻塞点提前说明。
                        </Paragraph>
                        <div style={{ display: 'flex', flexWrap: 'wrap', gap: 8 }}>
                          {loginGuideSteps.map((item, index) => (
                            <span
                              key={item}
                              style={{
                                display: 'inline-flex',
                                alignItems: 'center',
                                gap: 8,
                                padding: '6px 12px',
                                borderRadius: 999,
                                background: alphaColor(designColors.onDark, 0.06),
                                border: `1px solid ${alphaColor(designColors.onDark, 0.08)}`,
                                color: designColors.onDark,
                                fontSize: 12,
                              }}
                            >
                              <span style={{ color: designColors.coral, fontWeight: 700 }}>{index + 1}</span>
                              {item}
                            </span>
                          ))}
                        </div>
                      </Space>
                    </Col>
                    <Col xs={24} lg={9}>
                      <div
                        style={{
                          height: '100%',
                          borderRadius: 18,
                          padding: isDesktop ? '16px 18px 14px' : '14px 14px 12px',
                          background: alphaColor(designColors.onDark, 0.05),
                          border: `1px solid ${alphaColor(designColors.onDark, 0.08)}`,
                        }}
                      >
                        <Text style={{ display: 'block', color: designColors.onDarkSoft, fontSize: 12, letterSpacing: '0.12em', textTransform: 'uppercase' }}>
                          当前登录焦点
                        </Text>
                        <Title level={5} style={{ margin: '8px 0 6px', color: designColors.onDark, fontFamily: serifFontFamily, fontWeight: 400 }}>
                          {loginWorkspaceFocus.title}
                        </Title>
                        <Paragraph style={{ marginBottom: 0, color: designColors.onDarkSoft, lineHeight: 1.75 }}>
                          {loginWorkspaceFocus.note}
                        </Paragraph>
                      </div>
                    </Col>
                  </Row>
                </Card>

                <Card variant="borderless" style={loginShellStyle} styles={{ body: { padding: isDesktop ? 30 : 20 } }}>
                  <Space direction="vertical" size={18} style={{ width: '100%' }}>
                    <div>
                      <div style={badgeStyle()}>
                        <LockOutlined />
                        Secure Access
                      </div>
                      <Title
                        level={2}
                        style={{
                          margin: '18px 0 8px',
                          color: designColors.onDark,
                          fontFamily: serifFontFamily,
                          fontWeight: 400,
                          fontSize: isDesktop ? 40 : 34,
                          lineHeight: 1.06,
                          letterSpacing: '-0.03em',
                        }}
                      >
                        欢迎回来
                      </Title>
                      <Paragraph style={{ marginBottom: 0, color: designColors.onDarkSoft, fontSize: 15, lineHeight: 1.8 }}>
                        登录 {VERSION_INFO.projectName}，继续你的小说创作项目与多模型协作流程。
                      </Paragraph>
                    </div>

                    <div style={formSurfaceStyle}>
                      {serviceUnavailableMessage ? (
                        <div data-testid="login-service-unavailable-alert">
                          <Alert
                            type="warning"
                            showIcon
                            style={{ marginBottom: 16, borderRadius: 14 }}
                            message="服务暂时不可用"
                            description={serviceUnavailableMessage}
                          />
                        </div>
                      ) : null}

                      {loginErrorMessage ? (
                        <div data-testid="login-error-alert">
                          <Alert
                            type="error"
                            showIcon
                            style={{ marginBottom: 16, borderRadius: 14 }}
                            message="登录失败"
                            description={loginErrorMessage}
                          />
                        </div>
                      ) : null}

                      {localAuthEnabled ? renderLocalLogin() : null}

                      {linuxdoEnabled && localAuthEnabled ? (
                        <>
                          <Divider style={{ margin: '18px 0 18px', borderColor: isDark ? alphaColor(designColors.onDark, 0.1) : designColors.hairline, color: isDark ? designColors.onDarkSoft : designColors.muted }}>
                            或
                          </Divider>
                          {renderLinuxDOLogin()}
                        </>
                      ) : null}

                      {!localAuthEnabled && linuxdoEnabled ? (
                        <div style={{ marginTop: 6 }}>
                          {renderLinuxDOLogin()}
                        </div>
                      ) : null}

                      {!localAuthEnabled && !linuxdoEnabled ? (
                        <Alert
                          type="warning"
                          showIcon
                          style={{ borderRadius: 14 }}
                          message="当前未启用可用登录方式"
                          description="请联系管理员在系统配置中启用本地登录或 LinuxDO OAuth 登录。"
                        />
                      ) : null}

                      <div style={infoCardStyle}>
                        <Space align="center" size={10} style={{ marginBottom: 10 }}>
                          <div style={badgeStyle()}>
                            <SafetyCertificateOutlined />
                            登录说明
                          </div>
                        </Space>
                        <ul style={{ margin: 0, paddingLeft: 18, color: isDark ? designColors.onDarkSoft : designColors.body }}>
                          {loginTips.map((tip) => (
                            <li key={tip} style={{ marginBottom: 8, lineHeight: 1.75 }}>
                              {tip}
                            </li>
                          ))}
                        </ul>
                      </div>
                    </div>
                  </Space>
                </Card>
              </div>
            </section>
          </Col>
        </Row>
      </Layout>
    </>
  );
}
