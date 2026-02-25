# Frontend 模块文档

[根目录](../CLAUDE.md) > **frontend**

---

## 变更记录 (Changelog)

### 2026-02-21 23:14:25
- 初始化 frontend 模块文档
- 完成模块结构扫描

---

## 模块职责

Frontend 模块是 MuMuAINovel 的前端应用，基于 React 18 + TypeScript 构建，提供用户界面和交互体验。负责展示数据、处理用户输入、调用后端 API、管理应用状态等。

**核心职责：**
- 提供用户界面（基于 Ant Design）
- 处理用户交互和表单验证
- 调用后端 API 接口
- 管理应用状态（Zustand）
- SSE 流式数据接收与展示
- 路由管理（React Router）
- 主题配置与响应式布局

---

## 入口与启动

### 主入口文件

**`src/main.tsx`** - React 应用入口

```typescript
import { StrictMode } from 'react'
import { createRoot } from 'react-dom/client'
import { ConfigProvider } from 'antd'
import zhCN from 'antd/locale/zh_CN'
import App from './App.tsx'

createRoot(document.getElementById('root')!).render(
  <StrictMode>
    <ConfigProvider locale={zhCN} theme={{...}}>
      <App />
    </ConfigProvider>
  </StrictMode>,
)
```

**`src/App.tsx`** - 应用主组件与路由配置

```typescript
import { BrowserRouter, Routes, Route } from 'react-router-dom';

function App() {
  return (
    <BrowserRouter>
      <Routes>
        <Route path="/login" element={<Login />} />
        <Route path="/" element={<ProtectedRoute><ProjectList /></ProtectedRoute>} />
        <Route path="/project/:projectId" element={<ProjectDetail />}>
          <Route path="chapters" element={<Chapters />} />
          <Route path="characters" element={<Characters />} />
          {/* ... */}
        </Route>
      </Routes>
    </BrowserRouter>
  );
}
```

### 启动命令

```bash
# 开发模式
npm run dev

# 生产构建
npm run build

# 预览构建结果
npm run preview

# 代码检查
npm run lint
```

---

## 对外接口

### 页面路由

所有页面组件位于 `src/pages/` 目录：

| 路由路径 | 组件 | 功能 |
|---------|------|------|
| `/login` | `Login.tsx` | 登录页面 |
| `/auth/callback` | `AuthCallback.tsx` | OAuth 回调页面 |
| `/` | `ProjectList.tsx` | 项目列表 |
| `/wizard` | `ProjectWizardNew.tsx` | 智能向导创建项目 |
| `/inspiration` | `Inspiration.tsx` | 灵感模式 |
| `/settings` | `Settings.tsx` | 用户设置 |
| `/prompt-templates` | `PromptTemplates.tsx` | 提示词模板管理 |
| `/mcp-plugins` | `MCPPlugins.tsx` | MCP 插件管理 |
| `/user-management` | `UserManagement.tsx` | 用户管理（管理员） |
| `/project/:projectId` | `ProjectDetail.tsx` | 项目详情（嵌套路由） |
| `/project/:projectId/world-setting` | `WorldSetting.tsx` | 世界观设定 |
| `/project/:projectId/careers` | `Careers.tsx` | 职业等级体系 |
| `/project/:projectId/outline` | `Outline.tsx` | 大纲管理 |
| `/project/:projectId/characters` | `Characters.tsx` | 角色管理 |
| `/project/:projectId/relationships` | `Relationships.tsx` | 角色关系 |
| `/project/:projectId/organizations` | `Organizations.tsx` | 组织管理 |
| `/project/:projectId/chapters` | `Chapters.tsx` | 章节管理 |
| `/project/:projectId/chapter-analysis` | `ChapterAnalysis.tsx` | 章节分析 |
| `/project/:projectId/foreshadows` | `Foreshadows.tsx` | 伏笔管理 |
| `/project/:projectId/writing-styles` | `WritingStyles.tsx` | 写作风格 |
| `/project/:projectId/prompt-workshop` | `PromptWorkshop.tsx` | 提示词工坊 |
| `/project/:projectId/sponsor` | `Sponsor.tsx` | 赞助页面 |
| `/chapters/:chapterId/reader` | `ChapterReader.tsx` | 章节阅读器 |

