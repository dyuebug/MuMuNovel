# Design: Background Task Recovery Policy Registry

## Architecture Boundary

R2 extends the existing JSON-backed `TaskRegistry` startup recovery path:

```text
background_tasks.json
  -> TaskRegistry
  -> recover_orphan_tasks()
       -> recovery_policy_for(task_type)
       -> build recovery projection
       -> update TaskRecord terminal semantics
  -> existing /background-tasks list/detail/SSE payloads
  -> existing frontend background task model
```

It does not execute business recovery commands. Chapter batch/single resume remains owned by the
existing database runtime-state and resume command services.

## Data Model

Add backward-compatible optional fields to `TaskRecord`:

```rust
pub terminal_reason: Option<String>,
pub terminal_label: Option<String>,
pub review_required: Option<bool>,
pub can_resume: Option<bool>,
```

Each field uses serde defaults and skips serialization when `None`. `TaskRecord::new()` initializes
all four fields to `None`, preserving the public constructor signature and version-1 snapshot
compatibility.

## Recovery Policy Registry

Define a single static registry in `tasks/recovery.rs`:

```rust
pub enum TaskRecoveryPolicy {
    Restartable,
    CheckpointResumable,
    ManualConfirmation,
    NonResumable,
}

pub struct TaskRecoveryPolicyEntry {
    pub task_type: &'static str,
    pub policy: TaskRecoveryPolicy,
}

pub const TASK_RECOVERY_POLICIES: &[TaskRecoveryPolicyEntry] = &[...];

pub fn has_explicit_recovery_policy(task_type: &str) -> bool;
pub fn recovery_policy_for(task_type: &str) -> TaskRecoveryPolicy;
```

A slice is preferred over a runtime `HashMap`: the known set is small, startup lookup volume is
bounded, and a static slice is easy to audit and test for duplicates. Unknown values return
`NonResumable` without panicking.

## Policy Matrix

| Policy | Registered task types | Startup projection |
|---|---|---|
| Restartable | chapter analysis, inspiration operations, single-text polish | failed + restart-required message; no automatic resume |
| CheckpointResumable | chapter batch and chapter single generation | failed + resume available only when checkpoint is a non-empty object |
| ManualConfirmation | regeneration, imports, batch/mutating generation and wizard operations | failed + manual review required |
| NonResumable | unknown/unregistered | failed + explicit unrecoverable diagnostic |

The registry contains 23 unique production task types. Twenty are dispatched by the generic
`execute_task()` match, while `chapter_single_generate` and `chapters_batch_generate` are owned by
the existing chapter runtime-state/resume path and `chapter_analysis` is projected into the task
center by its API owner. Registry tests assert the 23-entry policy distribution and uniqueness.
A source-driven production contract extracts the real top-level string arms from `execute_task()`
and requires every dispatched type to satisfy `has_explicit_recovery_policy()`. A second cross-layer
contract extracts the frontend `BackgroundTaskType` string union and requires its 23 known values to
match the Rust recovery registry exactly. The only additional frontend value is the single `unknown`
safety sentinel, which must remain absent from the explicit registry and resolve through the
`NonResumable` fallback. The contract also fixes the only known values outside `execute_task()` to the
three explicit independent owners above. This avoids maintaining a second handwritten executor list
while detecting frontend, executor, owner, and recovery-policy drift in one Rust test target.

## Recovery Projection

Introduce an internal projection owner, for example:

```rust
struct OrphanRecoveryProjection {
    terminal_reason: &'static str,
    terminal_label: &'static str,
    error: &'static str,
    message: &'static str,
    review_required: bool,
    can_resume: bool,
}
```

`CheckpointResumable` is capability-based but also validates record state. A usable recovery
checkpoint must be a non-empty JSON object. Missing, null, scalar, array, or empty-object values
produce `checkpoint_missing` with `can_resume=false`.

All projections keep `TaskStatus::Failed` for compatibility. They set `completed_at`, explicitly
refresh `updated_at`, and preserve `started_at` exactly, including `None`; only the normal
pending-to-running lifecycle owner may initialize a missing start timestamp. Startup recovery must
not fabricate an execution start time for a pending task that never ran. Each recovered record
samples one recovery timestamp and reuses it for checkpoint `updated_at`, record `completed_at`, and
record `updated_at`, so the diagnostic checkpoint and terminal record describe the same atomic
recovery fact. `TaskRegistry::update()` only executes the supplied mutation and does not update
timestamps automatically.

## Checkpoint Diagnostics

Use the existing checkpoint helper owner. Keep `touch_checkpoint()` as the compatibility wrapper
for ordinary callers, and use its time-injecting variant when an atomic owner must share one timestamp
with surrounding record fields. Preserve existing keys and merge:

```json
{
  "event": "orphan_recovery",
  "recovery_policy": "manual_confirmation",
  "terminal_reason": "manual_review",
  "can_resume": false,
  "review_required": true,
  "has_result": false
}
```

No payload, result body, checkpoint body, or user content is written to logs.

## Startup Durability

Startup recovery must be persisted before periodic workers or the HTTP router become active:

```text
load snapshot
  -> recover orphan tasks
  -> if recovered_count > 0: save existing atomic snapshot
  -> start periodic save
  -> start cleanup
  -> build router
```

`recover_orphan_tasks()` first snapshots active task ids, then re-checks each record's latest status
inside the registry write lock before applying the projection. Projection inputs come from that locked
latest record, and only records actually changed from active to failed contribute to the returned count
and per-task recovery log. This prevents a stale candidate from overwriting a concurrently completed or
cancelled record and makes repeated recovery idempotent. The immediate save reuses R1's
primary/backup/temporary atomic snapshot protocol and keeps the existing best-effort error boundary:
persistence failures are logged but do not introduce a new fail-closed startup contract. A source-order
contract test protects this lifecycle sequence.

