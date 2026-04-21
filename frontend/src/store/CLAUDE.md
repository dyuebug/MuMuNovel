# Store 子模块文档

[根目录](../../CLAUDE.md) > [frontend](../CLAUDE.md) > **src/store**

---

## 变更记录

### 2026-04-20
- 新增 `src/store` 子模块文档
- 基于 Zustand store、后台任务 store、事件总线与同步 hooks 整理状态层结构
- 标注项目数据缓存、任务持久化与跨页面事件通信边界

---

## 模块职责

`frontend/src/store/` 负责前端运行态状态管理与轻量跨组件通信，负责：
- 维护项目、角色、大纲、章节等核心实体的客户端缓存
- 持久化后台任务跟踪状态
- 提供数据同步 hooks，把 API 结果写回 store
- 通过事件总线完成少量跨页面解耦通信

原则：store 负责前端状态，不负责路由和渲染；涉及远端 I/O 的具体调用应继续留在 `services/` 或同步 hooks。

---

## 真实入口关系

- 基础实体 store：`frontend/src/store/index.ts`
- 后台任务 store：`frontend/src/store/backgroundTasks.ts`
- 同步层：`frontend/src/store/hooks.ts`
- 跨组件事件总线：`frontend/src/store/eventBus.ts`

典型消费方：
- `ProjectDetail.tsx` / `ProjectList.tsx` / `Outline.tsx` / `Chapters.tsx` / `Characters.tsx` 直接消费 `useStore`
- `BackgroundTaskCenter.tsx`、`SSEProgressModal.tsx`、`SSELoadingOverlay.tsx`、多处页面消费 `useBackgroundTaskStore`
- `ProjectList.tsx` 与 `Settings.tsx` 通过 `eventBus` + `EventNames.SWITCH_TO_MCP_VIEW` 做视图切换联动
- `store/hooks.ts` 调用 `projectApi` / `chapterApi` / `outlineApi` / `characterApi` 并把结果写回 store

---

## 当前文件分组

### 1. 基础实体状态
- `index.ts`

关键点：
- 定义全局 `useStore`
- 管理：
  - `currentProject`
  - `projects`
  - `outlines`
  - `characters`
  - `chapters`
  - `currentChapter`
  - `loading`
  - `lastUpdated`
- 提供基础增删改写方法与 `clearProjectData()`

说明：这是面向项目内容实体的“轻量客户端缓存层”，不是复杂状态机。

### 2. 后台任务运行态状态
- `backgroundTasks.ts`

关键点：
- 定义 `TrackedBackgroundTask` 与 `useBackgroundTaskStore`
- 用 `persist` 中间件持久化任务状态
- 跟踪字段包含：
  - 任务状态、进度、消息
  - `checkpoint`
  - `failedChapters`
  - `activeStoryRepairPayload`
  - `terminalReason` / `reviewRequired` / `canResume`
- 内置任务裁剪、过期清理、按项目清理、按 scope 清理等逻辑

说明：这不是简单的“loading store”，而是后台任务恢复与全局任务中心的运行态基础设施。

### 3. 数据同步 hooks
- `hooks.ts`

关键点：
- 提供 `useProjectSync()`、`useCharacterSync()`、`useOutlineSync()`、`useChapterSync()`
- 负责把 API 结果写回 `useStore`
- 处理列表刷新去重、并发 refresh 复用、collection freshness 记录
- `useChapterSync()` 还封装了单章后台生成后的轮询、SSE 补充、候选稿兜底与章节同步逻辑

说明：`hooks.ts` 是 store 与 services 的衔接层，不只是 React hook 集合。

### 4. 事件总线
- `eventBus.ts`

关键点：
- 提供 `on/off/emit/once/removeAllListeners/listenerCount`
- 当前事件名覆盖：项目、角色、大纲、章节、视图切换
- 已知显式使用场景：`Settings.tsx` 触发 `SWITCH_TO_MCP_VIEW`，`ProjectList.tsx` 监听后切换首页宿主视图

说明：事件总线使用范围相对有限，主要用于少量跨页面/跨视图跳转联动。

---

## 状态层协作方式

### `useStore`
适合：
- 当前项目上下文
- 当前项目下的 outlines / characters / chapters 客户端缓存
- 页面间共享的当前实体状态

### `useBackgroundTaskStore`
适合：
- 长任务追踪
- 任务恢复
- 全局任务中心展示
- 流式生成/后台生成的状态统一入口

### `eventBus`
适合：
- 低频、跨视图、无需持久化的通知类事件
- 例如首页宿主页视图切换

说明：本项目已经形成“实体缓存 / 任务运行态 / 事件通知”三类分工，不应混用。

---

## 与 services 的耦合关系

- `store/hooks.ts` 当前直接依赖 `services/modularApi.ts` 中多个 `*Api`
- `services/api.ts` 现已退化为兼容门面；真实业务实现位于 `services/modules/*`，HTTP 客户端位于 `services/core/httpClient.ts`
  - `store/hooks.ts` 通过 `modularApi.ts` 读取项目、章节与任务相关 API
  - 后台任务相关变更仍需联动检查 `store/backgroundTasks.ts` 与消费组件
- 这意味着当前状态层和服务层仍有协作，但主要通过模块化服务入口完成，而不是回堆到 `services/api.ts`：
  - hooks 负责“请求后写 store”
  - api 负责“任务类接口直接同步 store”

关键点：
- 改动 `backgroundTasks.ts` 的字段定义时，要同时检查 `services/modularApi.ts`、`BackgroundTaskCenter.tsx` 与相关任务消费链路
- 改动 `useStore` 的实体字段时，要同时检查 `store/hooks.ts` 与主要页面消费点

---

## 关键事实

- `useStore` 主要缓存项目内容实体，不做持久化
- `useBackgroundTaskStore` 使用 `persist`，会保留任务恢复所需状态
- `hooks.ts` 不只是读 store，它还承担 refresh 合并、数据新鲜度与任务轮询兜底逻辑
- `eventBus.ts` 当前是补充型通信机制，不是主状态层
- `BackgroundTaskCenter.tsx` 是后台任务 store 的主要 UI 消费者，但不是唯一消费者

---

## 开发约定

- 实体列表/当前项目等可缓存数据优先放 `useStore`
- 可恢复的长任务状态优先放 `useBackgroundTaskStore`
- 需要“拉远端并写缓存”的逻辑优先放 `store/hooks.ts`，不要让页面重复写同步代码
- 低频跨页面通知优先复用 `eventBus`，但若状态需要持久存在，应回到 Zustand store
- 新增 store 字段或任务状态字段时，必须同步检查消费方、清理策略与持久化影响

---

## 风险与注意事项

- `backgroundTasks.ts` 字段和裁剪逻辑较多，轻微改动就可能影响任务恢复、人工复核、继续执行等体验
- `hooks.ts` 中既有网络请求又有 store 写入，改动时要注意并发、重复刷新与 stale 数据问题
- `useStore` 缓存按当前项目上下文组织，切项目时必须留意 `clearProjectData()` 与 hydration 时序
- 事件总线没有类型系统强约束，新增事件名时要统一常量并避免字符串漂移

---

## 推荐阅读

1. `frontend/src/store/index.ts`
2. `frontend/src/store/backgroundTasks.ts`
3. `frontend/src/store/hooks.ts`
4. `frontend/src/store/eventBus.ts`
5. `frontend/src/components/BackgroundTaskCenter.tsx`
6. `frontend/src/pages/ProjectDetail.tsx`

---

**最后更新**: 2026-04-20
**模块版本**: 1.3.9
