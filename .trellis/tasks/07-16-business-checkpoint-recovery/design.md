# Design: Business Checkpoint 标准与恢复验证

## 1. Architecture Boundary

R6 采用 additive typed owner，不创建新表或第二套状态机：

```text
R4 GenerationContractSnapshotV1.input_digest
                    │
                    ▼
BusinessCheckpointV1 builder/validator
                    │
                    ▼
BatchGenerationRuntimePersistencePlan::chapter_succeeded
                    │
                    ▼
workflow_runtime_state.business_checkpoint
                    │
                    ▼
BatchGenerationPersistedRuntimeContext → resume validation
```

职责划分：

- `business_checkpoint_service`：schema、canonical idempotency、JSON read/merge 与基础校验。
- batch runtime persistence owner：在章节成功业务边界收集 task/chapter/revision/digest 并持久化。
- batch resume owner：将 typed checkpoint 与当前 contract、DB chapter output 联合验证。
- route/AI client：不感知 checkpoint，不新增旁路写入。

## 2. Files and Owners

新增：

```text
backend-rs/src/services/business_checkpoint_service.rs
backend-rs/src/services/business_checkpoint_service/schema_owner.rs
backend-rs/src/services/business_checkpoint_service/canonical_owner.rs
backend-rs/src/services/business_checkpoint_service/snapshot_owner.rs
backend-rs/src/services/business_checkpoint_service/tests.rs
```

窄修改：

```text
backend-rs/src/services/mod.rs
backend-rs/src/services/chapter_batch_generation_runtime_state_service/runtime_persistence_owner.rs
backend-rs/src/services/chapter_batch_generation_runtime_state_service/resume_restore_owner.rs
backend-rs/src/services/chapter_batch_generation_runtime_state_service/runtime_driver_owner.rs
backend-rs/src/services/chapter_batch_generation_runtime_state_service.rs
backend-rs/src/services/chapter_batch_generation_resume_task_command_service.rs
```

如现有 command service 已能在 owner 内完成 DB 校验，则不修改公开 API handler。

## 3. Typed Contract

```rust
pub const BUSINESS_CHECKPOINT_SCHEMA_VERSION: &str = "business-checkpoint/v1";

pub enum BusinessCheckpointBoundary {
    ChapterDraftSaved,
}

pub enum BusinessCheckpointOutputReferenceV1 {
    Chapter { id: String },
}

pub struct BusinessCheckpointV1 {
    pub schema_version: String,
    pub boundary: BusinessCheckpointBoundary,
    pub revision: u64,
    pub idempotency_key: String,
    pub input_digest: String,
    pub output_reference: BusinessCheckpointOutputReferenceV1,
    pub recorded_at: String,
}
```

JSON 形态：

```json
{
  "schema_version": "business-checkpoint/v1",
  "boundary": "chapter_draft_saved",
  "revision": 1,
  "idempotency_key": "sha256:<64 hex>",
  "input_digest": "sha256:<64 hex>",
  "output_reference": {
    "kind": "chapter",
    "id": "chapter-id"
  },
  "recorded_at": "2026-07-16T00:00:00Z"
}
```

读取结果使用 typed enum 区分：

```rust
Missing | Valid(BusinessCheckpointV1) | UnsupportedSchema { schema_version } | Invalid
```

`Missing` 只表示 legacy。`UnsupportedSchema`/`Invalid` 可安全保留原 JSON，但不得作为恢复证据。

## 4. Canonical Idempotency

canonical hash 输入使用内部 allowlist struct/BTreeMap，不直接 hash 任意 runtime JSON：

```text
schema_version
batch_task_id
boundary
revision
input_digest
output_reference.kind
output_reference.id
```

序列化为 deterministic JSON 后计算 SHA-256，返回 `sha256:<hex>`。`recorded_at` 不进入 hash，
保证相同业务身份重复构建稳定。R4 `input_digest` 直接复用已验证 snapshot 值，不复制 Story Packet
摘要算法。

## 5. Revision Semantics

首个 batch 实现以 `completed_chapters.max(1)` 转为 `u64` 作为候选 revision：

```text
revision = max(candidate_completed_revision, existing_valid_checkpoint.revision)
```

