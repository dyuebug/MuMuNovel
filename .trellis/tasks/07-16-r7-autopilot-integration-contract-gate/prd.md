# R7 Autopilot Integration Contract Gate

## Goal

验证并收口现有 R7 Controlled Autopilot MVP 的跨层契约：项目工作流页发起的人工确认请求必须经严格 project-scoped API、既有通用后台任务、一次性 Coordinator、受控 Tool 与 workflow/audit owner 正确执行；项目所有者随后只能读取脱敏、只读的 invocation history。该任务只修复当前实现中被集成证据证明的断点。

## Confirmed Facts

- 已实现的创建入口为 `POST /projects/{project_id}/autopilot/actions`；认证 actor 仅来自 Claims，项目作用域仅来自 path，且请求要求 `confirmed_by_user: true`。
- `novel_autopilot` 只允许 `transition_project_workflow`，并保持 `NonResumable`；Coordinator 调用现有 Tool Contract，而非直接写 workflow。
- 创建任务前已经写入 queued durable audit；Coordinator 会将运行态、成功事务和安全失败码写入已有 audit owner。
- 已实现 owner-scoped `GET /projects/{project_id}/autopilot/invocations`，以及项目 workflow 面板中的按需、组件局部、脱敏只读 history Modal。
- 各纵切已有聚焦 Rust/Playwright 证据，但路线文档要求在进入 G2 前对 R7 做统一契约收口和门禁评审。

## Requirements

1. 建立可重复的跨层回归，证明 canonical confirmed action payload 能进入现有 generic task execution path，并由 Coordinator/Tool 产生受控 workflow 结果和 durable audit 状态。
2. 覆盖至少一个安全失败链，证明无效确认、错误作用域或 stale workflow 不会产生越权 workflow mutation，audit 仅保留稳定安全状态/错误码。
3. 验证 history read contract 与 UI 使用的类型模型一致：按 owner scope 返回安全摘要，不暴露或重构敏感字段，且不派生控制/恢复行为。
4. 在发现缺口时只做最小修复，保持既有 TaskRegistry、workflow Service、audit Service、task terminal owner 和前端状态所有权不变。
5. 补齐集成证据、更新 R7 任务/路线状态描述；R7 和 G2 不得因本任务自动标记完成，除非所有独立验收都由当前证据证明。

## Acceptance Criteria

- [x] 测试证明确认后的 action request 使用 path project + authenticated actor 生成 canonical `novel_autopilot` payload，且不会信任 body 内 scope/actor。
- [x] 测试证明该任务从 queued audit 经 Coordinator/Tool 到安全 terminal audit，成功场景的 workflow 结果与 audit summary 一致。
- [x] 测试证明失败场景不会乐观或越权修改 workflow，且不泄露 raw arguments、reason、Prompt、凭据、provider/model、digest、actor 或 raw error。
- [x] 现有项目工作流 Playwright 测试继续证明创建入口无 optimistic workflow mutation，历史 Modal 仅作只读展示。
- [x] Rust 格式化、类型/编译检查、聚焦 R7 测试和目标 Playwright 均通过，或对环境固有限制给出可复现的替代命令与证据。
- [x] 不新增 Pause、Resume、Steer、checkpoint、recovery、replay、自动重试、多步骤自治、Provider/MCP 调用、Schema migration 或第二套任务/状态 owner。

## Out of Scope

- G2 安全门禁本身、无人值守多步骤/整书/多卷生成。
- 新的 Autopilot Tool、LLM 决策、Provider 调用、任务控制命令或恢复协议。
- 替换通用 task lifecycle、workflow state machine、business checkpoint 或 durable audit storage。
- 审计列表分页、导出、筛选和全局审计中心。

## Planning Decision

用户已经明确授权后续直接开发。集成门禁的产品与安全边界已由路线、R7 纵切任务和现有规格确定；不需要额外产品决策。实现前必须依据当前源码和测试确认最小的测试装配点，并仅修复由该验证暴露的真实断点。
