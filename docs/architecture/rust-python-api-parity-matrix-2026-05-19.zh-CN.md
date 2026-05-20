# Rust / Python API parity matrix（2026-05-19）

## 1. 目的

本文件用于承接
`docs/architecture/rust-strangler-refactor-plan-2026-05-17.zh-CN.md`
中的 Phase 5 第 1 项：

- 建立 Python API -> Rust API parity matrix

本文档不追求一次性穷尽每一个 endpoint 的字段级行为，而是先建立
“路由组级别”的迁移全景图，回答以下问题：

1. Python 端当前注册了哪些 API route group
2. Rust 端当前已经实现了哪些对应 route group
3. gateway 当前把哪些路径切给了 Rust，哪些仍默认走 Python
4. 哪些 route group 已接近 cutover，哪些仍存在残留 fallback / owner 认知漂移

---

## 2. 证据来源

### 2.1 Python 注册面

- `backend/app/bootstrap/router_registry.py`

### 2.2 Rust 注册面

- `backend-rs/src/api/router.rs`
- `backend-rs/src/api/chapters.rs`

### 2.3 Gateway owner / 切流面

- `deploy/nginx/mumunovel.conf`
- `deploy/nginx/mumunovel-docker.conf`
- `deploy/strangler-gateway-probes.json`

### 2.4 边界设计上下文

- `docs/architecture/chapter-api-gateway-seams.zh-CN.md`
- `docs/architecture/rust-strangler-refactor-plan-2026-05-17.zh-CN.md`

---

## 3. 结论摘要

截至 2026-05-19，当前 strangler API 状态可以概括为：

1. **基础控制面与多数 CRUD/内容域已经具备 Rust 实现。**
2. **gateway 已经把大量 `/api/*` 路径显式切给 Rust，但仍保留 Python catch-all。**
3. **真正的 Phase 5 缺口，已经不再是“Rust 有没有代码”，而是“owner / smoke / rollback / schema assumption 是否成文并可执行”。**
4. **当前最大的风险之一是旧的 gateway 注释和迁移认知已经落后于真实 owner，需要用 smoke 持续校正。**

---

## 4. Route-group parity matrix

说明：

- “Python 注册”表示该 route group 当前仍由 Python backend 注册。
- “Rust 实现”表示 Rust router 中已存在对应能力入口。
- “Gateway owner”表示当前 Nginx 实际默认流量归属，不等同于“代码是否存在”。
- “状态”分为：
  - `rust-owned`：当前主要流量已由 Rust 接管
  - `mixed`：Python/Rust 并存，gateway 按子路径切分
  - `python-owned`：当前默认仍由 Python 承担
  - `rust-only`：仅 Rust 暴露，Python 注册面无对应组

