# MuMuNovel Frontend

MuMuNovel 的前端基于 React、TypeScript 与 Vite，负责项目管理、章节创作、大纲生成、后台任务中心、提示词工坊与设置管理等页面能力。

## 快速开始

```bash
cd frontend
npm install
npm run dev
```

默认开发命令会启动 Vite，本地页面通常通过后端代理后的 `/api` 路径访问业务接口。

## 常用命令

```bash
cd frontend
npm run dev
npm run build
npm run build:analyze
npm run lint
npm run e2e
npm run e2e:auth
```

## 目录概览

```text
frontend/
├── src/
│   ├── pages/                # 页面
│   ├── components/           # 组件与业务面板
│   ├── services/             # HTTP 客户端、模块化 API、兼容门面
│   ├── store/                # Zustand 状态与事件协作
│   ├── utils/                # SSE、session、通用工具
│   ├── routes/               # 懒加载与路由辅助
│   └── theme/                # 主题系统
├── e2e/                      # Playwright 用例
├── scripts/                  # 构建与分析脚本
└── package.json
```

## 服务层导入规范

当前前端服务层已经收口为三层结构：

- `src/services/core/httpClient.ts`：唯一真实 HTTP 客户端实现
- `src/services/modules/*.ts`：按业务域拆分的 API 实现
- `src/services/modularApi.ts`：推荐的聚合导入入口

兼容说明：

- `src/services/api.ts` 只保留历史导入路径与默认 `api` 转发
- 新运行时代码默认从 `src/services/modularApi.ts` 或对应 `src/services/modules/*` 导入
- ESLint 已限制新增代码继续从 `src/services/api.ts` 导入
- 详细约定见 `../docs/architecture/frontend-service-layer-conventions.zh-CN.md`

推荐写法：

```ts
import { projectApi, outlineApi } from '../services/modularApi'
```

按域直引也可以：

```ts
import { chapterApi } from '../services/modules/chapters'
```

不推荐新增：

```ts
import { projectApi } from '../services/api'
```

## 构建与产物

- `npm run build` 会执行 TypeScript 构建并产出前端静态文件
- Vite 产物输出到 `../backend/static`
- 修改构建配置时，需要同时考虑后端静态托管行为

## Lint 与测试

- `npm run lint`：执行 ESLint 检查，包括服务层导入约束
- `npm run e2e`：运行 Playwright E2E
- `npm run e2e:auth`：运行最小登录回归集

以下真实后端相关 E2E 仅在设置 `E2E_REAL_BACKEND=1` 时运行更有意义：

```bash
E2E_REAL_BACKEND=1 npx playwright test \
  e2e/wizard-background-tasks.spec.ts \
  e2e/inspiration-resume.spec.ts \
  e2e/inspiration-web-research-payload.spec.ts \
  --reporter=line
```

## 开发建议

- 页面尽量复用 `services`、`store`、`utils` 中已有能力，不要在页面里散落裸 `fetch`
- 长任务相关改动要同时检查进度展示、恢复链路、取消逻辑与错误提示
- 新增 API 时优先放入语义最接近的 `src/services/modules/*.ts`
- 修改响应结构时，要同时回看消费页面、组件、store hook 与 E2E