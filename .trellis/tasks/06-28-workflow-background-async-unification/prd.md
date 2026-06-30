# 全流程后台异步任务统一化

## Goal

把仍然同步阻塞的生成类流程逐步收敛到现有 Rust `background_tasks` 任务中心，让长耗时 AI 调用、联网检索、生成/补全/展开等动作可以后台执行、可轮询、可恢复呈现。

## Requirements

- 第一批切口聚焦 Inspiration 灵感模式的生成选项、智能补全和选项优化。
- 第二批切口聚焦章节局部重写，把 `partial-regenerate-stream` 从页面直连 SSE 收敛到后台任务结果等待。
- 第三批切口聚焦拆书导入，把 `apply-stream` 和失败步骤重试收敛到统一后台任务等待。
- 第四批切口聚焦 Wizard 世界观重生成，复用既有 `world_regenerate` 后台任务。
- 第五批切口聚焦 feature command 中残留的同步大纲/角色生成入口，复用既有 `outline_generate` / `character_generate` 后台任务。
- 第六批切口聚焦 AI 去味，新增 `polish_text` / `polish_batch` 后台任务封装。
- 前端默认通过统一后台任务 API 创建任务并等待结果，保留原同步 API 作为兼容回退。
- 后端复用现有 `TaskRegistry`、任务持久化、SSE/轮询和 `/background-tasks` 路由，不新增第二套任务系统。
- 异步任务完成后的 `result` 必须保持原 Inspiration API 的响应结构，避免重写页面状态机。
- 502/503/504 等临时 AI 上游错误仍按现有降级响应返回，不把可恢复临时错误升级成任务失败。
- 配置错误、鉴权错误和不支持的步骤仍必须明确失败，便于用户修正设置。

## Acceptance Criteria

- [x] Rust `background_tasks` 支持 Inspiration 相关 task type。
- [x] 前端 `inspirationApi` 提供后台任务版本，并由灵感页默认使用。
- [x] 灵感模式生成书名/简介/主题/类型、反馈优化和快速补全都能后台执行并拿到原响应结构。
- [x] Rust `background_tasks` 支持章节局部重写 task type。
- [x] 前端局部重写弹窗默认通过后台任务执行并拿到原结果结构。
- [x] 任务中心能看到对应后台任务状态。
- [x] Rust `background_tasks` 支持拆书导入 apply / retry task type。
- [x] 前端拆书导入默认通过后台任务执行并保留失败步骤可重试能力。
- [x] 任务中心能看到拆书导入相关后台任务状态。
- [x] 前端 Wizard 世界观重生成默认通过 `world_regenerate` 后台任务执行。
- [x] feature command 中的大纲/角色生成默认通过后台任务执行，服务层同步方法仅保留兼容入口。
- [x] AI 去味单条/批量具备后台任务封装，并由任务中心显示状态。
- [x] 通过 `cargo check` 和 TypeScript 构建检查。

## Notes

- 这是全流程异步化的示范闭环，后续再按相同模式迁移其它真实长耗时入口。
- 兼容同步/SSE service 方法仍保留，只有默认页面/command 调用路径计入本轮完成。
- 已确认仍保留的 Wizard `updateWorldBuildingStream` 是未发现调用方的轻量更新入口，`cleanupWizardDataStream` 是删除清理入口；二者不纳入本轮生成类长任务后台化范围。
