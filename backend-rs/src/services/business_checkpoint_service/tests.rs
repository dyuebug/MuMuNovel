use chrono::{TimeZone, Utc};
use serde_json::json;

use super::*;

fn input_digest(seed: char) -> String {
    format!("sha256:{}", seed.to_string().repeat(64))
}

fn sample_checkpoint() -> BusinessCheckpointV1 {
    build_business_checkpoint(
        "task-1",
        BusinessCheckpointBoundary::ChapterDraftSaved,
        1,
        &input_digest('a'),
        BusinessCheckpointOutputReferenceV1::Chapter {
            id: "chapter-1".to_owned(),
        },
        Utc.with_ymd_and_hms(2026, 7, 16, 1, 2, 3)
            .single()
            .expect("timestamp"),
    )
    .expect("checkpoint")
}

#[test]
fn business_checkpoint_should_round_trip_typed_allowlist_schema() {
    let checkpoint = sample_checkpoint();
    let value = serde_json::to_value(&checkpoint).expect("serialize checkpoint");
    let object = value.as_object().expect("checkpoint object");

    assert_eq!(
        object.keys().map(String::as_str).collect::<Vec<_>>(),
        vec![
            "boundary",
            "idempotency_key",
            "input_digest",
            "output_reference",
            "recorded_at",
            "revision",
            "schema_version",
        ]
    );
    assert_eq!(value["schema_version"], BUSINESS_CHECKPOINT_SCHEMA_VERSION);
    assert_eq!(value["boundary"], "chapter_draft_saved");
    assert_eq!(value["output_reference"]["kind"], "chapter");
    assert_eq!(value["output_reference"]["id"], "chapter-1");
    assert!(checkpoint.idempotency_key.starts_with("sha256:"));
    assert_eq!(checkpoint.idempotency_key.len(), 71);
    validate_business_checkpoint(&checkpoint).expect("validate checkpoint");
}

#[test]
fn business_checkpoint_idempotency_key_should_be_stable_and_identity_sensitive() {
    let reference = BusinessCheckpointOutputReferenceV1::Chapter {
        id: "chapter-1".to_owned(),
    };
    let first = compute_business_checkpoint_idempotency_key(
        "task-1",
        BusinessCheckpointBoundary::ChapterDraftSaved,
        1,
        &input_digest('a'),
        &reference,
    )
    .expect("first key");
    let repeated = compute_business_checkpoint_idempotency_key(
        "task-1",
        BusinessCheckpointBoundary::ChapterDraftSaved,
        1,
        &input_digest('a'),
        &reference,
    )
    .expect("repeated key");
    let next_revision = compute_business_checkpoint_idempotency_key(
        "task-1",
        BusinessCheckpointBoundary::ChapterDraftSaved,
        2,
        &input_digest('a'),
        &reference,
    )
    .expect("next revision key");
    let next_output = compute_business_checkpoint_idempotency_key(
        "task-1",
        BusinessCheckpointBoundary::ChapterDraftSaved,
        1,
        &input_digest('a'),
        &BusinessCheckpointOutputReferenceV1::Chapter {
            id: "chapter-2".to_owned(),
        },
    )
    .expect("next output key");

    assert_eq!(first, repeated);
    assert_ne!(first, next_revision);
    assert_ne!(first, next_output);
}

#[test]
fn persisted_business_checkpoint_idempotency_key_should_validate_canonical_identity() {
    let checkpoint = sample_checkpoint();

    validate_business_checkpoint_idempotency_key("task-1", &checkpoint)
        .expect("validate persisted checkpoint key");
}

#[test]
fn persisted_business_checkpoint_idempotency_key_should_reject_tampered_key() {
    let mut checkpoint = sample_checkpoint();
    checkpoint.idempotency_key = input_digest('b');

    let error = validate_business_checkpoint_idempotency_key("task-1", &checkpoint)
        .expect_err("tampered key must be rejected");
    assert!(matches!(
        error,
        BusinessCheckpointError::IdempotencyKeyMismatch { actual, .. }
            if actual == input_digest('b')
    ));

    let mut malformed = sample_checkpoint();
    malformed.idempotency_key = "Bearer secret-checkpoint-key".to_owned();
    let error = validate_business_checkpoint_idempotency_key("task-1", &malformed)
        .expect_err("malformed key must be rejected before comparison");
    assert_eq!(error, BusinessCheckpointError::InvalidIdempotencyKey);
    assert!(!error.to_string().contains("secret-checkpoint-key"));
}