### 核心组件

通用组件位于 `src/components/` 目录：

| 组件文件 | 功能 |
|---------|------|
| `ProtectedRoute.tsx` | 路由守卫（需要登录） |
| `UserMenu.tsx` | 用户菜单 |
| `AppFooter.tsx` | 页面底部 |
| `SpringFestival.tsx` | 春节装饰组件 |
| `AIProjectGenerator.tsx` | AI 项目生成器 |
| `CharacterCard.tsx` | 角色卡片 |
| `CharacterCareerCard.tsx` | 角色职业卡片 |
| `ChapterReader.tsx` | 章节阅读器 |
| `ChapterAnalysis.tsx` | 章节分析 |
| `ChapterRegenerationModal.tsx` | 章节重新生成弹窗 |
| `PartialRegenerateModal.tsx` | 局部重写弹窗 |
| `PartialRegenerateToolbar.tsx` | 局部重写工具栏 |
| `ChapterContentComparison.tsx` | 章节内容对比 |
| `ExpansionPlanEditor.tsx` | 大纲展开计划编辑器 |
| `FloatingIndexPanel.tsx` | 浮动目录面板 |
| `MemorySidebar.tsx` | 记忆侧边栏 |
| `SSEProgressBar.tsx` | SSE 进度条 |
| `SSEProgressModal.tsx` | SSE 进度弹窗 |
| `SSELoadingOverlay.tsx` | SSE 加载遮罩 |
| `ChangelogModal.tsx` | 更新日志弹窗 |
| `ChangelogFloatingButton.tsx` | 更新日志浮动按钮 |
| `AnnouncementModal.tsx` | 公告弹窗 |
| `AnnotatedText.tsx` | 带注释的文本 |
| `CardStyles.tsx` | 卡片样式 |

---

## 关键依赖与配置

### NPM 依赖 (`package.json`)

**核心框架：**
- `react@18.3.1` - React 框架
- `react-dom@18.3.1` - React DOM
- `react-router-dom@6.28.0` - 路由管理

**UI 组件库：**
- `antd@5.27.6` - Ant Design 组件库
- `@ant-design/icons@5.6.1` - Ant Design 图标

**状态管理：**
- `zustand@5.0.8` - 轻量级状态管理

**HTTP 客户端：**
- `axios@1.12.2` - HTTP 请求库

**工具库：**
- `dayjs@1.11.13` - 日期时间处理
- `canvas-confetti@1.9.4` - 彩纸动画效果
- `react-diff-viewer-continued@3.4.0` - 文本对比组件
- `@dnd-kit/core@6.3.1` - 拖拽功能
- `@dnd-kit/sortable@9.0.0` - 可排序列表

**开发工具：**
- `typescript@5.9.3` - TypeScript
- `vite@7.1.7` - 构建工具
- `@vitejs/plugin-react@5.0.4` - Vite React 插件
- `eslint@9.36.0` - 代码检查

### Vite 配置 (`vite.config.ts`)

```typescript
import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'

export default defineConfig({
  plugins: [react()],
  server: {
    port: 5173,
    proxy: {
      '/api': {
        target: 'http://localhost:8000',
        changeOrigin: true,
      }
    }
  },
  build: {
    outDir: '../backend/static',
    emptyOutDir: true,
  }
})
```

### TypeScript 配置 (`tsconfig.json`)

