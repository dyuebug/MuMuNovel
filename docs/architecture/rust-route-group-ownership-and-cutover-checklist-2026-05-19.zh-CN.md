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
| `settings` | Rust | 已补根路径读取、`/api-key`、`/models`、`/fetch-models`、`/test` 与 `/check-function-calling` 子路由未登录 owner probe；Python fallback 也已补 `/api/settings`、`/api/settings/api-key`、`/api/settings/fetch-models`、`/api/settings/test` 与 `/api/settings/check-function-calling` 五条线索；后续再补保存配置、模型探测成功/回退、preset 激活等更强 smoke | `/api/settings*` 指回 Python | `settings` 表默认值语义已完成第一轮收口；API key / preferences 仍允许为空 | owner 稳定，需补更强 smoke |
| `projects` | Rust | 现已补 `GET /api/projects` 未登录 owner probe、`POST /api/projects/validate-import` 同路径 public/business probe、`POST /api/projects/import` 的合法 multipart 鉴权 probe，以及 `POST /api/projects/{project_id}/export-data` 的最小 JSON 鉴权 probe；下一步再补更强的详情/修复类 smoke | 需要新增明确的 Python fallback location，或让 `/api/projects*` 临时回到 Python catch-all | `projects` 表字段已做一组中风险收口；当前更应警惕旧注释/旧认知与真实 owner 不一致 | Phase 5 重点 |
| `wizard-stream` | Rust | 现已补 `POST /api/wizard-stream/outline`、`POST /api/wizard-stream/world-building/{project_id}/regenerate`、`POST /api/wizard-stream/career-system`、`POST /api/wizard-stream/characters` 与 `POST /api/wizard-stream/cleanup/{project_id}` 五条 through-gateway probe；其中 `career-system` 与 `characters` 已进入真实 fallback 矩阵，而 `cleanup` 仍只作为 Rust owner 收口证据 | 回退方式是调整或删除 Rust 显式 location，让 `/api/wizard-stream/` catch-all 生效 | 流式 SSE 行为、keep-alive、模型访问配置兼容 | Phase 5 重点 |
| `inspiration` | Rust | 列表/生成至少 1 条 GET/POST smoke | `/api/inspiration*` 指回 Python | 依赖配置与 AI provider 能力稳定 | 可补 smoke 后固化 |
| `outlines` | Rust | CRUD + expand/create-chapters 中至少 1 条 | `/api/outlines*` 指回 Python | outlines 表 / compat 路径保持兼容 | 可补 smoke 后固化 |
| `characters` | Rust | CRUD + generate-stream 至少各 1 条 | `/api/characters*` 指回 Python | characters 表默认值与生成兼容字段保持一致 | 可补 smoke 后固化 |
| `careers` | Rust | 基础 CRUD + `generate-system` | `/api/careers*` 指回 Python | careers / character link 表兼容 | 可补 smoke 后固化 |
| `organizations` | Rust | CRUD + `generate-stream` | `/api/organizations*` 指回 Python | `organization_members` 字段级一致性仍需持续关注 | 有 schema 风险注记 |
| `relationships` | Rust | 基础 CRUD smoke | `/api/relationships*` 指回 Python | 关系表兼容 | 可补 smoke |
| `writing_styles` | Rust | CRUD smoke | `/api/writing-styles*` 指回 Python | styles/preset 数据结构兼容 | 可补 smoke |
| `foreshadows` | Rust | CRUD + context/stats 至少 1 条 | `/api/foreshadows*` 指回 Python | foreshadows 表已在 Alembic 覆盖 | 可补 smoke |
| `chapters` | Rust | 已补列表、analysis、batch analysis status、batch active tasks、batch stream、batch resume、single generate-background、regeneration tasks 八条 through-gateway probe；现在还新增 `batch-generate/{batch_id}/status` 与 `batch-generate/{batch_id}/cancel` 两条 P0 asymmetric 样本；下一步再补更强的 business smoke | `/api/chapters*` 指回 Python | 章节域使用 shared DB；任务语义、checkpoint、SSE event shape 已在 Phase 3/4 前后持续收口 | Phase 5 最高优先级之一 |
| `memories` | Rust | 已补 `GET /api/memories/projects/{project_id}/stats` 与 `POST /api/memories/projects/{project_id}/search?query=test` 两条 through-gateway probe，确认 API 侧仍命中 Rust，且不再只靠单一读侧路径 | 去掉 `/api/memories` 显式 Rust location，恢复 Python API owner | 需区分 API owner 与页面/非 API fallback owner | Phase 5 重点 |
| `mcp_plugins` | Rust | 列表 / 配置读写 smoke | `/api/mcp*` 指回 Python | mcp_plugins 表兼容 | 可补 smoke |
| `prompt_templates` | Rust | 列表 + 保存 smoke | `/api/prompt-templates*` 指回 Python | prompt_templates 表兼容 | 可补 smoke |
| `prompt_workshop` | Rust | 列表 + 互动/提交流程 smoke | `/api/prompt-workshop*` 指回 Python | prompt workshop 三张表 Alembic 已覆盖 | 可补 smoke |
| `background_tasks` | Rust | 任务列表 + 流式或状态查询 smoke | `/api/background-tasks*` 指回 Python | shared task registry / stream hub 正常 | 可补 smoke |
| `book_import` | Rust | 任务创建 + apply/retry-stream 至少各 1 条 | `/api/book-import*` 指回 Python | 导入工作流与 background task 兼容 | 可补 smoke |
| `changelog` | Rust | 列表读取 smoke | `/api/changelog*` 指回 Python | 兼容读取 | 低风险 |
| `polish` | Rust | 单条请求 smoke | `/api/polish*` 指回 Python 或下线该功能 | 依赖 provider 配置 | 低风险但需补 smoke |
| `ai_test / ai` | Rust | provider test / stream smoke | `/api/ai*` 指回 Python 或直接临时禁用 | provider 配置、超时策略、SSE 行为兼容 | 低风险但需补 smoke |

