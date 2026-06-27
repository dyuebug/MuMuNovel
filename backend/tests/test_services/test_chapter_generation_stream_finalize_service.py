import pytest

from tests.test_support import (
    single_generation_stream_orchestration_test_adapter as stream_finalize_service,
)


def test_should_build_analysis_followup_plan_for_manual_review():
    plan = stream_finalize_service.build_chapter_generation_analysis_followup_plan(
        enable_analysis=False,
        quality_gate_action='manual_review',
        quality_gate_requires_followup=True,
        full_content='generated content',
        candidate_word_count=456,
    )

    assert plan.should_schedule_analysis is True
    assert plan.analysis_reason == 'quality_gate_manual_review'
    assert plan.chapter_content_override == 'generated content'
    assert plan.chapter_word_count_override == 456
    assert plan.completion_message == '章节生成完成，已转入人工复核'
    assert plan.analysis_started_message == '人工复核分析任务已启动'


def test_should_build_stream_response_artifacts_for_retry_followup():
    chapter = type(
        'Chapter',
        (),
        {
            'id': 'chapter-1',
            'chapter_number': 8,
            'status': 'draft',
            'updated_at': '2026-06-17T10:00:00',
        },
    )()
    captured = {}

    def fake_build_candidate_draft_payload(**kwargs):
        captured['candidate_draft_kwargs'] = kwargs
        return {'draft_id': 'draft-1'}

    def fake_build_stream_result_payload(**kwargs):
        captured['result_payload_kwargs'] = kwargs
        return {'type': 'result', 'saved_word_count': kwargs['saved_word_count']}

    artifacts = stream_finalize_service.build_chapter_generation_stream_response_artifacts(
        chapter=chapter,
        draft_attempt={'id': 'draft-attempt'},
        quality_metrics={'overall_score': 93},
        quality_gate_action='retry',
        quality_gate_message='needs repair',
        quality_gate_snapshot={'decision': 'retry'},
        quality_gate_requires_followup=True,
        content_applied=False,
        saved_word_count=1450,
        task_id='task-1',
        story_runtime_contract={'runtime': True},
        analysis_started_message='质量修复分析任务已启动',
        build_candidate_draft_payload_fn=fake_build_candidate_draft_payload,
        build_stream_result_payload_fn=fake_build_stream_result_payload,
    )

    assert artifacts.quality_metrics_event_payload['type'] == 'quality_metrics'
    assert artifacts.quality_metrics_event_payload['overall_score'] == 93
    assert artifacts.quality_gate_event_payload['type'] == 'quality_gate_retry'
    assert artifacts.result_payload == {'type': 'result', 'saved_word_count': 1450}
    assert artifacts.analysis_started_event_data == {
        'task_id': 'task-1',
        'message': '质量修复分析任务已启动',
    }
    assert captured['candidate_draft_kwargs']['include_full_text'] is False
    assert captured['result_payload_kwargs']['candidate_draft'] == {'draft_id': 'draft-1'}
    assert captured['result_payload_kwargs']['hard_gate_blocked'] is True
    assert captured['result_payload_kwargs']['chapter_status'] == 'draft'


@pytest.mark.asyncio
async def test_should_emit_chapter_generation_stream_plan_in_order():
    plan = [
        stream_finalize_service.ChapterGenerationEmissionStep(
            kind='tracker_complete',
            message='done',
        ),
        stream_finalize_service.ChapterGenerationEmissionStep(
            kind='sse_payload',
            payload={'type': 'quality_metrics'},
        ),
        stream_finalize_service.ChapterGenerationEmissionStep(
            kind='tracker_result',
            payload={'type': 'result'},
        ),
        stream_finalize_service.ChapterGenerationEmissionStep(
            kind='sse_event',
            event='analysis_started',
            payload={'task_id': 'task-1'},
        ),
        stream_finalize_service.ChapterGenerationEmissionStep(kind='tracker_done'),
    ]

    async def fake_tracker_complete(message):
        return f'complete:{message}'

    async def fake_tracker_result(payload):
        return f"result:{payload['type']}"

    async def fake_tracker_done():
        return 'done'

    def fake_format_sse(payload):
        return f"sse:{payload['type']}"

    async def fake_send_event(**kwargs):
        return f"event:{kwargs['event']}:{kwargs['data']['task_id']}"

    emitted = []
    async for payload in stream_finalize_service.emit_chapter_generation_stream_plan(
        emission_plan=plan,
        tracker_complete_fn=fake_tracker_complete,
        tracker_result_fn=fake_tracker_result,
        tracker_done_fn=fake_tracker_done,
        format_sse_fn=fake_format_sse,
        send_event_fn=fake_send_event,
    ):
        emitted.append(payload)

    assert emitted == [
        'complete:done',
        'sse:quality_metrics',
        'result:result',
        'event:analysis_started:task-1',
        'done',
    ]