```json
{
  "compilerOptions": {
    "target": "ES2020",
    "useDefineForClassFields": true,
    "lib": ["ES2020", "DOM", "DOM.Iterable"],
    "module": "ESNext",
    "skipLibCheck": true,
    "moduleResolution": "bundler",
    "allowImportingTsExtensions": true,
    "resolveJsonModule": true,
    "isolatedModules": true,
    "noEmit": true,
    "jsx": "react-jsx",
    "strict": true,
    "noUnusedLocals": true,
    "noUnusedParameters": true,
    "noFallthroughCasesInSwitch": true
  },
  "include": ["src"],
  "references": [{ "path": "./tsconfig.node.json" }]
}
```

---

## 数据流与状态管理

### API 服务层 (`src/services/api.ts`)

所有 API 调用统一封装在 `api.ts` 中，按功能模块划分：

```typescript
// Axios 实例配置
const api = axios.create({
  baseURL: '/api',
  timeout: 120000,
  withCredentials: true,
});

// 响应拦截器：统一错误处理
api.interceptors.response.use(
  (response) => response.data,
  (error) => {
    // 统一错误提示
    message.error(errorMessage);
    return Promise.reject(error);
  }
);

// API 模块
export const authApi = { /* 认证相关 */ };
export const projectApi = { /* 项目管理 */ };
export const characterApi = { /* 角色管理 */ };
export const chapterApi = { /* 章节管理 */ };
// ...
```

**API 调用示例：**
```typescript
import { projectApi } from '../services/api';

const loadProjects = async () => {
  try {
    const projects = await projectApi.getProjects();
    setProjects(projects);
  } catch (error) {
    // 错误已在拦截器中处理
  }
};
```

### SSE 客户端 (`src/utils/sseClient.ts`)

封装 SSE（Server-Sent Events）流式数据接收：

```typescript
export async function ssePost<T>(
  url: string,
  data: any,
  options?: SSEClientOptions
): Promise<T> {
  return new Promise((resolve, reject) => {
    const eventSource = new EventSource(url);

    eventSource.onmessage = (event) => {
      const data = JSON.parse(event.data);
      options?.onMessage?.(data);
    };

    eventSource.onerror = (error) => {
      eventSource.close();
      reject(error);
    };
  });
}
```

**SSE 使用示例：**
```typescript
import { wizardStreamApi } from '../services/api';

const generateWorldBuilding = async () => {
  try {
    await wizardStreamApi.generateWorldBuildingStream(
      { title, description, theme, genre },
      {
        onMessage: (data) => {
          // 实时更新进度
          setProgress(data.progress);
          setStatus(data.status);
        }
      }
    );
  } catch (error) {
    message.error('生成失败');
  }
};
```

### 状态管理 (`src/store/`)

使用 Zustand 进行轻量级状态管理：

```typescript
// src/store/index.ts
import { create } from 'zustand';

interface AppState {
  user: User | null;
  setUser: (user: User | null) => void;
  currentProject: Project | null;
  setCurrentProject: (project: Project | null) => void;
}

export const useAppStore = create<AppState>((set) => ({
  user: null,
  setUser: (user) => set({ user }),
  currentProject: null,
  setCurrentProject: (project) => set({ currentProject: project }),
}));
```

**使用示例：**
```typescript
import { useAppStore } from '../store';

function MyComponent() {
  const { user, setUser } = useAppStore();

  return <div>{user?.display_name}</div>;
}
```

### 事件总线 (`src/store/eventBus.ts`)

用于跨组件通信：

```typescript
type EventCallback = (data?: any) => void;

class EventBus {
  private events: Map<string, EventCallback[]> = new Map();

  on(event: string, callback: EventCallback) {
    if (!this.events.has(event)) {
      this.events.set(event, []);
    }
    this.events.get(event)!.push(callback);
  }

  emit(event: string, data?: any) {
    const callbacks = this.events.get(event);
    if (callbacks) {
      callbacks.forEach(callback => callback(data));
    }
  }

  off(event: string, callback: EventCallback) {
    const callbacks = this.events.get(event);
    if (callbacks) {
      const index = callbacks.indexOf(callback);
      if (index > -1) {
        callbacks.splice(index, 1);
      }
    }
  }
}

export const eventBus = new EventBus();
```

