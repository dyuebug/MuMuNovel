from app.services.chapter_generation.stream.wiring_service import (
    build_default_chapter_generation_stream_dependencies,
)



def test_should_build_default_chapter_generation_stream_dependencies():
    def cancel_outline_postprocess_tasks(_project_id: str) -> int:
        return 0

    async def candidate_generator(**_kwargs):
        return {}

    dependencies = build_default_chapter_generation_stream_dependencies(
        cancel_outline_postprocess_tasks_fn=cancel_outline_postprocess_tasks,
        candidate_generator_fn=candidate_generator,
        candidate_rerank_limit=3,
    )

    assert dependencies.execution.cancel_outline_postprocess_tasks_fn is cancel_outline_postprocess_tasks
    assert dependencies.candidate.candidate_generator_fn is candidate_generator
    assert dependencies.candidate.candidate_rerank_limit == 3
    assert dependencies.execution.calculate_max_tokens_fn(1600) == 960
    assert dependencies.execution.build_request_options_fn(type('AI', (), {'api_provider': 'openai_responses', 'config': type('Cfg', (), {'retry': type('Retry', (), {'max_retries': 5})()})()})())['transport_max_retries'] == 2