| Route group | Python 注册 | Rust 实现 | 当前 gateway owner | 现状判断 | 关键证据 |
|---|---|---|---|---|---|
| `health / livez / readyz` | 否 | 是 | Rust | `rust-only` | `backend-rs/src/api/router.rs`, `deploy/nginx/mumunovel.conf`; `deploy/strangler-gateway-probes.json` 已覆盖 `/health`、`/livez`、`/readyz`、`/health/db-sessions` |
| `auth` | 是 | 是 | Rust | `rust-owned` | `router_registry.py`, `router.rs`, `mumunovel.conf` |
| `users` | 是 | 是 | Rust | `rust-owned` | 同上 |
| `settings` | 是 | 是 | Rust | `rust-owned` | Python 注册 + Rust merge + Nginx settings 全量切 Rust |
| `admin` | 是 | 是 | Rust | `rust-owned` | `location /api/admin/` 指向 Rust |
| `projects` | 是 | 是 | Rust | `rust-owned` | 当前 Python `projects.py` 注册 API 路径已被 Rust `projects.rs` + gateway 显式规则覆盖，through-gateway 未登录探测也命中 Rust |
| `wizard-stream` | 是 | 是 | Rust | `rust-owned` | 当前 Python `wizard_stream.py` 注册 API 路径已被 Rust `wizard.rs` 覆盖；残留 catch-all 更像过渡 fallback |
| `wizard` | 否（Python 为 `wizard_stream`） | 是 | Python | `python-owned` | Rust 有 `wizard.rs`，但 `location /api/wizard/` 仍指向 Python |
| `inspiration` | 是 | 是 | Rust | `rust-owned` | `/api/inspiration` 已切 Rust |
| `outlines` | 是 | 是 | Rust | `rust-owned` | `/api/outlines` 前缀已切 Rust |
| `characters` | 是 | 是 | Rust | `rust-owned` | CRUD + generation compatibility 已切 Rust |
| `careers` | 是 | 是 | Rust | `rust-owned` | `/api/careers` 已切 Rust |
| `organizations` | 是 | 是 | Rust | `rust-owned` | `/api/organizations` 已切 Rust |
| `relationships` | 是 | 是 | Rust | `rust-owned` | `/api/relationships` 已切 Rust |
| `writing_styles` | 是 | 是 | Rust | `rust-owned` | `/api/writing-styles` 已切 Rust |
| `foreshadows` | 是 | 是 | Rust | `rust-owned` | Nginx 注释已标注“all current endpoints are implemented in Rust” |
| `chapters` | Python 侧拆成多个 `chapter_*` router | 是（聚合到 `chapters.rs`） | Rust | `rust-owned` | `/api/chapters*` 明确切 Rust |
| `chapter_analysis` | 是 | 是 | Rust | `rust-owned` | Python 有 `chapter_analysis_routes` / `chapter_analysis_task_routes`，Nginx `analysis` 路径切 Rust |
| `chapter_batch_generation` | 是 | 是 | Rust | `rust-owned` | `/api/chapters/.../batch-generate*` 切 Rust |
| `chapter_regeneration` | 是 | 是 | Rust | `rust-owned` | regeneration / partial-regenerate 路径切 Rust |
| `chapter_crud` | 是 | 是 | Rust | `rust-owned` | `/api/chapters` CRUD 路径切 Rust |
| `chapter_annotation` | 是 | 间接兼容实现 | Rust | `rust-owned` | `/api/chapters/{id}/annotations` 切 Rust |
| `chapter_quality` | 是 | 间接兼容实现 | Rust | `rust-owned` | `/api/chapters/project/{id}/quality-trend` 切 Rust |
| `chapter_draft` | 是 | 间接兼容实现 | Rust | `rust-owned` | analysis/draft 路径现由 Rust chapter analysis 边界承载 |
| `chapter_expansion_plan` | 是 | 间接兼容实现 | Rust | `rust-owned` | `/api/chapters/{id}/expansion-plan` 切 Rust |
| `memories` | 是 | 是 | Rust | `rust-owned` | `/api/memories/` API 路径已切 Rust；`/memories/` 仍指向 Python，但属于非 API fallback 边界 |
| `mcp_plugins` | 是 | 是 | Rust | `rust-owned` | `/api/mcp*` 已切 Rust |
| `prompt_templates` | 是 | 是 | Rust | `rust-owned` | `/api/prompt-templates*` 已切 Rust |
| `changelog` | 是 | 是 | Rust | `rust-owned` | `/api/changelog*` 已切 Rust |
| `prompt_workshop` | 是 | 是 | Rust | `rust-owned` | `/api/prompt-workshop*` 已切 Rust |
| `background_tasks` | 是 | 是 | Rust | `rust-owned` | `/api/background-tasks*` 已切 Rust |
| `book_import` | 是 | 是 | Rust | `rust-owned` | `/api/book-import*` 已切 Rust |
| `ai_test / ai` | 否 | 是 | Rust | `rust-only` | Python 注册面无对应，Rust + Nginx 已暴露 |
| `polish` | 否 | 是 | Rust | `rust-only` | Python 注册面无对应，Rust + Nginx 已暴露 |

---

## 5. 当前 owner 治理重点组

### 5.1 `projects`

当前状态：

- Python 与 Rust 都有实现
- gateway 只把一部分显式列出的项目路由切给 Rust
- through-gateway 探测表明当前已暴露的 `projects` API 路径实际命中 Rust
- `POST /api/projects/validate-import` 现在已进入 P0 manifest，成为第一条
  同路径 public/business probe：Rust owner 与 Python fallback 都会对同一个
  最小导入文件返回 `200`，但统计字段和 warnings 结构稳定不同
- `POST /api/projects/import` 现在也已进入 P0 manifest，复用同一个最小
  multipart 导入文件，但断言的是合法写侧请求下的 auth boundary：Rust 侧
  命中共享鉴权中间件，Python 侧命中 `request.state.user_id` 登录检查
