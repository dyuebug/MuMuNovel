# 前端服务层约定

## 1. 目标

前端服务层的设计目标是：

- 统一请求入口
- 明确模块边界
- 避免页面直接依赖底层请求细节
- 为后续重构与兼容迁移提供稳定出口

## 2. 分层结构

推荐分层如下：

```text
core/httpClient.ts
    ↓
modules/*
    ↓
modularApi.ts
    ↓
api.ts（兼容门面）
    ↓
pages / hooks / store / components
```

### 2.1 `core/httpClient.ts`

职责：

- 封装底层 HTTP 请求能力
- 提供统一的错误处理辅助能力
- 提供请求配置与公共类型

不负责：

- 具体业务领域 API 命名
- 跨模块业务聚合

### 2.2 `modules/*`

职责：

- 按业务领域组织 API 调用
- 暴露领域级命名函数
- 维持领域内的相对内聚

建议按领域拆分，例如：

- chapters
- outlines
- characters
- projects
- background tasks

### 2.3 `modularApi.ts`

职责：

- 汇总 `modules/*` 的命名导出
- 汇总 `core/httpClient.ts` 中需要对外暴露的核心能力
- 作为新的统一导入出口

要求：

- 只做聚合，不写业务逻辑
- 只保留受约定保护的导出来源

### 2.4 `api.ts`

职责：

- 作为兼容门面保留旧调用入口
- 避免旧代码一次性大面积替换

要求：

- 必须显式标注兼容层性质
- 不直接从 `modules/*` 拼装业务实现
- 后续新代码优先依赖 `modularApi.ts`

## 3. 导入约定

推荐：

```ts
import { projectApi, chapterApi } from '../services/modularApi'
```

兼容场景下允许：

```ts
import api from '../services/api'
```

不推荐：

```ts
import { ... } from '../services/modules/some-module'
```

除非是在服务层内部，否则页面、组件、store、hooks 不应绕过统一出口直接依赖 `modules/*`。

## 4. 命名约定

- 聚合对象统一采用 `xxxApi`
- 请求配置与公共能力放在 `core/httpClient.ts`
- 兼容导出保持最小集合，不扩大 `api.ts` 的职责

## 5. 反模式

以下做法应避免：

- 在 `api.ts` 中直接实现新业务逻辑
- 页面层直接依赖底层 HTTP client
- 在多个页面中复制粘贴相同的请求包装逻辑
- 为兼容旧接口而无限扩大兼容层职责

## 6. 校验与治理

当前已接入以下治理措施：

- 服务层兼容门面语义校验
- 前端可见文本编码校验

推荐在合并前至少执行：

```bash
cd frontend
npm run build
```

## 7. 后续建议

- 新增 API 能力时，优先落在 `modules/*`
- 需要对外暴露时，再通过 `modularApi.ts` 聚合
- 只有在兼容旧代码时，才允许经过 `api.ts`
- 当兼容层使用点收敛到足够少时，再考虑逐步清理 `api.ts`