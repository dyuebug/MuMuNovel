from types import SimpleNamespace

from tests.test_support import (
    single_generation_stream_entry_test_adapter as stream_wiring_service,
)


DEPENDENCY_TYPES = SimpleNamespace(
    execution=SimpleNamespace,
    candidate=SimpleNamespace,
    finalize=SimpleNamespace,
    root=SimpleNamespace,
)


def test_should_build_default_chapter_generation_stream_dependencies():
    def cancel_outline_postprocess_tasks(_project_id: str) -> int:
        return 0

    async def candidate_generator(**_kwargs):
        return {}

    dependencies = stream_wiring_service.build_default_chapter_generation_stream_dependencies(
        dependency_types=DEPENDENCY_TYPES,
        cancel_outline_postprocess_tasks_fn=cancel_outline_postprocess_tasks,
        candidate_generator_fn=candidate_generator,
        candidate_rerank_limit=3,
        one_to_one_builder_cls='one-builder',
        one_to_many_builder_cls='many-builder',
        get_template_fn='get-template-fn',
        format_prompt_fn='format-prompt-fn',
        apply_style_to_prompt_fn='apply-style-fn',
        build_runtime_system_prompt_fn='system-prompt-fn',
        detect_style_profile_fn='detect-style-fn',
        resolve_generation_temperature_fn='temperature-fn',
        compute_story_quality_metrics_fn='metrics-fn',
        resolve_quality_gate_execution_plan_fn='quality-plan-fn',
        analyze_chapter_background_fn='analysis-fn',
        resolve_story_repair_state_fn='repair-state-fn',
        memory_service='memory-service',
        foreshadow_service='foreshadow-service',
        build_outline_structure_runtime_sources_fn='outline-runtime-fn',
        build_generation_runtime_bundle_fn='runtime-bundle-fn',
        calculate_max_tokens_fn=lambda count: int(count * 0.6),
        build_request_options_fn=lambda _ai_service: {'transport_max_retries': 2},
        build_quality_runtime_context_fn='quality-runtime-context-fn',
        build_draft_attempt_fn='draft-attempt-fn',
        attach_story_runtime_contract_fn='attach-runtime-contract-fn',
        build_generation_history_payload_fn='history-payload-fn',
        create_analysis_task_fn='create-analysis-task-fn',
        build_candidate_draft_payload_fn='candidate-draft-payload-fn',
        build_stream_result_payload_fn='stream-result-payload-fn',
    )

    assert dependencies.execution.cancel_outline_postprocess_tasks_fn is cancel_outline_postprocess_tasks
    assert dependencies.execution.resolve_story_repair_state_fn == 'repair-state-fn'
    assert dependencies.execution.memory_service == 'memory-service'
    assert dependencies.execution.foreshadow_service == 'foreshadow-service'
    assert dependencies.execution.calculate_max_tokens_fn(1600) == 960
    assert dependencies.execution.build_request_options_fn(object())['transport_max_retries'] == 2
    assert dependencies.candidate.candidate_generator_fn is candidate_generator
    assert dependencies.candidate.candidate_rerank_limit == 3
    assert dependencies.candidate.build_draft_attempt_fn == 'draft-attempt-fn'
    assert dependencies.finalize.analyze_chapter_background_fn == 'analysis-fn'
    assert dependencies.finalize.build_stream_result_payload_fn == 'stream-result-payload-fn'
