# API 模型获取功能实施总结

## 当前状态

API 模型获取功能已经从旧 Python FastAPI 实现迁移到 Rust settings API。
当前生产路径由 Rust backend 提供，前端通过统一 settings client 调用。

旧文档中提到的 `backend/app/api/settings.py`、
`backend/app/schemas/settings.py` 和 `backend/test_fetch_models.py` 已不是当前实现来源。

## 当前实现

### 后端 Rust owner

- `backend-rs/src/api/settings.rs`
  - `SETTINGS_FETCH_MODELS_ROUTE = "/settings/fetch-models"`
  - `FetchModelsRequest`
  - `fetch_models_endpoint(...)`
  - route 注册：`POST /settings/fetch-models`
  - focused tests 覆盖 route 常量、请求/响应和候选端点行为

Gateway 对外路径仍是：

```text
POST /api/settings/fetch-models
```

### 前端调用

- `frontend/src/services/modules/settings.ts`
  - `settingsApi.fetchModels(...)`
  - 调用 `/settings/fetch-models`，由 API client 统一补齐 gateway/API prefix
- `frontend/src/components/ModelInputWithFetch.tsx`
  - 输入框 + 获取按钮
  - 加载状态
  - 按 `owned_by` 分组展示模型
  - 保留手动输入兜底
- `frontend/src/components/SettingsCurrentTab.tsx`
  - 当前 Settings 页面集成位置
  - API Key、Base URL、Provider、Web Research 配置应走同一 settings/preset 保存链路

## 功能特性

- 支持 OpenAI 兼容的 `/v1/models` 与 `/models` 协议。
- 支持自定义 `models_url`。
- 支持兼容路径剥离，例如 `/anthropic`、`/api/anthropic`、`/coding`、`/step_plan`。
- 支持按 `owned_by` 分组返回模型。
- 对认证失败、端点不存在、超时、网络错误提供稳定错误类型。
- 前端允许用户在动态获取失败时继续手动输入模型名称。

## 支持场景

理论上支持所有 OpenAI 兼容模型列表接口：

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

## 验证方式

当前推荐验证不再使用已删除的 Python `test_fetch_models.py`。

后端 Rust focused validation：

```powershell
cargo test api::settings --manifest-path "backend-rs/Cargo.toml" --target-dir "E:/Code/ProjectsCode/WorkSpace/Codex/NovelAi/MuMuNovel/.codex-targets/settings-fetch-models"
```

Rust 全量类型检查：

```powershell
cargo check --manifest-path "backend-rs/Cargo.toml" --target-dir "E:/Code/ProjectsCode/WorkSpace/Codex/NovelAi/MuMuNovel/.codex-targets/story-continuity-ledger-owner"
```

Gateway manifest 验证：

```powershell
python -X utf8 "backend/tools/run_strangler_gateway_smoke.py" --manifest "deploy/strangler-gateway-probes.json" --validate-manifest-only
```

前端构建：

```powershell
cd frontend
npm run build
```

## 残留风险

- 真实外部 provider 模型列表接口差异较大，仍需要按实际代理站点验证。
- `api_key + api_base_url + provider` 的缓存策略尚未统一落地。
- Python migrator/Alembic 仍存在，但与该功能的生产 API runtime 无关。

## 后续优化

1. 对相同 provider/base URL 的模型列表结果增加短时缓存。
2. 在预设管理中复用同一模型获取组件。
3. 对错误类型提供更细的 UI 引导，例如认证错误、路径错误、代理不支持模型列表。
4. 增加 provider-specific 兼容测试，避免真实代理返回格式漂移。

---

**实施状态**：Rust-owned production API
**最后更新**：2026-06-25
