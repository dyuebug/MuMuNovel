# R4 Phase G Compatibility Audit

- Date: 2026-07-15
- Scope: Story Packet / Generation Intent R4 compatibility boundary
- Result: **PASS**

## Executive conclusion

R4 remains a Rust-service-owned internal generation contract. The current implementation does not expose
`StoryPacketV1`, `GenerationIntentV1`, `GenerationContractSnapshotV1`, or `input_digest` as new frontend
request/store state, does not add or rename public SSE event kinds, and does not change the public request or
response field shapes in the R4-sensitive API diffs. Existing provider/model request values remain runtime-only
execution inputs and are not copied into Story Packet, Generation Intent, history summary, or the canonical digest.

The working tree contains changes from multiple Trellis tasks. This audit therefore treats unrelated R3 workflow
DTO/store changes and background-task UI changes as out of R4 scope instead of attributing them to R4.

## Audit matrix

| Boundary | Result | Evidence |
|---|---|---|
| Public request/response DTO shape | PASS | Structural diff scan over `backend-rs/src/api/outlines.rs`, `backend-rs/src/api/outlines/plot_expansion_owner.rs`, and the regeneration stream owner found no added/removed public struct or enum fields. The single-generation route signature remains `Option<Json<SingleChapterGenerationRouteRequest>>` and returns the existing SSE/error tuple at `backend-rs/src/api/chapter_generation_routes.rs:159-179`. |
| Outline provider/model compatibility | PASS | The new outline contract is prepared internally, while the existing `body.provider` and `body.model` values are still passed to `wizard_service::generate_outline` at `backend-rs/src/api/outlines.rs:554-605`. Contract tests cover project defaults, non-empty request overrides, empty-value non-overwrite behavior, and runtime-only provider/model exclusion at `backend-rs/src/api/outlines/contract_prepare_owner.rs:628-716`. |
| SSE event kinds and error codes | PASS | R4 outline branches call the existing `SseChannel::progress/error/result/done` facade, for example `backend-rs/src/api/outlines.rs:573-576` and `backend-rs/src/api/outlines.rs:1057-1099`. The facade still projects the existing `progress`, `chunk`, `result`, `error`, and `done` payload kinds at `backend-rs/src/utils/sse.rs:52-100` and is unchanged by R4. No new named SSE event kind or error-code contract was added. |
| Frontend request types / Zustand / task store | PASS | Exact search for `StoryPacket`, `GenerationIntent`, `generation_contract`, `story_packet`, and `input_digest` under `frontend/src` returned no matches. The current `frontend/src/types/index.ts:117-152` additions belong to the separate R3 Novel Workflow State Machine boundary. R4 adds no frontend request or store field. |
| Story Packet ownership | PASS | The typed owner lives under `backend-rs/src/services/generation_contract_service/`; contract construction and digest validation remain server-side at `backend-rs/src/services/generation_contract_service/canonical_owner.rs:84-152`. Clients cannot submit a complete Story Packet through an R4 API DTO. |
| Provider/model and runtime-only digest behavior | PASS | Runtime-only compatibility metadata is stripped before digesting at `backend-rs/src/services/generation_contract_service/canonical_owner.rs:135-152` and `backend-rs/src/services/generation_contract_service/canonical_owner.rs:324-351`. The regression test verifies model/runtime metadata does not change `input_digest` at `backend-rs/src/services/generation_contract_service/tests.rs:84-105`. |
| Sensitive-field exclusion | PASS | Canonicalization rejects sensitive keys before snapshot creation at `backend-rs/src/services/generation_contract_service/canonical_owner.rs:135-152`. Tests reject embedded `apiKey` and `Authorization` values at `backend-rs/src/services/generation_contract_service/tests.rs:122-136`. History receives only the optional contract summary through `merge_generation_contract_history_summary` at `backend-rs/src/services/chapter_generation_history_persistence_service/persistence_owner.rs:148-168`. |
| Database/schema/task-system boundary | PASS | R4 uses existing runtime snapshot/history JSON extension points. No R4 migration, new database table, second task system, second novel-phase fact, or Autopilot Coordinator is introduced. |

## Mixed-worktree attribution

The following visible changes are not evidence of an R4 compatibility break:

- `frontend/src/types/index.ts` contains R3 Novel Workflow State Machine types.
- Background-task UI/store changes belong to existing reliability/async tasks.
- `backend-rs/src/api/outlines.rs` also contains pre-existing retry/context changes; R4-specific additions are
  contract preparation, resolved parameter projection, digest-only debug fields, and compatible error routing.

The audit did not revert or rewrite any unrelated change.

## Playwright decision

**Targeted Playwright E2E: N/A for R4.**

Reason: R4 does not change a frontend request type, request payload, response DTO, Zustand/task-store state shape,
or SSE event kind. The relevant frontend compatibility gate is therefore covered by static cross-layer audit plus
`npm run lint --prefix frontend` and `npm run build --prefix frontend`. Existing Rust route/adapter tests cover the
legacy DTO-to-contract projection. If a later task exposes contract metadata to a page request or response, that
later task must add the corresponding targeted Playwright coverage.

## Verification evidence

Focused and full command logs are stored in this directory:

- `phase-f-*.log`
- `phase-g-generation-contract.log`
- `phase-g-single-generation.log`
- `phase-g-batch-generation.log`
- `phase-g-restore.log`
- `phase-g-regeneration.log`
- `phase-g-outlines.log`
- `phase-g-analysis.log`
- `phase-g-history.log`
- `phase-g-final-cargo-check.log`
- `phase-g-final-cargo-test.log`
- `phase-g-final-frontend-lint.log`
- `phase-g-final-frontend-build.log`
- `phase-g-r4-encoding-audit.log`

No compatibility repair is required by this audit.
