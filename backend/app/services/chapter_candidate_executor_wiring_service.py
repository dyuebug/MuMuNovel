"""????????????????"""
from __future__ import annotations

from typing import Any, Callable

from app.services.chapter_candidate_executor_service import (
    build_chapter_candidate_executor_dependencies,
)
from app.services.chapter_candidate_finalize_service import (
    build_chapter_candidate_finalize_dependencies,
)
from app.services.chapter_candidate_generation_service import (
    build_chapter_candidate_generation_dependencies,
)
from app.services.chapter_candidate_rerank_service import (
    attach_candidate_selection_metadata,
    build_candidate_pool_summary,
    build_candidate_retry_prompt_suffix,
    build_candidate_retry_strategy_suffix,
    build_candidate_selection_metadata,
    build_targeted_final_repair_suffix,
    build_word_budget_repair_suffix,
    normalize_candidate_quality_gate_plan,
    resolve_candidate_retry_temperature,
    resolve_targeted_final_repair_char_limit,
    resolve_targeted_final_repair_max_tokens,
    resolve_targeted_final_repair_temperature,
    resolve_word_budget_repair_char_limit,
    resolve_word_budget_repair_max_tokens,
    resolve_word_budget_repair_temperature,
    select_best_generation_candidate,
    select_targeted_final_repair_seed_candidate,
    should_adopt_targeted_final_repair_candidate,
    should_apply_followup_targeted_final_repair,
    should_apply_targeted_final_repair,
    should_apply_word_budget_repair,
    should_generate_additional_candidate,
    should_keep_targeted_final_repair_candidate,
    should_keep_word_budget_repair_candidate,
    should_prefer_targeted_final_repair_candidate,
    should_prefer_word_budget_repair_candidate,
    should_relax_word_budget_repair_limits,
)
from app.services.chapter_candidate_targeted_final_repair_service import (
    build_chapter_candidate_targeted_final_repair_dependencies,
)
from app.services.chapter_candidate_word_budget_repair_service import (
    build_chapter_candidate_word_budget_repair_dependencies,
)


def build_default_chapter_candidate_executor_dependencies(
    *,
    resolve_generation_attempt_labels_fn: Callable[..., Any],
    sync_generation_runtime_state_fn: Callable[..., Any],
    collect_generation_candidate_output_fn: Callable[..., Any],
    build_generation_candidate_record_fn: Callable[..., Any],
):
    generation_dependencies = build_chapter_candidate_generation_dependencies(
        resolve_generation_attempt_labels_fn=resolve_generation_attempt_labels_fn,
        sync_generation_runtime_state_fn=sync_generation_runtime_state_fn,
        collect_generation_candidate_output_fn=collect_generation_candidate_output_fn,
        build_generation_candidate_record_fn=build_generation_candidate_record_fn,
        should_generate_additional_candidate_fn=should_generate_additional_candidate,
        build_candidate_retry_prompt_suffix_fn=build_candidate_retry_prompt_suffix,
        build_candidate_retry_strategy_suffix_fn=build_candidate_retry_strategy_suffix,
        resolve_candidate_retry_temperature_fn=resolve_candidate_retry_temperature,
        select_best_generation_candidate_fn=select_best_generation_candidate,
    )
    word_budget_repair_dependencies = build_chapter_candidate_word_budget_repair_dependencies(
        should_apply_word_budget_repair_fn=should_apply_word_budget_repair,
        build_word_budget_repair_suffix_fn=build_word_budget_repair_suffix,
        should_relax_word_budget_repair_limits_fn=should_relax_word_budget_repair_limits,
        resolve_word_budget_repair_temperature_fn=resolve_word_budget_repair_temperature,
        resolve_word_budget_repair_max_tokens_fn=resolve_word_budget_repair_max_tokens,
        resolve_generation_attempt_labels_fn=resolve_generation_attempt_labels_fn,
        sync_generation_runtime_state_fn=sync_generation_runtime_state_fn,
        collect_generation_candidate_output_fn=collect_generation_candidate_output_fn,
        resolve_word_budget_repair_char_limit_fn=resolve_word_budget_repair_char_limit,
        build_generation_candidate_record_fn=build_generation_candidate_record_fn,
        should_keep_word_budget_repair_candidate_fn=should_keep_word_budget_repair_candidate,
        select_best_generation_candidate_fn=select_best_generation_candidate,
        should_prefer_word_budget_repair_candidate_fn=should_prefer_word_budget_repair_candidate,
    )
    targeted_final_repair_dependencies = build_chapter_candidate_targeted_final_repair_dependencies(
        build_targeted_final_repair_suffix_fn=build_targeted_final_repair_suffix,
        resolve_targeted_final_repair_temperature_fn=resolve_targeted_final_repair_temperature,
        resolve_targeted_final_repair_max_tokens_fn=resolve_targeted_final_repair_max_tokens,
        sync_generation_runtime_state_fn=sync_generation_runtime_state_fn,
        collect_generation_candidate_output_fn=collect_generation_candidate_output_fn,
        resolve_targeted_final_repair_char_limit_fn=resolve_targeted_final_repair_char_limit,
        build_generation_candidate_record_fn=build_generation_candidate_record_fn,
        should_keep_targeted_final_repair_candidate_fn=should_keep_targeted_final_repair_candidate,
        should_adopt_targeted_final_repair_candidate_fn=should_adopt_targeted_final_repair_candidate,
        should_prefer_targeted_final_repair_candidate_fn=should_prefer_targeted_final_repair_candidate,
        should_apply_followup_targeted_final_repair_fn=should_apply_followup_targeted_final_repair,
    )
    finalize_dependencies = build_chapter_candidate_finalize_dependencies(
        resolve_generation_attempt_labels_fn=resolve_generation_attempt_labels_fn,
        build_candidate_selection_metadata_fn=build_candidate_selection_metadata,
        attach_candidate_selection_metadata_fn=attach_candidate_selection_metadata,
        normalize_candidate_quality_gate_plan_fn=normalize_candidate_quality_gate_plan,
        build_candidate_pool_summary_fn=build_candidate_pool_summary,
        sync_generation_runtime_state_fn=sync_generation_runtime_state_fn,
        select_best_generation_candidate_fn=select_best_generation_candidate,
        should_prefer_word_budget_repair_candidate_fn=should_prefer_word_budget_repair_candidate,
    )
    return build_chapter_candidate_executor_dependencies(
        generation_dependencies=generation_dependencies,
        word_budget_repair_dependencies=word_budget_repair_dependencies,
        targeted_final_repair_dependencies=targeted_final_repair_dependencies,
        finalize_dependencies=finalize_dependencies,
        should_apply_targeted_final_repair_fn=should_apply_targeted_final_repair,
        select_targeted_final_repair_seed_candidate_fn=select_targeted_final_repair_seed_candidate,
    )