- `POST /api/projects/{project_id}/export-data` 现在也已进入 P0 manifest，
  使用最小合法 JSON body `{}`，断言合法 JSON 写侧请求下的 auth boundary：
  Rust 侧停在共享鉴权中间件，Python 侧停在 `request.state.user_id` 登录检查

这意味着：

- 旧的 mixed 判断已经落后于真实代码和网关配置
- 该组当前更需要治理的是“陈旧注释/陈旧认知”，不是继续假设 owner 仍未切完
- `projects` 不应再只依赖 `GET /api/projects` 的未登录差异判断 owner；
  `validate-import` 已经提供了更强的同路径成功态 parity / fallback 线索
- `projects/import` 又补上了同路径 multipart 写侧鉴权线索，因此 `projects`
  不再只是“读侧 + public validator”二元证据
- `projects/export-data` 又补上了合法 JSON 写侧鉴权线索，因此 `projects`
  现在具备读侧、public-success、multipart 写侧、JSON 写侧四类 P0 证据

### 5.2 `wizard-stream`

当前状态：

- Rust 已承接 world-building、career-system、characters、outline、cleanup 等子路径
- `/api/wizard-stream/` catch-all 仍保留在 Nginx 中，但对当前已注册 API 路径更像残留 fallback
- `POST /api/wizard-stream/world-building/{project_id}/regenerate` 现在已进入
  P0 manifest，成为第二条同组 SSE auth-boundary probe：最小合法 JSON body
  `{}` 即可稳定落到 Rust 共享鉴权中间件或 Python `get_user_ai_service()`
  登录检查
- `POST /api/wizard-stream/career-system` 现在也进入 P0 owner/fallback
  manifest：最小合法 JSON body `{"projectId":"test-project-id"}` 即可稳定落到
  Rust 共享鉴权中间件或 Python `get_user_ai_service()` 登录检查，因此它比
  `cleanup` 更适合作为真实回切线索
- `POST /api/wizard-stream/characters` 现在也进入 P0 owner/fallback
  manifest：最小合法 JSON body `{"projectId":"test-project-id"}` 即可稳定落到
  Rust 共享鉴权中间件或 Python `get_user_ai_service()` 登录检查，因此它与
  `career-system` 一样适合作为真实回切线索
- `POST /api/wizard-stream/cleanup/{project_id}` 现在也进入 P0 manifest，
  作为第三条同组 SSE/stream-owner probe：Rust 侧同样在共享鉴权中间件先返回
  `401 {"detail":"未登录，请先登录"}`；而 Python 侧当前并不存在同路径路由，
  所以它更适合作为 owner 收口证据，而不是伪装成 `phase5-p0-fallback` 的
  auth-boundary 线索

这意味着：

- 当前更适合把它视为 `rust-owned with stale fallback config`
- Phase 5 后续应评估是否可以删除或收紧这条 catch-all
- `wizard-stream` 不应再只靠 `/outline` 单路径来判断 owner；regenerate 入口
  已能提供第二条更接近真实世界观工作流的 through-gateway 线索

### 5.3 `memories`

当前状态：

- API 路由 `/api/memories/` 已走 Rust
- 非 `/api` 的 `/memories/` 路径仍走 Python
- `memories-stats-auth-guard-rust` 已证明 stats 读侧入口当前命中 Rust
- `memories-search-auth-guard-rust` 现已补入，证明 `search` 查询入口也命中 Rust

这意味着：

- 需要区分 API owner 与页面/非 API fallback owner
- 不应再把 `memories` API 组本身标成 mixed

---

## 6. 当前 smoke 覆盖与空白

当前唯一进入结构化 gateway smoke manifest 的 probes 只有：

1. `rust-health`
2. `rust-readiness`
3. `python-fallback-root`

这足以证明：

- Rust control-plane path 可达
- Python fallback 根路径可达

但还不足以证明：

- `projects` 这类当前已 Rust-owned 的组在后续变更后不会重新漂移
- `wizard-stream` 残留 catch-all 不会重新接管已在 Rust 的路径
- `chapters` 大域的关键兼容接口在 gateway 层是否持续保持 Rust owner

补充：

- 当前 `route-groups` smoke 已经覆盖 `chapters` 的列表、analysis、batch analysis status、batch active tasks、regeneration tasks 五条低前提 owner probe
- 这足以证明章节大域在 through-gateway 未登录边界上保持 Rust owner，但还不足以替代后续更强的业务/stream smoke
- 当前还新增了独立 `phase5-p0` profile，把 `projects`、`wizard-stream`、
  `chapters`、`settings`、`memories` 五组 P0 route-group 从
  `route-groups` 大集合里拆出来，便于按 Phase 5 优先级单独执行和追踪
