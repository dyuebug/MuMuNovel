# Rust Phase 5 P0 route-group rollback runbook（2026-05-19）

## 1. 目的

本 runbook 承接：

- `docs/architecture/rust-strangler-refactor-plan-2026-05-17.zh-CN.md`
- `docs/architecture/rust-route-group-ownership-and-cutover-checklist-2026-05-19.zh-CN.md`

目标是把 Phase 5 P0 route group 的 rollback 从“表格里的口头说明”落成
可直接执行的操作步骤。

本 runbook 只覆盖以下 P0 API 组：

1. `settings`
2. `projects`
3. `chapters`
4. `wizard-stream`
5. `memories`

---

## 2. 使用范围

适用场景：

- strangler 部署后，`phase5-p0` owner smoke 失败
- 某个 P0 route group 出现 Rust owner 侧的明确回归
- 需要临时把单个 route group 的 through-gateway 默认流量切回 Python

不适用场景：

- shared DB schema 本身损坏
- Python/Rust 双侧都失败
- 需要修改业务数据、执行数据修复或做 schema rollback

---

## 3. 总体原则

1. 先改 gateway owner，再看服务日志；不要先改代码重发版。
2. 只回切一个 route group，不要一次性扩大回滚面。
3. 保持 shared DB 不变，不做 schema rollback。
4. 每次回滚后都要执行“定向 smoke + 全量 P0 smoke”两轮验证。

---

## 4. 前置检查

执行回滚前，先确认：

1. 当前 gateway 指向的是 strangler 环境，而不是纯 Python/纯 Rust 直连。
2. `backend/tools/run_strangler_gateway_smoke.py` 可正常执行。
3. `deploy/nginx/mumunovel.conf` 与 `deploy/nginx/mumunovel-docker.conf`
   中对应 route group 的当前 owner 规则与生产/测试环境一致。
4. 已记录当前失败 probe 名称、HTTP 状态、返回体摘要、触发时间。

建议先执行：

```powershell
python backend/tools/run_strangler_gateway_smoke.py `
  --manifest deploy/strangler-gateway-probes.json `
  --profile phase5-p0 `
  --base-url http://127.0.0.1:8005
```

如果只想确认失败组，使用定向 probe：

```powershell
python backend/tools/run_strangler_gateway_smoke.py `
  --manifest deploy/strangler-gateway-probes.json `
  --profile phase5-p0 `
  --route-group "<route-group>" `
  --base-url http://127.0.0.1:8005
```

---

## 5. 通用回滚流程

### 5.1 定位失败组

把失败 probe 映射到 route group：

| Route group | 当前 P0 probe |
|---|---|
| `settings` | `settings-auth-guard-rust` / `settings-fetch-models-auth-guard-rust` / `settings-test-auth-guard-rust` / `settings-check-function-calling-auth-guard-rust` |
| `projects` | `projects-list-auth-guard-rust` / `projects-validate-import-public-rust` / `projects-import-auth-guard-rust` / `projects-export-data-auth-guard-rust` |
| `chapters` | `chapters-list-auth-guard-rust` / `chapters-analysis-auth-guard-rust` / `chapters-batch-analysis-status-auth-guard-rust` / `chapters-batch-active-tasks-auth-guard-rust` / `chapters-batch-stream-auth-guard-rust` / `chapters-batch-resume-auth-guard-rust` / `chapters-generate-background-auth-guard-rust` / `chapters-regeneration-tasks-auth-guard-rust` |
| `wizard-stream` | `wizard-stream-outline-auth-guard-rust` / `wizard-stream-world-building-regenerate-auth-guard-rust` / `wizard-stream-cleanup-auth-guard-rust` / `wizard-stream-career-system-auth-guard-rust` / `wizard-stream-characters-auth-guard-rust` |
| `memories` | `memories-stats-auth-guard-rust` / `memories-search-auth-guard-rust` |

### 5.2 修改 gateway owner

只修改目标 route group 的 Nginx `location`：

- 删除或注释掉显式指向 `rust_backend` 的 location
- 或把该组 location 改为 `proxy_pass http://python_backend`
- 保留其他 P0 组不变