---

## 4. 当前最值得补 smoke 的 route group

### 4.1 P0

这些组直接决定能否进入“稳定切流后移除 Python fallback”的下一阶段：

1. `chapters`
2. `projects`
3. `wizard-stream`
4. `settings`
5. `memories`

补充执行资产：

- `deploy/strangler-gateway-probes.json` 现已新增独立 `phase5-p0` profile，
  仅选择这五组的 through-gateway owner probes，便于把 P0 治理验证从
  `route-groups` 大集合里拆出来单独执行、单独汇报。
- `projects` 现在还拥有 `POST /api/projects/validate-import` 的第一条
  P0 public/business probe：它对同一个最小导入文件分别固定 Rust owner 与
  Python fallback 的不同成功响应结构，使 `phase5-p0` 不再只依赖未登录
  `401` 边界。
- `projects` 现在又补入 `POST /api/projects/import`：它与
  `validate-import` 共用同一个最小 multipart 文件，但断言的是合法导入形态下
  的 Rust/Python 鉴权分界，而不是 public-success 结构。
- `projects` 现在还补入 `POST /api/projects/{project_id}/export-data`：它使用
  最小合法 JSON body `{}`，把 `projects` 的 owner/fallback 证据再扩到 JSON
  写侧导出入口，而不是只停留在列表和 multipart 路径。
- `wizard-stream` 现在也从单一 `outline` probe 扩到
  `world-building/{project_id}/regenerate`：它使用最小合法 JSON body `{}`，
  把该组的 owner/fallback 证据从一个 SSE 入口扩到第二个同层级入口。
- `wizard-stream/cleanup/{project_id}` 现在也补入 `phase5-p0`，把该组 owner
  证据再扩到第三条显式 SSE/cleanup 子路径；由于 Python 当前没有同路径 API，
  这条路径暂不进入 `phase5-p0-fallback`。
- `wizard-stream/career-system` 现在也进入 `phase5-p0` 与
  `phase5-p0-fallback`：它使用最小合法 JSON body
  `{"projectId":"test-project-id"}`，把该组的真实 owner/fallback 交集扩到
  第三条同路径 SSE 入口。
- `wizard-stream/characters` 现在也进入 `phase5-p0` 与
  `phase5-p0-fallback`：它使用最小合法 JSON body
  `{"projectId":"test-project-id"}`，把该组的真实 owner/fallback 交集扩到
  第四条同路径 SSE 入口。
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
- `memories` 现在也从单一 `stats` probe 扩到
  `POST /api/memories/projects/{project_id}/search?query=test`：它利用 Python
  侧必填 `query` 查询参数与 Rust 侧可接受 `{}` body 的交集，补上同组第二条
  查询入口 owner/fallback 证据。
- `settings/api-key` 现在也加入 `phase5-p0` 与 `phase5-p0-fallback`，因此
  `settings` 不再只覆盖“根路径 + provider 探测子路由”，还补上了已保存凭据
  读取入口的同路径 owner/fallback 证据。

### 4.2 P1

这些组已经基本 Rust-owned，但缺少部署后证据链：

1. `auth`
2. `users`
3. `characters`
4. `outlines`
5. `book_import`

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
  `book-import-apply-stream-auth-guard-rust`
- 这还不是完整的 P1 业务 smoke，只是把 `auth` / `users` 从“纯文档待办”
  推进到第一版可执行 profile；现在又把 `characters`、`outlines`、
  `book_import` 三组推进到 starter slice，但后续仍需补真实登录态、
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
  以及 cancel/apply/retry-stream/apply-stream 写侧边界）
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
- `projects` 的 fallback 现在不只剩未登录列表线索，还补入了同路径公开
  `validate-import` 成功断言，可直接证明 owner 已切回 Python 而不是只靠
  `401` 语义侧面判断