- `projects` 现在还新增 `projects-validate-import-public-rust`，把
  `POST /api/projects/validate-import` 纳入 `phase5-p0` 与 `business`
  profile；这是目前第一条 P0 stronger public/business owner probe
- `projects-import-auth-guard-rust` 现在也补入 `phase5-p0`，它使用同一个最小
  multipart 文件，但稳定落在 Rust 鉴权中间件 `401 {"detail":"未登录，请先登录"}`
  上，可补足 `projects` 写侧 owner 线索
- `projects-export-data-auth-guard-rust` 现在也补入 `phase5-p0`，它使用最小
  JSON body `{}`，稳定落在同一个 Rust 鉴权中间件边界上，可补足 `projects`
  的 JSON 写侧 owner 线索
- `wizard-stream-world-building-regenerate-auth-guard-rust` 现在也补入
  `phase5-p0`，它使用最小 JSON body `{}`，稳定落在 Rust 鉴权中间件
  `401 {"detail":"未登录，请先登录"}` 上，可补足 `wizard-stream` 的第二条
  SSE owner 线索
- `wizard-stream-cleanup-auth-guard-rust` 现在也补入 `phase5-p0`：它使用
  最小 JSON body `{}`，稳定落在同一个 Rust 鉴权中间件
  `401 {"detail":"未登录，请先登录"}` 上，并把 `wizard-stream` 的 owner
  证据扩到第三条显式 SSE/cleanup 子路径
- `wizard-stream-career-system-auth-guard-rust` 现在也补入 `phase5-p0`：
  它使用最小合法 JSON body `{"projectId":"test-project-id"}`，稳定落在同一个
  Rust 鉴权中间件 `401 {"detail":"未登录，请先登录"}` 上，并把
  `wizard-stream` 的真实 owner/fallback 交集扩到第三条同路径 SSE 入口
- `wizard-stream-characters-auth-guard-rust` 现在也补入 `phase5-p0`：
  它使用最小合法 JSON body `{"projectId":"test-project-id"}`，稳定落在同一个
  Rust 鉴权中间件 `401 {"detail":"未登录，请先登录"}` 上，并把
  `wizard-stream` 的真实 owner/fallback 交集扩到第四条同路径 SSE 入口
- `chapters-batch-status-auth-guard-rust-asymmetric` 与
  `chapters-batch-status-task-not-found-python-fallback` 现在也补入
  `phase5-p0-asymmetric`：同一路径下，Rust 会先停在共享鉴权并返回
  `401 {"detail":"未登录，请先登录"}`，而 Python 当前状态查询路由不读登录态，
  在缺失 task 时直接返回
  `404 {"detail":"Batch generation task not found"}`；这条路径因此不能误写成
  `phase5-p0-fallback` 的 auth-boundary 证据
- `chapters-batch-cancel-auth-guard-rust-asymmetric` 与
  `chapters-batch-cancel-task-not-found-python-fallback` 现在也补入
  `phase5-p0-asymmetric`：同一路径下，Rust 会先停在共享鉴权并返回
  `401 {"detail":"未登录，请先登录"}`，而 Python 当前取消路由不检查登录态，
  在缺失 task 时直接返回
  `404 {"detail":"Batch generation task not found"}`；这条路径同样不能误写成
  `phase5-p0-fallback` 的 auth-boundary 证据
- `chapters-batch-stream-auth-guard-rust` 与
  `chapters-batch-stream-auth-guard-python-fallback` 现在也补入
  `phase5-p0` / `phase5-p0-fallback`：同一路径下，Rust 会先停在共享鉴权并返回
  `401 {"detail":"未登录，请先登录"}`，Python stream access 校验则在
  `request.state.user_id` 缺失时返回 `401 {"detail":"未登录"}`，因此它是
  同路径 SSE 查询入口的真实 fallback 线索，而不是 asymmetric
- `memories-search-auth-guard-rust` 现在也补入 `phase5-p0`：它使用
  `?query=test` 加最小 JSON body `{}`，既满足 Python fallback 的 transport
  形态，也稳定落在 Rust 鉴权中间件 `401 {"detail":"未登录，请先登录"}`
  上，可补足 `memories` 的第二条查询侧 owner 线索