## API and Frontend Compatibility

`compatible_task_payload()` already serializes the full `TaskRecord` at both top level and `data`.
No endpoint or response wrapper changes are required. The frontend model already maps the four new
snake_case fields. Existing chapter resume UI remains restricted to chapter batch/single task types.

Generic restartable tasks deliberately expose `can_resume=false`: they can be recreated from their
business UI, but there is no persisted payload or generic resume endpoint.

## Failure and Fallback Behavior

| Input | Effective behavior |
|---|---|
| Active known restartable task | terminal restart-required projection |
| Active checkpoint task with usable checkpoint | terminal resume-available projection |
| Active checkpoint task without usable checkpoint | terminal checkpoint-missing projection |
| Active manual-confirmation task | terminal manual-review projection |
| Active unknown task | terminal non-resumable projection |
| Existing terminal task | unchanged |
| Registry update races with another owner | startup ordering prevents normal execution owners from starting before recovery; the recovery helper also re-checks the latest status under the registry write lock and skips stale terminal candidates |

## Compatibility and Rollback

- No dependency, migration, route, constructor signature, or TaskStatus change.
- Reverting `recovery.rs` and the optional `TaskRecord` fields restores prior behavior.
- Old snapshots load because all new fields are optional.
- New snapshots remain JSON; older binaries would ignore unknown fields under serde's default
  behavior if rollback is required.
- R1 file names and atomic persistence protocol are untouched.

## Main Risks

1. A task may be classified too optimistically. Mitigation: conservative manual-confirmation
   classification for mutating/batch operations and non-resumable fallback for unknown types.
2. A malformed checkpoint may incorrectly enable resume. Mitigation: require a non-empty JSON
   object and test null/scalar/array/empty-object cases.
3. Task-type drift may bypass the registry. Mitigation: keep the 23-entry uniqueness/policy test
   and derive generic executor coverage from the real `execute_task()` match; future executor types
   must add an explicit registry policy in the same change.
4. Frontend may infer resumability when fields are absent. Mitigation: recovered records always set
   all four terminal semantics fields explicitly.

## Generic TaskRecord 终态单调性补强（2026-07-13）

### 生命周期与记录复用裁决

- 通用后台任务创建始终生成新 UUID，并通过 `TaskRecord::new()` 创建新记录；dedup 只返回已存在的
  active task。生产路径不存在 recovered terminal record 重新激活或复用为新任务的合同，因此不新增
  “清空恢复字段后重启旧记录”的假设性 API。
- `Pending -> Running` 是执行器唯一准入转换。该转换必须在 registry 单一写锁内检查当前状态并
  返回布尔准入结果；延迟启动的 spawn 未取得准入时直接退出，不得开始业务执行。
- `Completed`、`Failed`、`Cancelled` 是不可回退终态。完成、失败、取消 owner 只能从 active record
  原子转换；迟到的 executor、channel progress 或取消请求不得覆盖任一终态，也不得改写 recovered
  terminal record 的 `recovery_policy`、`recovery_action`、`can_resume`、`review_required`。

### 原子 owner 与投影边界

- `TaskRegistry::update_if()` 是多条件状态转换的原子 owner，在同一写锁内执行 predicate 与 updater；
  禁止先 `get()` 检查状态/用户后再无条件 `update()`，否则会产生 TOCTOU。
- `complete_task()` 是 generic executor 唯一的 `Completed` 投影 owner。channel adapter 只同步 active
  record 的 progress/message/result，不得因 channel status 为 `success` 提前写入 `Completed`，否则会
  阻止最终 result、`completed_at` 和统一事实时间落盘。
- 取消 owner 在同一写锁内完成用户归属、active 状态、checkpoint、`Cancelled` 与时间戳更新；同一次
  取消使用唯一事实时间写入 checkpoint `updated_at`、record `completed_at` 和 `updated_at`。

### 明确保留的边界

本轮保证 registry 状态机终态单调、Pending 取消后不启动业务执行，以及迟到 registry 写入安全。
已经进入 `Running` 的底层 AI/数据库操作尚无统一 cooperative cancellation token，用户取消后外部操作
可能继续至自身返回，但其迟到结果不能覆盖 `Cancelled`。统一深度取消需要独立设计，不在 R2 内扩大。

## 启动恢复原子 owner 统一（2026-07-13）

### 审计结论

- 对 Generic TaskRecord 的 `status`、`started_at`、`completed_at`、恢复 metadata 与终态写入点完成
  全量审计，未发现新的终态回退路径；章节 runtime-state owner 仍是独立数据库 owner，不纳入本次
  registry 生命周期重构。
- `recover_orphan_task()` 原实现虽然在 registry 单一写锁的 updater 闭包内复核 `is_active()`，行为上
  不会覆盖终态，但绕过了统一的条件更新合同，并依赖闭包外可变 metadata 回传审计结果。

### 统一合同

- 启动 orphan recovery 必须复用 `TaskRegistry::update_if()`，由 `status.is_active()` predicate 与恢复
  投影在同一写锁内组成唯一原子 owner；不得用普通 `update()` 加闭包内早退模拟条件 owner。
- 恢复日志 metadata 从 `update_if()` 返回的最新 `TaskRecord` 派生，删除闭包外 mutable metadata
  回传；predicate 失败直接返回 `None`，terminal/stale candidate 保持不变。
- 该统一只收敛内部 owner：恢复策略、单一事实时间、checkpoint、日志隐私、API、Schema、TaskStatus
  与启动持久化边界均保持不变。
