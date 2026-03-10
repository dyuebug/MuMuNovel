import { useEffect, useState } from 'react';
import type { ReactNode } from 'react';
import { Navigate, useLocation } from 'react-router-dom';
import LoadingScreen from './LoadingScreen';
import { authApi } from '../services/api';
import { sessionManager } from '../utils/sessionManager';

interface ProtectedRouteProps {
  children: ReactNode;
}

const AUTH_STATUS_CACHE_MS = 10000;

let cachedAuthStatus: { value: boolean; expiresAt: number } | null = null;
let authStatusPromise: Promise<boolean> | null = null;

const resolveAuthStatus = async (): Promise<boolean> => {
  const now = Date.now();
  if (cachedAuthStatus && cachedAuthStatus.expiresAt > now) {
    return cachedAuthStatus.value;
  }

  if (!authStatusPromise) {
    authStatusPromise = (async () => {
      try {
        await authApi.getCurrentUser();
        sessionManager.start();
        cachedAuthStatus = {
          value: true,
          expiresAt: Date.now() + AUTH_STATUS_CACHE_MS,
        };
        return true;
      } catch {
        sessionManager.stop();
        cachedAuthStatus = {
          value: false,
          expiresAt: Date.now() + 2000,
        };
        return false;
      } finally {
        authStatusPromise = null;
      }
    })();
  }

  return authStatusPromise;
};

export default function ProtectedRoute({ children }: ProtectedRouteProps) {
  const [isAuthenticated, setIsAuthenticated] = useState<boolean | null>(null);
  const location = useLocation();

  useEffect(() => {
    let cancelled = false;

    const checkAuth = async () => {
      const authenticated = await resolveAuthStatus();
      if (!cancelled) {
        setIsAuthenticated(authenticated);
      }
    };

    void checkAuth();

    return () => {
      cancelled = true;
    };
  }, []);

  if (isAuthenticated === null) {
    return <LoadingScreen message="加载中..." minHeight="100vh" />;
  }

  if (!isAuthenticated) {
    return <Navigate to={`/login?redirect=${encodeURIComponent(location.pathname)}`} replace />;
  }

  return <>{children}</>;
}