- `settings` 在 `phase5-p0` 中也已从根路径未登录探针扩到 `/api/settings/api-key`、
  `/api/settings/models`、`/api/settings/fetch-models`、`/api/settings/test` 与
  `/api/settings/check-function-calling` 子路由 owner probe，说明该组开始进入
  读写与业务探测子路由级资产化，而不再只停留在 `/api/settings` 单点
- `settings-api-key-auth-guard-python-fallback` 现在也补入
  `phase5-p0-fallback`：同路径下，Python 应稳定返回
  `401 {"detail":"需要登录"}`，可证明回切后的已保存 API key 读取入口也已回到
  Python，而不是只靠根路径或 provider 探测子路由来判断 owner
- `settings-fetch-models-auth-guard-python-fallback` 现在也补入
  `phase5-p0-fallback`：同一路径、同样最小 JSON body 下，Python 应稳定
  返回 `401 {"detail":"需要登录"}`，可证明回切后的模型列表探测入口已回到
  Python，而不是只靠 `/api/settings` 根路径做单点判断
- `settings-test-auth-guard-python-fallback` 现在也补入
  `phase5-p0-fallback`：同一路径、最小连接测试 JSON body 下，Python 应稳定
  返回 `401 {"detail":"需要登录"}`，可证明回切后的连接测试入口也已回到
  Python，而不会先进入外部 API 测试逻辑
- `settings-check-function-calling-auth-guard-python-fallback` 现在也补入
  `phase5-p0-fallback`：同一路径、最小 Function Calling 探测 JSON body 下，
  Python 应稳定返回 `401 {"detail":"需要登录"}`，可证明回切后的工具调用
  探测入口也已回到 Python，而不会先进入能力探测逻辑
- `settings/models` 现进入独立的 `phase5-p0-asymmetric` profile：
  Rust owner 侧在同路径查询参数下稳定停在共享鉴权
  `401 {"detail":"未登录，请先登录"}`，但 Python fallback 侧不会先停在登录边界，
  而是继续进入公开模型列表逻辑，并在不可达 `api_base_url=http://127.0.0.1:9/v1`
  下稳定返回 `400 {"detail":"无法连接到 API: All connection attempts failed"}`
  这类路径需要单独建模，不能误写成 `phase5-p0-fallback` 的 auth-boundary 证据
- `phase5-p0-fallback` 现在也补入
  `projects-validate-import-public-python-fallback`：同一路径下，Python 返回
  `organization_members` / `character_careers` / `story_memories` /
  `has_default_style=false` 与两条空项目 warnings，可作为比 `401` 更强的
  public-success fallback 线索
- `projects-import-auth-guard-python-fallback` 现在也补入
  `phase5-p0-fallback`：同一路径、同文件形态下，Python 应稳定返回
  `401 {"detail":"未登录"}`，可证明回切后的 multipart 写侧入口已回到 Python
- `projects-export-data-auth-guard-python-fallback` 现在也补入
  `phase5-p0-fallback`：同一路径、最小 JSON body 下，Python 应稳定返回
  `401 {"detail":"未登录"}`，可证明回切后的 JSON 写侧入口已回到 Python
- `wizard-stream-world-building-regenerate-auth-guard-python-fallback`
  现在也补入 `phase5-p0-fallback`：同一路径、最小 JSON body 下，Python 应
  稳定返回 `401 {"detail":"需要登录"}`，可证明回切后的世界观重生成 SSE
  入口已回到 Python
- `memories-search-auth-guard-python-fallback` 现在也补入
  `phase5-p0-fallback`：同一路径、同样带 `?query=test` 与 `{}` body 时，
  Python 应稳定返回 `401 {"detail":"未登录"}`，可证明回切后的查询入口已
  回到 Python，而不是停留在缺少必填参数的 transport 422
- 现在还新增了第一版 `phase5-p1` profile，先承载 `auth` 与 `users` 的
  最小 owner smoke：公开 `auth config` 读取、公开 `logout` 动作、
  LinuxDO URL 未配置分支、`/api/auth/callback` 缺参 public error，
  以及 `/api/auth/local/login` 与 `/api/auth/bind/login`
  错误凭证 public failure，
  以及受保护 `/api/auth/user` 与
  `/api/auth/password/status` 读侧，以及 `users/current`、`users`、
  `users/set-admin` 与 `users/reset-password` 的未登录鉴权边界
