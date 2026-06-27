# API 模型获取功能集成指南

## 概述

MuMuNovel 当前通过 Rust settings API 提供“从 AI 提供商获取可用模型列表”功能。
前端 Settings 页面通过 `settingsApi.fetchModels(...)` 调用该接口，用户仍可在获取失败时手动输入模型名。

旧 Python FastAPI 路径已退役。不要再使用 `app.schemas.settings`、
`backend/app/api/settings.py`、`backend/app/schemas/settings.py` 或
`backend/test_fetch_models.py` 作为集成依据。

## 当前 API

### Endpoint

```text
POST /api/settings/fetch-models
```

Rust 内部 route 常量：

```text
backend-rs/src/api/settings.rs
SETTINGS_FETCH_MODELS_ROUTE = "/settings/fetch-models"
```

### Request

```json
{
  "api_key": "sk-xxx",
  "api_base_url": "https://api.deepseek.com/anthropic",
  "provider": "deepseek",
  "models_url": null
}
```

字段说明：

- `api_key`：模型提供商或代理站 API Key。
- `api_base_url`：模型提供商或代理站 Base URL。
- `provider`：提供商标识，用于兼容逻辑与 UI 语义。
- `models_url`：可选，自定义模型列表 URL；提供后优先尝试。

### Response

```json
{
  "success": true,
  "models": [
    {
      "id": "deepseek-chat",
      "owned_by": "deepseek"
    }
  ],
  "message": "成功获取 1 个可用模型",
  "error": null,
  "error_type": null
}
```

错误响应保持同一外壳：

```json
{
  "success": false,
  "models": [],
  "message": "获取模型列表失败",
  "error": "认证失败",
  "error_type": "AuthenticationError"
}
```

## 前端集成

### API service

文件：

```text
frontend/src/services/modules/settings.ts
```

调用：

```typescript
settingsApi.fetchModels({
  api_key: apiKey,
  api_base_url: apiBaseUrl,
  provider,
  models_url: modelsUrl,
});
```

### UI component

文件：

```text
frontend/src/components/ModelInputWithFetch.tsx
```

示例：

```tsx
import ModelInputWithFetch from './ModelInputWithFetch';

<ModelInputWithFetch
  value={modelName}
  onChange={setModelName}
  apiKey={apiKey}
  apiBaseUrl={apiBaseUrl}
  provider={provider}
/>
```

### Settings 页面

文件：

```text
frontend/src/components/SettingsCurrentTab.tsx
```

集成原则：

- API Key、Base URL、Provider、Model、Web Research 配置必须走同一个 settings/preset 表单保存链路。
- 不要只在组件局部 state 中保存 Web Research API Key 或 Base URL。
- 获取模型按钮只负责读取当前表单值并调用 API，不应绕过 Settings 保存逻辑。

## 路径剥离逻辑

模型获取会尝试多个候选端点。常见路径：

```text
/api/claudecode
/api/anthropic
/apps/anthropic
/api/coding
/claudecode
/anthropic
/step_plan
/coding
/claude
```

示例：

```text
输入: https://api.deepseek.com/anthropic
候选:
1. https://api.deepseek.com/anthropic/v1/models
2. https://api.deepseek.com/v1/models
3. https://api.deepseek.com/models
```

```text
输入: https://open.bigmodel.cn/api/anthropic
候选:
1. https://open.bigmodel.cn/api/anthropic/v1/models
2. https://open.bigmodel.cn/v1/models
3. https://open.bigmodel.cn/models
```

## 支持的提供商

支持所有 OpenAI 兼容模型列表接口，包括：

- OpenAI
- Azure OpenAI 兼容端点
- DeepSeek
- 智谱 GLM
- Kimi
- 阶跃星辰
- 豆包
- 百炼
- 硅基流动
- OpenRouter
- NewAPI / Sub2API 等聚合代理

用户仍然可以手动输入未返回的模型名称。

## 验证

### Rust focused tests

```powershell
cargo test api::settings --manifest-path "backend-rs/Cargo.toml" --target-dir "E:/Code/ProjectsCode/WorkSpace/Codex/NovelAi/MuMuNovel/.codex-targets/settings-fetch-models"
```

### Rust type check

```powershell
cargo check --manifest-path "backend-rs/Cargo.toml" --target-dir "E:/Code/ProjectsCode/WorkSpace/Codex/NovelAi/MuMuNovel/.codex-targets/story-continuity-ledger-owner"
```

### Gateway manifest

```powershell
python -X utf8 "backend/tools/run_strangler_gateway_smoke.py" --manifest "deploy/strangler-gateway-probes.json" --validate-manifest-only
```

### Manual API check

部署后通过 gateway 测试：

```powershell
curl -X POST http://localhost:8005/api/settings/fetch-models `
  -H "Content-Type: application/json" `
  -H "Cookie: token=YOUR_TOKEN" `
  -d "{\"api_key\":\"sk-xxx\",\"api_base_url\":\"https://api.openai.com/v1\",\"provider\":\"openai\"}"
```

## 错误处理

前端应根据 `success`、`message`、`error`、`error_type` 显示用户反馈：

- `ValidationError`：提示补全 API Key / Base URL。
- `AuthenticationError`：提示 API Key 无效或权限不足。
- `EndpointNotFound`：提示该提供商可能不支持模型列表接口。
- `TimeoutError`：提示请求超时。
- `NetworkError`：提示网络或代理配置异常。

## 后续优化

1. 为相同配置增加短时缓存。
2. 在预设管理中复用模型获取能力。
3. 为不同 provider 增加更细的兼容测试。
4. 对 Web Research API Key/Base URL 保存路径增加回归测试。

## 当前文件

- `backend-rs/src/api/settings.rs` - Rust API owner
- `frontend/src/services/modules/settings.ts` - 前端 API client
- `frontend/src/components/ModelInputWithFetch.tsx` - 模型输入与获取组件
- `frontend/src/components/SettingsCurrentTab.tsx` - Settings 页面集成点

---

**最后更新**：2026-06-25
**状态**：Rust-owned production API
