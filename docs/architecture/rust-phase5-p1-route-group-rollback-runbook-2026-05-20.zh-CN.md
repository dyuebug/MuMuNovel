# Rust Phase 5 P1 route-group rollback runbook（2026-05-20）

## 1. 目的

本 runbook 承接：

- `docs/architecture/rust-strangler-refactor-plan-2026-05-17.zh-CN.md`
- `docs/architecture/rust-route-group-ownership-and-cutover-checklist-2026-05-19.zh-CN.md`
- `docs/architecture/rust-python-api-parity-matrix-2026-05-19.zh-CN.md`

目标是把 Phase 5 P1 route group 的 rollback / fallback 验证从“已有 owner
smoke，但缺少回切纪律”推进到可直接执行的操作步骤。

本 runbook 只覆盖以下 P1 API 组：

1. `auth`
2. `users`

---

## 2. 使用范围

适用场景：

- strangler 部署后，`phase5-p1` owner smoke 失败
- `auth` 或 `users` 的 Rust owner 侧出现明确回归
- 需要临时把 `auth` / `users` 的 through-gateway 默认流量切回 Python

不适用场景：

- shared DB schema 本身损坏
- Python/Rust 双侧都失败
- 需要修改业务数据、执行数据修复或做 schema rollback

---

## 3. 总体原则

1. 先改 gateway owner，再看服务日志；不要先改代码重发版。
2. `auth` 与 `users` 分组回切，不要一次性扩大到所有 Rust-owned 组。
3. 保持 shared DB 不变，不做 schema rollback。
4. 每次回滚后都要执行“定向 smoke + fallback smoke + 全量 P1 smoke”。
5. `users` 的 Python fallback 不能机械复用 Rust probe；必须先区分路径语义。

---

## 4. 前置检查

执行回滚前，先确认：

1. 当前 gateway 指向的是 strangler 环境，而不是纯 Python/纯 Rust 直连。
2. `backend/tools/run_strangler_gateway_smoke.py` 可正常执行。
3. `deploy/nginx/mumunovel.conf` 与 `deploy/nginx/mumunovel-docker.conf`
   中对应 route group 的当前 owner 规则与实际部署环境一致。
4. 已记录当前失败 probe 名称、HTTP 状态、返回体摘要、触发时间。

建议先执行：

```powershell
python backend/tools/run_strangler_gateway_smoke.py `
  --manifest deploy/strangler-gateway-probes.json `
  --profile phase5-p1 `
  --base-url http://127.0.0.1:8005
```

如果只想确认失败组，使用定向 probe：

```powershell
python backend/tools/run_strangler_gateway_smoke.py `
  --manifest deploy/strangler-gateway-probes.json `
  --profile phase5-p1 `
  --route-group "<route-group>" `
  --base-url http://127.0.0.1:8005
```

---

## 5. 通用回滚流程

### 5.1 定位失败组

把失败 probe 映射到 route group：

| Route group | 当前 P1 Rust owner probes |
|---|---|
| `auth` | `auth-config-public-rust` / `auth-logout-public-rust` / `auth-linuxdo-url-misconfig-rust` / `auth-user-auth-guard-rust` / `auth-password-status-auth-guard-rust` / `auth-password-set-auth-guard-rust` / `auth-password-initialize-auth-guard-rust` / `auth-refresh-auth-guard-rust` / `auth-callback-missing-code-rust` / `auth-local-login-invalid-credentials-rust` / `auth-bind-login-invalid-credentials-rust` |
| `users` | `users-current-auth-guard-rust` / `users-list-auth-guard-rust` / `users-set-admin-auth-guard-rust` / `users-reset-password-auth-guard-rust` |

### 5.2 修改 gateway owner

只修改目标 route group 的 Nginx `location`：

- `auth`：把 `/api/auth*` 从 `rust_backend` 回切到 `python_backend`
- `users`：把 `/api/users*` 从 `rust_backend` 回切到 `python_backend`

修改文件：

- 本地 / 非 Docker：`deploy/nginx/mumunovel.conf`
- Docker Compose：`deploy/nginx/mumunovel-docker.conf`

### 5.3 重载 gateway

使用当前部署方式重载 Nginx，使新 owner 生效。

### 5.4 先跑定向 owner smoke

先验证当前回滚组原本的 Rust owner smoke，确认它们已不再继续命中原 Rust 特征：

```powershell
python backend/tools/run_strangler_gateway_smoke.py `
  --manifest deploy/strangler-gateway-probes.json `
  --profile phase5-p1 `
  --route-group "<route-group>" `
  --base-url http://127.0.0.1:8005