若重复持久化同一章节且业务身份相同，revision 和 idempotency key 保持不变。若后续章节成功，
completed 增加，revision 单调增长。未知/非法旧 checkpoint 不参与 revision 计算，避免信任未验证字段。

## 6. Persistence Data Flow

`persist_post_generation_success` 当前在正文/分析/quality gate 完成后调用 `chapter_succeeded`。
R6 保持该边界不变：

1. persistence owner 加载当前 snapshot runtime state。
2. 从 `generation_contract_snapshot` 读取并验证 R4 contract。
3. 若 contract 合法，构建 `BusinessCheckpointV1`。
4. `BatchGenerationRuntimePersistencePlan` 生成原 runtime checkpoint，并 additive 合并
   `business_checkpoint`。
5. 仍通过现有 `upsert_batch_generation_runtime_snapshot` 写入。
6. legacy 缺 contract 时只写旧 runtime checkpoint，保持兼容。

不要求 task update 与 snapshot update 在 R6 新增跨表 transaction；R6 不扩大既有原子性边界。
恢复验证将通过 output reference 防止把不完整写入误认作有效业务边界。

## 7. Resume Validation

resume command 在 reset/launch 之前执行联合验证：

1. 从 runtime state 读取 R4 contract 和 business checkpoint。
2. `Missing`：返回 legacy-compatible 结果，继续旧恢复。
3. `UnsupportedSchema`/`Invalid`：返回 typed unsupported/invalid 错误，阻止基于未知证据启动。
4. `Valid`：校验 schema/boundary/revision/idempotency 和 digest 格式。
5. checkpoint digest 必须等于恢复的 R4 contract digest。
6. chapter reference 必须能按 id 查询，且属于 task/project 的目标范围。
7. `chapter.content.trim()` 必须非空。
8. 验证通过后将 checkpoint 保留在 resume runtime seed 中，现有 merge 继续保留其他 state。

错误不回显正文、Prompt 或完整 runtime JSON，只返回稳定分类：

```text
UnsupportedBusinessCheckpoint
InvalidBusinessCheckpoint
BusinessCheckpointInputDigestMismatch
BusinessCheckpointOutputMissing
BusinessCheckpointOutputEmpty
BusinessCheckpointOutputOutOfScope
```

## 8. Compatibility

- 旧 JSON 无 `business_checkpoint`：完全沿用旧路径。
- 旧 `workflow_runtime_state.checkpoint`：名称和语义不变。
- 新字段只写入已有 JSON object；非 object runtime state 继续使用现有 merge fallback。
- 不修改公开 request/response/SSE schema，不要求前端升级。
- 未来 v2 通过 read result 的 `UnsupportedSchema` 明确隔离，不让 v1 reader误解析。

## 9. Security

- schema 构造只接受 typed primitives 和 chapter id，不接受任意 metadata map。
- persisted JSON 不包含 Prompt、正文、API key、Authorization、endpoint URL 或 diagnostics。
- 错误 `Display` 仅输出分类和非敏感标识，不输出 runtime payload。
- 测试使用含敏感诱饵的外围 runtime state，断言 checkpoint 子树严格等于 allowlist。

## 10. Testing Strategy

单元测试：

- schema round-trip、allowlist、安全字段缺失。
- canonical key stability/change sensitivity。
- missing/unknown/invalid read compatibility。
- revision monotonic merge。

owner 测试：

- chapter success additive merge 保留 generation contract、quality/candidate/runtime checkpoint。
- legacy missing contract 不生成伪 checkpoint。
- resume digest mismatch、dangling/empty/out-of-scope output typed error。

DB-backed 测试：

- 建立 project/chapter/task/snapshot；snapshot 含 R4 contract。
- 保存章节正文并执行 chapter success persistence。
- 读取 business checkpoint 并执行 resume prepare/launch validation。
- 证明已保存章节作为业务恢复证据、后续 runtime seed 保留旧字段。

## 11. Rollout and Rollback

- rollout 为 additive Rust owner + JSON key，无 migration。
- 如需回滚，停止构建/读取 `business_checkpoint` 即可；旧 task、snapshot 和 API 仍可工作。
- 已写入的 additive key 可被旧版本忽略，无需数据 downgrade。
- 禁止使用 git reset、整文件回退、生产 migration downgrade。
