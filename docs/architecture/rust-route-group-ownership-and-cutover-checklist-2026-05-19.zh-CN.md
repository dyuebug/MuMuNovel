# Rust route-group ownership and cutover checklist（2026-05-19）

## 1. 目的

本文件承接
`docs/architecture/rust-strangler-refactor-plan-2026-05-17.zh-CN.md`
中的 Phase 5 第 2 项：

- 给每个 route group 标记 owner / smoke / rollback / schema assumptions

该文档的目标不是替代 Nginx 配置，而是把当前 strangler 迁移进入
Phase 5 之后最容易漂移的治理信息固化下来：

1. 当前谁是 owner
2. 需要什么 smoke 才能证明 owner 成立
3. 失败时如何回滚
4. 该组对 shared DB / schema 的前提假设是什么

---

## 2. 使用原则

### 2.1 owner 的定义

owner 指当前 **through-gateway 默认流量** 的归属，而不是“谁有代码”。

### 2.2 smoke 的定义

smoke 指能在部署后快速证明：

- 请求被路由到了预期 owner
- 返回结构没有明显回退
- 关键读写链路在 shared DB 下仍工作

### 2.3 rollback 的定义

rollback 指在不改业务数据语义的前提下，把 route group 流量切回 Python 的最小动作。

---

## 3. Route-group ownership checklist