```

说明：

- 这里预期很可能出现断言失败，因为 probe 仍然按 Rust owner 结果定义。
- 这一步的目标不是“继续通过”，而是确认 owner 已经不再表现为 Rust。

### 5.5 再跑 P1 fallback smoke

当前第一版 fallback 已覆盖 `auth`、`characters`、`outlines` 与 `book_import`：

```powershell
python backend/tools/run_strangler_gateway_smoke.py `
  --manifest deploy/strangler-gateway-probes.json `
  --profile phase5-p1-fallback `
  --route-group "<route-group>" `
  --base-url http://127.0.0.1:8005
```

当前 `phase5-p1-fallback` 覆盖：

1. `POST /api/auth/logout`
2. `GET /api/auth/user`
3. `GET /api/auth/password/status`
4. `POST /api/auth/password/set`
5. `POST /api/auth/password/initialize`
6. `POST /api/auth/refresh`
7. `GET /api/auth/callback`
8. `POST /api/auth/local/login`
9. `POST /api/auth/bind/login`
10. `GET /api/users/current`
11. `GET /api/users`
12. `POST /api/users/set-admin`
13. `POST /api/users/reset-password`
14. `GET /api/characters/project/{project_id}`
15. `GET /api/characters?project_id=...`
16. `POST /api/characters/generate-stream`
17. `POST /api/characters/export`
18. `POST /api/characters/import`
19. `GET /api/outlines/project/{project_id}`
20. `GET /api/outlines?project_id=...`
21. `POST /api/outlines/generate-stream`
22. `POST /api/outlines/batch-expand-stream`
23. `POST /api/outlines/{outline_id}/create-chapters-from-plans`
24. `GET /api/book-import/tasks/{task_id}`
25. `GET /api/book-import/tasks/{task_id}/preview`
26. `POST /api/book-import/tasks`
27. `DELETE /api/book-import/tasks/{task_id}`
28. `POST /api/book-import/tasks/{task_id}/apply`
29. `POST /api/book-import/tasks/{task_id}/retry-stream`
30. `POST /api/book-import/tasks/{task_id}/apply-stream`

### 5.5A 再跑 P1 非对称接口 smoke

当前 `phase5-p1-asymmetric` 用来承载“同路径存在，但 Rust owner 与 Python fallback
不是同一类入口语义”的接口证据，避免把它们误写成 auth-boundary fallback：

```powershell
python backend/tools/run_strangler_gateway_smoke.py `
  --manifest deploy/strangler-gateway-probes.json `
  --profile phase5-p1-asymmetric `
  --route-group "<route-group>" `
  --base-url http://127.0.0.1:8005
```

当前 `phase5-p1-asymmetric` 第一版覆盖：

1. `POST /api/characters/validate-import`

### 5.6 再跑全量 P1 smoke

```powershell
python backend/tools/run_strangler_gateway_smoke.py `
  --manifest deploy/strangler-gateway-probes.json `
  --profile phase5-p1 `
  --base-url http://127.0.0.1:8005
```

目的：

- 确认其他 P1 组没被误伤
- 避免一次 location 修改导致更大范围 owner 漂移

---

## 6. Python fallback 成功线索（第一版）

### 6.1 `auth`

`auth` 现在已经有第一版可执行 fallback smoke，核心差异如下：

