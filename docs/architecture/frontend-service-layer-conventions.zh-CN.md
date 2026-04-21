# 前端服务层约定

## 目标

本文档用于固化 MuMuNovel 前端服务层的长期约定，避免新增代码或文档再次把
`src/services/api.ts` 误当成主要实现入口。

## 当前结构

前端服务层采用以下分层：

- `src/services/core/httpClient.ts`：唯一真实 HTTP 客户端实现，统一承载 axios 配置、鉴权和通用拦截逻辑。
- `src/services/modules/*.ts`：按业务域拆分的 API 实现文件。
- `src/services/modularApi.ts`：推荐的聚合导入入口，对外统一暴露常用 `*Api` 与类型导出。
- `src/services/api.ts`：兼容门面，仅保留历史导出路径与默认 `api` 转发，不再承载新的业务实现。

## 导入规则

### 默认规则

- 新运行时代码优先从 `src/services/modularApi.ts` 导入。
- 只有在代码确实需要强聚焦、并且只依赖单一业务域时，才直接从 `src/services/modules/*` 导入。
- 除兼容需求外，不再为新代码增加对 `src/services/api.ts` 的运行时依赖。

### 推荐写法

```ts
import { projectApi, outlineApi } from '../services/modularApi'
```

### 按域直引示例

```ts
import { chapterApi } from '../services/modules/chapters'
```

### 不推荐新增

```ts
import { projectApi } from '../services/api'
```

## 维护规则

### 新增 API 时

1. 优先在对应 `src/services/modules/*.ts` 中添加实现。
2. 如需给多数运行时代码复用，再在 `src/services/modularApi.ts` 中补充聚合导出。
3. 不要把新的业务实现重新堆回 `src/services/api.ts`。

### 调整 HTTP 行为时

- 统一修改 `src/services/core/httpClient.ts`。
- 不要在多个模块里复制新的 axios 实例或重复拦截器逻辑。

### 兼容层边界

- `src/services/api.ts` 只处理历史导出兼容。
- 除非存在真实的遗留集成需求，否则不要继续扩大该文件职责。

## 约束与验证

- `frontend/eslint.config.js` 已限制新增运行时代码继续导入 `services/api.ts`。
- `frontend/scripts/check-service-facade.mjs` 会语义校验 `services/api.ts` 与 `services/modularApi.ts` 的导出关系，确保兼容层仍是薄门面、主入口仍暴露核心 HTTP 合同。
- `npm run lint` 与 `npm run build` 已接入该校验，兼容层一旦回退到手工维护导出清单、或主入口丢失核心导出时会立即失败。
- 变更服务层后，至少执行一次：
  - `cd frontend && npm run validate:services`
  - `cd frontend && npm run lint`
  - `cd frontend && npm run build`

## 相关文档

- `docs/05-代码结构.md`
- `docs/07-前端开发.md`
- `frontend/README.md`
- `frontend/src/services/CLAUDE.md`