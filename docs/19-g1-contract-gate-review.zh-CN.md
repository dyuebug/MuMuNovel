# G1 统一契约门禁审查报告

> 审查日期：2026-07-16
>
> 审查范围：R3、R4、R5、R6、G1-Cancel 与旧 API/UI 兼容边界
>
> 最终决议：**GO**
>
> 路线影响（审查当日）：**R7 受控 Autopilot MVP 已解锁；后续实施状态以路线文档为准**

## 1. 审查结论

G1 的六项统一契约条件均具备可定位的实现 owner 和测试/质量证据。审查期间发现
cooperative cancellation registry 在同 key replacement registration 时没有取消旧 token；该缺口已在
`CooperativeCancellationRegistry::register()` 中修复，并通过 focused tests 与完整 Rust 测试集回归。

因此，G1 于 2026-07-16 判定为 **GO**。正式路线从
`G1 -> R7 -> G2 -> R8` 前进到 `R7 -> G2 -> R8`。本结论只解除 R7 的前置门禁，
不代表在审查当日已经实现 Coordinator、Tool API、Autopilot 页面、Pause/Resume UI 或新任务类型。

后续更新（2026-07-16）：R7 已在 G1 解锁后完成受控 Tool Contract、最小 `novel_autopilot`
任务/Coordinator、认证且 project-scoped 的控制 API、一次性人工 gate、durable invocation audit 与
owner-scoped readonly history。该更新不倒灌为“G1 当日已验证 durable audit”；G1 只解除 R7 前置门禁。
R7 仍保持单次、人工确认、`NonResumable` 边界，未授权 Pause/Resume/Steer、replay、自动 retry 或
多步骤无人值守执行。详见 `docs/15-ainovel-cli-comparison-and-mumunovel-optimization.zh-CN.md`。

## 2. 六项门禁审查矩阵

| # | G1 条件 | 判定 | 实现证据 | 测试/质量证据 |
|---|---|---|---|---|
| 1 | 项目级阶段只有一个权威来源 | **GO** | `projects.status` 被声明为 workflow source；转换 owner 与 CAS 均集中在 `NovelWorkflowService`，旧项目写入口复用同一转换 owner。见 `backend-rs/src/services/novel_workflow_service.rs:9-75`、`backend-rs/src/services/novel_workflow_service.rs:347-410`、`backend-rs/src/services/project_service.rs:318-345`。 | R3 完整 Rust 门禁 1646/1646；CAS owner 在 SeaORM/mock/SQLite harness 中验证。见 `.trellis/tasks/07-14-novel-workflow-state-machine/implement.md:75`、`.trellis/tasks/07-14-novel-workflow-state-machine/implement.md:94-95`。 |
| 2 | 核心生成入口能够生成统一 Story Packet | **GO** | `generation-contract/v1`、`StoryPacketV1`、`GenerationIntentV1` 由单一 schema owner 定义，canonical digest 由统一 owner 生成。见 `backend-rs/src/services/generation_contract_service/schema_owner.rs:6-18`、`backend-rs/src/services/generation_contract_service/schema_owner.rs:141-231`、`backend-rs/src/services/generation_contract_service/canonical_owner.rs:86-143`。 | 单章、批量、恢复、重生成、大纲和审校入口已通过兼容适配；R4 完整 Rust 门禁 1689/1689。见 `.trellis/tasks/07-14-story-packet-generation-intent/implement.md:153-181`。 |
| 3 | 模型选择和 fallback 可追溯 | **GO** | `role-model-policy/v1` 记录 role mapping 与来源，execution trace 记录实际 provider/model 和 fallback taxonomy，generation audit 合并 requested/resolved/actual/fallback。见 `backend-rs/src/services/role_model_policy_service/schema_owner.rs:7-81`、`backend-rs/src/ai/execution_trace.rs:8-105`、`backend-rs/src/services/generation_execution_audit_service/schema_owner.rs:14-109`。 | Role policy、tracked execution、fallback、digest 隔离和兼容审计均通过；完整 Rust 门禁 1736/1736。见 `.trellis/tasks/07-15-role-model-policy-generation-execution-audit/implement.md:142-164`。 |
| 4 | 至少一种长流程通过业务 checkpoint 完成恢复验证 | **GO** | `business-checkpoint/v1` 定义 revision、canonical idempotency key、R4 input digest 与 typed output reference；resume 在 dispatch 前完成合同和 DB 校验。见 `backend-rs/src/services/business_checkpoint_service/schema_owner.rs:6-37`、`backend-rs/src/services/chapter_batch_generation_resume_task_command_service/resume_launch_owner.rs:174-236`。 | DB-backed 测试执行章节成功持久化、后续失败和 resume，而不是直接伪造 checkpoint。见 `backend-rs/src/services/chapter_batch_generation_resume_task_command_service.rs:1879-2000`、`.trellis/tasks/07-16-business-checkpoint-recovery/implement.md:93-106`。 |
| 5 | 旧页面和旧 API 仍通过兼容门面工作 | **GO** | R3-R6 与 G1-Cancel 均保持既有 HTTP/JSON/SSE/status/schema；未新增第二套 task store、项目阶段事实、checkpoint/resume owner 或 Coordinator。 | R4 兼容审计明确确认旧 DTO、response、SSE event、前端请求类型及 Zustand/task store 未变；完整门禁通过。见 `.trellis/tasks/07-14-story-packet-generation-intent/implement.md:153-181`、`.trellis/tasks/07-16-cooperative-cancellation/implement.md:73-85`。 |
| 6 | workflow/task/checkpoint/cancellation 职责边界有文档和测试保护 | **GO** | workflow 保存项目业务阶段；task 保存执行生命周期；business checkpoint 保存可恢复业务边界；cancellation 只提供进程内协作停止。terminal task/snapshot 使用条件更新和同事务持久化。合同见 `.trellis/spec/backend/quality-guidelines.md:6577-6689`。 | replacement registration 现会在释放 registry 写锁后取消旧 token，旧 cleanup 仍不能删除新 registration；另有 progress bridge、迟到结果拒绝和 cancel-vs-completion 竞态测试。见 `backend-rs/src/services/cooperative_cancellation_service.rs:99-127`、`backend-rs/src/services/cooperative_cancellation_service.rs:139-152`、`backend-rs/src/services/cooperative_cancellation_service.rs:238-270`、`backend-rs/src/api/background_tasks.rs:1927-1958`、`backend-rs/src/api/background_tasks.rs:2163-2197`、`backend-rs/src/services/chapter_batch_generation_runtime_state_service.rs:3733-3825`。 |

