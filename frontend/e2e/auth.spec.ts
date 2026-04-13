import { expect, test } from '@playwright/test';

const usernameSelector = 'input[autocomplete="username"]';
const passwordSelector = 'input[autocomplete="current-password"]';
const submitSelector = 'button[type="submit"]';
const invalidCredentialMessage = '用户名或密码错误';
const callbackErrorMessage = '登录失败，请重试';
const serviceUnavailableMessage = '数据库服务暂时不可用，请先启动 PostgreSQL 或 Docker Desktop 后重试。';
const loginErrorAlertSelector = '[data-testid="login-error-alert"]';
const loginServiceUnavailableAlertSelector = '[data-testid="login-service-unavailable-alert"]';

const mockProjectsAfterOAuthRedirect = async (
  page: import('@playwright/test').Page,
) => {
  await page.route('**/api/projects**', async (route) => {
    await route.fulfill({
      status: 200,
      contentType: 'application/json; charset=utf-8',
      body: JSON.stringify([]),
    });
  });
};

const login = async (
  page: import('@playwright/test').Page,
  username: string,
  password: string,
) => {
  await page.locator(usernameSelector).fill(username);
  await page.locator(passwordSelector).fill(password);
  await page.locator(submitSelector).click();
};

test.describe('auth flow', () => {
  test.beforeEach(async ({ page, context }) => {
    await context.clearCookies();
    await page.addInitScript(() => {
      localStorage.setItem('announcement_hide_forever', 'true');
    });
    await page.goto('/login');
  });

  test('redirects unauthenticated users and preserves query and hash after login', async ({ page }) => {
    await page.goto('/projects?tab=all#toolbar');

    await expect(page).toHaveURL(/\/login\?redirect=/);
    await expect(page.locator(usernameSelector)).toBeVisible();

    await login(page, 'admin', 'admin123');

    await expect(page).toHaveURL(/\/projects\?tab=all#toolbar$/);
  });

  test('shows precise error message for invalid password', async ({ page }) => {
    await login(page, 'admin', 'wrong-password');

    await expect(page).toHaveURL(/\/login/);
    await expect(page.locator(loginErrorAlertSelector)).toBeVisible();
    await expect(page.locator(loginErrorAlertSelector)).toContainText(invalidCredentialMessage);
    await expect(page.locator(usernameSelector)).toHaveValue('admin');
    await expect(page.locator(passwordSelector)).toHaveValue('wrong-password');
  });
  test('shows service unavailable alert when auth service returns 503', async ({ page }) => {
    await page.route('**/api/auth/local/login', async (route) => {
      await route.fulfill({
        status: 503,
        contentType: 'application/json; charset=utf-8',
        body: JSON.stringify({ detail: serviceUnavailableMessage }),
      });
    });

    await login(page, 'admin', 'admin123');

    await expect(page).toHaveURL(/\/login/);
    await expect(page.locator(loginServiceUnavailableAlertSelector)).toBeVisible();
    await expect(page.locator(loginServiceUnavailableAlertSelector)).toContainText(serviceUnavailableMessage);
    await expect(page.locator(loginErrorAlertSelector)).toHaveCount(0);
  });

  test('redirects to saved path after OAuth callback succeeds', async ({ page }) => {
    await mockProjectsAfterOAuthRedirect(page);

    await page.route('**/api/auth/user', async (route) => {
      await route.fulfill({
        status: 200,
        contentType: 'application/json; charset=utf-8',
        body: JSON.stringify({
          id: 'oauth-user-1',
          username: 'oauth-user',
          is_admin: false,
        }),
      });
    });

    await page.evaluate(() => {
      localStorage.setItem('announcement_hide_forever', 'true');
      sessionStorage.setItem('login_redirect', '/projects?tab=all#toolbar');
    });

    await page.goto('/auth/callback?code=fake-code&state=fake-state');

    await expect(page).toHaveURL(/\/projects\?tab=all#toolbar$/);
  });

  test('returns to login when OAuth callback validation fails', async ({ page }) => {
    let handledUserRequest = false;
    await page.route('**/api/auth/user', async (route) => {
      if (!handledUserRequest) {
        handledUserRequest = true;
        await route.fulfill({
          status: 401,
          contentType: 'application/json; charset=utf-8',
          body: JSON.stringify({ detail: '未登录' }),
        });
        return;
      }

      await route.continue();
    });

    await page.goto('/auth/callback?code=fake-code&state=fake-state');

    await expect(page.getByText('登录失败', { exact: true })).toBeVisible();
    await expect(page.getByText(callbackErrorMessage)).toBeVisible();
    await page.getByRole('button', { name: '返回登录' }).click();
    await expect(page).toHaveURL(/\/login$/);
  });
  test('prompts first OAuth login user to initialize password before redirect', async ({ page }) => {
    let initializePasswordPayload: { password?: string } | null = null;

    await mockProjectsAfterOAuthRedirect(page);

    await page.route('**/api/auth/user', async (route) => {
      await route.fulfill({
        status: 200,
        contentType: 'application/json; charset=utf-8',
        body: JSON.stringify({
          id: 'oauth-user-2',
          username: 'first-oauth-user',
          is_admin: false,
        }),
      });
    });

    await page.route('**/api/auth/password/initialize', async (route) => {
      initializePasswordPayload = route.request().postDataJSON() as { password?: string };
      await route.fulfill({
        status: 200,
        contentType: 'application/json; charset=utf-8',
        body: JSON.stringify({ success: true, message: '密码初始化成功' }),
      });
    });

    await page.evaluate(() => {
      localStorage.setItem('announcement_hide_forever', 'true');
      sessionStorage.setItem('login_redirect', '/projects?tab=all#toolbar');
    });

    await page.goto('/auth/callback?code=fake-code&state=fake-state&first_login=1');

    await expect(page.getByRole('dialog', { name: '设置登录密码' })).toBeVisible();
    await expect(page.getByText('当前账号：')).toBeVisible();
    await expect(page.getByText('first-oauth-user')).toBeVisible();
    await page.getByPlaceholder('请输入新密码').fill('custom-pass-123');
    await page.getByPlaceholder('请再次输入密码').fill('custom-pass-123');
    await page.getByRole('button', { name: '确认设置' }).click();

    await expect.poll(() => initializePasswordPayload?.password).toBe('custom-pass-123');
    await expect(page).toHaveURL(/\/projects\?tab=all#toolbar$/);
  });

  test('requires login again after cookies are cleared', async ({ page, context }) => {
    await page.goto('/projects');
    await login(page, 'admin', 'admin123');
    await expect(page).toHaveURL(/\/projects/);

    await context.clearCookies();
    await page.goto('/projects?tab=all#toolbar');

    await expect(page).toHaveURL(/\/login\?redirect=/);
  });
});
