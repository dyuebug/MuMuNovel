# Frontend 模块文档

[根目录](../CLAUDE.md) > **frontend**

---

## 变更记录

### 2026-04-20
- 按当前 React 代码刷新模块文档
- 修正入口、主题系统、路由树、E2E 与构建信息
- 删除对旧 `ConfigProvider` 入口结构的过时描述

---

## 模块职责

`frontend/` 是 MuMuNovel 的 React 客户端，负责：
- 页面路由与受保护访问控制
- 小说项目、章节、角色、设置等可视化操作
- 后台任务中心与长任务进度反馈
- SSE / HTTP 客户端消费
- 主题模式与全局样式
- Playwright E2E 回归

---

## 真实入口与启动

### 入口文件
- `src/main.tsx`：挂载 React 根节点，并通过 `ThemeProvider` 注入主题上下文
- `src/App.tsx`：定义整棵路由树、懒加载页面、全局 `BackgroundTaskCenter`
- `src/routes/projectPageLoaders.ts`：项目页核心子页面按需加载

### 启动命令

```bash
cd frontend
npm install
npm run dev
```

### 常用命令

```bash
cd frontend
npm run build
npm run build:analyze
npm run lint
npm run e2e
npm run e2e:auth
```

---

## 目录地图

```text
frontend/
├── src/
│   ├── main.tsx
│   ├── App.tsx
│   ├── pages/                 # 页面
│   ├── components/            # 组件
│   ├── services/              # API 与业务客户端
│   ├── store/                 # Zustand / event bus
│   ├── theme/                 # ThemeProvider 与主题状态
│   ├── routes/                # 懒加载 helper
│   ├── utils/                 # SSE / session / 其他工具
│   └── assets/
├── e2e/                       # Playwright 用例
├── public/
├── package.json
├── vite.config.ts
└── playwright.config.ts
```

---

## 路由结构

### 公开路由
- `/login`
- `/auth/callback`

### 受保护路由
- `/`
- `/projects`
- `/wizard`
- `/inspiration`
- `/settings`
- `/prompt-templates`
- `/mcp-plugins`
- `/user-management`
- `/chapters/:chapterId/reader`

### 项目内嵌套路由
挂在 `/project/:projectId`：
- `world-setting`
- `careers`
- `outline`
- `characters`
- `relationships`
- `relationships-graph`
- `organizations`
- `chapters`
- `chapter-analysis`
- `foreshadows`
- `writing-styles`
- `prompt-workshop`
- `sponsor`

说明：`src/pages/` 中还有 `BookImport.tsx`、`BookshelfPage.tsx` 等页面文件，但当前 `src/App.tsx` 未暴露对应路由。

---

## 主题、状态与数据流

### 主题系统
- `src/theme/ThemeProvider.tsx`
- `src/theme/themeConfig.ts`
- `src/theme/themeContext.ts`
- `src/theme/themeStorage.ts`
- `src/theme/useThemeMode.ts`

### 状态与通信
- `src/store/index.ts`：Zustand store
- `src/store/eventBus.ts`：跨组件事件总线

### API / SSE
- `src/services/`：封装 HTTP 请求与业务 API
- `src/utils/sseClient.ts`：SSE 客户端
- `BackgroundTaskCenter`：后台任务 UI 汇总入口

---

## 构建与测试

### Vite 构建
- `vite.config.ts` 默认把构建产物输出到 `../backend/static`
- 支持 `build:analyze`
- 启用了按依赖类型与部分业务模块的 manual chunks

### Playwright
- 配置文件：`playwright.config.ts`
- 测试目录：`e2e/`
- 默认本地地址：`http://127.0.0.1:5175`
- `webServer.command`：`npm run dev -- --host 127.0.0.1 --port 5175`
- `E2E_REAL_BACKEND=1` 时会切换为单 worker 以降低不稳定性

### 当前测试状态
- 已有 E2E：`auth.spec.ts`、`background-task-pages.spec.ts`、`wizard-background-tasks.spec.ts`、`outline-expand-flow.spec.ts`、`inspiration-resume.spec.ts`、`inspiration-web-research-payload.spec.ts`
- 目前没有前端单元测试或组件测试

---

## 关键依赖

- `react`, `react-dom`, `react-router-dom`
- `antd`, `@ant-design/icons`
- `zustand`, `axios`, `dayjs`
- `@xyflow/react`, `dagre`
- `@dnd-kit/core`, `@dnd-kit/sortable`
- `canvas-confetti`, `react-diff-viewer-continued`
- `vite`, `typescript`, `eslint`, `@playwright/test`

---

## 开发约定

- 页面放 `src/pages/`，共享 UI 放 `src/components/`
- 新路由必须同步修改 `src/App.tsx`
- 与项目页强相关的大型页面优先走懒加载，保持首屏体积可控
- API 调用优先复用已有 `services` 封装，不要在页面中散落裸 `fetch`
- 涉及长任务体验的改动要同时检查进度展示、恢复入口与错误态
- 构建输出目录指向后端静态目录，修改 Vite 配置时要考虑后端托管行为

---

## 风险与注意事项

- `src/pages/` 中存在未接入路由的页面文件，阅读代码时要区分“已上线入口”与“潜在/遗留页面”
- 项目详情页采用嵌套路由，父级布局变更可能影响所有子页面
- 懒加载入口分散在 `App.tsx` 与 `projectPageLoaders.ts`，改名时必须同步
- 前端缺少组件级测试，较大 UI 重构时应依赖 E2E 与手动回归双重验证

---

## 下一步推荐阅读

1. `src/main.tsx`
2. `src/App.tsx`
3. `src/routes/projectPageLoaders.ts`
4. `src/services/api.ts`
5. 当前任务相关页面与对应组件
6. `playwright.config.ts` 与目标 E2E 用例

---

**最后更新**: 2026-04-20
**模块版本**: 1.3.9