## 3. 审查阻塞项与修复

### 3.1 发现的问题

原 `register()` 直接调用 `HashMap::insert()` 并忽略返回的旧 `ActiveRegistration`。当恢复或重启在同一
scope/task ID 上注册新实例时，旧 lifecycle token 不会收到 cancellation signal。数据库 terminal CAS
虽然可以阻止旧执行覆盖终态，但不能阻止旧 Future 继续消耗 AI/HTTP/数据库资源。

该行为违反已固化合同：replacement registration 必须取消 previous token，同时旧 cleanup 不得删除
replacement registration。

### 3.2 最小修复

`register()` 现在在持有写锁时只完成 replacement insert 并取出旧 registration，释放锁后再调用旧
registration 的 token `cancel()`。这避免在持锁状态唤醒 waiter，也不改变 scope/key、registration ID、
cleanup、公开 API 或 durable state 语义。

现有 `old_cleanup_does_not_remove_replacement_registration` 测试已增强，验证：

1. replacement 创建后 old token 已取消；
2. replacement token 未被误取消；
3. old cleanup 返回 false 且不删除 replacement；
4. registry cancel 只取消 current replacement；
5. replacement cleanup 保持幂等。

## 4. 验证证据

本轮验证结果：

```text
cargo fmt --check                                      PASS
cargo check                                            PASS
cargo check --tests                                    PASS
cargo test cooperative_cancellation_service            3 passed
cargo test api::background_tasks                       26 passed
cargo test chapter_batch_generation_runtime_state_service
                                                       139 passed
cargo test                                             1761 passed / 0 failed / 0 ignored
```

默认 MSVC linker 在测试链接阶段仍可能触发 `LNK1318: PDB LIMIT (12)`。本轮按既有环境回退使用
`rust-lld`、`-C debuginfo=0` 和 `-C link-arg=/DEBUG:NONE` 完成测试链接；这是本地工具链规避，
不是产品逻辑变更。

## 5. 职责边界裁决

```text
projects.status
  = 小说级业务阶段的唯一事实

TaskRegistry / batch_generation_task
  = 执行生命周期、进度与终态所有权

business-checkpoint/v1
  = 可恢复的业务边界、revision、幂等键、输入摘要与输出引用

CooperativeCancellationRegistry
  = 当前进程内的协作停止 signal，不是 durable task state
```

由此明确：取消信号不能替代数据库终态，数据库 task status 不能替代业务 checkpoint，checkpoint 也不承诺
从任意 token 位置恢复。

## 6. 非阻塞风险

1. **PostgreSQL 并发证据**：workflow CAS 当前主要由 SeaORM/mock/SQLite-compatible harness 验证，
   尚无真实 PostgreSQL 隔离级并发专项测试；这是后续强化项，不阻塞当前 G1。
2. **ChapterReview 持久审计**：当前分析结果边界可以追溯当次 provider/model/fallback，但 background
   wrapper 没有独立 durable audit 字段；不影响现有生成入口合同，后续可在不改公共 API 的前提下补强。
3. **进程内 cancellation**：registry 不跨进程，进程退出后依赖 durable task state 与 business checkpoint
   恢复；不承诺跨进程 signal 或任意 token 位置断点续跑。
4. **首个 checkpoint 覆盖面**：R6 当前正式验证的业务边界为 `chapter_draft_saved`；扩大到更多业务边界时
   必须沿用同一 schema、revision、digest 和 typed output 合同。

## 7. 最终 GO 决议与下一步

**G1 = GO（2026-07-16）**。

R7 已解除前置阻塞，唯一下一主线为受控 Autopilot MVP，顺序保持：

```text
R7 -> G2 -> R8
```

R7 的第一项实施必须是定义受控 Tool Contract，并将 Tool 映射到现有 Rust Service。不得绕过权限、
Service 事务、现有 task store、workflow owner 或 business checkpoint；不得在 G2 前扩展为无人值守多卷生成。