| 路径 | Rust owner 成功线索 | Python fallback 成功线索 |
|---|---|---|
| `POST /api/auth/logout` | `200 {"success": true, "message": "已登出"}` 且 `Set-Cookie` 包含 `token=` | `200 {"message": "退出登录成功"}` 且 `Set-Cookie` 包含 `user_id=` |
| `GET /api/auth/user` | `401 {"detail":"未登录，请先登录"}` | `401 {"detail":"未登录"}` |
| `GET /api/auth/password/status` | `401 {"detail":"未登录，请先登录"}` | `401 {"detail":"未登录"}` |
| `POST /api/auth/password/set` | `401 {"detail":"未登录，请先登录"}` | `401 {"detail":"未登录"}` |
| `POST /api/auth/password/initialize` | `401 {"detail":"未登录，请先登录"}` | `401 {"detail":"未登录"}` |
| `POST /api/auth/refresh` | `401 {"detail":"未登录，请先登录"}` | `401 {"detail":"未登录，无法刷新会话"}` |
| `GET /api/auth/callback` | `400 {"detail":"缺少 code 参数"}` | `400 {"detail":"缺少 code 或 state 参数"}` |
| `POST /api/auth/local/login` | `401 {"success": false, "message": "用户名或密码错误"}` | `401 {"detail":"用户名或密码错误"}` |
| `POST /api/auth/bind/login` | `401 {"success": false, "message": "用户名或密码错误"}` | `401 {"detail":"用户名或密码错误"}` |

补充说明：

- `GET /api/auth/config` 不适合作为 Python fallback 成功条件，因为 Python 与 Rust
  在未配置 LinuxDO 时都可能返回稳定 JSON，而 owner 区分度不高。
- `GET /api/auth/linuxdo/url` 也不适合作为 Python fallback smoke，因为 Python 正常分支
  会生成外部 OAuth URL，不是一个低前提、低时变的固定失败信号。
- `password/set` 与 `password/initialize` 现在也已进入 P1 fallback 资产线。
  两条路径都只需要最小合法 JSON 请求体，就能稳定落到未登录边界，不需要真实会话、
  也不会误触密码长度或首次初始化等业务分支。
- `refresh` 现在也已进入 P1 fallback 资产线。它不需要请求体，但 Python 侧的稳定
  未登录差异是 `未登录，无法刷新会话`，因此能够补强会话相关 fallback 线索，而不是
  继续只依赖 `user` / `password/status` 读侧边界。
- `callback` 现在也已进入 P1 owner/fallback 资产线。它不依赖真实 OAuth 成功分支、
  外部回调或临时 state，只用空查询请求就能稳定停在本地参数校验边界；Rust owner
  先报 `缺少 code 参数`，Python fallback 则报 `缺少 code 或 state 参数`，
  因而是比 `linuxdo/url` 更低时变、更适合回滚验证的 public error probe。
- `local/login` 现在也已进入 P1 owner/fallback 资产线。它依赖
  `local_auth_enabled=true` 这一当前部署前提，但不依赖真实用户存在、也不需要会话；
  只用一组明确错误的账号密码，就能稳定落在同一路径的公开失败分支。Rust owner
  返回 `401 {"success": false, "message": "用户名或密码错误"}`，Python fallback
  则返回 `401 {"detail":"用户名或密码错误"}`，因此能补强 `auth` 组的
  public/business failure 结构差异，而不是继续只堆 auth-boundary `401`。
- `bind/login` 现在也已进入 P1 owner/fallback 资产线。它与 `local/login`
  一样能够稳定命中错误凭证分支，但 Python 侧不再先受 `local_auth_enabled`
  开关影响，而是直接走“绑定账号登录”路径，因此是更低前提的第二条真实登录入口
  失败线索。Rust owner 仍返回 compat 风格
  `401 {"success": false, "message": "用户名或密码错误"}`，Python fallback
  则仍返回 `401 {"detail":"用户名或密码错误"}`。

### 6.2 `users`

`users` 现在可以补第一版 fallback probe，但必须明确它验证的是“Python owner
已接管同路径鉴权边界”，而不是完整业务语义等价：

| Rust 路径 | Python 近似路径 | 风险 |
|---|---|---|
| `GET /api/users/current` | `GET /api/users/current` | 现在确认 Python 侧存在同路径路由；未登录时可稳定落在 `require_login("需要登录")`，适合作为 fallback auth-boundary 线索 |
| `GET /api/users` | `GET /api/users` | Python 需要管理员权限；未登录只证明 auth-boundary，不证明列表语义完全等价 |
| `POST /api/users/set-admin` | `POST /api/users/set-admin` | Python 同路径存在；未登录时先停在 `require_login("需要登录")`，但这仍不证明管理员写侧语义完全等价 |
| `POST /api/users/reset-password` | `POST /api/users/reset-password` | Python 同路径存在；未登录时先停在 `require_login("需要登录")`，但这仍不证明密码重置写侧语义完全等价 |
| `/api/admin/users*` | `/api/admin/users*` | 这是 `admin` 组，不应混进 `users` fallback 判定 |