修改文件：

- 本地 / 非 Docker：`deploy/nginx/mumunovel.conf`
- Docker Compose：`deploy/nginx/mumunovel-docker.conf`

### 5.3 重载 gateway

使用当前部署方式重载 Nginx，使新 owner 生效。

### 5.4 先跑定向 smoke

只验证当前回滚组对应 probe，确认 owner 已发生预期变化：

```powershell
python backend/tools/run_strangler_gateway_smoke.py `
  --manifest deploy/strangler-gateway-probes.json `
  --profile phase5-p0 `
  --route-group "<route-group>" `
  --base-url http://127.0.0.1:8005
```

`<route-group>` 当前取值：

- `settings`
- `projects`
- `chapters`
- `wizard-stream`
- `memories`

说明：

- 如果 probe 仍命中 Rust 旧行为，说明 gateway owner 没有真正切换成功。
- 如果 probe 进入 Python，但断言失败，说明需要补 Python fallback 对应 smoke，
  或该组原先 probe 只适合证明 Rust owner，不适合作为回切后成功条件。
- 如需继续缩小到单条 probe，可追加 `--probe-name "<probe-name>"`。

如需执行当前第一版 Python fallback smoke：

```powershell
python backend/tools/run_strangler_gateway_smoke.py `
  --manifest deploy/strangler-gateway-probes.json `
  --profile phase5-p0-fallback `
  --route-group "<route-group>" `
  --base-url http://127.0.0.1:8005
```

### 5.4A 再跑 P0 非对称接口 smoke

当前 `phase5-p0-asymmetric` 用来承载“同路径存在，但 Rust owner 与 Python fallback
不是同一类入口语义”的接口证据，避免把它们误写成 auth-boundary fallback：

```powershell
python backend/tools/run_strangler_gateway_smoke.py `
  --manifest deploy/strangler-gateway-probes.json `
  --profile phase5-p0-asymmetric `
  --route-group "<route-group>" `
  --base-url http://127.0.0.1:8005
```

当前 `phase5-p0-asymmetric` 第一版覆盖：

1. `GET /api/settings/models?provider=openai&api_key=test-key&api_base_url=http://127.0.0.1:9/v1`
2. `GET /api/chapters/batch-generate/{batch_id}/status`
3. `POST /api/chapters/batch-generate/{batch_id}/cancel`

### 5.5 再跑全量 P0 smoke

```powershell
python backend/tools/run_strangler_gateway_smoke.py `
  --manifest deploy/strangler-gateway-probes.json `
  --profile phase5-p0 `
  --base-url http://127.0.0.1:8005