| Route group | 当前 owner | 最低 smoke 要求 | 回滚方式 | schema / runtime assumptions | 当前判断 |
|---|---|---|---|---|---|
| `health / livez / readyz` | Rust | 保持现有 `rust-health` / `rust-livez` / `rust-readiness` probe；另补 `rust-health-db-sessions` 作为结构化 DB 统计 smoke | Nginx health path 指回 Python 或临时下线 Rust health gate | Rust 启动链、DB readiness、config bootstrap 正常 | 已成型 |
| `auth` | Rust | 已补公开 `config` 读取、`logout` 动作、`linuxdo/url` 未配置分支、`callback` 缺参 public error、`local/login` 与 `bind/login` 错误凭证 public failure，以及 `/user`、`/password/status`、`/password/set`、`/password/initialize`、`/refresh` 五条登录边界；后续再补真实登录态建立与 callback 成功分支 smoke | `/api/auth*` 指回 Python | JWT secret、cookie policy、public path policy 已与 Phase 4 收口一致；`local/login` probe 还依赖 `local_auth_enabled=true` 的当前部署前提，而 `bind/login` 不需要这一前置检查 | 已有第一版 owner/fallback 资产，仍需更强业务 smoke |
| `users` | Rust | 已补 `/api/users` 列表读侧与 `/api/users/current` 两条未登录鉴权 probe；后续再补管理员权限成功/403 分支等更强 smoke | `/api/users*` 指回 Python；但需额外确认前端当前用户链路是否改回 `/api/auth/user` | users 表字段语义保持兼容；`users/current` 与 Python 当前用户路径不完全同构 | 已有第一版 smoke，fallback 仍需谨慎 |
| `settings` | Rust | 已补根路径读取、`/api-key`、`/presets`、preset 创建/更新/删除/from-current/activate/test、`/models`、`/fetch-models`、`/test` 与 `/check-function-calling` 子路由 owner probe；2026-06-02/03 已补登录态 business smoke 与 probe transport owner 收口；2026-06-07 起，gateway `/api/settings` 已改为 Rust exact-root + shared-prefix 收口，active same-path Python fallback / business-fallback / models asymmetric probes 已退役 | 回退方式是删除或改写 Rust `/api/settings/` 前缀规则，并视需要把 `/api/settings` root 与前缀交还 Python `/api/` catch-all 或临时 Python prefix | `settings` 表默认值语义已完成第一轮收口；API key / preferences 仍允许为空；`presets` 仍存放于 `preferences` JSON，不引入新 schema owner；`settings/models` 的 Python public network-error 线索仅在显式 gateway rollback 后复用；真实 business smoke 仍依赖 `LOCAL_AUTH_USERNAME / LOCAL_AUTH_PASSWORD` 环境前提 | 已完成 same-path gateway cutover，后续补 provider-success / transport stronger smoke |
| `projects` | Rust | 已补列表、详情、基础 CRUD、TXT/JSON 导出、`validate-import` public/business、`import` multipart，以及 `check-consistency` / `fix-organizations` / `fix-member-counts` owner probe；2026-06-07 起，gateway `/api/projects` 已改为 Rust exact-root + shared-prefix 收口，active same-path Python fallback probes 已退役 | 回退方式是删除或改写 Rust `/api/projects/` 前缀规则，并视需要把 `/api/projects` root 与前缀交还 Python `/api/` catch-all 或临时 Python prefix | `projects` 表字段仍沿用 Python schema owner；当前更应警惕旧注释/旧认知与真实 owner 不一致；历史 Python `validate-import` public-success、`import` / `export-data` / CRUD / 维护类 `401 {"detail":"未登录"}` 线索仅在显式 gateway rollback 后复用；维护类入口仍依赖 `organization_members` / relationships 等共享表语义 | 已完成 same-path gateway cutover，后续补登录态 stronger smoke |
| `wizard-stream` | Rust | 现已补 `POST /api/wizard-stream/outline`、`POST /api/wizard-stream/world-building`、`POST /api/wizard-stream/world-building/{project_id}/regenerate`、`POST /api/wizard-stream/career-system`、`POST /api/wizard-stream/characters` 与 `POST /api/wizard-stream/cleanup/{project_id}` 六条 through-gateway Rust owner probes；2026-06-07 起，同路径 Python fallback probes 已退役，因为 gateway `wizard-stream` 前缀已直接切 Rust | 回退方式是删除或改写 Rust `/api/wizard-stream/` 前缀规则，让 `/api/` catch-all 或临时 Python prefix 接管 | 流式 SSE 行为、keep-alive、模型访问配置兼容；当前 probes 仍主要证明登录边界，不代表 SSE 业务成功态已完整等价 | 已完成 same-path gateway cutover，后续补 stronger smoke |
| `inspiration` | Rust | 已补 `POST /api/inspiration/generate-options` 与 `POST /api/inspiration/quick-generate` 两条低前提 Rust owner probe；2026-06-07 起 active same-path Python fallback probes 已退役；后续再补 refine-options 与登录态 AI 生成成功/失败分支 stronger smoke | 只在显式 gateway rollback 时恢复 `/api/inspiration*` 到 Python | 依赖配置、AI provider、prompt template 与可选 web research 语义稳定；当前 Rust 组无 GET 列表入口 | P1 cutover 已完成，下一步转向 stronger smoke |
| `outlines` | Rust | CRUD + expand/create-chapters 中至少 1 条 | `/api/outlines*` 指回 Python | outlines 表 / compat 路径保持兼容 | 可补 smoke 后固化 |
| `characters` | Rust | CRUD + generate-stream 至少各 1 条 | `/api/characters*` 指回 Python | characters 表默认值与生成兼容字段保持一致 | 可补 smoke 后固化 |
| `careers` | Rust | 已补 `GET /api/careers?project_id=...` 与 `GET /api/careers/generate-system?project_id=...` 两条低前提 Rust owner probe；2026-06-07 起 active same-path Python fallback probes 已退役；后续再补职业 CRUD、角色职业绑定和生成成功态 stronger smoke | 只在显式 gateway rollback 时恢复 `/api/careers*` 到 Python | careers / character link 表兼容；`generate-system` 还依赖 AI 配置、SSE 与职业落库语义 | P1 cutover 已完成，下一步转向 stronger smoke |
| `organizations` | Rust | 已补 `GET /api/organizations/project/{project_id}` 与 `POST /api/organizations/generate-stream` 两条低前提 Rust owner probe；2026-06-07 起 active same-path Python fallback probes 已退役；后续再补组织详情、成员列表、成员增删改和生成成功态 stronger smoke | 只在显式 gateway rollback 时恢复 `/api/organizations*` 到 Python | `organization_members` 字段级一致性仍需持续关注；生成流还依赖角色组织化、成员计数与 generation_history 语义 | P1 cutover 已完成，下一步转向 stronger smoke |
| `relationships` | Rust | 已补 `GET /api/relationships/project/{project_id}` 与 `GET /api/relationships/graph/{project_id}` 两条低前提 Rust owner probe；2026-06-07 起 active same-path Python fallback probes 已退役；后续再补类型列表 public 读取、创建/更新/删除成功态等更强 smoke | 只在显式 gateway rollback 时恢复 `/api/relationships*` 到 Python | 关系表兼容；graph 还依赖 `organization_members` 与角色节点 join 语义 | P1 cutover 已完成，下一步转向 stronger smoke |
| `writing_styles` | Rust | 已补 `GET /api/writing-styles/user` 与 `GET /api/writing-styles/project/{project_id}` 两条低前提 Rust owner probe；2026-06-07 起 active same-path Python fallback probes 已退役；后续再补 presets public list、创建/更新/删除、set-default、init-defaults 等 stronger smoke | 只在显式 gateway rollback 时恢复 `/api/writing-styles*` 到 Python | styles/preset 数据结构兼容；project default style 仍依赖 `project_default_styles` 与用户自定义风格语义 | P1 cutover 已完成，下一步转向 stronger smoke |
| `foreshadows` | Rust | 已补 `GET /api/foreshadows/projects/{project_id}` 与 `GET /api/foreshadows/projects/{project_id}/stats` 两条低前提 Rust owner probe；2026-06-07 起 active same-path Python fallback probes 已退役；后续再补 context、pending-resolve、plant/resolve/abandon 写侧和登录态业务 smoke | 只在显式 gateway rollback 时恢复 `/api/foreshadows*` 到 Python | foreshadows 表已在 Alembic 覆盖；章节生成上下文依赖 chapter_number 与 pending/overdue 查询语义 | P1 cutover 已完成，下一步转向 stronger smoke |
| `chapters` | Rust | 已补列表、analysis、batch analysis status、batch active tasks、batch stream、batch resume、single generate-background、single generate-stream、regeneration tasks 九条 through-gateway probe；现在还新增 `batch-generate/{batch_id}/status` 与 `batch-generate/{batch_id}/cancel` 两条 P0 asymmetric 样本；下一步再补更强的 business smoke | `/api/chapters*` 指回 Python | 章节域使用 shared DB；任务语义、checkpoint、SSE event shape 已在 Phase 3/4 前后持续收口 | Phase 5 最高优先级之一 |
| `memories` | Rust | 已补 `stats`、`memories` 列表、`analysis/{chapter_id}`、`foreshadows`、`search`、`chapters/{chapter_id}/memories` 删除六条 `/api/memories/projects/{project_id}` through-gateway probe，确认 API 侧仍命中 Rust，且覆盖读、查、删三类入口；2026-06-07 起 active same-path Python fallback probes 已退役 | 只在显式 gateway rollback 时恢复 `/api/memories*` 到 Python；不要误动 `/memories/` 页面/非 API fallback | 需区分 API owner 与页面/非 API fallback owner；记忆 API 仍依赖 shared DB / vector memory 侧效应 | Phase 5 重点，API cutover 已完成，下一步转向 business smoke |
| `mcp_plugins` | Rust | 已补 `GET /api/mcp/plugins` 与 `POST /api/mcp/plugins/simple` 两条低前提 Rust owner probe；2026-06-07 起 active same-path Python fallback probes 已退役；后续再补插件详情、toggle/status/tools/test/call 和登录态创建成功/失败 stronger smoke | 只在显式 gateway rollback 时恢复 `/api/mcp*` 到 Python | mcp_plugins 表兼容；插件注册/断开还依赖 MCP client session 与后台任务语义 | P1 cutover 已完成，下一步转向 stronger smoke |
| `prompt_templates` | Rust | 已补 `GET /api/prompt-templates` 与 `GET /api/prompt-templates/system-defaults` 两条低前提 Rust owner probe；2026-06-07 起 active same-path Python fallback probes 已退役；后续再补 categories、sync-status、保存/删除/导入/预览 stronger smoke | 只在显式 gateway rollback 时恢复 `/api/prompt-templates*` 到 Python | prompt_templates 表兼容；managed template sync、系统默认模板和 preview 参数替换语义需继续对齐 | P1 cutover 已完成，下一步转向 stronger smoke |
| `prompt_workshop` | Rust | 已补 `POST /api/prompt-workshop/submit` 与 `POST /api/prompt-workshop/items/{item_id}/like` 两条低前提 Rust owner probe；2026-06-07 起 active same-path Python fallback probes 已退役；后续再补公开 items/status、import/download/my-submissions/admin 和登录态业务 stronger smoke | 只在显式 gateway rollback 时恢复 `/api/prompt-workshop*` 到 Python | prompt workshop 三张表 Alembic 已覆盖；该组公开接口与登录接口混合，不能把未登录 probe 误读为公开列表等价 | P1 cutover 已完成，下一步转向 stronger smoke |
| `background_tasks` | Rust | 已补 `GET /api/background-tasks` 与 `POST /api/background-tasks` 两条低前提 owner/fallback probe；2026-06-07 起 active same-path Python fallback probes 已退役；后续再补 status/stream/cancel/workflow-state 和真实任务生命周期 stronger smoke | 只在显式 gateway rollback 时恢复 `/api/background-tasks*` 到 Python | shared task registry / stream hub 正常；SSE keep-alive 与任务缺失 payload 仍需后续验证 | P1 cutover 已完成，下一步转向 stronger smoke |
| `book_import` | Rust | 任务创建 + apply/retry-stream 至少各 1 条 | `/api/book-import*` 指回 Python | 导入工作流与 background task 兼容 | 可补 smoke |
| `changelog` | Rust | 已补 `GET /api/changelog` 与 `POST /api/changelog/refresh` 两条 public Rust owner probe；2026-06-07 起 active same-path Python fallback probes 已退役；后续若要作为业务 smoke，需要接受 GitHub API 网络可用性与限流波动 | 只在显式 gateway rollback 时恢复 `/api/changelog*` 到 Python | 兼容读取；该组无登录依赖，但真实 smoke 依赖 GitHub API 外部可用性 | P1 cutover 已完成，下一步转向 stronger smoke |
| `polish` | Rust | 已补 `POST /api/polish` 与 `POST /api/polish/batch` 两条低前提 Rust owner probe；2026-06-07 起 active same-path Python fallback probes 已退役；后续再补登录态 provider 调用、history 写入和批量结果 stronger smoke | 只在显式 gateway rollback 时恢复 `/api/polish*` 到 Python 或下线该功能 | 依赖 provider 配置；Python fallback 通过 `get_user_ai_service` 先停在登录依赖 | P1 cutover 已完成，下一步转向 stronger smoke |
| `ai_test / ai` | Rust | 已补 `POST /api/ai-test` 与 `POST /api/ai/test` 两条 Rust auth-boundary asymmetric probe；当前仓库未发现 Python fallback router，暂不纳入 `phase5-p1-fallback` | `/api/ai*` 当前更适合按 Rust-only 或禁用策略处理，而不是假设可回 Python | provider 配置、超时策略、SSE 行为兼容；Python fallback 缺失需单独确认产品保留策略 | 已进入 P1 asymmetric starter evidence |

