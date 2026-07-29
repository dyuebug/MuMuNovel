mod adapter_owner;
mod canonical_owner;
mod schema_owner;
mod snapshot_owner;

pub use adapter_owner::{
    apply_generation_intent_overrides, fill_missing_continuity, generation_intent_to_legacy_value,
    merge_story_packet_layers, story_packet_to_legacy_flat_value, GenerationIntentOverrides,
    StoryPacketFactLayer,
};
pub use canonical_owner::{
    build_generation_contract_snapshot, canonical_json_string, canonicalize_json_value,
    compute_input_digest, validate_generation_contract_snapshot, GenerationContractError,
};
pub use schema_owner::{
    GenerationContractSnapshotV1, GenerationCreativeOverrides, GenerationIntentKind,
    GenerationIntentV1, GenerationRegenerationScope, GenerationSelection, GenerationTarget,
    GenerationTargetKind, StoryContinuitySnapshot, StoryLedgerEntry, StoryPacketSource,
    StoryPacketSourceKind, StoryPacketV1, GENERATION_CONTRACT_SCHEMA_VERSION,
};
pub use snapshot_owner::{
    generation_contract_history_summary, generation_contract_runtime_value,
    merge_generation_contract_history_summary, merge_generation_contract_runtime_snapshot,
    read_generation_contract_history_summary, read_generation_contract_runtime_snapshot,
    read_generation_contract_snapshot_value, GenerationContractHistorySummaryV1,
    GenerationContractSnapshotRead, GENERATION_CONTRACT_HISTORY_FIELD,
    GENERATION_CONTRACT_RUNTIME_NAMESPACE,
};

#[cfg(test)]
mod tests;