结论：

- `users/current` 现在可以直接用同路径 probe 验证 Python owner 的登录边界。
- `users` 列表也可以用同路径 probe 验证 Python owner 的登录边界，但这仍不等于
  验证管理员列表业务语义完全等价。
- `set-admin` 与 `reset-password` 现在也可以用同路径 probe 验证 Python owner
  的登录边界，但它们同样不等于管理员写侧业务语义完全等价。
- 因此 `users` 的 fallback smoke 现阶段应解释为“same-path auth-boundary
  ownership clue”，而不是“full business parity proof”。

### 6.3 `characters`

`characters` 现在已经具备第一版可执行 fallback smoke：

| 路径 | Rust owner 成功线索 | Python fallback 成功线索 |
|---|---|---|
| `GET /api/characters/project/{project_id}` | `401 {"detail":"未登录，请先登录"}` | `401 {"detail":"未登录"}` |
| `GET /api/characters?project_id=...` | `401 {"detail":"未登录，请先登录"}` | `401 {"detail":"未登录"}` |
| `POST /api/characters/generate-stream` | `401 {"detail":"未登录，请先登录"}` | `401 {"detail":"需要登录"}` |
| `POST /api/characters/export` | `401 {"detail":"未登录，请先登录"}` | `401 {"detail":"未登录"}` |
| `POST /api/characters/import` | `401 {"detail":"未登录，请先登录"}` | `401 {"detail":"未登录"}` |

说明：

- Python 侧该路径直接走 `verify_project_access(project_id, user_id, db)`，未登录语义稳定。
- `generate-stream` 这条写侧路径会先经过 `get_user_ai_service()`，因此 Python fallback
  的稳定未登录差异是 `401 {"detail":"需要登录"}`，而不是 `未登录`。
- `export` 与 `import` 这两条写侧路径不依赖真实角色数据存在，只要求请求形状合法，
  因此更适合补成低前提 rollback 线索；二者的 Python fallback 稳定未登录差异
  仍是 `401 {"detail":"未登录"}`。
- `validate-import` 现在进入独立的 `phase5-p1-asymmetric` profile：
  Rust owner 侧用最小合法 `.json` 文件命中公开校验成功，
  Python fallback 侧则稳定落到 `401 {"detail":"未登录"}`。
  这条证据明确表达“同路径存在，但入口语义不对称”，不再伪装成 auth-boundary fallback。
- 这五条 probe 共同覆盖 path/query 版角色列表读侧、`generate-stream` 提交边界，
  以及导入导出入口，不证明真实生成成功、文件内容语义或 SSE 事件链路已自动验证。

补充非对称接口说明：

| 路径 | Rust owner 成功线索 | Python fallback 成功线索 | 归类 |
|---|---|---|---|
| `POST /api/characters/validate-import` | `200` + `valid=true` + 空数据 warning | `401 {"detail":"未登录"}` | `phase5-p1-asymmetric` |

### 6.4 `outlines`

`outlines` 现在已经具备第一版可执行 fallback smoke：

| 路径 | Rust owner 成功线索 | Python fallback 成功线索 |
|---|---|---|
| `GET /api/outlines/project/{project_id}` | `401 {"detail":"未登录，请先登录"}` | `401 {"detail":"未登录"}` |
| `GET /api/outlines?project_id=...` | `401 {"detail":"未登录，请先登录"}` | `401 {"detail":"未登录"}` |
| `POST /api/outlines/generate-stream` | `401 {"detail":"未登录，请先登录"}` | `401 {"detail":"需要登录"}` |
| `POST /api/outlines/batch-expand-stream` | `401 {"detail":"未登录，请先登录"}` | `401 {"detail":"需要登录"}` |
| `POST /api/outlines/{outline_id}/create-chapters-from-plans` | `401 {"detail":"未登录，请先登录"}` | `401 {"detail":"需要登录"}` |

说明：