---

## 4. 当前最值得补 smoke 的 route group

### 4.1 P0

这些组直接决定能否进入“稳定切流后移除 Python fallback”的下一阶段：

1. `chapters`
2. `projects`
3. `memories`
4. `settings`
5. `wizard-stream`

补充执行资产：

- `deploy/strangler-gateway-probes.json` 现已新增独立 `phase5-p0` profile，
  仅选择这五组的 through-gateway owner probes，便于把 P0 治理验证从
  `route-groups` 大集合里拆出来单独执行、单独汇报。
- `projects` 现在还拥有 `POST /api/projects/validate-import` 的第一条
  P0 public/business probe：它对同一个最小导入文件分别固定 Rust owner 与
  Python fallback 的不同成功响应结构，使 `phase5-p0` 不再只依赖未登录
  `401` 边界。
- `projects` 现在也补入 `GET /api/projects/{project_id}` 详情读侧：
  它与列表入口一样是低前提未登录边界，但覆盖 Nginx 中单项目详情的显式
  Rust location，可直接证明 `projects` 不只列表路径具备 owner/fallback
  双侧证据。
- `projects` 现在又补入 `POST /api/projects/import`：它与
  `validate-import` 共用同一个最小 multipart 文件，但断言的是合法导入形态下
  的 Rust/Python 鉴权分界，而不是 public-success 结构。
- `projects` 现在还补入 `POST /api/projects/{project_id}/export-data`：它使用
  最小合法 JSON body `{}`，把 `projects` 的 owner/fallback 证据再扩到 JSON
  写侧导出入口，而不是只停留在列表和 multipart 路径。
- `projects` 现在继续补入基础 CRUD 与 TXT 导出低前提边界：
  `POST /api/projects`、`PUT /api/projects/{project_id}`、
  `DELETE /api/projects/{project_id}` 与 `GET /api/projects/{project_id}/export`
  同时进入 `phase5-p0` 与 `phase5-p0-fallback`。这把第一批 fallback shrink
  readiness 的覆盖面从列表、详情、导入、JSON 导出、维护修复扩展到项目基础
  生命周期与 TXT 导出入口；当前仍只证明 owner/fallback 鉴权边界，不代表
  登录态创建、更新、删除级联清理或章节 TXT 内容导出完全等价。
- `projects` 现在还补入 `POST /api/projects/{project_id}/check-consistency`：
  它覆盖显式 Rust location 中的数据维护入口，并在未登录边界验证 Python
  fallback 同路径仍可回切。该 probe 只证明 owner/fallback 边界，不代表已验证
  登录态下的一致性修复报告等价。
- `projects` 现在继续补入 `POST /api/projects/{project_id}/fix-organizations`
  和 `POST /api/projects/{project_id}/fix-member-counts`：这两条与
  `check-consistency` 属于同一组维护类显式 Rust location，补齐后 `projects`
  的 P0 owner/fallback 证据已覆盖当前 Nginx 中所有项目维护修复入口。
- `wizard-stream` 现在也从单一 `outline` probe 扩到
  `world-building/{project_id}/regenerate`：它使用最小合法 JSON body `{}`，
  把该组的 owner/fallback 证据从一个 SSE 入口扩到第二个同层级入口。
- `wizard-stream/world-building` 基础入口现在也进入 `phase5-p0`；
  它使用最小 JSON body `{}`，覆盖 Rust owner 的初始世界观生成 SSE 入口。
  历史上它也曾作为 same-path Python fallback 线索使用，但 2026-06-07 起
  dedicated gateway fallback 已收口，因此后续回切验证改为显式 gateway 动作
  后的手工/定向 smoke。
- `wizard-stream/cleanup/{project_id}` 现在也补入 `phase5-p0`，把该组 owner
  证据再扩到第三条显式 SSE/cleanup 子路径；由于 Python 当前没有同路径 API，
  这条路径暂不进入 `phase5-p0-fallback`。
- `wizard-stream/career-system` 现在也进入 `phase5-p0`：它使用最小合法
  JSON body `{"projectId":"test-project-id"}`，把该组 Rust owner 证据扩到
  第三条同路径 SSE 入口。历史 Python fallback 线索仍可在显式回切后复用。
- `wizard-stream/characters` 现在也进入 `phase5-p0`：它使用最小合法 JSON
  body `{"projectId":"test-project-id"}`，把该组 Rust owner 证据扩到第四条
  同路径 SSE 入口。历史 Python fallback 线索仍可在显式回切后复用。
- `chapters/batch-generate/{batch_id}/resume` 现在也进入 `phase5-p0` 与
  `phase5-p0-fallback`：它使用无 body 的最小 POST 形态，把 `chapters`
  的 owner/fallback 证据扩到批量生成恢复写侧入口；Rust 侧稳定停在共享鉴权
  `401 {"detail":"未登录，请先登录"}`，Python fallback 则稳定停在
  `401 {"detail":"Not logged in"}`。
- `chapters/{chapter_id}/generate-background` 现在也进入 `phase5-p0` 与
  `phase5-p0-fallback`：它使用最小 JSON body `{}`，把 `chapters`
  的 owner/fallback 证据扩到单章后台生成写侧入口；Rust 侧稳定停在共享鉴权
  `401 {"detail":"未登录，请先登录"}`，Python fallback 则稳定停在
  `require_authenticated_user_id()` 的 `401 {"detail":"未登录"}`。