- `phase5-p1` 现已继续扩到 `characters`、`outlines` 与 `book_import`
  的 starter owner smoke：
  `GET /api/characters/project/{project_id}`、
  `GET /api/outlines/project/{project_id}` 与
  `GET /api/book-import/tasks/{task_id}` 三条 through-gateway 未登录边界
  现在也能直接证明请求仍命中 Rust owner
- 这三组现在又各补了一条更贴近真实读侧的同路径 probe：
  `GET /api/characters?project_id=...`、
  `GET /api/outlines?project_id=...` 与
  `GET /api/book-import/tasks/{task_id}/preview`
- `characters` 与 `outlines` 现在还分别补入了第一条生成流写侧 probe：
  `POST /api/characters/generate-stream` 与
  `POST /api/outlines/generate-stream`
- `characters` 现在还补入了两条更贴近导入导出治理边界的写侧 probe：
  `POST /api/characters/export` 与
  `POST /api/characters/import`
- `outlines` 现在还补入了两条更贴近真实大纲工作流的写侧 probe：
  `POST /api/outlines/batch-expand-stream` 与
  `POST /api/outlines/{outline_id}/create-chapters-from-plans`
- `book_import` 现在还补入了第一条不依赖请求体的轻量写侧 probe：
  `DELETE /api/book-import/tasks/{task_id}`
- `book_import` 现在还补入了 multipart 上传创建边界 probe：
  `POST /api/book-import/tasks`
- `book_import` 现在还补入了两条带最小合法 JSON 的提交边界 probe：
  `POST /api/book-import/tasks/{task_id}/apply` 与
  `POST /api/book-import/tasks/{task_id}/retry-stream`
- `book_import` 现在还补入了对应的 SSE 提交边界 probe：
  `POST /api/book-import/tasks/{task_id}/apply-stream`
- smoke 工具本身现已支持 per-probe `headers`、`json_body/body`、`expected_text_startswith`、`expected_text_contains`，因此后续可以把一部分 `wizard-stream` / `chapters` SSE 与业务边界验证纳入同一 manifest，而不必另起一套脚本
- smoke 工具现在还支持 `expected_header_contains`，因此像 `auth/logout`
  这种需要确认清 cookie 响应头存在的业务动作，也可以进入结构化 probe
- runner 现在还会保留重复响应头的多值内容，因此 `Set-Cookie` 这类多 header
  响应不会在采集阶段丢失后续值
- 现有 `POST /api/chapters/analysis/status/batch` 与 `POST /api/wizard-stream/outline` probe 已开始发送最小 JSON body，这让 owner 证据更贴近真实调用形态
- manifest 现在新增了 `business` profile，用来承载公开 JSON/HTML 业务断言，不必和默认 `deploy` profile 混在一起
- `deploy` profile 现在还覆盖 `/livez` 和 `/health/db-sessions`，所以健康态 smoke 已从单一健康检查扩展为更完整的 Rust 控制面固定 JSON 证据链
- `expected_json_has_keys` 现在可以替代时变字段值断言，用来稳定描述像 `/api/changelog` 这类返回体结构固定但具体值会变化的接口

因此，Phase 5 后续必须补 route-group 级 smoke，而不能继续只停留在 health probe。

---

## 7. 对 Phase 5 的直接建议

### 7.1 第一优先级

优先为下列 route group 建立显式 owner + smoke：

1. `projects`
2. `wizard-stream`
3. `chapters`
4. `settings`
5. `memories`

当前执行资产：

- 以上五组已进入 `phase5-p0` smoke profile，可作为 Phase 5 第一波治理
  切片的独立验证入口

### 7.2 第二优先级

为已经基本 Rust-owned 的中低风险组补“回滚方式”文档化：

1. `auth`
2. `users`
3. `outlines`
4. `characters`
5. `book_import`

当前进展：

- P0 route group（`settings` / `projects` / `chapters` / `wizard-stream` /
  `memories`）已新增第一版 rollback runbook：
  `docs/architecture/rust-phase5-p0-route-group-rollback-runbook-2026-05-19.zh-CN.md`
- smoke 工具已支持 `--route-group` 定向执行单个 route group；必要时仍可叠加
  `--probe-name` 精准执行单条 probe，便于 owner 回切后的最小验证
