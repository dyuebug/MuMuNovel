# G2 Autopilot Safety Gate

## Goal

为已通过 R7 的受控、单次、人工确认 `novel_autopilot` MVP 建立可重复的安全门禁：
使用固定 workflow fixture 验证允许与拒绝结果，使用故障注入验证 audit/Tool/workflow
失败不会破坏人工工作台或扩大权限/恢复能力，并形成跨层证据矩阵。

## Confirmed Facts

- R7 已于 2026-07-16 正式收口为 PASS（受控 MVP）；唯一 Tool 为
  `transition_project_workflow`，task 仍为 `NonResumable`。
- 现有 action API、generic task、Coordinator、Tool Contract、workflow service、durable audit
  与 owner-scoped readonly history 已有独立及跨层回归。
- R7 已验证正常成功、确认/作用域拒绝、stale phase 安全失败、取消与 history 隐私；G2 的
  路线缺口是固定样本回归和系统化 failure injection 证据。
- 现有路线明确禁止在 G2 前实现 Pause/Resume/Steer、checkpoint/recovery/replay、自动重试、
  Provider/MCP runtime、多 Tool/多步骤自治或整书/多卷无人值守生成。

## Requirements

1. 新增确定性的固定样本 fixture，仅表达已允许的 R7 workflow transition 及其安全拒绝场景；
   fixture 不能包含 Prompt、凭据、Provider/model 或原始 audit arguments。
2. 基于同一 fixture 建立 Rust 回归，至少证明：允许的 confirmed transition；未确认/越权或
   scope mismatch 拒绝；stale CAS 拒绝；返回和 history 仅含脱敏稳定摘要。
3. 建立故障注入回归，覆盖：queued audit 持久化失败、Tool 或 workflow CAS 失败、terminal
   audit 写入失败，以及 owner history 读取失败。每个场景都必须断言人工 workflow 状态保持安全，
   generic task terminal owner 不被替换，且不泄露 raw input/error。
4. 定义跨层安全矩阵，覆盖 route、generic task、Coordinator、Tool、workflow、audit、history
   和 frontend readonly UI 的 owner/边界；仅在测试可达性缺口被证实存在时做最小 production-code
   修复。
5. 固定样本与故障注入必须可通过本地命令重复执行；不得依赖真实 Provider、网络服务、生产数据
   或运行时 Prompt。
6. 更新路线文档并给出明确的 G2 GO/NO-GO 结论。若任何路线条件无证据，必须为 NO-GO，而不是
   用未来工作替代当前验证。

## Acceptance Criteria

- [x] G2 fixture 对 R7 的成功、拒绝和 stale 状态拥有稳定、可读、无敏感信息的样本输入/期望输出。
- [x] 固定样本回归证明确认、作用域、schema、CAS 与 audit/history 摘要符合 R7 已定义合同。
- [x] queued audit 写入失败不会创建或执行 task；workflow 不变且调用方仅得到稳定错误。
- [x] Tool/CAS 失败不会产生越权或乐观 workflow mutation；audit 仅含稳定失败码。
- [x] terminal audit 写入失败不会逆转或抢占 generic task terminal owner，且不暴露内部持久化错误。
- [x] owner history 读取失败和 non-owner history 请求均不暴露 audit 内容，也不改变 workflow/task。
- [x] 前端回归继续证明 history 为只读、无控制/恢复操作，确认动作不乐观改变 workflow。
- [x] fmt、check、G2 聚焦 Rust tests、frontend lint/build/E2E 全部通过；仅记录既有非阻断告警。
- [x] 文档明确 G2 是否 GO；无论结论如何，均不实现无人值守 Autopilot 能力。

## Out of Scope

- 新的 Autopilot Tool、LLM/Provider/MCP 调用、Prompt 执行或保存。
- Pause、Resume、Steer、retry、replay、checkpoint 或恢复协议。
- 多步骤、多章节、整书、多卷、无人值守 Autopilot。
- 生产数据库 migration、schema 扩展、真实网络/Provider E2E。
- 新的 task store、workflow owner、audit owner、任务中心控制 API 或 UI。

## Planning Decision

用户已明确授权持续直接开发；G2 的产品与安全边界已由路线文档和 R7 收口评审确定。
本任务只使用本地 fixture、SQLite/现有 test seams 与 frontend API mock；若现有 seam 不足，
优先在测试模块添加局部 fault hook，不向运行时公开控制或恢复能力。