- `chapters/{chapter_id}/generate-stream` 现在也进入 `phase5-p0` 与
  `phase5-p0-fallback`：它同样使用最小 JSON body `{}`，把 `chapters`
  的 owner/fallback 证据扩到单章流式生成写侧入口；Rust 侧稳定停在共享鉴权
  `401 {"detail":"未登录，请先登录"}`，Python fallback 则稳定停在
  `require_authenticated_user_id()` 的 `401 {"detail":"未登录"}`。这条 probe
  也把本轮已完成的 Rust stream follow-up analysis owner 收口正式映射进
  cutover 资产，而不再只让 `generate-background` 代表单章生成链路。
- `memories` 现在也从单一 `stats` probe 扩到
  `POST /api/memories/projects/{project_id}/search?query=test`：它利用 Python
  侧必填 `query` 查询参数与 Rust 侧可接受 `{}` body 的交集，补上同组第二条
  查询入口 owner/fallback 证据。
- `memories` 现在进一步补入列表、章节分析读取、未完伏笔读取、章节记忆删除
  四类 `/api/memories/projects/{project_id}` route。这样该组不再只靠 stats/search
  判断 owner，而是覆盖 API 侧读、查、删三类低前提边界。
- `settings/api-key` 现在也加入 `phase5-p0` 与 `phase5-p0-fallback`，因此
  `settings` 不再只覆盖“根路径 + provider 探测子路由”，还补上了已保存凭据
  读取入口的同路径 owner/fallback 证据。
- `settings/presets` 现在也加入 `phase5-p0` 与 `phase5-p0-fallback`，因此
  `settings` 的第一批 cutover 资产开始覆盖 `preferences` JSON 内的 preset
  读取入口。这条路径仍是低前提未登录边界，不证明 preset 业务成功态完整等价，
  但可直接证明 owner / fallback 在同一路由形态下可切换。
- `settings/presets` 写侧现在继续补入 create、update、delete、
  `from-current`、`activate` 与 `test` 六条同路径低前提 probe。至此
  `settings` 的 P0 owner/fallback 证据已经从 preset 读取扩展到主要 preset
  管理入口，但仍只证明登录边界和网关 owner/fallback 可切换，不证明登录态下
  `preferences` JSON 写入、激活应用主字段或 provider 测试结果完整等价。

### 4.2 P1

这些组已经基本 Rust-owned，但缺少部署后证据链：

1. `auth`
2. `users`
3. `characters`
4. `outlines`
5. `book_import`
6. `relationships`
7. `foreshadows`
8. `writing_styles`
9. `organizations`
10. `careers`
11. `inspiration`
12. `mcp_plugins`
13. `prompt_templates`
14. `background_tasks`
15. `prompt_workshop`
16. `polish`

补充执行资产：

- `deploy/strangler-gateway-probes.json` 现已新增第一版 `phase5-p1` profile，
  现收录三十条低前提、稳定度较高的 owner smoke：
  `auth-config-public-rust`、`auth-logout-public-rust`、
  `auth-linuxdo-url-misconfig-rust`、`auth-callback-missing-code-rust`、
  `auth-local-login-invalid-credentials-rust`、
  `auth-bind-login-invalid-credentials-rust`、
  `auth-user-auth-guard-rust`、
  `auth-password-status-auth-guard-rust`、
  `auth-password-set-auth-guard-rust`、
  `auth-password-initialize-auth-guard-rust`、
  `auth-refresh-auth-guard-rust`、
  `users-current-auth-guard-rust`、`users-list-auth-guard-rust`、
  `users-set-admin-auth-guard-rust`、`users-reset-password-auth-guard-rust`、
  `characters-project-list-auth-guard-rust`、
  `characters-list-auth-guard-rust`、
  `characters-generate-stream-auth-guard-rust`、
  `characters-export-auth-guard-rust`、
  `characters-import-auth-guard-rust`、
  `outlines-project-list-auth-guard-rust`、
  `outlines-list-auth-guard-rust`、
  `outlines-generate-stream-auth-guard-rust`、
  `outlines-batch-expand-stream-auth-guard-rust`、
  `outlines-create-chapters-from-plans-auth-guard-rust`、
  `book-import-create-task-auth-guard-rust`、
  `book-import-task-status-auth-guard-rust`、
  `book-import-preview-auth-guard-rust` 与
  `book-import-cancel-auth-guard-rust`、
  `book-import-apply-auth-guard-rust` 与
  `book-import-retry-stream-auth-guard-rust`、
  `book-import-apply-stream-auth-guard-rust`、
  `relationships-project-list-auth-guard-rust` 与
  `relationships-graph-auth-guard-rust`、
  `foreshadows-project-list-auth-guard-rust` 与
  `foreshadows-stats-auth-guard-rust`、
  `writing-styles-user-auth-guard-rust` 与
  `writing-styles-project-auth-guard-rust`、
  `organizations-project-list-auth-guard-rust` 与
  `organizations-generate-stream-auth-guard-rust`、
  `careers-list-auth-guard-rust` 与
  `careers-generate-system-auth-guard-rust`、
  `inspiration-generate-options-auth-guard-rust` 与
  `inspiration-quick-generate-auth-guard-rust`、
  `mcp-plugins-list-auth-guard-rust` 与
  `mcp-plugins-simple-create-auth-guard-rust`、
  `prompt-templates-list-auth-guard-rust` 与
  `prompt-templates-system-defaults-auth-guard-rust`、
  `background-tasks-list-auth-guard-rust` 与
  `background-tasks-create-auth-guard-rust`、
  `prompt-workshop-submit-auth-guard-rust` 与
  `prompt-workshop-like-auth-guard-rust`、
  `polish-text-auth-guard-rust` 与
  `polish-batch-auth-guard-rust`、
  `changelog-public-rust` 与 `changelog-refresh-public-rust`
- 这还不是完整的 P1 业务 smoke，只是把 `auth` / `users` 从“纯文档待办”
  推进到第一版可执行 profile；现在又把 `characters`、`outlines`、
  `book_import`、`relationships`、`foreshadows`、`writing_styles`、
  `organizations`、`careers`、`inspiration`、`mcp_plugins`、
  `prompt_templates`、`background_tasks`、`prompt_workshop`、`polish`
  十四组推进到 starter slice，并把 `changelog` 推进到 public starter slice；
  但后续仍需补真实登录态、
  cookie/session 刷新、管理员权限读写、导入上传/流式等更强业务断言
