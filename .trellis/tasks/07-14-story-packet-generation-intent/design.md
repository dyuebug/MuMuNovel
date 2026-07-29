# Design: Story Packet / Generation Intent 统一生成契约

## 1. Architecture Boundary

R4 在现有 API DTO 与现有 Prompt/Runtime/Provider 链路之间插入一个单一契约层：

```text
Legacy API DTO
  -> entry-specific compatibility adapter
  -> generation_contract_service
       -> StoryPacketV1
       -> GenerationIntentV1
       -> GenerationContractSnapshotV1
       -> canonical input digest
  -> existing domain prepare/runtime/prompt service
  -> existing provider execution
  -> existing snapshot/history persistence owner
```

契约层负责“输入事实和意图”，不负责 provider 选择、后台任务状态或业务 checkpoint。

## 2. Module Ownership

建议新增聚焦模块：

```text
backend-rs/src/services/generation_contract_service.rs
backend-rs/src/services/generation_contract_service/
  schema_owner.rs
  canonical_owner.rs
  adapter_owner.rs
  snapshot_owner.rs
```

- `schema_owner`：强类型 schema、version、intent kind、target、source metadata。
- `canonical_owner`：规范化 JSON、稳定序列化、SHA-256 digest。
- `adapter_owner`：旧入口输入到 canonical contract 的纯映射和覆盖优先级。
- `snapshot_owner`：runtime/history JSON 投影、可选读取、旧格式 fallback 辅助。

不放入 `models/`，因为该目录主要拥有数据库实体；不放入 API 文件，避免 transport 成为
业务契约 owner。

## 3. Core Contract

### 3.1 Versioned snapshot

```rust
GenerationContractSnapshotV1 {
    schema_version,
    story_packet,
    generation_intent,
    input_digest,
}
```

`input_digest` 对不含 digest 自身的 canonical snapshot 计算。格式固定为
`sha256:<lowercase hex>`。

### 3.2 Story Packet

Story Packet 表示服务端权威、可复用的故事上下文：

- schema version；
- source metadata；
- project reference；
- generation target reference；
- chapter position/target word count 等稳定目标上下文；
- story long-term goal、character focus、foreshadow payoff plan；
- character/relationship/foreshadow/organization/career continuity ledgers；
- prompt context provider 中可安全快照的故事事实；
- compatibility metadata，用于说明输入来自旧 DTO 或旧 snapshot，但不改变旧 API。

现有 ledger 载荷可先以“强类型外壳 + 受控 `Value` 内容”承载，避免在 R4 中复制全部领域
schema；自由 JSON 只能存在于明确命名的 opaque context 字段，不能再作为整个契约类型。

### 3.3 Generation Intent

`GenerationIntentKind` 至少覆盖：

- `OutlineGenerate`
- `OutlineExpand`
- `ChapterGenerate`
- `BatchChapterGenerate`
- `ChapterRegenerate`
- `ChapterPartialRegenerate`
- `ChapterReview`
- `ChapterRepair`

Intent 包含目标、目标字数、创作 override、质量要求和可选重生成/局部范围。重试次数、日志
前缀、运行进度和 provider 结果属于 execution runtime，不进入 immutable input contract。

## 4. Merge and Provenance Rules

固定归并顺序：

```text
system defaults
  -> authoritative project/chapter facts
  -> compatible persisted story packet/runtime snapshot
  -> current validated request overrides
```

规则：

- 空字符串、空可选值不得清除已有权威事实；显式允许清空的字段需单独建模。
- continuity ledger 只补齐缺失值，不覆盖已持久化的同版本 runtime snapshot。
- request 只能覆盖创作意图允许的字段，不能覆盖 project/chapter ownership 或完整 ledger。
- 每个来源写入 typed source metadata；自由字符串 `source` 只作为旧 JSON 兼容投影。

## 5. Canonical Serialization

规范化步骤：

1. 完成默认值和 override 归并。
2. 去除时间戳、task progress、retry count、provider result 等 runtime-only 字段。
3. 对可识别的嵌套 JSON 字符串解析为 JSON value。
4. 对 object key 递归排序；数组保持业务顺序。
5. UTF-8 无 BOM紧凑序列化。
6. 使用现有 `sha2` 和 `hex` 依赖计算 digest。

实际 provider/model/fallback 不进入 digest，确保 R5 切换执行策略时同一输入仍具有相同 digest。

## 6. Entry Adapters

### Single chapter

替换当前 `build_single_generation_story_packet() -> Value` 的 owner，运行时持有 typed
snapshot；只在现有候选质量/兼容边界序列化为 `Value`。保留当前 flat field projection，避免
一次性修改所有质量消费者。

### Batch chapter

批次保存一个稳定项目 Story Packet snapshot，每章构建独立 Chapter intent。resume 优先使用
原 snapshot，禁止静默根据当前数据库重建并改变 digest；旧批次继续 compat fallback。

### Regeneration

全章和局部重生成复用相同 Story Packet，通过不同 intent kind 与 typed regeneration scope
表达选择范围、指令和 preserve constraints，不创建第二套 packet。

### Outline

现有 outline generate/expand DTO 经 adapter 生成 outline target 与 intent。Provider/model 请求字段
继续保持旧行为，但其实际选择属于 R5，不纳入 Story Packet digest。

### Review and repair

现有分析、quality context 和 story repair owner 消费同一 packet projection；review/repair 由 intent
kind 区分，不让质量模块成为 canonical packet owner。

## 7. Persistence and Recovery

### Runtime snapshot

在现有 `workflow_runtime_state` 下新增独立命名空间：

```json
{
  "story_packet": {
    "schema_version": "generation-contract/v1",
    "story_packet": {},
    "generation_intent": {},
    "input_digest": "sha256:..."
  }
}
```

使用现有 merge owner，不覆盖 progress、quality、gateway 或 checkpoint。

### Restore

```text
valid supported story_packet snapshot
  -> deserialize and validate digest/version
otherwise missing/unsupported legacy snapshot
  -> existing compat options/request runtime state fallback
malformed new snapshot
  -> explicit error or safe fallback according to current route contract; never panic
```

不迁移历史快照，不要求旧记录补写。

### Generation history

章节历史现有 JSON payload增加 `story_packet` 摘要。读取 view 对字段保持 optional；旧 payload
完全兼容。当前固定 `model` 列不被当作真实执行模型审计证据。

## 8. Frontend Compatibility

默认不要求前端提交 Story Packet。旧请求字段、页面表单、Zustand store、task payload、SSE 解析和
response shape 保持不变。只有后端返回新增可选 metadata 且前端确需展示时，才在共享
`frontend/src/types/index.ts` 增加可选类型；禁止页面内局部 cast 或 ad hoc fetch。

## 9. Error and Security Boundary

- schema version 不支持、digest 不匹配、目标引用不合法时返回 typed service error，再由旧 route
  翻译为原有 HTTP/SSE 错误形状。
- snapshot 不保存 API key、Authorization、完整 provider request、认证 cookie 或内部 secret。
- project/chapter ownership 继续由现有访问控制 owner 校验，contract adapter 不绕过权限。

## 10. Rollout and Rollback

按入口渐进接入，每阶段保持旧 DTO 和旧投影：

1. schema/canonical owner；
2. 单章；
3. 批量和 resume；
4. 重生成；
5. 大纲；
6. review/repair；
7. history snapshot。

回滚时可按入口撤销 adapter 消费；旧 API 和旧 snapshot fallback 始终保留，因此无需数据回滚或
migration downgrade。
