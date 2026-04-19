# 登录与鉴权回归检查清单

## 背景

本清单用于回归以下高风险链路：登录、鉴权、redirect 传递、query/hash 保留、LinuxDO OAuth 回跳以及 backend `503` 降级提示。

本轮回归通过标准：
- `ProtectedRoute` 在未登录或服务不可用时表现正常
- `redirect` 参数与 `pathname + search + hash` 能被正确保留
- LinuxDO OAuth 登录不丢失原始跳转目标
- backend 返回 `401` / `503` 时前端有明确反馈

## 环境准备

请确认以下服务已启动：
- Backend API：`http://127.0.0.1:8003`
- Frontend：`http://127.0.0.1:5175`
- Docker fallback backend：`http://127.0.0.1:8004`
- PostgreSQL：`5436`

默认测试账号：
- 用户名：`admin`
- 密码：`admin123`

快速 smoke check：

```powershell
powershell -ExecutionPolicy Bypass -File .\check-auth-flow.ps1
```

## 回归范围

1. 未登录访问受保护页面
2. 账号密码登录成功
3. 账号密码登录失败
4. 登录成功后刷新页面
5. query/hash 保留与回跳
6. LinuxDO OAuth 登录与回跳
7. 退出登录
8. backend `503` 降级与提示

---

## 用例 1：未登录访问受保护页面

### 操作
1. 直接访问 `http://127.0.0.1:5175/projects`
2. 确保当前无有效 Cookie 或 token

### 预期
- 自动跳转至 `/login`
- URL 中包含 `redirect` 参数
- 不出现空白页或无限 loading

---

## 用例 2：账号密码登录成功

### 操作
1. 停留在登录页
2. 输入 `admin`
3. 输入 `admin123`
4. 提交登录

### 预期
- 跳转回原始目标页面
- 会话状态被正确写入
- 首屏无异常闪烁

---

## 用例 3：账号密码登录失败

### 操作
1. 访问 `http://127.0.0.1:5175/login`
2. 输入 `admin`
3. 输入 `wrong-password`
4. 提交

### 预期
- 留在登录页
- 显示明确的失败提示
- 不写入无效会话

---

## 用例 4：登录后刷新页面

### 操作
1. 完成正常登录
2. 在 `/projects` 页面刷新

### 预期
- 仍保持登录状态
- 不被误重定向回 `/login`

---

## 用例 5：query/hash 保留

### 操作
1. 登出或清空会话
2. 访问 `http://127.0.0.1:5175/projects?tab=all#toolbar`
3. 完成登录

### 预期
- 登录后返回同一个 query/hash 地址
- `?tab=all` 与 `#toolbar` 均保留

---

## 用例 6：LinuxDO OAuth 登录

### 操作
1. 退出当前会话
2. 访问 `http://127.0.0.1:5175/projects?tab=all#toolbar`
3. 点击 LinuxDO OAuth 登录
4. 完成 OAuth 授权回跳

### 预期
- 能正确进入 `/auth/callback`
- 回跳后仍然保持原始 redirect
- 不出现回跳循环

---

## 用例 7：退出登录

### 操作
1. 先登录
2. 执行退出
3. 再次访问受保护页面

### 预期
- 会话被清理
- 重定向回登录页
- 不保留过期 redirect 状态

---

## 用例 8：backend `503` 降级

### 操作
1. 暂停 backend 或使 `8003` 不可用
2. 刷新受保护页面或检查登录状态

### 预期
- 前端能感知服务不可用
- `ProtectedRoute` 显示 fallback 或明确提示
- 不出现空白页或无限重试

## 完成标准

- 上述 8 个场景全部通过
- query/hash 保留正确
- LinuxDO OAuth 回跳正确
- `503` 降级行为正确

## 附加建议

- 可配合执行 `npm run build`
- 可配合执行 `npm run lint`
- 如需自动化，建议补充 Playwright 回归用例