- 现已新增第一版 `phase5-p1-fallback` profile，当前覆盖 `auth` 的
  `logout`、`/api/auth/user`、`/api/auth/password/status`，现在还继续补入
  `/api/auth/password/set`、`/api/auth/password/initialize` 与
  `/api/auth/refresh`，以及 `GET /api/auth/callback` 的缺参 public error
  、`POST /api/auth/local/login` 与 `POST /api/auth/bind/login`
  的错误凭证 public failure，
  以及
  `users` 的 `/api/users/current`、`/api/users`、`/api/users/set-admin`
  与 `/api/users/reset-password` 同路径未登录差异，
  以及
  `characters` / `outlines` / `book_import` 的同路径未登录读侧差异
  （现已同时包含 project-path、query-list、`generate-stream`、
  `export`、`import`、
  `batch-expand-stream`、`create-chapters-from-plans`、上传创建、task preview，
  以及 cancel/apply/retry-stream/apply-stream 写侧边界），
  以及 `relationships` 的 project-list 与 graph 两条同路径未登录边界，
  以及 `foreshadows` 的 project-list 与 stats 两条同路径未登录边界，
  以及 `writing_styles` 的 user 与 project 两条同路径未登录边界，
  以及 `organizations` 的 project-list 与 generate-stream 两条同路径未登录边界，
  以及 `careers` 的 list 与 generate-system 两条同路径未登录边界，
  以及 `inspiration` 的 generate-options 与 quick-generate 两条同路径未登录边界，
  以及 `mcp_plugins` 的 list 与 simple-create 两条同路径未登录边界，
  以及 `prompt_templates` 的 list 与 system-defaults 两条同路径未登录边界，
  以及 `background_tasks` 的 list 与 create 两条同路径未登录边界
  （现仅作为显式 rollback 后的历史线索保留），
  以及 `prompt_workshop` 的 submit 与 like 两条同路径未登录边界，
  以及 `polish` 的 text 与 batch 两条同路径未登录边界，
  以及 `changelog` 的 public list 与 refresh 两条同路径 public 边界
- `users` 现在已进入 fallback profile，但应明确解释为同路径
  auth-boundary 线索，而不是管理员列表/当前用户业务语义的完整等价证明
- `users` 现在也开始具备写侧 fallback 线索，但这些写侧 probe 仍只证明
  Python owner 已接管同路径登录边界，不证明管理员操作或密码重置语义完整等价
- `characters/validate-import` 现新增到独立 `phase5-p1-asymmetric` profile：
  它是同路径但非同语义的治理样本，Rust owner 为公开校验入口，Python fallback
  仍是登录依赖；这类路径应单独记录，不混入 `phase5-p1-fallback`
- `settings/models` 现进入独立的 `phase5-p0-asymmetric` profile：
  Rust owner 侧在同一路径先停在共享鉴权 `401`，Python fallback 侧则继续进入
  公开模型列表逻辑并在最小不可达 base URL 下稳定返回连接失败 `400`；这类路径
  同样不应混入 `phase5-p0-fallback`
- `ai_test` 现进入独立的 `phase5-p1-asymmetric` profile：当前只收录
  `POST /api/ai-test` 与别名 `POST /api/ai/test` 的 Rust 未登录边界。
  仓库未发现对应 Python router，因此这组暂不补 `phase5-p1-fallback`，
  也不应被统计为“可直接回 Python 的同路径 fallback”。
- `relationships/project` 与 `relationships/graph` 现新增到 `phase5-p1`
  与 `phase5-p1-fallback`：这是该组第一版可执行 owner/fallback 证据，证明
  Nginx 已将默认 API 流量交给 Rust，且 path 级回切后 Python 同路径仍会先停在
  `verify_project_access()` 的未登录边界。该证据不代表已验证登录态下的关系
  graph 节点/边聚合语义完整等价。
- `foreshadows/projects` 与 `foreshadows/projects/{project_id}/stats` 现新增到
  `phase5-p1` 与 `phase5-p1-fallback`：这是该组第一版可执行
  owner/fallback 证据，覆盖伏笔列表和统计两个低前提读侧入口。该证据不代表
  已验证登录态下的 context、pending-resolve、plant/resolve/abandon 等写侧和
  章节生成上下文语义完整等价。
- `writing-styles/user` 与 `writing-styles/project/{project_id}` 现新增到
  `phase5-p1` 与 `phase5-p1-fallback`：这是该组第一版可执行
  owner/fallback 证据，覆盖用户可用风格列表和项目可用风格列表两个低前提
  读侧入口。该证据不代表已验证登录态下的 preset 同步、自定义风格 CRUD、
  默认风格写入或 `project_default_styles` 侧效应完整等价。
- `organizations/project` 与 `organizations/generate-stream` 现新增到
  `phase5-p1` 与 `phase5-p1-fallback`：这是该组第一版可执行
  owner/fallback 证据，覆盖组织列表读侧和组织生成流入口两个低前提边界。
  该证据不代表已验证登录态下的组织 CRUD、成员增删改、生成结果落库、
  `organization_members` 字段一致性或 generation_history 语义完整等价。
- `careers` 列表与 `careers/generate-system` 的历史 Python fallback
  线索现仅保留为显式 gateway rollback 后的回切证据；2026-06-07 起，
  active same-path `careers` Python fallback probes 已退役
- `inspiration/generate-options` 与 `inspiration/quick-generate` 的历史
  Python fallback 线索现仅保留为显式 gateway rollback 后的回切证据；
  2026-06-07 起，active same-path `inspiration` Python fallback probes 已退役
- `mcp/plugins` 列表与 `mcp/plugins/simple` 现新增到 `phase5-p1`
  与 `phase5-p1-fallback`：这是该组第一版可执行 owner/fallback 证据，
  覆盖插件列表读侧和标准 JSON 配置创建入口两个低前提边界。该证据不代表
  已验证登录态下的插件创建/更新、toggle、status/tools/test/call、MCP session
  注册/断开或后台任务语义完整等价。
- `prompt-templates` 列表与 `prompt-templates/system-defaults` 现新增到
  `phase5-p1` 与 `phase5-p1-fallback`：这是该组第一版可执行
  owner/fallback 证据，覆盖用户模板列表和系统默认模板读取两个低前提边界。
  该证据不代表已验证登录态下的 categories、sync-status、保存/删除/导入/预览、
  managed template sync 或 prompt formatting 语义完整等价。
- `background-tasks` 列表与创建入口现新增到 `phase5-p1` 与
  `phase5-p1-fallback`：这是该组第一版可执行 owner/fallback 证据，覆盖
  任务列表读侧和任务创建写侧两个低前提边界。该证据不代表已验证登录态下的
  task registry 生命周期、SSE stream、cancel、workflow-state 或任务缺失 payload
  语义完整等价。
- `prompt-workshop/submit` 与 `prompt-workshop/items/{item_id}/like` 现新增到
  `phase5-p1` 与 `phase5-p1-fallback`：这是该组第一版可执行
  owner/fallback 证据，覆盖提交和互动两个登录态入口边界。该证据不代表已验证
  公开 items/status、import/download、my-submissions、admin 审核或云端代理语义
  完整等价。
- `polish` 单条与批量入口现新增到 `phase5-p1` 与 `phase5-p1-fallback`：
  这是该组第一版可执行 owner/fallback 证据，覆盖 AI 去味单条和批量两个入口。
  该证据不代表已验证登录态 provider 调用、PromptService 模板、generation_history
  写入或批量结果 payload 语义完整等价。
- `changelog` 列表与刷新入口现新增到 `phase5-p1` 与
  `phase5-p1-fallback`：这是该组第一版 public owner/fallback 证据，覆盖
  `GET /api/changelog` 与 `POST /api/changelog/refresh`。该证据证明双侧同路径
  public contract 形态存在，但真实 smoke 仍受 GitHub API 可用性、限流和网络
  超时影响，不应和本地纯 auth-boundary probe 等价解读。