```

目的：

- 确认其他 P0 route group 没被误伤
- 避免一次 location 修改导致更大范围 owner 漂移

### 5.6 Python fallback 成功线索（第一版）

当前 `phase5-p0` manifest 里的 probes 主要还是 Rust owner smoke，因此回切到
Python 后，短期更适合先用“稳定差异线索”判断 owner 是否真的已经切回。

以下线索可作为第一版候选：

| Route group | 当前 probe / 路径 | Python fallback 成功线索（第一版） | 说明 |
|---|---|---|---|
| `settings` | `GET /api/settings` | `401 {"detail":"需要登录"}` | Python `settings` 路由统一走 `require_login()`，未登录语义与 Rust 的 `未登录，请先登录` 可区分 |
| `settings` | `GET /api/settings/api-key` | `401 {"detail":"需要登录"}` | 这是最小同路径凭据读取入口；Python 侧同样先走 `Depends(require_login)`，因此回切后应稳定停在登录边界，而不会先进入设置加载或 provider 分支 |
| `settings` | `POST /api/settings/fetch-models` | `401 {"detail":"需要登录"}` | 这是最小合法模型拉取请求体；Python 侧同样先走 `Depends(require_login)`，因此回切后应稳定停在同一个登录边界，而不会先掉进缺参或网络探测异常 |
| `settings` | `POST /api/settings/test` | `401 {"detail":"需要登录"}` | 这是最小合法连接测试请求体；Python 侧同样先经过 `Depends(require_login)`，因此回切后应稳定停在登录边界，而不会先进入外部 API 连通性探测 |
| `settings` | `POST /api/settings/check-function-calling` | `401 {"detail":"需要登录"}` | 这是最小合法 Function Calling 探测请求体；Python 侧同样先经过 `Depends(require_login)`，因此回切后应稳定停在登录边界，而不会先进入工具调用能力探测逻辑 |
| `projects` | `GET /api/projects` | `401 {"detail":"未登录"}` | Python `projects` 路由直接读取 `request.state.user_id`，未登录返回短语义 `未登录` |
| `projects` | `POST /api/projects/validate-import` | `200` 且 `valid=true`；Python fallback 应返回 `organization_members` / `character_careers` / `story_memories` / `has_default_style=false`，并带 `warnings=["项目没有章节数据","项目没有角色数据"]` | 这是同路径 public validator，不依赖登录态；Rust owner 与 Python fallback 会对同一个最小导入文件返回不同但稳定的统计/告警结构，适合作为更强的回切成功线索 |
| `projects` | `POST /api/projects/import` | `401 {"detail":"未登录"}` | 这是与 `validate-import` 共用最小 multipart 文件的写侧入口；Python 在读文件业务前就会先做 `request.state.user_id` 检查，可作为合法导入形态下的 fallback 鉴权线索 |
| `projects` | `POST /api/projects/{project_id}/export-data` | `401 {"detail":"未登录"}` | 这是最小合法 JSON body 为 `{}` 的导出写侧入口；Python 在读取导出选项后仍会先停在 `request.state.user_id` 登录检查上，可作为 JSON 写侧 fallback 线索 |
| `chapters` | `GET /api/chapters/project/{project_id}` | `401 {"detail":"未登录"}` | Python章节列表主路径是 project-path 形态，现已进入 `phase5-p0-fallback` |
| `chapters` | `GET /api/chapters/{id}/analysis` | `401 {"detail":"未登录"}` | Python analysis 兼容服务走 `require_authenticated_user_id()` |
| `chapters` | `POST /api/chapters/analysis/status/batch` | `401 {"detail":"未登录"}` | Python batch analysis status 兼容服务也走 `require_authenticated_user_id()` |
| `chapters` | `GET /api/chapters/batch-generate/active-tasks` | `401 {"detail":"Not logged in"}` | Python batch-generation 路由这条是英文未登录语义，可与 Rust 区分 |
| `chapters` | `GET /api/chapters/batch-generate/{batch_id}/stream` | `401 {"detail":"未登录"}` | Python stream access 校验先检查 `request.state.user_id`，未登录直接返回 `未登录`，因此可作为同路径 SSE 查询入口的 fallback 线索 |
| `chapters` | `POST /api/chapters/batch-generate/{batch_id}/resume` | `401 {"detail":"Not logged in"}` | Python batch-generation resume 入口先检查 `request.state.user_id`，未登录直接返回英文 `Not logged in`，因此可作为同路径恢复写侧入口的 fallback 线索 |
| `chapters` | `POST /api/chapters/{chapter_id}/generate-background` | `401 {"detail":"未登录"}` | Python 单章后台生成入口走 `require_authenticated_user_id()`，未登录直接返回 `未登录`，因此可作为同路径单章生成写侧入口的 fallback 线索 |
| `chapters` | `GET /api/chapters/{id}/regeneration/tasks` | `401 {"detail":"未登录"}` | Python regeneration query 路由走 `require_authenticated_user_id()` |
| `wizard-stream` | `POST /api/wizard-stream/outline` | `401 {"detail":"需要登录"}` | Python `wizard_stream` 在进入 SSE handler 前就会先经过 `get_user_ai_service -> require_login()` |
| `wizard-stream` | `POST /api/wizard-stream/world-building/{project_id}/regenerate` | `401 {"detail":"需要登录"}` | 这是最小合法 JSON body 为 `{}` 的 SSE 重生成入口；Python 同样先经过 `get_user_ai_service -> require_login()`，适合作为第二条同组 fallback 线索 |
| `wizard-stream` | `POST /api/wizard-stream/career-system` | `401 {"detail":"需要登录"}` | 这是最小合法 JSON body 为 `{"projectId":"test-project-id"}` 的 SSE 职业体系入口；Python 同样先经过 `get_user_ai_service -> require_login()`，可作为第三条同组 fallback 线索 |
| `wizard-stream` | `POST /api/wizard-stream/characters` | `401 {"detail":"需要登录"}` | 这是最小合法 JSON body 为 `{"projectId":"test-project-id"}` 的 SSE 角色入口；Python 同样先经过 `get_user_ai_service -> require_login()`，可作为第四条同组 fallback 线索 |
| `memories` | `GET /api/memories/projects/{project_id}/stats` | `401 {"detail":"未登录"}` | Python `memories` 走 `verify_project_access()`，未登录优先返回 `未登录` |
| `memories` | `POST /api/memories/projects/{project_id}/search?query=test` | `401 {"detail":"未登录"}` | 这是最小合法查询形态：Python 侧 `query` 是必填 query 参数，而 Rust 侧 body 可保持 `{}`；回切到 Python 后应稳定先停在 `verify_project_access()` 的未登录边界 |

当前明确不能直接复用为 Python 成功条件的 Rust-side probe：

- `GET /api/chapters?project_id=test-project-id`
- `GET /api/settings/models?provider=openai&api_key=test-key&api_base_url=http://127.0.0.1:9/v1`