#[test]
fn persisted_business_checkpoint_idempotency_key_should_detect_canonical_input_changes() {
    let checkpoint = sample_checkpoint();

    let mut changed_digest = checkpoint.clone();
    changed_digest.input_digest = input_digest('c');
    assert!(matches!(
        validate_business_checkpoint_idempotency_key("task-1", &changed_digest),
        Err(BusinessCheckpointError::IdempotencyKeyMismatch { .. })
    ));

    let mut changed_output = checkpoint.clone();
    changed_output.output_reference = BusinessCheckpointOutputReferenceV1::Chapter {
        id: "chapter-2".to_owned(),
    };
    assert!(matches!(
        validate_business_checkpoint_idempotency_key("task-1", &changed_output),
        Err(BusinessCheckpointError::IdempotencyKeyMismatch { .. })
    ));

    assert!(matches!(
        validate_business_checkpoint_idempotency_key("task-2", &checkpoint),
        Err(BusinessCheckpointError::IdempotencyKeyMismatch { .. })
    ));
}

#[test]
fn business_checkpoint_runtime_read_should_distinguish_legacy_unknown_and_invalid() {
    assert_eq!(
        read_business_checkpoint_runtime_state(&json!({"checkpoint": {"stage": "running"}})),
        BusinessCheckpointRead::Missing
    );
    assert_eq!(
        read_business_checkpoint_runtime_state(&json!({
            "business_checkpoint": {
                "schema_version": "business-checkpoint/v2",
                "prompt": "must-not-be-read"
            }
        })),
        BusinessCheckpointRead::UnsupportedSchema {
            schema_version: "business-checkpoint/v2".to_owned()
        }
    );
    assert_eq!(
        read_business_checkpoint_runtime_state(&json!({
            "business_checkpoint": {
                "schema_version": BUSINESS_CHECKPOINT_SCHEMA_VERSION,
                "revision": 0
            }
        })),
        BusinessCheckpointRead::Invalid
    );
}

#[test]
fn business_checkpoint_merge_should_preserve_runtime_state_and_exclude_sensitive_payloads() {
    let checkpoint = sample_checkpoint();
    let mut runtime_state = json!({
        "checkpoint": {"stage": "chapter_succeeded"},
        "generation_contract_snapshot": {"input_digest": input_digest('a')},
        "prompt": "sensitive prompt outside checkpoint",
        "content": "sensitive body outside checkpoint",
        "authorization": "Bearer secret",
        "url": "https://user:secret@example.test/private"
    });

    merge_business_checkpoint_runtime_state(&mut runtime_state, &checkpoint)
        .expect("merge checkpoint");

    assert_eq!(runtime_state["checkpoint"]["stage"], "chapter_succeeded");
    assert_eq!(
        read_business_checkpoint_runtime_state(&runtime_state),
        BusinessCheckpointRead::Valid(checkpoint)
    );
    let serialized = serde_json::to_string(&runtime_state[BUSINESS_CHECKPOINT_RUNTIME_FIELD])
        .expect("serialize checkpoint subtree");
    for forbidden in [
        "sensitive prompt",
        "sensitive body",
        "Bearer secret",
        "example.test",
        "authorization",
        "prompt",
        "content",
        "url",
    ] {
        assert!(!serialized.contains(forbidden), "leaked {forbidden}");
    }
}

#[test]
fn business_checkpoint_validation_should_reject_invalid_fields() {
    let mut checkpoint = sample_checkpoint();
    checkpoint.input_digest = "not-a-digest".to_owned();
    assert_eq!(
        validate_business_checkpoint(&checkpoint),
        Err(BusinessCheckpointError::InvalidInputDigest)
    );

    let mut checkpoint = sample_checkpoint();
    checkpoint.output_reference = BusinessCheckpointOutputReferenceV1::Chapter {
        id: "  ".to_owned(),
    };
    assert_eq!(
        validate_business_checkpoint(&checkpoint),
        Err(BusinessCheckpointError::InvalidOutputReference)
    );
}