---

## 5. 当前 rollback 原则

### 5.1 优先使用 gateway 回滚

对当前 strangler 架构，最小回滚动作应优先是：

- 调整 Nginx `location` owner
- 保留 shared DB 不变
- 不做 schema 回退

原因：

- 当前 schema owner 仍是 Python Alembic
- Rust Phase 2 的目标是停止启动期 schema mutation，而不是建立双向 schema rollback

### 5.2 避免把 rollback 设计成“改代码再重发版”

对于已在 Nginx 中有显式 location 的 route group，回滚优先级应是：

1. 改 gateway owner
2. 重新跑 smoke
3. 仅在 owner 回切仍失败时，再进入服务级诊断

### 5.3 P0 rollback 资产

Phase 5 P0 route-group 的第一版可执行 rollback 手册已单独整理为：

- `docs/architecture/rust-phase5-p0-route-group-rollback-runbook-2026-05-19.zh-CN.md`

当前覆盖：

1. `settings`
2. `projects`
3. `chapters`
4. `wizard-stream`
5. `memories`

补充说明：

- runbook 现在已经把“回滚后先跑定向 smoke，再跑全量 `phase5-p0` smoke”
  固化为标准步骤
- `backend/tools/run_strangler_gateway_smoke.py` 现已支持 `--route-group`
  定向执行单个 route group；如有必要也可继续叠加 `--probe-name`
  精准到单条 probe
- runbook 现已补入第一版 “Python fallback 成功线索” 矩阵，用于说明
  `settings / projects / chapters / wizard-stream / memories` 回切到 Python
  后应优先观察到哪些稳定 401 / SSE 边界差异
- 其中最稳定的一部分差异现已进入独立 `phase5-p0-fallback` smoke profile，
  可以按 route-group 执行第一版 Python fallback 验证
- `settings/fetch-models` 现在也进入 `phase5-p0-fallback`，因此 `settings`
  不再只靠根路径 `/api/settings` 来证明回切后的 Python API owner
- `settings/test` 现在也进入 `phase5-p0-fallback`，因此 `settings` 已开始
  同时拥有根路径、模型列表探测、连接测试三条低前提回切线索
- `settings/check-function-calling` 现在也进入 `phase5-p0-fallback`，因此
  `settings` 的低前提探测子路由矩阵已经基本补齐
- `settings/presets` 现在也进入 `phase5-p0-fallback`，因此 `settings`
  已开始覆盖 `preferences` JSON 内 preset 读取入口的同路径回切线索，
  可作为第一批 fallback shrink readiness 模板的一部分
- `settings/presets` 写侧入口历史上也曾进入 `phase5-p0-fallback`，覆盖
  create、update、delete、from-current、activate、test；这些 same-path
  Python 线索现在仅作为显式 gateway rollback 后的历史验证资产保留。
- `settings` 曾固化为三类 shrink-readiness profile：
  `phase5-settings-owner`（13 条）、`phase5-settings-fallback`（12 条）与
  `phase5-settings-asymmetric`（2 条）。2026-06-07 起，随着 gateway
  `/api/settings` exact-root + shared-prefix 已直接切到 Rust，active
  `phase5-settings-fallback` 与 `phase5-settings-asymmetric` 已退役；其中
  `settings/models` 的 Python public network-error 线索仅在显式 rollback 后
  作为手工或定向 smoke clue 复用。
- `settings` 现已保留第一组与第二组登录态 business owner smoke：
  `settings-get-business-rust` 与 `settings-presets-get-business-rust`。
  历史上的 Python fallback 对照 probes 现已退役，因为当前 cutover 不再依赖
  active same-path Python profile 来证明 owner/fallback 可切换。
- 与这组新 business smoke 同步，Rust preset owner 也已补齐一轮真实业务语义
  收口：preset 读取 / 创建 / 更新 / 删除 / 激活 / from-current 在缺少
  `settings` 行时会自动创建默认设置，不再暴露 `settings not found`；
  同时对齐了“激活中的预设不可删除”的 `400` 保护和 activate 返回摘要。
  这说明 `settings` route-group 不只是 probe 数量在增长，而是已经开始把
  Python 定义的 preset success/failure 业务契约收进 Rust owner。
- `settings` 主设置写侧现在也完成了一轮真实 Python 契约收口：
  - `POST /api/settings` 保持 upsert
  - `PUT /api/settings` 不再隐式创建，而是在缺失设置时返回
    `404 {"detail":"设置不存在，请先创建设置"}`
  - `DELETE /api/settings` 在缺失设置时返回
    `404 {"detail":"设置不存在"}`
  - `POST /api/settings` 手动修改当前配置且偏离激活中的 preset config 时，
    Rust 现在也会自动取消该 preset 的激活状态
  这意味着 `settings` route-group 的剩余 Phase 5 缺口，已经从基础
  save/update/delete 语义进一步收缩到更强的登录态 provider-test /
  preset create-update-delete-activate success smoke，而不是仍卡在主写路径契约。
- `settings` preset action owner 现在也继续收口到 Python 语义：
  - `POST /api/settings/presets/{preset_id}/activate` 不再借由 preset config
    回写 `api_backup_urls`、`provider_type`、`fallback_strategy`、
    `azure_api_version`
  - `POST /api/settings/presets/from-current` 不再把当前 `settings` 行里的
    扩展 provider 状态原样快照进 preset config，而是改为 Python-shaped
    snapshot 默认值
  - 这让 `settings` 的剩余 Phase 5 缺口进一步聚焦到登录态 success smoke、
    provider-test/result parity，而不是仍卡在 preset action contract drift
- `settings/models` 的 provider-specific success contract 现在也完成了一轮
  真实 Python 语义收口：
  - openai-compatible providers 已按 Python 风格尝试 candidate URL fallback，
    不再只打单一路径 `/models`
  - Azure 在这条路由上改为 `api-key` header，并在 `404/403` 或空结果时
    返回 `200 + 空列表 + 友好 message`
  - Anthropic 不再走 Rust 本地 curated model 列表，而是改为真实请求
    `/v1/models`
  - Gemini 现在只暴露支持 `generateContent` 的模型
  - 这意味着 `settings` route-group 的剩余 Phase 5 缺口，进一步从
    model-list success path 收缩到 `settings/test` /
    `check-function-calling` 的 probe parity 与更强的登录态 success smoke
- `settings/check-function-calling` 现在也完成了一轮真实 Python 核心契约收口：
  - Rust AI owner path 新增 `ToolChoice` 能力，并由 `AIService` 透传到
    OpenAI / Anthropic client，不再只能依赖默认 `auto`
  - `POST /api/settings/check-function-calling` 现在改用 Python 对齐的
    `get_weather` 工具，并显式强制 `required` tool choice
  - 当模型成功返回但仅输出纯文本时，Rust 现在也与 Python 一样保持
    `success = true`、`supported = false`，而不是误判为整次 probe 失败
  - 成功与失败路径都补上了 Python 风格的最小 `details` 外壳：
    `endpoint_diagnostics / finish_reason / has_tool_calls /
    tool_call_count / test_tool / response_type`
  - 这意味着 `settings` route-group 的剩余 Phase 5 缺口，又从
    function-calling 的核心 success/error 壳层继续收缩到
    `settings/test` transport parity、backup/fallback/request-options 更深层
    owner 收口，以及更强的登录态 success smoke
