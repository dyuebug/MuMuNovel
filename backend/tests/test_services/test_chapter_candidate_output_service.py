import pytest

from tests.test_support.chapter_candidate_executor_test_support import (
    ChapterCandidateOutputRequest,
    collect_generation_candidate_output,
)


pytestmark = pytest.mark.asyncio


class StubAIService:
    def __init__(self, chunks: list[str]):
        self._chunks = list(chunks)
        self.calls: list[dict] = []

    async def generate_text_stream(self, **kwargs):
        self.calls.append(dict(kwargs))
        for chunk in self._chunks:
            yield chunk


async def test_should_collect_candidate_output_and_sync_runtime_state():
    runtime_state = {'candidate_total': 3}
    ai_service = StubAIService(['abc', 'def', 'ghi'])

    full_content, chunks = await collect_generation_candidate_output(
        request=ChapterCandidateOutputRequest(
            ai_service=ai_service,
            generate_kwargs={'prompt': 'hello'},
            candidate_index=2,
            runtime_state=runtime_state,
        ),
    )

    assert full_content == 'abcdefghi'
    assert chunks == ['abc', 'def', 'ghi']
    assert ai_service.calls == [{'prompt': 'hello'}]
    assert runtime_state['candidate_index'] == 2
    assert runtime_state['candidate_total'] == 3
    assert runtime_state['current_chars'] == 9
    assert runtime_state['chunk_count'] == 3


async def test_should_stop_when_reaching_max_output_chars_and_trim_result():
    ai_service = StubAIService([
        '第一句。',
        '第二句。',
        '第三句。',
    ])

    full_content, chunks = await collect_generation_candidate_output(
        request=ChapterCandidateOutputRequest(
            ai_service=ai_service,
            generate_kwargs={'prompt': 'trim'},
            candidate_index=1,
            max_output_chars=5,
        ),
    )

    assert full_content == '第一句。'
    assert chunks == ['第一句。']


async def test_should_keep_candidate_total_equal_to_index_without_runtime_state():
    ai_service = StubAIService(['xyz'])

    full_content, chunks = await collect_generation_candidate_output(
        request=ChapterCandidateOutputRequest(
            ai_service=ai_service,
            generate_kwargs={'prompt': 'solo'},
            candidate_index=4,
        ),
    )

    assert full_content == 'xyz'
    assert chunks == ['xyz']
