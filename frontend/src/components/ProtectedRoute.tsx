import { useEffect, useState } from 'react';
import type { ReactNode } from 'react';
import { Button, Result } from 'antd';
import { Navigate, useLocation } from 'react-router-dom';
import LoadingScreen from './LoadingScreen';
import { clearAuthStatusCache, resolveAuthStatus } from '../utils/authStatus';
import type { AuthResolution } from '../utils/authStatus';
import { buildLoginUrl, getLocationRedirect } from '../utils/loginRedirect';

interface ProtectedRouteProps {
  children: ReactNode;
}

export default function ProtectedRoute({ children }: ProtectedRouteProps) {
  const [authState, setAuthState] = useState<AuthResolution | null>(null);
  const location = useLocation();

  useEffect(() => {
    let cancelled = false;

    const checkAuth = async () => {
      const resolvedState = await resolveAuthStatus();
      if (!cancelled) {
        setAuthState(resolvedState);
      }
    };

    void checkAuth();

    return () => {
      cancelled = true;
    };
  }, []);

  if (authState === null) {
    return <LoadingScreen message="加载中..." minHeight="100vh" />;
  }

  if (authState.serviceUnavailable) {
    return (
      <div
        style={{
          minHeight: '100vh',
          display: 'flex',
          alignItems: 'center',
          justifyContent: 'center',
          padding: '24px',
        }}
      >
        <Result
          status="warning"
          title="服务暂时不可用"
          subTitle="认证服务当前无法访问，请确认 PostgreSQL 或 Docker Desktop 已启动后重试。"
          extra={(
            <Button
              type="primary"
              onClick={() => {
                clearAuthStatusCache();
                window.location.reload();
              }}
            >
              重新检测
            </Button>
          )}
        />
      </div>
    );
  }

  if (!authState.authenticated) {
    return <Navigate to={buildLoginUrl(getLocationRedirect(location))} replace />;
  }

  return <>{children}</>;
}
