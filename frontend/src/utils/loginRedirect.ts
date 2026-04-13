const LOGIN_REDIRECT_SESSION_KEY = 'login_redirect';

const normalizeRedirect = (value: string | null | undefined): string | null => {
  if (!value) {
    return null;
  }

  const trimmedValue = value.trim();
  if (!trimmedValue.startsWith('/') || trimmedValue.startsWith('//')) {
    return null;
  }

  return trimmedValue;
};

export const getLocationRedirect = (location: {
  pathname?: string;
  search?: string;
  hash?: string;
}): string => {
  const pathname = location.pathname || '/';
  const search = location.search || '';
  const hash = location.hash || '';

  return normalizeRedirect(`${pathname}${search}${hash}`) || '/';
};

export const getRedirectFromSearchParams = (
  searchParams: URLSearchParams,
  fallback = '/',
): string => normalizeRedirect(searchParams.get('redirect')) || fallback;

export const saveLoginRedirect = (redirect: string | null | undefined): void => {
  const normalizedRedirect = normalizeRedirect(redirect);
  if (!normalizedRedirect || normalizedRedirect === '/') {
    sessionStorage.removeItem(LOGIN_REDIRECT_SESSION_KEY);
    return;
  }

  sessionStorage.setItem(LOGIN_REDIRECT_SESSION_KEY, normalizedRedirect);
};

export const consumeLoginRedirect = (fallback = '/'): string => {
  const normalizedRedirect = normalizeRedirect(
    sessionStorage.getItem(LOGIN_REDIRECT_SESSION_KEY),
  );
  sessionStorage.removeItem(LOGIN_REDIRECT_SESSION_KEY);
  return normalizedRedirect || fallback;
};

export const buildLoginUrl = (redirect: string | null | undefined): string => {
  const normalizedRedirect = normalizeRedirect(redirect);
  if (!normalizedRedirect || normalizedRedirect === '/') {
    return '/login';
  }

  return `/login?redirect=${encodeURIComponent(normalizedRedirect)}`;
};

export const buildLoginUrlFromLocation = (location: {
  pathname?: string;
  search?: string;
  hash?: string;
}): string => buildLoginUrl(getLocationRedirect(location));
