const { chromium } = require('playwright');

(async () => {
  const browser = await chromium.launch({ headless: true });
  const context = await browser.newContext({ baseURL: 'http://127.0.0.1:5175' });
  const page = await context.newPage();
  let initializePasswordPayload = null;
  let authUserHits = 0;

  page.on('framenavigated', frame => {
    if (frame === page.mainFrame()) {
      console.log('NAV=', frame.url());
    }
  });
  page.on('request', req => {
    if (req.url().includes('/api/auth/user') || req.url().includes('/api/auth/password/initialize')) {
      console.log('REQ=', req.method(), req.url());
    }
  });
  page.on('response', async (res) => {
    if (res.url().includes('/api/auth/user') || res.url().includes('/api/auth/password/initialize')) {
      console.log('RES=', res.status(), res.url());
    }
  });

  await page.route('**/api/auth/user', async (route) => {
    authUserHits += 1;
    console.log('ROUTE auth/user hit', authUserHits);
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
    initializePasswordPayload = route.request().postDataJSON();
    console.log('ROUTE password/initialize', JSON.stringify(initializePasswordPayload));
    await route.fulfill({
      status: 200,
      contentType: 'application/json; charset=utf-8',
      body: JSON.stringify({ success: true, message: '密码初始化成功' }),
    });
  });

  await context.clearCookies();
  await page.addInitScript(() => {
    localStorage.setItem('announcement_hide_forever', 'true');
    sessionStorage.setItem('login_redirect', '/projects?tab=all#toolbar');
  });

  await page.goto('/login');
  await page.goto('/auth/callback?code=fake-code&state=fake-state&first_login=1');
  await page.getByPlaceholder('请输入新密码').fill('custom-pass-123');
  await page.getByPlaceholder('请再次输入密码').fill('custom-pass-123');
  await page.getByRole('button', { name: '确认设置' }).click();

  await page.waitForTimeout(5000);
  console.log('FINAL=', page.url());
  console.log('INIT_PAYLOAD=', JSON.stringify(initializePasswordPayload));
  console.log('AUTH_USER_HITS=', authUserHits);

  await browser.close();
})();