- `settings/test` 现在也完成了一轮真实 Python probe contract 收口：
  - `POST /api/settings/test` 现在接受 widened probe request body：
    `api_backup_urls` / `fallback_strategy`
  - probe 成功路径现在回到 Python 风格的 `details` 壳层：
    `api_available / model_accessible / response_valid / temperature /
    max_tokens / probe_max_tokens / endpoint_diagnostics`
  - `endpoint_diagnostics` 现在开始对齐 Python 的归一化 owner 语义：
    - `backup_endpoints`
    - `configured_endpoint_count`
    - `fallback_strategy`
    - `auto_failover_enabled`
    不再固定为 Rust 本地 `[] + auto(false)` 的占位形态
  - 失败路径现在也补上了 `details.endpoint_diagnostics`
  - `settings/presets/{preset_id}/test` 复用链路也开始透传
    `api_backup_urls` / `fallback_strategy`，避免 preset probe 与主 probe
    在 transport 相关字段上继续分叉
  - 这意味着 `settings` route-group 的剩余 Phase 5 缺口，又从
    API-connection probe 的核心 response shell 继续收缩到更深层的
    transport parity（`request_options` / provider-specific probe pathing /
    `transport_diagnostics`）与更强的登录态 success smoke
- `settings` 现又补入第三组、也是第一组 probe-lane 登录态 business smoke：
  - `settings-test-business-rust`
  - `settings-check-function-calling-business-rust`
  - 历史上的 Python fallback 对照 probes 现已退役；
    2026-06-07 起，active 登录态 smoke 只保留
    `phase5-settings-business-owner`，显式 gateway rollback 后如需复核 Python
    业务外壳，可复用这些历史路径做手工或临时 probe 校验
  - 它们刻意只锁稳定的 `200 + failure shell`：
    - `settings/test` 断言 `success=false` 与稳定 message/shell
    - `check-function-calling` 断言 `success=false`、`supported=null` 与
      稳定 error/details shell；不要把 owner smoke 绑死到单一 generic
      message，因为 Python/Rust 现在都会按 `5xx/429/401/404/timeout`
      产生不同失败文案
  - 这样 cutover readiness 证明的是“请求已进入真实登录态业务 handler 且
    owner/fallback 契约壳层稳定”，而不是把 route-group smoke 误绑到外网
    provider 成功率
  - 这让 `settings` 的真实登录态 business smoke 已经覆盖根设置读取、
    preset 读取、API probe 与 function-calling probe 四条主线；下一步再把
    success smoke 和 transport parity 拆成独立 lane 推进
- `projects` 的 fallback 现在不只剩未登录列表线索，还补入了同路径公开
  `validate-import` 成功断言，可直接证明 owner 已切回 Python 而不是只靠
  `401` 语义侧面判断
- `projects/detail` 现在也进入 `phase5-p0-fallback`，因此 `projects`
  同时拥有列表与详情两条读侧回切线索，覆盖 `/api/projects` 与
  `/api/projects/{project_id}` 两类 Nginx owner 规则
- `projects/import` 现在也进入 `phase5-p0-fallback`，因此 `projects` 已同时
  拥有列表读侧、public validator 成功态、multipart 写侧三类回切线索
- `projects/export-data` 现在也进入 `phase5-p0-fallback`，因此 `projects`
  又增加了一条合法 JSON body 进入后的写侧回切线索
- `projects` 基础 CRUD 与 TXT 导出现在也进入 `phase5-p0-fallback`，覆盖
  create、update、delete、export；因此 `projects` 的回切线索已覆盖基础项目
  生命周期、读侧、导入、两类导出和维护修复入口。下一阶段应转向登录态
  business smoke 与 fallback shrink checklist，而不是继续只扩未登录边界
- `projects` 历史上也曾固化 `phase5-projects-owner` 与
  `phase5-projects-fallback` 两个专用 shrink-readiness profile，各覆盖 12 条
  同路径 probe。2026-06-07 起，随着 gateway `/api/projects` exact-root +
  shared-prefix 已直接切到 Rust，active `phase5-projects-fallback` 已退役；
  当前日常 cutover 判断只保留 `phase5-projects-owner` 与
  `projects-validate-import-public-rust` 这类 Rust owner / public-business
  样本，历史 Python 线索仅在显式 gateway rollback 后复用。
- `projects/check-consistency` 现在也进入 `phase5-p0-fallback`，因此
  `projects` 的回切线索已经覆盖数据维护类入口，不再只覆盖列表、导入、
  导出路径
- `projects/fix-organizations` 与 `projects/fix-member-counts` 现在也进入
  `phase5-p0-fallback`，因此 `projects` 的维护类回切线索已经覆盖
  check + 两条 fix 入口
- 上述 `projects` fallback 叙述现在仅保留为历史回切线索说明；
  2026-06-07 起，active same-path `phase5-p0-fallback` /
  `phase5-projects-fallback` probes 已退役，后续仅在显式 gateway rollback
  后作为定向复核资产复用
- `wizard-stream/world-building/{project_id}/regenerate` 现在也进入
  `phase5-p0-fallback`，因此该组不再只靠 `outline` 单路径来证明回切后的
  SSE 入口 owner
- `wizard-stream/world-building` 基础入口现在也进入 `phase5-p0-fallback`，
  因此该组的回切线索同时覆盖初始世界观生成与重新生成两类 SSE 入口；但仍需
  后续登录态 stronger smoke 才能判定流式 payload 与落库副作用等价
- 上述 `wizard-stream` fallback 叙述现在仅保留为历史回切线索说明；
  2026-06-07 起，active same-path `phase5-p0-fallback` probes 已退役
- `memories/search`、`memories/list`、`memories/analysis`、
  `memories/foreshadows`、`memories/delete-chapter` 这批
  `phase5-p0-fallback` 资产现在仅保留为历史回切线索说明；
  2026-06-07 起，active same-path `memories` Python fallback probes 已退役
- `/api/memories/*` 已完成 Rust API owner 收口；`/memories/` 仍继续作为页面/非
  API fallback 边界单独处理
- `relationships/project` 与 `relationships/graph` 现在也进入
  `phase5-p1-fallback`，因此 `relationships` 不再只是 ownership checklist
  里的“可补 smoke”待办，而是开始拥有第一组同路径 owner/fallback 线索
- `foreshadows/projects` 与 `foreshadows/stats` 现在也进入
  `phase5-p1-fallback`，因此 `foreshadows` 不再只是 ownership checklist
  里的“可补 smoke”待办，而是开始拥有第一组同路径 owner/fallback 线索