---

## 测试与质量

### 当前状态

- **单元测试**: 暂无
- **组件测试**: 暂无
- **E2E 测试**: 暂无
- **代码检查**: ESLint 配置

### 建议补充

1. **组件测试（Jest + React Testing Library）**
   - 测试核心组件渲染
   - 测试用户交互
   - 测试状态变化

2. **E2E 测试（Playwright）**
   - 测试完整用户流程
   - 测试跨页面交互
   - 测试 SSE 流式响应

3. **视觉回归测试**
   - 使用 Storybook 管理组件
   - 使用 Chromatic 进行视觉测试

---

## 常见问题 (FAQ)

### 如何添加新页面？

1. 在 `src/pages/` 创建页面组件
2. 在 `src/App.tsx` 添加路由配置
3. 如需 API 调用，在 `src/services/api.ts` 添加 API 方法
4. 如需状态管理，在 `src/store/` 添加状态定义

**示例：**
```typescript
// src/pages/NewPage.tsx
export default function NewPage() {
  return <div>New Page</div>;
}

// src/App.tsx
<Route path="/new-page" element={<ProtectedRoute><NewPage /></ProtectedRoute>} />
```

### 如何处理 SSE 流式数据？

1. 使用 `src/utils/sseClient.ts` 中的 `ssePost` 方法
2. 在 `onMessage` 回调中处理实时数据
3. 使用 `SSEProgressModal` 或 `SSEProgressBar` 组件展示进度

**示例：**
```typescript
import { ssePost } from '../utils/sseClient';

const handleGenerate = async () => {
  try {
    await ssePost('/api/wizard-stream/world-building', data, {
      onMessage: (msg) => {
        if (msg.progress) setProgress(msg.progress);
        if (msg.status) setStatus(msg.status);
        if (msg.result) setResult(msg.result);
      }
    });
  } catch (error) {
    message.error('生成失败');
  }
};
```

### 如何自定义主题？

在 `src/main.tsx` 中修改 `ConfigProvider` 的 `theme` 配置：

```typescript
<ConfigProvider
  locale={zhCN}
  theme={{
    token: {
      colorPrimary: '#4D8088',  // 主色调
      colorBgBase: '#F8F6F1',   // 背景色
      colorTextBase: '#2B2B2B', // 文字色
      borderRadius: 6,
    },
    components: {
      Layout: {
        bodyBg: '#F8F6F1',
        headerBg: '#FFFFFF',
      },
      Card: {
        colorBgContainer: '#FFFFFF',
      }
    }
  }}
>
```

### 如何处理用户认证？

1. **登录流程**：
   - 用户在 `/login` 页面输入凭证
   - 调用 `authApi.localLogin()` 或 `authApi.getLinuxDOAuthUrl()`
   - 登录成功后，后端设置 Cookie
   - 前端跳转到首页

2. **路由守卫**：
   - 使用 `ProtectedRoute` 组件包裹需要登录的页面
   - 未登录用户自动跳转到 `/login`

3. **会话管理**：
   - 使用 `authApi.getCurrentUser()` 获取当前用户
   - 使用 `authApi.refreshSession()` 刷新会话
   - 使用 `authApi.logout()` 登出

### 如何调试 API 调用？

1. **浏览器开发者工具**：
   - Network 标签查看请求和响应
   - Console 标签查看日志

2. **Axios 拦截器**：
   - 在 `src/services/api.ts` 中添加请求/响应日志

3. **后端 API 文档**：
   - 访问 http://localhost:8000/docs 查看 Swagger UI
   - 直接在 Swagger UI 中测试 API

---

## 相关文件清单

### 核心文件