原因：

- 当前 Rust owner probe 使用的是 query-shape 列表接口
- Python 章节列表主路径是 `/api/chapters/project/{project_id}`，不是同一条路由形态
- 因此如果 `chapters` 回切到 Python，不能直接拿现有列表 probe 的失败来判断
  “Python fallback 不工作”

对于 `settings/models`：

- Rust owner 这条路径会先过共享鉴权，因此未登录直接返回
  `401 {"detail":"未登录，请先登录"}`
- Python 同路径是公开模型列表接口，不经过 `require_login()`，会继续进入 provider
  探测逻辑
- 因此这条路径应进入独立的 `phase5-p0-asymmetric` profile，而不是伪装成
  `phase5-p0-fallback` 的 auth-boundary 证据

这一节现在已经不只是 operator clue：

- 其中一部分最稳定的未登录差异已经进入独立 `phase5-p0-fallback` profile
- 但该 profile 仍然是第一版，只覆盖低前提、低时变的 auth-boundary 线索
- `backend/tools/run_strangler_gateway_smoke.py` 输出到 `tmp/smoke/` 的 JSON
  现已同时附带 `owner_counts`、`route_group_counts` 和
  `route_group_probe_names` 汇总，可直接作为定向 rollback / fallback 验证的
  报告骨架，而不必人工再从 probe 明细里二次归类

当前 `chapters` fallback 已同时覆盖：

- `/api/chapters/project/{project_id}`
- `/api/chapters/{id}/analysis`
- `/api/chapters/analysis/status/batch`
- `/api/chapters/batch-generate/active-tasks`
- `/api/chapters/batch-generate/{batch_id}/stream`
- `/api/chapters/batch-generate/{batch_id}/resume`
- `/api/chapters/{chapter_id}/generate-background`
- `/api/chapters/{id}/regeneration/tasks`

同时，`chapters/batch-generate/{batch_id}/status` 现在进入独立
`phase5-p0-asymmetric`：

- Rust 同路径会先经过共享鉴权，未登录返回
  `401 {"detail":"未登录，请先登录"}`
- Python 同路径当前不读登录态，查询缺失 task 时直接返回
  `404 {"detail":"Batch generation task not found"}`
- 因此这条路径不能伪装成 `phase5-p0-fallback` 的 auth-boundary 证据，而应作为
  `chapters` 第一条章节批量生成状态查询非对称样本

`chapters/batch-generate/{batch_id}/cancel` 现在也进入独立
`phase5-p0-asymmetric`：

- Rust 同路径会先经过共享鉴权，未登录返回
  `401 {"detail":"未登录，请先登录"}`