- `projects/import` 现在也进入 `phase5-p0-fallback`，因此 `projects` 已同时
  拥有列表读侧、public validator 成功态、multipart 写侧三类回切线索
- `projects/export-data` 现在也进入 `phase5-p0-fallback`，因此 `projects`
  又增加了一条合法 JSON body 进入后的写侧回切线索
- `wizard-stream/world-building/{project_id}/regenerate` 现在也进入
  `phase5-p0-fallback`，因此该组不再只靠 `outline` 单路径来证明回切后的
  SSE 入口 owner
- `memories/search` 现在也进入 `phase5-p0-fallback`，因此该组不再只靠
  `/stats` 单路径来证明回切后的 Python API owner
- `chapters` 的 Python fallback 现在也补进了 project-path 列表探针
  `/api/chapters/project/{project_id}`，不再只靠 analysis/batch/regeneration
  子路由侧面证明

---

## 6. 当前 schema assumptions

以下假设在 Phase 5 期间仍然成立：

1. **schema owner 仍是 Python Alembic，不是 Rust。**
2. **Rust 只消费既有 schema，不应在部署时隐式修表。**
3. **shared DB 下的 route-group cutover，不应绑定新的 schema 扩张。**
4. **`analysis_tasks`、`batch_generation_tasks`、`regeneration_tasks` 的第一轮默认值语义已在仓库层收口，但真实环境仍依赖迁移已落地。**
5. **`organization_members`、`projects`、`settings` 等中风险共享表，仍需保持“先验证字段级一致性，再扩大切流”的纪律。**

---

## 7. Phase 5 的建议推进顺序

建议按以下顺序推进，而不是平铺所有 route group：

1. 先补 `chapters` / `projects` / `wizard-stream` / `memories` 的 smoke manifest
2. 再把 `settings` / `auth` / `users` 做成稳定业务 smoke
3. 之后按 route group 把 rollback 步骤写成可直接执行的运维手册
4. 最后才评估是否收缩 Python `/api/` catch-all 或移除特定 fallback

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
5. **现有 `chapters` / `wizard-stream` 的 POST owner probes 已经开始发送真实 JSON body，而不是只依赖空请求命中 `401`。**
6. **manifest 现在还区分了 `business` profile，可把公开 JSON/HTML 业务断言和 `route-groups` / `deploy` 物理拆分。**
7. **下一步关键工作是利用这套能力补更强的 stream/business smoke 和 rollback 资产，而不是继续口头判断“差不多可以切了”。**
8. **`phase5-p0` profile 已经可执行，且 `projects` 现在同时拥有更强的
   `validate-import` public/business probe、`import` multipart 写侧鉴权 probe，
   以及 `export-data` JSON 写侧鉴权 probe；但整体仍以低前提 owner 证据为主，
   下一步仍需继续补更多 business/SSE smoke，才能支撑更激进的 cutover 判断。**
9. **`memories` 已从单一 `/stats` 扩到 `stats + search` 双 probe，说明 P0
   资产建设正在从“是否命中 owner”单点判断，向“同组多入口最小可执行证据链”
   演进。**
10. **`settings` 的 P0 fallback 也已从单一 `/api/settings` 扩到
    `/api/settings + /api/settings/fetch-models + /api/settings/test +
    /api/settings/check-function-calling`，说明低前提回切证据正在向真实子路由
    而不是根路径单点推进。**
11. **P0 route-group rollback 已经有第一版 operator-ready runbook，但回切后的
   Python 成功条件还没有完全 probe 化，短期内仍需配合人工核对。**
12. **rollback smoke 已经可以按 `route_group` 直接筛选，这比手工维护 probe
   名映射更接近真正可执行的运维步骤。**
13. **第一版 Python fallback 成功条件已经从 operator clue 前进到独立
    `phase5-p0-fallback` smoke profile，且 `projects/validate-import` 已进入
    同路径 public-success 断言；但整体仍未覆盖全部 P0 业务路径，不代表
    fallback 验证已经完整自动化。**
14. **gateway smoke 输出现在还会附带 `owner_counts` / `route_group_counts` /
    `route_group_probe_names` 汇总，因此 P0 rollback / fallback 结果已经更适合
    被直接引用到操作记录，而不是只保留一串 probe 明细。**
15. **P1 现在也开始拥有 `users` 的读写 fallback 资产；但这些 `users`
    probe 目前只证明同路径 auth-boundary owner 已回到 Python，不证明
    管理员列表、管理员写侧或当前用户业务语义已经完整等价。**
16. **`users` 现在还开始拥有写侧 fallback 资产；但 `set-admin` /
    `reset-password` 现阶段同样只应解读为同路径登录边界证据，不应误读成
    管理员写侧行为已完成语义对齐。**
14. **`characters` / `outlines` / `book_import` 现在也已进入 `phase5-p1`
    的 starter owner smoke，并补入了第一版同路径 Python fallback 线索；但当前
    仍只有未登录边界证据，不应误判为已经完成了导入流、SSE 或生成流的完整业务验证。**