- Python 侧该路径最终走 `get_outlines()` 与 `verify_project_access()`，未登录差异稳定。
- `generate-stream`、`batch-expand-stream` 与 `create-chapters-from-plans`
  这三条写侧路径都会先经过 `get_user_ai_service()`，因此 Python fallback
  的稳定未登录差异是 `401 {"detail":"需要登录"}`。
- `create-chapters-from-plans` 的 probe 使用一条最小合法 `chapter_plans`
  记录，目的是越过 body schema 校验，稳定落到同一路径的鉴权边界。
- 这五条 probe 共同覆盖 path/query 版大纲列表读侧，以及生成、批量展开、
  基于既有规划创建章节三类提交边界，不代表 SSE 成功路径或真实章节创建成功
  已经验证。

### 6.5 `book_import`

`book_import` 现在已经具备第一版可执行 fallback smoke：

| 路径 | Rust owner 成功线索 | Python fallback 成功线索 |
|---|---|---|
| `GET /api/book-import/tasks/{task_id}` | `401 {"detail":"未登录，请先登录"}` | `401 {"detail":"未登录"}` |
| `GET /api/book-import/tasks/{task_id}/preview` | `401 {"detail":"未登录，请先登录"}` | `401 {"detail":"未登录"}` |
| `POST /api/book-import/tasks` | `401 {"detail":"未登录，请先登录"}` | `401 {"detail":"未登录"}` |
| `DELETE /api/book-import/tasks/{task_id}` | `401 {"detail":"未登录，请先登录"}` | `401 {"detail":"未登录"}` |
| `POST /api/book-import/tasks/{task_id}/apply` | `401 {"detail":"未登录，请先登录"}` | `401 {"detail":"未登录"}` |
| `POST /api/book-import/tasks/{task_id}/retry-stream` | `401 {"detail":"未登录，请先登录"}` | `401 {"detail":"未登录"}` |
| `POST /api/book-import/tasks/{task_id}/apply-stream` | `401 {"detail":"未登录，请先登录"}` | `401 {"detail":"未登录"}` |

说明：

- Python 侧该路径直接检查 `request.state.user_id`，未登录差异稳定。
- 这七条 probe 共同覆盖上传创建、任务状态、预览、取消，以及 `apply` /
  `retry-stream` / `apply-stream` 的提交边界，不代表真实登录态下的上传成功、
  导入结果语义或 SSE 成功路径已经自动验证。

---

## 7. 分组执行要点

### 7.1 `auth`

当前 Rust owner location：

- `/api/auth/`
- `/api/auth/linuxdo/`

回滚原则：

- 优先整组回切 `/api/auth*`
- 不建议只回切 LinuxDO callback 或 logout 单一路径，除非已确认是 isolated regression

### 7.2 `users`

当前 Rust owner location：

- `/api/users`
- `/api/users/`

回滚原则：

- 优先整组回切 `/api/users*`
- 不要把 `/api/admin/users*` 一起当成 `users` 组回滚；它属于 `admin` 组
- 回滚后要额外确认前端“当前用户”读取是否改走 `/api/auth/user`，而不是继续假设
  `/api/users/current` 可用

---

## 8. 失败升级条件

出现以下任一情况，不要继续扩大回滚面，应转入服务级诊断：

1. gateway owner 已回切，但响应仍持续表现为 Rust 特征
2. `auth` 的 fallback smoke 也失败，说明 Python owner 本身不可用
3. 回切 `users` 后，前端当前用户链路仍依赖 `/api/users/current`
4. `auth` 与 `users` 同时回切后出现 cookie/session 语义漂移

---

## 9. 操作记录模板

每次执行后至少记录：

1. 时间
2. 环境
3. 回滚 route group
4. 修改的 Nginx 文件与 location
5. 定向 owner smoke 结果
6. fallback smoke 结果（若适用）
7. 全量 `phase5-p1` 结果
8. 是否需要进一步服务级诊断

---

## 10. 当前结论

截至 2026-05-20，P1 route-group rollback 已经从“只有 Rust owner smoke”推进到：

1. `auth` 已具备第一版可执行 Python fallback smoke
2. `characters`、`outlines` 与 `book_import` 也已具备第一版同路径 Python fallback smoke
3. `users` 的 fallback 风险已被明确文档化，不再假设它和 Rust 路径一一对应
4. P1 仍未达到可移除 Python fallback 的程度，但已经具备更像运维资产的回滚纪律
