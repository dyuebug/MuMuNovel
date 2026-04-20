# Pages 子模块文档

[根目录](../../CLAUDE.md) > [frontend](../CLAUDE.md) > **src/pages**

---

## 变更记录

### 2026-04-20
- 新增 `src/pages` 子模块文档
- 基于当前 `App.tsx` 路由树整理页面挂载情况
- 标注项目内嵌套路由、首页多视图页与未挂载页面

---

## 模块职责

`frontend/src/pages/` 存放页面级 React 组件，负责：
- 对应 URL 路由与页面布局
- 组织页面级数据加载、交互与状态同步
- 调用 `services/` 和 `store/`，组合 `components/` 渲染业务界面

原则：页面负责“组装”，通用 UI 和纯逻辑尽量下沉到 `components/` / `utils/` / `services/`。

---

## 真实入口关系

- 路由总入口：`frontend/src/App.tsx`
- 项目页懒加载辅助：`frontend/src/routes/projectPageLoaders.ts`
- 项目内页容器：`ProjectDetail.tsx`
- 首页并不是只有项目列表；`ProjectList.tsx` 内部还承载 `settings/mcp/prompts/book-import` 多视图切换

---

## 当前页面分组

### 1. 认证与入口页
- `Login.tsx`
- `AuthCallback.tsx`
- `ProjectList.tsx`
- `ProjectWizardNew.tsx`
- `Inspiration.tsx`

关键点：
- `ProjectList.tsx` 是 `/` 与 `/projects` 的入口页，同时通过 query 参数承载多个子视图
- `Inspiration.tsx` 是独立工作流页面，不在项目详情嵌套路由内

### 2. 全局功能页
- `Settings.tsx`
- `MCPPlugins.tsx`
- `PromptTemplates.tsx`
- `UserManagement.tsx`
- `ChapterReader.tsx`

### 3. 项目详情嵌套路由页
挂在 `/project/:projectId` 下：
- `WorldSetting.tsx`
- `Careers.tsx`
- `Outline.tsx`
- `Characters.tsx`
- `Relationships.tsx`
- `RelationshipGraph.tsx`
- `Organizations.tsx`
- `Chapters.tsx`
- `ChapterAnalysis.tsx`
- `Foreshadows.tsx`
- `WritingStyles.tsx`
- `PromptWorkshop.tsx`
- `Sponsor.tsx`

关键点：
- 这些页面的父容器是 `ProjectDetail.tsx`
- `ProjectDetail.tsx` 除布局外，还负责项目数据预取、导航预加载、移动端侧边栏与主题交互

### 4. 已存在但当前未在 `App.tsx` 直接挂载的页面
- `BookImport.tsx`
- `BookshelfPage.tsx`

说明：
- 这两个页面文件存在，但当前并不是独立顶级路由
- `BookImport.tsx` 通过 `ProjectList.tsx` 的多视图方式懒加载使用
- `BookshelfPage.tsx` 当前也被 `ProjectList.tsx` 懒加载引用，但并未出现在 `App.tsx` 明示路由里

---

## 懒加载与预加载

### `App.tsx` 懒加载
绝大多数页面通过 `lazy()` 加载。

### `projectPageLoaders.ts` 预加载页面
当前显式提供的项目页预加载键：
- `outline`
- `characters`
- `chapters`
- `organizations`
- `careers`
- `relationships`

说明：
- 不是所有项目页都进入预加载表
- 若新增项目内高频页面，可评估是否加入预加载策略

---

## 关键事实

- `ProjectList.tsx` 不是“单纯列表页”，它还是 settings/mcp/prompts/book-import 的宿主页
- `ProjectDetail.tsx` 不只是 layout，还承担项目数据 hydration、导航预取与缓存策略
- `BookImport.tsx` 页面虽然存在，但当前更像首页宿主页中的内部视图
- `Inspiration.tsx` 已带有缓存恢复、联网研究参数与生成设置，不是简单表单页
- `Settings.tsx` 承担 provider/model/research 多分区配置与连接测试，不应被视为普通设置表单

---

## 开发约定

- 新页面先确认是：顶级独立路由、项目内嵌套路由，还是首页宿主页中的内嵌视图
- 新增页面后必须同步修改 `App.tsx`；若属于项目页，还要评估是否接入 `projectPageLoaders.ts`
- 页面内的重型逻辑与大块 UI 应继续拆到 `components/`
- 涉及项目详情导航的改动，要同时检查 `ProjectDetail.tsx` 的预取、菜单 key 和移动端行为
- 涉及首页视图切换的改动，要检查 `ProjectList.tsx` 的 `view` query 参数协议

---

## 风险与注意事项

- 只看文件名容易误判是否已上线；必须以 `App.tsx` 和宿主页面真实引用为准
- `ProjectList.tsx` 与 `ProjectDetail.tsx` 都不是“薄页面”，改动时要警惕副作用范围
- 项目页很多能力依赖 store hydration，单改某个页面可能引入数据新鲜度问题
- 页面懒加载与预加载分散在多个入口，改名或移动文件要同步更新所有 loader

---

## 推荐阅读

1. `frontend/src/App.tsx`
2. `frontend/src/routes/projectPageLoaders.ts`
3. `frontend/src/pages/ProjectList.tsx`
4. `frontend/src/pages/ProjectDetail.tsx`
5. `frontend/src/pages/Inspiration.tsx`
6. 当前任务相关页面

---

**最后更新**: 2026-04-20
**模块版本**: 1.3.9