- runbook 现已补入第一版 Python fallback success clues，说明这五组回切后
  应优先观察到哪些未登录语义或 SSE 入口差异
- 其中最稳定的一部分回切信号现已进入独立 `phase5-p0-fallback` profile，
  可执行第一版 Python fallback smoke
- `settings` 现在也不再只拥有根路径 fallback 线索；`fetch-models` 已加入
  `phase5-p0-fallback`，使该组开始拥有真实子路由级回切证据
- `settings/api-key` 现在也加入 `phase5-p0-fallback`，因此 `settings` 已开始
  同时拥有根路径、已保存凭据读取和 provider 探测子路由三类低前提回切线索
- `settings/test` 现在也加入 `phase5-p0-fallback`，因此 `settings` 已具备
  根路径 + 模型拉取 + 连接测试三条回切线索，而不再是单一子路由补丁
- `settings/check-function-calling` 现在也加入 `phase5-p0-fallback`，因此
  `settings` 这组的探测子路由回切证据已基本形成完整小矩阵
- `projects` 现在已同时拥有 `GET /api/projects` 的 auth-boundary fallback
  线索与 `POST /api/projects/validate-import` 的 public-success fallback 线索，
  因而不再只是“回切后人工目测”的组
- 现在又补入 `POST /api/projects/import` 的 multipart auth-boundary 线索，
  使 `projects` 在 P0 里拥有读侧、public-success、写侧 multipart 三类可执行
  fallback 证据
- 现在又补入 `POST /api/projects/{project_id}/export-data` 的 JSON auth-boundary
  线索，使 `projects` 在 P0 里拥有读侧、public-success、multipart 写侧、
  JSON 写侧四类可执行 fallback 证据
- `memories` 现在也从单一 `/stats` fallback 线索扩到
  `/stats + /search?query=test` 双 probe，说明该组不再只是单路径 owner 判断
- `wizard-stream-career-system-auth-guard-python-fallback` 现在也补入
  `phase5-p0-fallback`：同一路径、同样最小合法 JSON body 下，Python 应稳定
  返回 `401 {"detail":"需要登录"}`，使 `wizard-stream` 不再只靠
  `outline + world-building/regenerate` 两条路径来证明回切后的 SSE owner
- `wizard-stream-characters-auth-guard-python-fallback` 现在也补入
  `phase5-p0-fallback`：同一路径、同样最小合法 JSON body 下，Python 应稳定
  返回 `401 {"detail":"需要登录"}`，使 `wizard-stream` 的真实回切线索进一步
  扩到第四条同路径 SSE 入口
- `wizard-stream/cleanup/{project_id}` 目前仍不进入 `phase5-p0-fallback`：
  Rust 有显式同路径实现，但 Python 当前没有对应 API 路由，因此这条路径更像
  “Rust owner + Python 缺路由”的收口证据，不能误写成 Python auth-boundary
  fallback 成功条件
- `chapters` 的 Python fallback 现在已同时覆盖 project-path 列表、
  analysis、batch status、active-tasks、regeneration tasks 五类读侧入口
- P1 route group（`auth` / `users`）现已新增第一版 rollback runbook：
- P1 route group（`auth` / `users`）现已新增第一版 rollback runbook：
  `docs/architecture/rust-phase5-p1-route-group-rollback-runbook-2026-05-20.zh-CN.md`
- 其中 `auth` 已进入独立 `phase5-p1-fallback` profile，覆盖
  `POST /api/auth/logout`、`GET /api/auth/user` 与
  `GET /api/auth/password/status`，现在还继续补入
  `POST /api/auth/password/set` 与
  `POST /api/auth/password/initialize`，以及
  `POST /api/auth/refresh`，以及 `GET /api/auth/callback`
  缺参 public error，以及 `POST /api/auth/local/login` 与
  `POST /api/auth/bind/login` 错误凭证 public failure
  这六类 Python fallback 低前提信号
- `users-current-auth-guard-python-fallback` 与
  `users-list-auth-guard-python-fallback` 现在也补入 `phase5-p1-fallback`：
  同路径下，Python 应稳定返回 `401 {"detail":"需要登录"}`，可证明回切后的
  `users/current` 与 `users` 入口已回到 Python 登录边界
- `users-set-admin-auth-guard-python-fallback` 与
  `users-reset-password-auth-guard-python-fallback` 现在也补入
  `phase5-p1-fallback`：同路径、最小合法 JSON body 下，Python 应稳定返回
  `401 {"detail":"需要登录"}`，可证明回切后的 `users` 写侧入口也已回到
  Python 登录边界