- `writing-styles/user` 与 `writing-styles/project` 的历史 Python fallback
  线索现仅保留为显式 gateway rollback 后的回切证据；2026-06-07 起，
  active same-path `writing_styles` Python fallback probes 已退役
- `organizations/project` 与 `organizations/generate-stream` 的历史 Python
  fallback 线索现仅保留为显式 gateway rollback 后的回切证据；2026-06-07 起，
  active same-path `organizations` Python fallback probes 已退役
- `chapters` 的 Python fallback 现在也补进了 project-path 列表探针
  `/api/chapters/project/{project_id}`，不再只靠 analysis/batch/regeneration
  子路由侧面证明
- `chapters/generate-stream` 现在也进入 `phase5-p0-fallback`，因此 `chapters`
  不再只靠 `generate-background` 来证明单章生成写侧已经回切到 Python；
  单章流式生成入口现在也有了同路径、低前提的回切线索

---

## 6. 当前 schema assumptions

以下假设在 Phase 5 期间仍然成立：

1. **schema owner 仍是 Python Alembic，不是 Rust。**
2. **Rust 只消费既有 schema，不应在部署时隐式修表。**
3. **shared DB 下的 route-group cutover，不应绑定新的 schema 扩张。**
4. **`analysis_tasks`、`batch_generation_tasks`、`regeneration_tasks` 的第一轮默认值语义已在仓库层收口，但真实环境仍依赖迁移已落地。**
5. **`organization_members`、`projects`、`settings` 等中风险共享表，仍需保持“先验证字段级一致性，再扩大切流”的纪律。**
6. **`settings` 的当前 cutover 证据已覆盖主设置读写删除基础契约，以及 `settings/presets` 读取、主要写侧入口和 preset action owner 的一轮 Python 语义收口；但仍不代表已完成登录态 preset 创建、更新、删除、激活、from-current 或 provider 测试业务等价验证。**
7. **`organizations` 的当前 cutover 证据只覆盖未登录 owner/fallback 边界；
   因为它触碰 `organization_members`、组织角色映射、成员计数与生成历史，
   后续扩大切流前必须补字段级一致性和登录态 business smoke。**

---

## 7. Phase 5 的建议推进顺序

建议按以下顺序推进，而不是平铺所有 route group：

1. 先把 `projects` 做成与已完成 `settings` / `wizard-stream` 同级的 gateway cutover 模板。
2. 再补 `chapters` / `memories` / `settings` / `wizard-stream` 的 stronger smoke 与边界枚举。
3. 之后把 `auth` / `users` / P1 组迁入同一模板，但保持更高安全门槛。
4. 最后才评估是否收缩 Python `/api/` catch-all 或移除特定 fallback。

---

## 8. 阶段结论

截至 2026-05-19，Phase 5 已经具备第一版治理资产，但还没有达到可以移除
Python fallback 的程度。

更准确的判断应是：

1. **owner 版图已经基本可描述。**
2. **一部分旧的 mixed 判断已经被真实 gateway 响应证伪，说明 owner 资产必须持续校正。**
3. **`backend/tools/run_strangler_gateway_smoke.py` 已经不再只支持状态码/JSON 子集断言，还支持 per-probe `headers`、`json_body/body`、`expected_text_startswith`、`expected_text_contains`。**
4. **smoke runner 现在也支持 `expected_header_contains`，可以把 cookie / 关键响应头存在性纳入业务 smoke，而不必只看 body。**
   当前实现还会保留重复响应头的多值内容，因此 `Set-Cookie` 这类多 header
   场景不会在采集阶段被静默折叠掉。
5. **现有 `chapters` / `wizard-stream` 的 POST owner probes 已经开始发送真实 JSON body，而不是只依赖空请求命中 `401`。其中 `chapters` 已同时覆盖 `generate-background` 与 `generate-stream` 两条单章生成写侧入口。**
6. **manifest 现在还区分了 `business` profile，可把公开 JSON/HTML 业务断言和 `route-groups` / `deploy` 物理拆分。**
7. **下一步关键工作是利用这套能力补更强的 stream/business smoke 和 rollback 资产，而不是继续口头判断“差不多可以切了”。**
8. **`phase5-p0` profile 已经可执行，且 `projects` 现在同时拥有更强的
   `validate-import` public/business probe、`import` multipart 写侧鉴权 probe，
   以及 `export-data` JSON 写侧鉴权 probe；但整体仍以低前提 owner 证据为主，
   下一步仍需继续补更多 business/SSE smoke，才能支撑更激进的 cutover 判断。**
9. **`memories` 已从单一 `/stats` 扩到 `stats + search` 双 probe，说明 P0
   资产建设正在从“是否命中 owner”单点判断，向“同组多入口最小可执行证据链”
   演进。**
10. **`settings` 已在 2026-06-07 完成 gateway `/api/settings*` 收口；
    历史 P0 fallback / business-fallback / models asymmetric 线索现仅在显式
    gateway rollback 后作为手工或定向 smoke clue 复用，不再作为 active
    same-path probe 参与当前切流治理。**
11. **P0 route-group rollback 已经有第一版 operator-ready runbook，但回切后的
   Python 成功条件还没有完全 probe 化，短期内仍需配合人工核对。**
12. **rollback smoke 已经可以按 `route_group` 直接筛选，这比手工维护 probe
   名映射更接近真正可执行的运维步骤。**
13. **第一版 Python fallback 成功条件已经从 operator clue 前进到独立
    `phase5-p0-fallback` smoke profile；但随着 `settings` / `wizard-stream` /
    `projects` 的 same-path gateway 收口，其中一部分 P0 资产已经转为
    “显式 rollback 后的历史线索”，不再代表当前 active fallback 自动化。**
14. **`background_tasks` 也已在 2026-06-07 进入同样的治理阶段：active
    same-path Python fallback probes 已退役，后续只在显式 gateway rollback 后
    复用历史 list/create 线索。**
15. **gateway smoke 输出现在还会附带 `owner_counts` / `route_group_counts` /
    `route_group_probe_names` 汇总，因此 P0 rollback / fallback 结果已经更适合
    被直接引用到操作记录，而不是只保留一串 probe 明细。**
16. **P1 现在也开始拥有 `users` 的读写 fallback 资产；但这些 `users`
    probe 目前只证明同路径 auth-boundary owner 已回到 Python，不证明
    管理员列表、管理员写侧或当前用户业务语义已经完整等价。**
16. **`users` 现在还开始拥有写侧 fallback 资产；但 `set-admin` /
    `reset-password` 现阶段同样只应解读为同路径登录边界证据，不应误读成
    管理员写侧行为已完成语义对齐。**
17. **`characters` / `outlines` / `book_import` 现在也已进入 `phase5-p1` 的
    starter owner smoke，并补入了第一版同路径 Python fallback 线索；
    `relationships` / `foreshadows` / `writing_styles` / `organizations`
    已进一步完成 same-path fallback 收口，当前剩余量是 stronger smoke，
    不再是 active Python fallback。**
