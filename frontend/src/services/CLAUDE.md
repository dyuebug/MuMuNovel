# Services 子模块文档

[根目录](../../CLAUDE.md) > [frontend](../CLAUDE.md) > **src/services**

---

## 变更记录

### 2026-04-20
- 新增 `src/services` 子模块文档
- 基于当前服务文件、页面调用与后台任务链路整理职责边界
- 标注统一 API 客户端、章节任务客户端与项目职业辅助服务的关系
- 同步更新为模块化服务结构：`modularApi.ts` 为主入口，`api.ts` 为兼容门面

---

## 模块职责

`frontend/src/services/` 负责前端侧 HTTP 业务客户端与少量业务辅助服务，负责：
- 统一封装 `/api` 请求与错误处理
- 对项目、章节、角色、大纲、设置、提示词工坊、后台任务等接口做按域分组
- 为页面、组件与 store hook 提供稳定的服务调用入口
- 把历史兼容入口与新的模块化入口分离，降低中心化风险

原则：页面与组件尽量依赖 `services/` 暴露的语义化 API，而不是散落裸 `axios` / `fetch`。

---

## 真实入口关系

- 推荐主入口：`frontend/src/services/modularApi.ts`
- 兼容门面：`frontend/src/services/api.ts`
- 统一 HTTP 客户端：`frontend/src/services/core/httpClient.ts`
- 配套基础设施：`frontend/src/utils/sseClient.ts`
- 真实业务实现：`frontend/src/services/modules/*.ts`

典型调用关系：
- `ProjectDetail.tsx` / `ProjectList.tsx` → `projectApi`
- `Outline.tsx` → `outlineApi` / `backgroundTaskApi` / `settingsApi`
- `Chapters.tsx` → `chapterApi` / `chapterBatchTaskApi` / `projectApi`
- `BackgroundTaskCenter.tsx` → `backgroundTaskApi` / `chapterBatchTaskApi` / `chapterSingleTaskApi` / `chapterApi`
- `AIProjectGenerator.tsx` → `backgroundTaskApi` / `wizardStreamApi`

说明：当前 `services/` 已经演进为“`core/httpClient.ts` + `modules/*` + `modularApi.ts` + `api.ts` 兼容层”的结构，新增代码应优先走 `modularApi.ts` 或直接按域导入模块。

---

## 当前文件分组

### 1. 统一 HTTP 客户端
- `core/httpClient.ts`

关键点：
- 创建统一 `axios` 实例，默认 `baseURL='/api'`、`withCredentials=true`
- 在响应拦截器中统一做错误翻译、登录跳转、toast 节流与错误日志控制
- 这是 HTTP 请求层唯一真实实现，不承担业务域聚合职责

### 2. 业务域模块
- `modules/projects.ts`
- `modules/outlines.ts`
- `modules/chapters.ts`
- `modules/chapterBatchTasks.ts`
- `modules/chapterSingleTasks.ts`
- `modules/backgroundTasks.ts`
- `modules/wizardStreams.ts`
- 以及其他按域拆分的 `modules/*.ts`

关键点：
- 各业务域 API 已按模块拆分，避免继续堆叠在单一超级文件中
- 高复杂度域如章节、后台任务、向导流式生成已单独收口，便于后续维护与测试
- `modules/*` 是后续新增接口的首选落点

### 3. 入口导出层
- `modularApi.ts`
- `api.ts`

关键点：
- `modularApi.ts` 是当前推荐的前端服务聚合入口
- `api.ts` 只保留兼容导出与默认 `api` 转发，用于旧代码或历史导入路径
- 新功能不应继续向 `api.ts` 塞业务实现

### 4. SSE 与长任务协作层
- `src/utils/sseClient.ts`
- `modules/wizardStreams.ts`
- `modules/wizardBackgroundPolling.ts`
- `modules/backgroundTasks.ts`

关键点：
- 流式生成与后台任务不再混在 `api.ts` 中，而是拆到对应模块
- 长任务调用需要同时关注进度消费、恢复链路与页面侧交互反馈
- SSE 通用能力仍在 `utils/`，业务侧通过模块化 API 使用它

---

## API / SSE 协作方式

### HTTP
- 使用 `core/httpClient.ts` 中的统一 `axios` 实例处理绝大多数 JSON 请求
- 统一错误翻译为用户可读提示，并控制 toast 去重/节流

### SSE
- `src/utils/sseClient.ts` 提供 `SSEClient`、`SSEPostClient`、`ssePost()`
- `modules/wizardStreams.ts` 等模块基于 `ssePost()` 封装业务流式接口
- SSE 封装支持：
  - `progress`
  - `chunk`
  - `result`
  - `error`
  - `done`
  - 心跳与 inactivity timeout

说明：严格来说 SSE 基础设施在 `utils/`，但页面与组件应优先通过 `modularApi.ts` 暴露的语义化接口消费，而不是直接拼装底层调用。

---

## 与 store 的耦合关系

- `store/hooks.ts` 当前通过 `services/modularApi.ts` 消费多个 `*Api`
- 后台任务与章节任务的调用链仍需要连看 `store/backgroundTasks.ts`、消费页面与相关组件
- `services/api.ts` 已经不再承载业务实现，只作为兼容门面存在

关键点：
- 当前状态层与服务层仍然存在协作关系，但业务实现已不再反向堆回 `api.ts`
- 改动后台任务、章节生成或恢复链路时，必须同时检查 store 与 UI 消费点
- 若要进一步解耦，应优先从模块边界与调用入口一致性入手

---

## 关键事实

- `modularApi.ts` 是当前前端推荐服务主入口
- `api.ts` 已退化为兼容层，应保持冻结，避免继续扩张
- `core/httpClient.ts` 是统一 HTTP 客户端唯一真实实现
- `chapterApi` 与章节任务 API 共同构成章节生成/分析/恢复的前端调用面
- 后台任务与流式生成链路已拆到独立模块，但依然是高回归风险区域

---

## 开发约定

- 新接口优先追加到语义最接近的 `modules/*.ts` 中
- 页面/组件默认从 `services/modularApi.ts` 或直接从对应模块导入
- 除兼容需求外，不要新增对 `services/api.ts` 的运行时代码依赖
- 改接口返回结构前，先追踪页面、组件、store 是否直接消费该字段
- 新增流式接口时，优先复用 `ssePost()` 与现有消息类型约定

---

## 风险与注意事项

- 后台任务、章节生成、向导流式生成仍然跨越 REST、SSE、轮询与 UI 状态恢复，多链路改动要连看整条调用面
- 兼容门面 `api.ts` 与主入口 `modularApi.ts` 并存，团队需要避免把兼容层重新当成主入口使用
- 全局错误处理集中在 `core/httpClient.ts` 的拦截器，修改时要评估全站提示体验
- 章节生成相关接口仍是最高复杂度服务域，局部修改容易带来跨页面回归

---

## 推荐阅读

1. `frontend/src/services/modularApi.ts`
2. `frontend/src/services/core/httpClient.ts`
3. `frontend/src/utils/sseClient.ts`
4. `frontend/src/services/modules/backgroundTasks.ts`
5. `frontend/src/pages/Chapters.tsx`
6. `frontend/src/pages/Outline.tsx`