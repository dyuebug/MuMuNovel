import { useEffect, useRef, useState } from 'react';
import type { ReactNode } from 'react';
import { Button, Card, Space, Tag, Typography, theme } from 'antd';
import { Navigate, useLocation } from 'react-router-dom';
import LoadingScreen from './LoadingScreen';
import { clearAuthStatusCache, resolveAuthStatus } from '../utils/authStatus';
import type { AuthResolution } from '../utils/authStatus';
import { buildLoginUrl, getLocationRedirect } from '../utils/loginRedirect';
import { designDisplayFont } from '../theme/themeConfig';

interface ProtectedRouteProps {
  children: ReactNode;
}

export default function ProtectedRoute({ children }: ProtectedRouteProps) {
  const [authState, setAuthState] = useState<AuthResolution | null>(null);
  const location = useLocation();
  const { token } = theme.useToken();
  const alphaColor = (color: string, alpha: number) => `color-mix(in srgb, ${color} ${(alpha * 100).toFixed(0)}%, transparent)`;
  const mountedRef = useRef(true);
  const requestIdRef = useRef(0);
  const { Paragraph, Text, Title } = Typography;
  const editorialInk = '#f7f1e8';
  const heroBackground = `linear-gradient(135deg, #171411 0%, color-mix(in srgb, #171411 58%, ${token.colorPrimary} 42%) 100%)`;
  const quietPanelBackground = `linear-gradient(180deg, color-mix(in srgb, ${token.colorBgContainer} 95%, ${token.colorFillAlter} 5%) 0%, color-mix(in srgb, ${token.colorBgContainer} 87%, ${token.colorFillAlter} 13%) 100%)`;
  const currentRouteLabel = location.pathname === '/' ? '首页工作区入口' : `目标页面 ${location.pathname}`;
  const serviceGuideSteps = [
    '先把这次拦截当成访问网关提示，不是账号失效；当前只是在等待认证依赖恢复。',
    '再根据右侧焦点卡确认优先排查 PostgreSQL 或 Docker Desktop，避免直接回到业务页面重复刷新。',
    '最后使用重新检测按钮恢复鉴权检查，底层服务可用后会自动继续放行到原本想访问的页面。',
  ];

  useEffect(() => {
    mountedRef.current = true;

    const checkAuth = async () => {
      requestIdRef.current += 1;
      const requestId = requestIdRef.current;
      const resolvedState = await resolveAuthStatus();
      if (mountedRef.current && requestIdRef.current === requestId) {
        setAuthState(resolvedState);
      }
    };

    void checkAuth();

    return () => {
      mountedRef.current = false;
      requestIdRef.current += 1;
    };
  }, []);

  if (authState === null) {
    return <LoadingScreen message="正在检查工作区访问权限..." minHeight="100vh" />;
  }

  if (authState.serviceUnavailable) {
    const serviceWorkspaceFocus = {
      title: `当前需要先恢复认证依赖，再继续进入${currentRouteLabel}`,
      note: '这里只升级阅读顺序与当前焦点提示，不改变鉴权检查、缓存清理、页面刷新或放行逻辑。底层依赖恢复后，重新检测会继续原有访问路径。',
    };

    return (
      <div
        style={{
          minHeight: '100vh',
          padding: '40px 24px',
          background: `radial-gradient(circle at top, ${alphaColor(token.colorPrimaryBg, 0.92)} 0%, ${alphaColor(token.colorBgLayout, 0.98)} 56%, ${alphaColor(token.colorBgContainer, 1)} 100%)`,
        }}
      >
        <div style={{ width: 'min(960px, 100%)', margin: '0 auto' }}>
          <Card
            bordered={false}
            style={{
              marginBottom: 18,
              borderRadius: 24,
              overflow: 'hidden',
              background: heroBackground,
            }}
            styles={{ body: { padding: 24 } }}
          >
            <Text style={{ color: alphaColor(token.colorWhite, 0.68), letterSpacing: '0.14em', textTransform: 'uppercase' }}>
              Access Gateway
            </Text>
            <Title level={2} style={{ margin: '10px 0 12px', color: editorialInk, fontFamily: designDisplayFont, letterSpacing: '-0.03em' }}>
              服务暂时不可用
            </Title>
            <Paragraph style={{ margin: 0, color: alphaColor(token.colorWhite, 0.82), lineHeight: 1.8, maxWidth: 640 }}>
              认证服务当前无法访问，请先确认 PostgreSQL 或 Docker Desktop 已启动，再回到这里重新检测。恢复后，工作区会继续按原有鉴权流程放行。
            </Paragraph>
          </Card>

          <Card
            bordered={false}
            style={{
              marginBottom: 18,
              borderRadius: 22,
              background: quietPanelBackground,
              border: `1px solid ${alphaColor(token.colorPrimary, 0.08)}`,
            }}
            styles={{ body: { padding: 20 } }}
          >
            <div
              style={{
                display: 'grid',
                gridTemplateColumns: 'repeat(auto-fit, minmax(260px, 1fr))',
                gap: 16,
              }}
            >
              <div>
                <Text style={{ fontSize: 12, letterSpacing: '0.12em', textTransform: 'uppercase', color: token.colorTextTertiary }}>
                  Route Guide
                </Text>
                <Title level={5} style={{ margin: '6px 0 8px', fontFamily: designDisplayFont }}>
                  服务恢复前的处理顺序
                </Title>
                <Paragraph style={{ margin: 0, color: token.colorTextSecondary, lineHeight: 1.75 }}>
                  这里现在只增强落地态的信息层级与阅读顺序，不改变受保护路由判断、登录跳转、缓存失效或页面刷新行为。
                </Paragraph>
                <div style={{ display: 'flex', flexWrap: 'wrap', gap: 8, marginTop: 12 }}>
                  {serviceGuideSteps.map((item, index) => (
                    <span
                      key={item}
                      style={{
                        display: 'inline-flex',
                        alignItems: 'center',
                        gap: 8,
                        padding: '6px 12px',
                        borderRadius: 999,
                        background: token.colorBgContainer,
                        border: `1px solid ${token.colorBorderSecondary}`,
                        color: token.colorTextSecondary,
                        fontSize: 12,
                      }}
                    >
                      <span style={{ color: token.colorPrimary, fontWeight: 700 }}>{index + 1}</span>
                      {item}
                    </span>
                  ))}
                </div>
              </div>
              <div
                style={{
                  borderRadius: 18,
                  padding: '16px 18px',
                  background: token.colorBgContainer,
                  border: `1px solid ${token.colorBorderSecondary}`,
                }}
              >
                <Text style={{ display: 'block', fontSize: 12, letterSpacing: '0.12em', textTransform: 'uppercase', color: token.colorTextTertiary }}>
                  当前工作焦点
                </Text>
                <Title level={5} style={{ margin: '8px 0 6px', fontFamily: designDisplayFont }}>
                  {serviceWorkspaceFocus.title}
                </Title>
                <Paragraph style={{ margin: 0, color: token.colorTextSecondary, lineHeight: 1.75 }}>
                  {serviceWorkspaceFocus.note}
                </Paragraph>
                <Space wrap size={[8, 8]} style={{ marginTop: 12 }}>
                  <Tag color="gold" style={{ margin: 0, borderRadius: 999, paddingInline: 10 }}>
                    依赖待恢复
                  </Tag>
                  <Tag color="processing" style={{ margin: 0, borderRadius: 999, paddingInline: 10 }}>
                    认证检查已暂停
                  </Tag>
                  <Tag color="blue" style={{ margin: 0, borderRadius: 999, paddingInline: 10 }}>
                    {currentRouteLabel}
                  </Tag>
                </Space>
              </div>
            </div>
          </Card>

          <Card
            bordered={false}
            style={{
              borderRadius: 22,
              background: token.colorBgContainer,
              border: `1px solid ${alphaColor(token.colorPrimary, 0.08)}`,
              boxShadow: `0 24px 52px ${alphaColor(token.colorText, 0.1)}`,
            }}
            styles={{ body: { padding: 22 } }}
          >
            <div style={{ display: 'grid', gap: 16 }}>
              <div>
                <Text style={{ fontSize: 12, letterSpacing: '0.12em', textTransform: 'uppercase', color: token.colorTextTertiary }}>
                  Recovery Workspace
                </Text>
                <Title level={4} style={{ margin: '6px 0 8px', fontFamily: designDisplayFont }}>
                  恢复认证服务后重新检测
                </Title>
                <Paragraph style={{ margin: 0, color: token.colorTextSecondary, lineHeight: 1.75 }}>
                  当前页面需要先完成登录鉴权检查。认证依赖恢复后，点击重新检测即可回到原本的受保护工作流，不需要额外重配账号或权限。
                </Paragraph>
              </div>

              <div
                style={{
                  padding: '14px 16px',
                  borderRadius: 18,
                  background: quietPanelBackground,
                  border: `1px solid ${token.colorBorderSecondary}`,
                  color: token.colorTextSecondary,
                  fontSize: 13,
                  lineHeight: 1.75,
                }}
              >
                建议优先确认本地数据库与容器环境是否已启动，再执行重新检测；如果问题仍然存在，说明这次阻塞仍发生在认证服务入口而不是具体业务页面。
              </div>

              <div style={{ display: 'flex', justifyContent: 'flex-start' }}>
                <Button
                  type="primary"
                  size="large"
                  onClick={() => {
                    clearAuthStatusCache();
                    window.location.reload();
                  }}
                >
                  重新检测
                </Button>
              </div>
            </div>
          </Card>
        </div>
      </div>
    );
  }

  if (!authState.authenticated) {
    return <Navigate to={buildLoginUrl(getLocationRedirect(location))} replace />;
  }

  return <>{children}</>;
}
