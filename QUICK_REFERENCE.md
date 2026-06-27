# API 模型获取功能 - 快速参考

## 当前状态

模型获取接口已经由 Rust backend 接管。生产调用路径：

```text
POST /api/settings/fetch-models
```

Rust 内部 route：

```text
/settings/fetch-models
```

## 相关文件

- `backend-rs/src/api/settings.rs` - Rust API owner、请求/响应结构、候选端点逻辑、focused tests
- `frontend/src/services/modules/settings.ts` - `settingsApi.fetchModels(...)`
- `frontend/src/components/ModelInputWithFetch.tsx` - 输入框 + 获取按钮 + 模型下拉
- `frontend/src/components/SettingsCurrentTab.tsx` - Settings 页面集成点
- `INTEGRATION_GUIDE.md` - 当前集成说明
- `IMPLEMENTATION_SUMMARY.md` - 当前实现摘要

## 请求示例

```json
{
  "api_key": "sk-xxx",
  "api_base_url": "https://api.deepseek.com/anthropic",
  "provider": "deepseek"
}
```

可选自定义模型列表 URL：

```json
{
  "api_key": "sk-xxx",
  "api_base_url": "https://api.openai.com/v1",
  "provider": "openai",
  "models_url": "https://api.openai.com/v1/models"
}
```

## 响应示例

```json
{
  "success": true,
  "models": [
    {
      "id": "deepseek-chat",
      "owned_by": "deepseek"
    }
  ],
  "message": "成功获取 1 个可用模型"
}
```

## 智能路径剥离

支持识别并剥离常见兼容子路径：

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
输入: https://api.stepfun.com/step_plan
候选:
1. https://api.stepfun.com/step_plan/v1/models
2. https://api.stepfun.com/v1/models
3. https://api.stepfun.com/models
```

## 支持的提供商

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

用户始终可以手动输入模型名作为兜底。

## 前端用法

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

Settings 页面注意事项：

- API Key、Base URL、Provider、Model、Web Research 配置应通过同一 settings/preset 保存链路持久化。
- 获取模型按钮不负责保存配置，只读取当前表单值并请求模型列表。
- 如果 Web Research API Key 或 Base URL 无法保存，优先检查 `SettingsCurrentTab.tsx` 的表单字段绑定和提交 payload。

## 验证命令

```powershell
cargo test api::settings --manifest-path "backend-rs/Cargo.toml" --target-dir "E:/Code/ProjectsCode/WorkSpace/Codex/NovelAi/MuMuNovel/.codex-targets/settings-fetch-models"
```

```powershell
cargo check --manifest-path "backend-rs/Cargo.toml" --target-dir "E:/Code/ProjectsCode/WorkSpace/Codex/NovelAi/MuMuNovel/.codex-targets/story-continuity-ledger-owner"
```

```powershell
python -X utf8 "backend/tools/run_strangler_gateway_smoke.py" --manifest "deploy/strangler-gateway-probes.json" --validate-manifest-only
```

## 已退役内容

以下旧 Python runtime 文件或命令不再适用：

- `backend/app/api/settings.py`
- `backend/app/schemas/settings.py`
- `backend/test_fetch_models.py`
- `python test_fetch_models.py`
- `python -m uvicorn app.main:app`

---

**最后更新**：2026-06-25
**状态**：Rust-owned production API
