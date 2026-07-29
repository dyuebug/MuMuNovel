use std::collections::BTreeMap;

use serde_json::{json, Value};

use super::*;

fn sample_contract() -> (StoryPacketV1, GenerationIntentV1) {
    let target = GenerationTarget::chapter("project-1", "chapter-1");
    let mut packet = StoryPacketV1::new("project-1", target.clone());
    packet.sources.push(StoryPacketSource {
        kind: StoryPacketSourceKind::AuthoritativeDatabase,
        reference: Some("projects/project-1".to_owned()),
    });
    packet.current_chapter_number = Some(1);
    packet.chapter_count = Some(12);
    packet.target_word_count = Some(2_000);
    packet.story_long_term_goal = Some("完成主角成长弧".to_owned());

    let mut intent = GenerationIntentV1::new(GenerationIntentKind::ChapterGenerate, target);
    intent.target_word_count = Some(2_000);
    (packet, intent)
}

#[test]
fn generation_contract_service_should_round_trip_typed_schema() {
    let (packet, intent) = sample_contract();
    let snapshot = build_generation_contract_snapshot(packet, intent).expect("build snapshot");
    let serialized = serde_json::to_value(&snapshot).expect("serialize snapshot");
    let restored: GenerationContractSnapshotV1 =
        serde_json::from_value(serialized).expect("deserialize snapshot");

    assert_eq!(restored, snapshot);
    assert_eq!(restored.schema_version, GENERATION_CONTRACT_SCHEMA_VERSION);
    assert!(restored.input_digest.starts_with("sha256:"));
    assert_eq!(restored.input_digest.len(), "sha256:".len() + 64);
    validate_generation_contract_snapshot(&restored).expect("validate restored snapshot");
}