```
frontend/
├── src/
│   ├── main.tsx                # 应用入口
│   ├── App.tsx                 # 主组件与路由
│   ├── index.css               # 全局样式
│   ├── App.css                 # 应用样式
│   ├── pages/                  # 页面组件
│   │   ├── Login.tsx
│   │   ├── ProjectList.tsx
│   │   ├── ProjectDetail.tsx
│   │   ├── Characters.tsx
│   │   ├── Chapters.tsx
│   │   └── ...
│   ├── components/             # 通用组件
│   │   ├── ProtectedRoute.tsx
│   │   ├── UserMenu.tsx
│   │   ├── CharacterCard.tsx
│   │   └── ...
│   ├── services/               # API 服务
│   │   ├── api.ts
│   │   ├── changelogService.ts
│   │   └── versionService.ts
│   ├── store/                  # 状态管理
│   │   ├── index.ts
│   │   ├── eventBus.ts
│   │   └── hooks.ts
│   ├── types/                  # 类型定义
│   │   └── index.ts
│   ├── utils/                  # 工具函数
│   │   ├── sseClient.ts
│   │   └── sessionManager.ts
│   └── config/                 # 配置文件
│       └── version.ts
├── public/                     # 静态资源
│   ├── favicon.ico
│   ├── qq.jpg
│   └── WX.png
├── package.json                # NPM 依赖
├── tsconfig.json               # TypeScript 配置
├── vite.config.ts              # Vite 配置
├── eslint.config.js            # ESLint 配置
└── README.md                   # 前端说明文档
```

### 配置文件

- `package.json` - NPM 依赖和脚本
- `tsconfig.json` - TypeScript 编译配置
- `tsconfig.app.json` - 应用 TypeScript 配置
- `tsconfig.node.json` - Node.js TypeScript 配置
- `vite.config.ts` - Vite 构建配置
- `eslint.config.js` - ESLint 代码检查配置

### 静态资源

- `public/favicon.ico` - 网站图标
- `public/qq.jpg` - QQ 群二维码
- `public/WX.png` - 微信群二维码

---

## 开发建议

### 组件开发规范

1. **使用函数组件 + Hooks**
2. **Props 类型定义**：使用 TypeScript 接口定义 Props
3. **状态管理**：优先使用 `useState`，跨组件状态使用 Zustand
4. **副作用处理**：使用 `useEffect`，注意清理副作用
5. **错误处理**：使用 `try-catch` 捕获异步错误，使用 `message.error` 显示错误
6. **加载状态**：使用 `loading` 状态和 Ant Design 的 `Spin` 组件

**示例：**
```typescript
import { useState, useEffect } from 'react';
import { Card, Spin, message } from 'antd';
import { projectApi } from '../services/api';
import type { Project } from '../types';

interface ProjectCardProps {
  projectId: string;
}

export default function ProjectCard({ projectId }: ProjectCardProps) {
  const [project, setProject] = useState<Project | null>(null);
  const [loading, setLoading] = useState(false);

  useEffect(() => {
    loadProject();
  }, [projectId]);

  const loadProject = async () => {
    try {
      setLoading(true);
      const data = await projectApi.getProject(projectId);
      setProject(data);
    } catch (error) {
      message.error('加载项目失败');
    } finally {
      setLoading(false);
    }
  };

  if (loading) return <Spin />;
  if (!project) return null;

  return (
    <Card title={project.title}>
      {project.description}
    </Card>
  );
}
```

### 性能优化建议

1. **使用 React.memo** 避免不必要的重渲染
2. **使用 useMemo 和 useCallback** 缓存计算结果和函数
3. **懒加载路由** 使用 `React.lazy` 和 `Suspense`
4. **虚拟滚动** 对长列表使用虚拟滚动（如 `react-window`）
5. **图片优化** 使用合适的图片格式和尺寸

---

**最后更新**: 2026-02-21 23:14:25
**模块版本**: 1.3.5
