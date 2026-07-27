# Bug Analysis: 章节返修质量重试丢失跨尝试证据

## 1. Root Cause Category

- **Category**: B - Cross-Layer Contract
- **Specific Cause**: 候选生成层返回了正文、digest、质量指标和质量消息，但适配器到耐久仓储的失败提交契约只保存通用错误码、质量决策与计数。外层调度又隐式假设下一次返修能利用上一轮结果，导致每次都从同一已接受正文和同一分析重新开始。
- **Secondary Categories**: D - Test Coverage Gap；E - Implicit Assumption。原测试覆盖单次质量路由，但未覆盖“失败候选持久化 -> 下一次消费 -> 预算耗尽人工复核”的完整跨尝试链路。

## 2. Why Fixes Failed

1. **仅增加重试次数**：只能重复同一输入，未恢复候选和反馈，无法提高收敛概率。
2. **仅调整错误码或时间线**：改善了表象诊断，但下一轮 prompt 仍拿不到上一轮失败证据。
3. **直接覆盖正式章节**：会绕过质量门禁和章节快照 CAS，破坏已接受正文的权威边界。
4. **只保存 digest**：可以关联 Step，却无法重建可继续修复或可人工复核的完整候选。

## 3. Prevention Mechanisms

| Priority | Mechanism | Specific Action | Status |
|---|---|---|---|
| P0 | Architecture | 复用 `chapter_draft_attempts` 保存完整 retry 候选，Run/Step 只保存安全摘要与 digest | DONE |
| P0 | Runtime transaction | 候选 insert、Run CAS、Step CAS 和质量计数在同一事务提交 | DONE |
| P0 | Scope validation | 强制匹配 project/chapter/run/epoch/source digest/analysis ID，并校验全文 digest 与 Unicode 字数 | DONE |
| P0 | Terminal routing | 重试预算耗尽时把最后候选转为标准人工候选，停止调度下一次返修 | DONE |
| P1 | Test coverage | 覆盖作用域隔离、损坏回退、事务回滚、人工编辑 CAS 和无第 4 次重试 | DONE |
| P1 | Documentation | 把跨尝试质量重试证据合同写入 Durable Novel Autopilot 规范 | DONE |

## 4. Systematic Expansion

- **Similar Issues**: 其他具有 `retry`/`auto_repair` 的生成步骤若只持久化状态而不持久化可恢复业务证据，也可能出现盲重试。后续修改此类步骤时应检查证据是否跨尝试耐久化。
- **Design Improvement**: 重试不是单纯的调度状态；只要下一次需要上一轮业务输出，候选和反馈就属于 checkpoint，必须有 typed evidence 和 owner-scoped storage。
- **Process Improvement**: 重试测试必须至少覆盖两次连续尝试，并断言第二次输入来自第一次候选，而不是只验证 `retry_scheduled` 状态。
- **Knowledge Gap**: “正式业务对象不能提前更新”不等于“失败候选不能保存”。两者应通过 owner 分离，而不是丢弃候选。

## 5. Knowledge Capture

- [x] 更新 `.trellis/spec/backend/durable-novel-autopilot.md`。
- [x] 回填当前任务的实现与验证记录。
- [x] 新增跨尝试、事务回滚、作用域隔离和预算耗尽回归测试。
- [x] 确认仓库不存在 `src/templates/markdown/spec/`，无可同步模板。
- [ ] 不执行 `git commit`；当前会话未获得提交授权。
