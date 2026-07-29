# Design: R8 Eval / 创作档案 / 运行指标

## 1. 架构边界

R8 采用“**既有事实 owner → 安全投影 → readonly consumer**”的单向模型：

```text
workflow / generic task / generation contract / execution audit / quality metrics / invocation audit
                                      |
                                      v
                       R8 allowlisted eval / archive / metrics projection
                                      |
                                      v
                         owner-scoped readonly API / export / frontend display
```

R8 projection 不写回任何上游 owner，不成为 workflow、task、audit 或 checkpoint 的事实来源。
所有生产读取使用既有 owner-scoped context；static golden sample 仅在 `cfg(test)` 下存在。

## 2. 评测（Eval）设计

- 输入来自脱敏的 generation contract summary、generation execution audit summary、quality summary
  和固定 expected verdict；不读取真实 Prompt 或 Provider payload。
- 样本格式必须 versioned，并显式声明 `schema_version`、`case_id`、安全摘要与判定结果。
- evaluator 为纯函数：同样输入产生同样 verdict；未知 schema、缺失字段或不安全字段采用
  fail-closed / unsupported verdict，不回退到原始 history。
- golden sample 只用于测试和回归，不写数据库、不提供 runtime upload endpoint。

## 3. 创作档案（Archive）设计

- 复用 `load_project_export_context(...)` 的 owner/scope 校验和现有导出链路。
- 新 archive projection 必须是命名 DTO/allowlist，而不是序列化整个 export context 或 ORM model。
- 允许输出：archive schema version、项目展示元数据、workflow safe phase/version、generation
  contract summary、execution audit safe summary、quality summary、人工反馈关联键和生成时间。
- 禁止输出：Prompt、credential、Provider secret、raw arguments/errors、内部 actor/project ID、
  digest（除非已有公开 DTO 已明确允许且能证明不用于跨项目关联）。
- 输出未知/旧 history 时保持“字段缺失”或安全 `null`，不因 archive 读取改变既有数据。

## 4. 运行指标（Metrics）设计

- 复用 project quality trend、chapter quality summary、generic task lifecycle 与 workflow/audit
  safe projection；R8 仅聚合当前已公开或内部可安全投影的摘要。
- 每个字段须标注来源 owner 与缺失语义，例如：

| 指标类别 | 事实来源 | R8 语义 |
| --- | --- | --- |
| workflow phase/revision | canonical workflow service | 当前 workflow 安全快照，不是 task progress |
| task lifecycle counts | generic task store | 只读生命周期计数，不提供控制 |
| quality trend | chapter quality metrics read model | 已分析章节的摘要，缺失不代表失败 |
| generation/audit coverage | generation execution audit / invocation audit | 已有安全审计记录数量或状态分布，不暴露原始记录 |

- 任何 aggregation 失败返回稳定 unavailable/empty safe summary；不得将内部数据库错误或 audit
  内容直接出网。

## 5. API/UI 与兼容性

- 如果现有 project export 或 quality trend API 足以表达首个 R8 步骤，优先扩展其明确 DTO，
  不新增泛用“导出全部数据”或“metrics 控制台”端点。
- 如果必须新增 endpoint，路径必须 project scoped、Claims owner-checked、readonly，返回 versioned
  DTO；未知数据和历史数据保持向后兼容。
- 前端只请求/展示安全 DTO；不从原始 JSON 推断控制状态，不乐观写 workflow，不显示控制/恢复按钮。
- 不做 schema/migration；如现有持久化无法承载，则保持派生 read model 或放弃该字段。

## 6. 回滚与风险

- 回滚只撤销 R8 projection、DTO、route/UI 和 tests，不触碰既有 task/workflow/audit owner。
- 最大风险是 export/metrics 投影泄露内部字段；使用 named allowlist DTO、negative tests 和
  unknown-schema fail-closed 作为预防。
- 第二风险是把 aggregate 当作 canonical fact；所有 DTO/documentation 标注 source/derived/
  readonly，并验证 API 读取不改变 workflow/task/audit。
