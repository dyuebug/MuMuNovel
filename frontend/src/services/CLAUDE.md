# Services 子模块文档

[根目录](../../CLAUDE.md) > [frontend](../CLAUDE.md) > **src/services**

---

## 变更记录

### 2026-04-20
- 新增 `src/services` 子模块文档
- 基于当前服务文件、页面调用与后台任务链路整理职责边界
- 标注统一 API 客户端、章节任务客户端与项目职业辅助服务的关系

---

## 模块职责

`frontend/src/services/` 负责前端侧 HTTP 业务客户端与少量业务辅助服务，负责：
- 统一封装 `/api` 请求与错误处理
- 对项目、章节、角色、大纲、设置、提示词工坊、后台任务等接口做按域分组
- 把后台任务状态同步进前端 store
- 为页面与组件提供稳定的服务调用入口

原则：页面与组件尽量依赖 `services/` 暴露的语义化 API，而不是散落裸 `axios` / `fetch`。

---

## 真实入口关系

- 主入口文件：`frontend/src/services/api.ts`
- 配套基础设施：`frontend/src/utils/sseClient.ts`
- 页面/组件主要通过以下服务对象消费接口：
  - `projectApi`
  - `outlineApi`
  - `chapterApi`
  - `chapterBatchTaskApi`
  - `chapterSingleTaskApi`
  - `backgroundTaskApi`
  - `settingsApi`
  - `characterApi`
  - 其他按域导出的 `*Api`

典型调用关系：
- `ProjectDetail.tsx` / `ProjectList.tsx` → `projectApi`
- `Outline.tsx` → `outlineApi` / `backgroundTaskApi` / `settingsApi`
- `Chapters.tsx` → `chapterApi` / `chapterBatchTaskApi` / `projectApi`
- `BackgroundTaskCenter.tsx` → `backgroundTaskApi` / `chapterBatchTaskApi` / `chapterSingleTaskApi` / `chapterApi`
- `AIProjectGenerator.tsx` → `backgroundTaskApi` / `wizardStreamApi`

说明：当前 `services/` 以 `api.ts` 为绝对核心，其他服务文件更多是补充型业务 helper。

---

## 当前文件分组

### 1. 统一 API 客户端
- `api.ts`

关键点：
- 创建统一 `axios` 实例，默认 `baseURL='/api'`、`withCredentials=true`
- 在响应拦截器中统一做错误翻译、登录跳转、toast 节流与错误日志控制
- 既包含传统 REST API，也包含 SSE POST 场景封装入口
- 文件体量大，实际是“前端 API 门面层”

### 2. 项目与内容域 API
主要定义在 `api.ts` 中：
- `projectApi`
- `outlineApi`
- `characterApi`
- `chapterApi`
- `writingStyleApi`
- `foreshadowApi`
- 以及其他内容管理相关对象

关键点：
- 这些对象按业务域分组，但物理上大多仍集中在 `api.ts`
- `chapterApi` 是高复杂度域，不只做 CRUD，还覆盖分析、候选稿、局部重写、质量指标、质量趋势等能力
- `projectApi` 不只是项目列表/详情，也承载导入导出等项目级能力

### 3. 后台任务与长任务客户端
主要定义在 `api.ts` 中：
- `backgroundTaskApi`
- `chapterBatchTaskApi`
- `chapterSingleTaskApi`
- `wizardStreamApi`

关键点：
- 这组 API 负责创建、查询、取消、恢复后台任务，并与 `useBackgroundTaskStore` 同步状态
- `chapterBatchTaskApi` 与 `chapterSingleTaskApi` 专门处理章节生成任务，不完全等同于通用 `backgroundTaskApi`
- 后台任务接口会主动写入 store，而不是只返回远端数据

### 4. 业务补充服务
- `projectCareers.ts`
- `changelogService.ts`
- `versionService.ts`

关键点：
- 这些文件属于较轻量的业务辅助服务，不像 `api.ts` 那样承担统一入口职责
- `changelogService.ts`、`versionService.ts` 偏展示/版本信息读取
- `projectCareers.ts` 面向项目职业数据聚合或辅助处理

---

## API / SSE 协作方式

### HTTP
- 使用统一 `axios` 实例处理绝大多数 JSON 请求
- 统一错误翻译为中文提示，并控制 toast 去重/节流

### SSE
- `src/utils/sseClient.ts` 提供 `SSEClient`、`SSEPostClient`、`ssePost()`
- `api.ts` 和组件会用 `ssePost()` 处理流式生成、流式分析、长连接进度更新
- SSE 封装支持：
  - `progress`
  - `chunk`
  - `result`
  - `error`
  - `done`
  - 心跳与 inactivity timeout

说明：严格来说 SSE 基础设施在 `utils/`，但业务入口和状态消费都由 `services/api.ts` 牵引。

---

## 与 store 的耦合关系

- `api.ts` 直接依赖：
  - `useStore`
  - `useBackgroundTaskStore`
- 典型行为：
  - 根据 `useStore.getState().projects` 识别已知项目
  - 在任务创建、轮询、取消、恢复时调用 `useBackgroundTaskStore.getState().upsertTask()` / `removeTask()` / `pruneTasksByProjectIds()`

关键点：
- `services/` 在本项目中不是纯无状态网络层，已经承担了部分前端运行态同步职责
- 因此改 `api.ts` 时，必须连看 `store/backgroundTasks.ts` 与相关页面/组件

---

## 关键事实

- `api.ts` 是前端最核心的服务文件，聚合了大部分业务域 API
- `chapterApi` 与两个章节任务 API 共同构成章节生成/分析/恢复的前端调用面
- `backgroundTaskApi` 不是独立于 UI 的纯请求层，它会联动后台任务 store
- SSE 通用能力在 `utils/sseClient.ts`，但实际业务使用面主要通过 `services/api.ts` 暴露
- 当前 `services/` 的主要风险不是文件太多，而是 `api.ts` 过于中心化

---

## 开发约定

- 新接口优先追加到语义最接近的现有 `*Api` 对象，而不是在页面里直接发请求
- 改接口返回结构前，先追踪页面、组件、store 是否直接消费该字段
- 涉及后台任务的接口必须同步考虑 `useBackgroundTaskStore` 的写入策略
- 新增流式接口时，优先复用 `ssePost()` 与现有消息类型约定
- 若 `api.ts` 继续膨胀，优先按业务域拆分，但拆分前要保护现有调用路径

---

## 风险与注意事项

- `api.ts` 体量大、调用广，局部修改很容易引发跨页面回归
- 某些 API 调用除了返回数据，还会带 store 副作用，不能按“纯函数”心智理解
- 登录态与错误提示都集中在拦截器，改动全局错误处理要评估全站提示体验
- 章节生成相关接口混合 REST、SSE、后台任务轮询三种模式，改一处要补看整条链路

---

## 推荐阅读

1. `frontend/src/services/api.ts`
2. `frontend/src/utils/sseClient.ts`
3. `frontend/src/components/BackgroundTaskCenter.tsx`
4. `frontend/src/pages/Chapters.tsx`
5. `frontend/src/pages/Outline.tsx`
6. `frontend/src/components/AIProjectGenerator.tsx`

---

**最后更新**: 2026-04-20
**模块版本**: 1.3.9
