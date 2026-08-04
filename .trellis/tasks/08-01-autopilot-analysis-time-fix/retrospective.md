# Bug Analysis: 自动创作质量、失败路由与时间契约反复漂移

## 1. Root Cause Category

- **Category**: B / E / D - 跨层契约、隐式假设、测试覆盖缺口。
- **Specific Cause**: `waiting_human` 同时表示有候选质量耗尽和无候选运行故障，
  但 UI 曾把状态隐式等同于“可接受候选”，且隐藏 Accept 按钮没有撤销后端接受
  能力；分析/返修边界又把配置、上下文、响应无效和 Provider 故障压成同一个
  字符串错误。数据库时间是 UTC 语义的 `NaiveDateTime`，API 未携带时区，浏览器
  只能按本地时间猜测。

## 2. Why Fixes Failed

1. **Surface Fix**: 只调整聚合错误文案，无法区分是否存在候选，也无法阻止错误 Accept。
2. **Incomplete Scope**: 只在 Adapter 增加重试判断，没有保留 typed HTTP 状态，
   导致端口、请求 ID 或模型构建号中的数字可能被当成 HTTP 状态。
3. **Change Propagation Failure**: 完整质量对象进入 Task/SSE，候选 digest 只在单一
   位置校验，跨存储边界后仍可能出现状态、正文和摘要不一致。
4. **Test Coverage Gap**: 单元测试未同时覆盖候选存在/不存在、Generate Repair、
   北京时间显示和 Task 安全投影，跨层回归只能在真实运行后暴露。
5. **UI-only Mitigation**: 前端根据 `candidate_id` 隐藏 Accept 只能改善界面，构造 API
   请求仍可能绕过 UI；操作权限必须由 API 和协调器基于持久化证据共同强制执行。

## 3. Prevention Mechanisms

| Priority | Mechanism | Specific Action | Status |
|---|---|---|---|
| P0 | Architecture | 使用 typed runtime error 和 `FailureCounterKind::{Provider, Quality, None}` | DONE |
| P0 | Runtime invariant | 候选持久化校验正文摘要，Accept 校验正文/私有 payload/Step 三方 digest | DONE |
| P0 | Capability enforcement | API 预检候选/错误证据，协调器最终检查；无候选 Accept 返回 409 且不产生副作用 | DONE |
| P1 | Data minimization | Task/SSE 使用 `quality_diagnostics` allowlist，不复制完整质量上下文 | DONE |
| P1 | Boundary contract | API 统一输出 RFC 3339 UTC `Z`，前端只做标准本地时区显示 | DONE |
| P1 | Test coverage | Rust 聚焦测试和 Workbench E2E 覆盖有/无候选、Repair、时间与脱敏 | DONE |

## 4. Systematic Expansion

- **Similar Issues**: 其他 Autopilot Adapter 仍可能用聚合字符串分类 Provider 故障，
  后续修改应复用安全诊断与显式计数模式，而不是复制关键词判断。
- **Design Improvement**: 将“运行状态”和“允许操作”分离；状态描述流程位置，
  `candidate_id`、版本、epoch、digest 和 owner row 决定能力；UI 仅呈现能力，API 与
  服务端必须独立校验并 fail-closed。
- **Process Improvement**: 跨存储、服务、Task/SSE、API、UI 的修复必须同时包含
  数据流审查与 E2E，不能用 `cargo check --tests` 代替实际测试执行。

## 5. Knowledge Capture

- [x] 更新 `.trellis/spec/backend/durable-novel-autopilot.md` 的可执行契约。
- [x] 更新 `.trellis/spec/guides/cross-layer-thinking-guide.md` 的状态/能力检查项。
- [x] 当前任务 PRD、设计和实施记录同步最终人工复核决策。
- [x] 检查模板同步目录；本仓库不存在 `src/templates/markdown/spec`，无需同步。
- [ ] Git 提交：未执行；项目规则要求用户再次明确确认后才能 commit。
