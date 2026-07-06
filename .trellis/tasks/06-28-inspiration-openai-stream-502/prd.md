# 灵感模式 OpenAI stream 502

## Goal

修复灵感模式在“联网搜索增强”开启后生成选项时遇到 OpenAI stream HTTP 502 的用户体验和恢复路径。上游临时 502 不应让创作流程只显示裸错误并卡住，系统应给出可操作降级选项、保留重试上下文，并维持现有 Rust inspiration 路由的可维护性。

## Requirements

- 当 `/inspiration/generate-options` 或 `/inspiration/refine-options` 的上游 AI 流式调用返回 HTTP 502/Bad Gateway/临时网关错误时，后端应返回 200 降级响应，而不是 500 硬失败。
- 降级响应必须包含 `options`，让前端能继续展示“重新生成/我自己输入”等可操作选项。
- 降级响应必须保留 `error`，用中文说明是上游 AI 服务临时不可用，建议稍后重试或手动输入。
- 开启 `enable_web_research` 时，降级响应仍应回传本次 `research_query` 与空 `research_assets`，避免前端丢失联网检索上下文。
- 非临时类错误（例如 API Key、Base URL、鉴权、配置缺失）仍应保持 400/错误提示，不伪装成成功。
- 前端应能保存 `lastFailedRequest`，用户点击“重新生成”时复用原请求。
- 不引入 Python runtime fallback；修复应在 Rust route/service 和现有前端交互内完成。

## Acceptance Criteria

- [x] Rust 单元测试覆盖 OpenAI stream 502 降级响应，验证 `options`、`error`、`research_query`。
- [x] Rust 单元测试覆盖 API Key/Base URL 等配置错误仍映射为错误状态，不被降级吞掉。
- [x] 前端 TypeScript 校验通过。
- [x] `cargo check --manifest-path backend-rs/Cargo.toml` 通过。
- [x] 手动或自动验证灵感模式在 502 场景下前端不只弹出裸错误，页面可继续重试或手动输入。

## Notes

- 已确认相关路径：`frontend/src/pages/Inspiration.tsx`、`frontend/src/services/modules/inspiration.ts`、`backend-rs/src/api/inspiration.rs`、`backend-rs/src/ai/clients/openai.rs`。
- 当前失败文案示例：`生成选项失败: AI调用失败: OpenAI stream HTTP 502 Bad Gateway: error code: 502`。
- 附带交付：新增 `redeploy-fast.bat` / `redeploy-fast.ps1`，用于代码改动后的快速重建、自动前端变更识别、交互确认、健康检查和 gateway smoke。
- 验证命令：`cargo test --manifest-path backend-rs/Cargo.toml inspiration -- --nocapture`、`cargo check --manifest-path backend-rs/Cargo.toml`、`npm exec tsc -b`、PowerShell AST 解析 `redeploy-fast.ps1`。