- `characters` / `outlines` / `book_import` 现也已进入 `phase5-p1-fallback`
  profile，分别覆盖
  `GET /api/characters/project/{project_id}`、
  `GET /api/outlines/project/{project_id}` 与
  `GET /api/book-import/tasks/{task_id}` 三条同路径、低前提的 Python fallback
  未登录差异信号
- 现在这三组又各补了第二条 fallback 读侧入口：
  `GET /api/characters?project_id=...`、
  `GET /api/outlines?project_id=...` 与
  `GET /api/book-import/tasks/{task_id}/preview`
- `characters` 与 `outlines` 现在还补入了同路径 fallback 的生成流入口：
  `POST /api/characters/generate-stream` 与
  `POST /api/outlines/generate-stream`
- `characters` 现在还补入了两条同路径 fallback 的导入导出入口：
  `POST /api/characters/export` 与
  `POST /api/characters/import`
- `outlines` 现在还补入了两条同路径 fallback 的工作流写侧入口：
  `POST /api/outlines/batch-expand-stream` 与
  `POST /api/outlines/{outline_id}/create-chapters-from-plans`
- `book_import` 现在还补入了对应的 fallback 写侧入口：
  `DELETE /api/book-import/tasks/{task_id}`
- `book_import` 现在还补入了同路径 fallback 的 multipart 上传创建入口：
  `POST /api/book-import/tasks`
- `book_import` 现在还补入了同路径 fallback 的 SSE 提交边界入口：
  `POST /api/book-import/tasks/{task_id}/apply-stream`
- `users` 现在已进入 fallback profile；但这两条 probe 仍只证明 same-path
  auth-boundary ownership，不等于证明管理员列表或当前用户业务语义完全等价
- `users` 现在也开始拥有写侧 fallback probe；但它们同样只证明 same-path
  auth-boundary ownership，不等于证明管理员写侧业务语义完全等价
- `characters/validate-import` 现进入独立的 `phase5-p1-asymmetric` profile：
  Rust 侧是公开文件校验入口，Python 侧仍要求登录；这类同路径非对称接口需要
  单独建模，不能伪装成 `phase5-p1-fallback` 的 auth-boundary 证据
- `characters` / `outlines` / `book_import` 现在已同时拥有 starter owner
  smoke 与第一版 fallback profile；短期内仍应优先把列表/任务状态这类低前提
  路径稳定下来，再决定是否值得继续向上传、成功态导入流或更重的 SSE 路径 probe 化

### 7.3 暂不建议的动作

当前不建议直接：

- 删除 Python `/api/` catch-all
- 宣布“已完成去 Python 化”
- 在没有 smoke / rollback 证据链前，直接删除残留 fallback 配置或宣布 Python 可完全退场

---

## 8. 阶段结论

截至 2026-05-19，可以给出如下结论：

1. **Rust API parity 已经在 route-group 粒度上达到较高覆盖。**
2. **当前最大缺口是 owner 治理，而不是代码存在性。**
3. **Phase 5 应该把重点放在 route-group smoke、owner 文档和 rollback 纪律，而不是继续只统计“又迁了多少接口”。**
4. **P0 rollback 资产已经开始从表格式说明转成可执行 runbook，这说明
   Phase 5 正在进入真正的运维治理阶段，但还未达到移除 Python fallback 的条件。**
5. **当前 Python fallback 成功条件已经不再只是文档化：P0 与 P1 都开始拥有
   独立 fallback smoke profile。**
6. **但这种自动化仍是分组分层推进的，不代表所有 Rust-owned 组都已具备同等级
   的 fallback 证据链；`users` 虽然现在已同时拥有读写 owner/fallback probe，
   但其业务语义等价性仍需和 auth-boundary owner 线索区分处理。**
7. **`characters` / `outlines` / `book_import` 目前已经从“只有 owner smoke”
   前进到“同路径 fallback smoke + owner smoke”的层级，并开始覆盖 path/query
   两种列表读侧、`characters generate-stream / export / import`、
   `outlines generate-stream / batch-expand-stream / create-chapters-from-plans`
   与 `book_import create/apply/cancel/retry/apply-stream` 这类未登录提交边界；
   但仍然不代表生成流、导入流或 SSE 成功路径已经具备同等级证据链。**