#[test]
fn generation_contract_service_should_normalize_key_order_and_nested_json_strings() {
    let (mut left_packet, left_intent) = sample_contract();
    left_packet.opaque_story_facts.insert(
        "world_state".to_owned(),
        Value::String(r#"{"b":2,"a":{"y":2,"x":1}}"#.to_owned()),
    );

    let (mut right_packet, right_intent) = sample_contract();
    right_packet.opaque_story_facts.insert(
        "world_state".to_owned(),
        json!({"a": {"x": 1, "y": 2}, "b": 2}),
    );

    let left = compute_input_digest(&left_packet, &left_intent).expect("left digest");
    let right = compute_input_digest(&right_packet, &right_intent).expect("right digest");
    assert_eq!(left, right);

    let canonical = canonical_json_string(json!({
        "z": 1,
        "a": r#"{"d":4,"c":3}"#,
    }))
    .expect("canonical JSON");
    assert_eq!(canonical, r#"{"a":{"c":3,"d":4},"z":1}"#);
}

#[test]
fn generation_contract_service_should_preserve_array_business_order() {
    let (mut left_packet, left_intent) = sample_contract();
    left_packet
        .opaque_story_facts
        .insert("beats".to_owned(), json!(["setup", "payoff"]));

    let (mut right_packet, right_intent) = sample_contract();
    right_packet
        .opaque_story_facts
        .insert("beats".to_owned(), json!(["payoff", "setup"]));

    assert_ne!(
        compute_input_digest(&left_packet, &left_intent).expect("left digest"),
        compute_input_digest(&right_packet, &right_intent).expect("right digest")
    );
}

#[test]
fn generation_contract_service_should_ignore_runtime_only_fields_in_digest() {
    let (packet, intent) = sample_contract();
    let baseline = compute_input_digest(&packet, &intent).expect("baseline digest");

    let mut runtime_packet = packet;
    runtime_packet
        .compatibility_metadata
        .insert("progress".to_owned(), json!(75));
    runtime_packet
        .compatibility_metadata
        .insert("retry_count".to_owned(), json!(3));
    runtime_packet
        .compatibility_metadata
        .insert("model".to_owned(), json!("runtime-model"));

    assert_eq!(
        compute_input_digest(&runtime_packet, &intent).expect("runtime digest"),
        baseline
    );
}

#[test]
fn generation_contract_service_should_apply_serde_defaults() {
    let (_, intent) = sample_contract();
    let mut value = serde_json::to_value(intent).expect("serialize intent");
    let object = value.as_object_mut().expect("intent object");
    object.remove("creative_overrides");
    object.remove("compatibility_metadata");

    let restored: GenerationIntentV1 = serde_json::from_value(value).expect("defaulted intent");
    assert_eq!(
        restored.creative_overrides,
        GenerationCreativeOverrides::default()
    );
    assert!(restored.compatibility_metadata.is_empty());
}

#[test]
fn generation_contract_service_should_reject_sensitive_fields() {
    let (mut packet, intent) = sample_contract();
    packet.compatibility_metadata.insert(
        "provider_context".to_owned(),
        Value::String(r#"{"apiKey":"secret"}"#.to_owned()),
    );

    let error = build_generation_contract_snapshot(packet, intent)
        .expect_err("sensitive field must be rejected");
    assert!(matches!(error, GenerationContractError::SensitiveField(_)));

    let error = canonicalize_json_value(json!({"Authorization": "Bearer secret"}))
        .expect_err("authorization must be rejected");
    assert!(matches!(error, GenerationContractError::SensitiveField(_)));
}

#[test]
fn generation_contract_service_should_validate_version_project_and_digest() {
    let (mut packet, intent) = sample_contract();
    packet.schema_version = "generation-contract/v2".to_owned();
    assert!(matches!(
        build_generation_contract_snapshot(packet, intent),
        Err(GenerationContractError::UnsupportedSchemaVersion(_))
    ));

    let (packet, mut intent) = sample_contract();
    intent.target.project_id = "project-2".to_owned();
    assert!(matches!(
        build_generation_contract_snapshot(packet, intent),
        Err(GenerationContractError::ProjectMismatch { .. })
    ));

    let (packet, mut intent) = sample_contract();
    intent.target_word_count = Some(0);
    assert!(matches!(
        build_generation_contract_snapshot(packet, intent),
        Err(GenerationContractError::InvalidTarget(_))
    ));

    let (packet, intent) = sample_contract();
    let mut snapshot = build_generation_contract_snapshot(packet, intent).expect("snapshot");
    snapshot.input_digest = "sha256:tampered".to_owned();
    assert!(matches!(
        validate_generation_contract_snapshot(&snapshot),
        Err(GenerationContractError::DigestMismatch { .. })
    ));
}

#[test]
fn generation_contract_service_should_validate_target_shape() {
    let invalid_target = GenerationTarget::chapter_selection(
        "project-1",
        "chapter-1",
        GenerationSelection {
            start_index: 9,
            end_index: 3,
            selected_text: None,
        },
    );
    let packet = StoryPacketV1::new("project-1", invalid_target.clone());
    let intent = GenerationIntentV1::new(
        GenerationIntentKind::ChapterPartialRegenerate,
        invalid_target,
    );
    assert!(matches!(
        build_generation_contract_snapshot(packet, intent),
        Err(GenerationContractError::InvalidTarget(_))
    ));
}

#[test]
fn generation_contract_service_should_merge_layers_in_fixed_precedence() {
    let target = GenerationTarget::chapter("project-1", "chapter-1");
    let mut defaults = StoryPacketV1::new("project-1", target);
    defaults.target_word_count = Some(800);
    defaults.story_long_term_goal = Some("system goal".to_owned());

    let authoritative = StoryPacketFactLayer {
        target_word_count: Some(1_500),
        story_long_term_goal: Some("authoritative goal".to_owned()),
        character_focus: Some("authoritative focus".to_owned()),
        ..StoryPacketFactLayer::default()
    };
    let persisted = StoryPacketFactLayer {
        target_word_count: Some(2_000),
        story_long_term_goal: Some("   ".to_owned()),
        character_focus: Some("persisted focus".to_owned()),
        ..StoryPacketFactLayer::default()
    };

    let merged = merge_story_packet_layers(defaults, authoritative, Some(persisted));
    assert_eq!(merged.target_word_count, Some(2_000));
    assert_eq!(
        merged.story_long_term_goal.as_deref(),
        Some("authoritative goal")
    );
    assert_eq!(merged.character_focus.as_deref(), Some("persisted focus"));
}

#[test]
fn generation_contract_service_should_limit_request_overrides_to_intent() {
    let target = GenerationTarget::chapter("project-1", "chapter-1");
    let mut intent = GenerationIntentV1::new(GenerationIntentKind::ChapterGenerate, target);
    intent.creative_overrides.narrative_style = Some("existing style".to_owned());

    apply_generation_intent_overrides(
        &mut intent,
        GenerationIntentOverrides {
            target_word_count: Some(2_400),
            creative_overrides: GenerationCreativeOverrides {
                narrative_style: Some("  ".to_owned()),
                story_direction: Some("向北推进".to_owned()),
                extra_constraints: vec!["保留伏笔".to_owned(), "保留伏笔".to_owned()],
                ..GenerationCreativeOverrides::default()
            },
            ..GenerationIntentOverrides::default()
        },
    );

    assert_eq!(intent.target.project_id, "project-1");
    assert_eq!(intent.target_word_count, Some(2_400));
    assert_eq!(
        intent.creative_overrides.narrative_style.as_deref(),
        Some("existing style")
    );
    assert_eq!(
        intent.creative_overrides.story_direction.as_deref(),
        Some("向北推进")
    );
    assert_eq!(
        intent.creative_overrides.extra_constraints,
        vec!["保留伏笔"]
    );
}

#[test]
fn generation_contract_service_should_fill_only_missing_continuity_ledgers() {
    let existing_character = StoryLedgerEntry {
        entity_type: "character".to_owned(),
        entity_id: "hero".to_owned(),
        opaque_state: json!({"mood": "calm"}),
    };
    let fallback_character = StoryLedgerEntry {
        entity_type: "character".to_owned(),
        entity_id: "hero".to_owned(),
        opaque_state: json!({"mood": "angry"}),
    };
    let fallback_relationship = StoryLedgerEntry {
        entity_type: "relationship".to_owned(),
        entity_id: "hero-rival".to_owned(),
        opaque_state: json!({"status": "tense"}),
    };
    let mut existing = StoryContinuitySnapshot {
        character_state_ledger: vec![existing_character.clone()],
        ..StoryContinuitySnapshot::default()
    };
    let fallback = StoryContinuitySnapshot {
        character_state_ledger: vec![fallback_character],
        relationship_state_ledger: vec![fallback_relationship.clone()],
        ..StoryContinuitySnapshot::default()
    };

    fill_missing_continuity(&mut existing, fallback);
    assert_eq!(existing.character_state_ledger, vec![existing_character]);
    assert_eq!(
        existing.relationship_state_ledger,
        vec![fallback_relationship]
    );
}

#[test]
fn generation_contract_service_should_merge_and_restore_runtime_snapshot() {
    let (packet, intent) = sample_contract();
    let snapshot = build_generation_contract_snapshot(packet, intent).expect("snapshot");
    let mut runtime_state = json!({
        "progress": {"completed": 2},
        "quality": {"status": "passed"},
        "candidate_gateway": {"selected": "candidate-1"},
        "checkpoint": {"stage": "prepared"},
    });

    merge_generation_contract_runtime_snapshot(&mut runtime_state, &snapshot)
        .expect("merge runtime snapshot");
    assert_eq!(runtime_state["progress"]["completed"], 2);
    assert_eq!(runtime_state["quality"]["status"], "passed");
    assert_eq!(
        runtime_state["candidate_gateway"]["selected"],
        "candidate-1"
    );
    assert_eq!(runtime_state["checkpoint"]["stage"], "prepared");
    assert!(runtime_state[GENERATION_CONTRACT_RUNTIME_NAMESPACE].is_object());

    assert_eq!(
        read_generation_contract_runtime_snapshot(&runtime_state),
        GenerationContractSnapshotRead::Valid(snapshot)
    );
}

#[test]
fn generation_contract_service_should_classify_legacy_unsupported_and_malformed_snapshots() {
    assert_eq!(
        read_generation_contract_runtime_snapshot(&json!({})),
        GenerationContractSnapshotRead::Missing
    );
    assert_eq!(
        read_generation_contract_runtime_snapshot(&json!({
            "story_packet": {"project_id": "legacy-project"}
        })),
        GenerationContractSnapshotRead::Legacy
    );
    assert_eq!(
        read_generation_contract_runtime_snapshot(&json!({
            "story_packet": {"schema_version": "generation-contract/v9"}
        })),
        GenerationContractSnapshotRead::UnsupportedVersion("generation-contract/v9".to_owned())
    );
    assert!(matches!(
        read_generation_contract_runtime_snapshot(&json!({
            "story_packet": {"schema_version": GENERATION_CONTRACT_SCHEMA_VERSION}
        })),
        GenerationContractSnapshotRead::Malformed(_)
    ));
}

#[test]
fn generation_contract_service_should_round_trip_optional_history_summary() {
    let (packet, intent) = sample_contract();
    let snapshot = build_generation_contract_snapshot(packet, intent).expect("snapshot");
    let mut history_payload = json!({"quality": {"score": 92}});

    merge_generation_contract_history_summary(&mut history_payload, &snapshot)
        .expect("merge history summary");
    assert_eq!(history_payload["quality"]["score"], 92);
    let restored = read_generation_contract_history_summary(&history_payload)
        .expect("read history summary")
        .expect("summary present");
    assert_eq!(restored.input_digest, snapshot.input_digest);
    assert_eq!(restored.intent_kind, GenerationIntentKind::ChapterGenerate);

    assert_eq!(
        read_generation_contract_history_summary(&json!({"quality": {}}))
            .expect("read legacy history"),
        None
    );
}

#[test]
fn generation_contract_service_should_preserve_exact_legacy_projection_shape() {
    let (mut packet, mut intent) = sample_contract();
    packet.compatibility_metadata.insert(
        "legacy_source".to_owned(),
        json!("single_generation_active_route"),
    );
    packet.character_focus = None;
    packet
        .opaque_story_facts
        .insert("character_focus".to_owned(), json!(["沈砚", "苏槿"]));
    packet.foreshadow_payoff_plan = None;
    packet
        .opaque_story_facts
        .insert("foreshadow_payoff_plan".to_owned(), json!(["回收旧约定"]));
    packet.continuity.character_state_ledger = vec![StoryLedgerEntry {
        entity_type: "character".to_owned(),
        entity_id: "character-1".to_owned(),
        opaque_state: json!({
            "label": "沈砚",
            "summary": "情绪收紧"
        }),
    }];
    intent.compatibility_metadata.insert(
        "legacy_mode".to_owned(),
        json!("single_generation_active_route"),
    );

    let legacy_packet = story_packet_to_legacy_flat_value(&packet);
    let legacy_intent = generation_intent_to_legacy_value(&intent);

    assert_eq!(legacy_packet["source"], "single_generation_active_route");
    assert_eq!(legacy_packet["character_focus"], json!(["沈砚", "苏槿"]));
    assert_eq!(
        legacy_packet["foreshadow_payoff_plan"],
        json!(["回收旧约定"])
    );
    assert_eq!(
        legacy_packet["character_state_ledger"],
        json!([{"label": "沈砚", "summary": "情绪收紧"}])
    );
    assert!(legacy_packet.get("schema_version").is_none());
    assert!(legacy_packet.get("sources").is_none());
    assert!(legacy_packet.get("target").is_none());
    assert!(legacy_packet.get("compatibility_metadata").is_none());
    assert!(legacy_packet["character_state_ledger"][0]
        .get("entity_type")
        .is_none());
    assert!(legacy_packet["character_state_ledger"][0]
        .get("entity_id")
        .is_none());
    assert_eq!(
        legacy_intent,
        json!({"mode": "single_generation_active_route"})
    );
}

#[test]
fn generation_contract_service_should_keep_public_projection_helpers_stable() {
    let (packet, intent) = sample_contract();
    let snapshot = build_generation_contract_snapshot(packet, intent).expect("snapshot");
    let runtime_value = generation_contract_runtime_value(&snapshot).expect("runtime value");
    assert_eq!(
        read_generation_contract_snapshot_value(&runtime_value),
        GenerationContractSnapshotRead::Valid(snapshot.clone())
    );
    let history = generation_contract_history_summary(&snapshot).expect("history summary");
    assert_eq!(history.schema_version, GENERATION_CONTRACT_SCHEMA_VERSION);
    assert_eq!(GENERATION_CONTRACT_HISTORY_FIELD, "story_packet");
}

#[test]
fn generation_contract_service_should_keep_contract_type_exports_used() {
    let target = GenerationTarget::outline("project-1", Some("outline-1".to_owned()));
    assert_eq!(target.kind, GenerationTargetKind::Outline);
    let batch = GenerationTarget::chapter_batch(
        "project-1",
        vec!["chapter-1".to_owned(), "chapter-2".to_owned()],
    );
    assert_eq!(batch.kind, GenerationTargetKind::ChapterBatch);

    let scope = GenerationRegenerationScope {
        reason: Some("修复连续性".to_owned()),
        ..GenerationRegenerationScope::default()
    };
    assert_eq!(scope.reason.as_deref(), Some("修复连续性"));

    let metadata = BTreeMap::<String, Value>::new();
    assert!(metadata.is_empty());
}