- Python 同路径当前不检查登录态，缺失 task 时直接返回
  `404 {"detail":"Batch generation task not found"}`
- 因此这条路径也不能伪装成 `phase5-p0-fallback` 的 auth-boundary 证据，而应作为
  `chapters` 第二条章节批量生成写侧非对称样本

---

## 6. 分组执行要点

### 6.1 `settings`

当前 Rust owner location：

- `/api/settings`
- `/api/settings/models`
- `/api/settings/test`
- `/api/settings/fetch-models`
- `presets` 相关路径

当前 P0 治理资产：

- `GET /api/settings`
- `GET /api/settings/api-key`
- `POST /api/settings/fetch-models`
- `POST /api/settings/test`
- `POST /api/settings/check-function-calling`

当前 P0 非对称资产：

- `GET /api/settings/models?provider=openai&api_key=test-key&api_base_url=http://127.0.0.1:9/v1`

回滚原则：

- 优先整组回切 `/api/settings*`
- 不建议只回切某一个 `preset` 子路径，除非已经明确是单子路径回归

### 6.2 `projects`

当前 Rust owner 是若干显式 location，不是完整前缀：

- `/api/projects`
- `/api/projects/{id}`
- `/api/projects/validate-import`
- `export-data` / `check-consistency` / `fix-*`

回滚原则：

- 优先移除这一组显式 Rust location，让它重新落到 Python `/api` catch-all
- 不要只回切单条 `fix-*` 子路由，除非已确认是 isolated regression

### 6.3 `chapters`

当前 Rust owner 范围最大，含 CRUD、analysis、batch、regeneration、SSE。

回滚原则：

- 先按章节大域整体评估，不建议零散回切单条子路径
- 若必须局部回切，至少按以下子域分批：
  - CRUD/read
  - analysis
  - batch generation
  - regeneration

注意：

- `chapters` 对 shared runtime tables 依赖最强，回滚后必须追加任务读写/轮询人工检查

### 6.4 `wizard-stream`

当前 Rust owner 仅覆盖显式 SSE 子路径，仍存在 `/api/wizard-stream/` Python catch-all。

回滚原则：

- 回滚最小动作通常是删除目标 Rust 显式 location，让 residual catch-all 生效
- 这组是最接近“删规则即可回退”的 P0 组

### 6.5 `memories`

当前需区分：

- API 路径 `/api/memories/*` 走 Rust
- 页面/非 API 路径 `/memories/*` 仍走 Python

当前 P0 治理资产：

- `GET /api/memories/projects/{project_id}/stats`
- `POST /api/memories/projects/{project_id}/search?query=test`

回滚原则：

- 只动 `/api/memories*`，不要误改 `/memories/`
- 回滚后要确认 API owner 与页面 owner 没被混淆

---

## 7. 失败升级条件

出现以下任一情况，不要继续扩大回滚面，应转入服务级诊断：

1. gateway owner 已回切，但失败 probe 仍返回 Rust 特征响应
2. 回切后 Python 也无法通过最基本请求
3. 回切单组后，其他 P0 route group 同时漂移
4. `chapters` 回切后出现共享任务状态、checkpoint、resume 语义异常

---

## 8. 操作记录模板

每次执行后至少记录：

1. 时间
2. 环境
3. 回滚 route group
4. 修改的 Nginx 文件与 location
5. 定向 probe 结果
6. 全量 `phase5-p0` 结果
7. 是否需要进一步服务级诊断

---

## 9. 当前结论

截至 2026-05-19，P0 route-group rollback 已经有第一版可执行 runbook，
但仍然存在两个现实限制：

1. 当前 `phase5-p0` probes 主要还是 Rust owner smoke，不是 Python 回切成功
   smoke，因此回切后仍需要补充人工核对或后续新增 Python fallback probe。
2. `chapters` 的更强 business/SSE smoke 仍未稳定，而 `wizard-stream` 已补到 `cleanup` owner probe、但其 Python 回切成功 smoke 仍未成型，因此
   Phase 5 目前更适合先完善 rollback 纪律，而不是贸然推进 fallback 移除。
