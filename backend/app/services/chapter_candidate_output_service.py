"""Chapter candidate streamed output collection service."""
from __future__ import annotations

import asyncio
from dataclasses import dataclass
from typing import Any, Dict, List, Optional

from app.services.ai_service import AIService
from app.services.chapter_candidate_runtime_state_service import (
    snapshot_chapter_candidate_runtime_state,
    sync_chapter_candidate_runtime_state,
)
from app.services.chapter_generated_text_service import trim_text_to_sentence_boundary


@dataclass(slots=True)
class ChapterCandidateOutputRequest:
    ai_service: AIService
    generate_kwargs: Dict[str, Any]
    candidate_index: int = 1
    max_output_chars: Optional[int] = None
    runtime_state: Optional[Dict[str, Any]] = None


async def collect_generation_candidate_output(
    *,
    request: ChapterCandidateOutputRequest,
) -> tuple[str, List[str]]:
    full_content = ''
    chunks: List[str] = []
    candidate_index = max(int(request.candidate_index or 1), 1)
    candidate_total = candidate_index
    runtime_state = request.runtime_state
    max_output_chars = request.max_output_chars

    if runtime_state is not None:
        candidate_total = snapshot_chapter_candidate_runtime_state(
            runtime_state,
            default_candidate_total=candidate_index,
        ).candidate_total
        sync_chapter_candidate_runtime_state(
            runtime_state,
            candidate_index=candidate_index,
            candidate_total=candidate_total,
            current_chars=0,
            chunk_count=0,
        )

    async for chunk in request.ai_service.generate_text_stream(**request.generate_kwargs):
        full_content += chunk
        chunks.append(chunk)
        if runtime_state is not None:
            sync_chapter_candidate_runtime_state(
                runtime_state,
                candidate_index=candidate_index,
                candidate_total=candidate_total,
                current_chars=len(full_content),
                chunk_count=len(chunks),
            )
        if max_output_chars and len(full_content) >= max_output_chars:
            break
        await asyncio.sleep(0)

    if max_output_chars and len(full_content) > max_output_chars:
        full_content = trim_text_to_sentence_boundary(
            full_content,
            hard_limit=max_output_chars,
        )
        chunks = [full_content] if full_content else []

    return full_content, chunks
