from __future__ import annotations

from typing import Any, Dict, List, Optional, Sequence

from app.models.chapter import Chapter
from app.models.memory import PlotAnalysis, StoryMemory


def _as_list(value: Any) -> List[Dict[str, Any]]:
    return [item for item in value if isinstance(item, dict)] if isinstance(value, list) else []


def _find_keyword_position(chapter_content: str, keyword: Any) -> tuple[int, int]:
    keyword_text = str(keyword or '').strip()
    if not keyword_text or not chapter_content:
        return -1, 0
    position = chapter_content.find(keyword_text)
    if position == -1:
        return -1, 0
    return position, len(keyword_text)


def _resolve_annotation_position_and_metadata(
    memory: StoryMemory,
    analysis: Optional[PlotAnalysis],
    chapter_content: str,
) -> tuple[int, int, Dict[str, Any]]:
    position = memory.chapter_position if memory.chapter_position is not None else -1
    length = memory.text_length if memory.text_length is not None else 0
    metadata_extra: Dict[str, Any] = {}

    hooks = _as_list(getattr(analysis, 'hooks', None) if analysis else None)
    foreshadows = _as_list(getattr(analysis, 'foreshadows', None) if analysis else None)
    plot_points = _as_list(getattr(analysis, 'plot_points', None) if analysis else None)

    if position == -1 and analysis and chapter_content:
        if memory.memory_type == 'hook':
            for hook in hooks:
                hook_type = str(hook.get('type') or '').strip()
                if memory.title and hook_type and hook_type in memory.title:
                    found_position, found_length = _find_keyword_position(chapter_content, hook.get('keyword'))
                    if found_position != -1:
                        position = found_position
                        length = found_length
                    metadata_extra['strength'] = hook.get('strength', 5)
                    metadata_extra['position_desc'] = hook.get('position', '')
                    break
        elif memory.memory_type == 'foreshadow':
            for foreshadow in foreshadows:
                if str(foreshadow.get('content') or '').strip() and str(foreshadow.get('content')) in str(memory.content or ''):
                    found_position, found_length = _find_keyword_position(chapter_content, foreshadow.get('keyword'))
                    if found_position != -1:
                        position = found_position
                        length = found_length
                    metadata_extra['foreshadow_type'] = foreshadow.get('type', 'planted')
                    metadata_extra['strength'] = foreshadow.get('strength', 5)
                    break
        elif memory.memory_type == 'plot_point':
            for plot_point in plot_points:
                if str(plot_point.get('content') or '').strip() and str(plot_point.get('content')) in str(memory.content or ''):
                    found_position, found_length = _find_keyword_position(chapter_content, plot_point.get('keyword'))
                    if found_position != -1:
                        position = found_position
                        length = found_length
                    break
    elif analysis:
        if memory.memory_type == 'hook':
            for hook in hooks:
                hook_type = str(hook.get('type') or '').strip()
                if memory.title and hook_type and hook_type in memory.title:
                    metadata_extra['strength'] = hook.get('strength', 5)
                    metadata_extra['position_desc'] = hook.get('position', '')
                    break
        elif memory.memory_type == 'foreshadow':
            for foreshadow in foreshadows:
                if str(foreshadow.get('content') or '').strip() and str(foreshadow.get('content')) in str(memory.content or ''):
                    metadata_extra['foreshadow_type'] = foreshadow.get('type', 'planted')
                    metadata_extra['strength'] = foreshadow.get('strength', 5)
                    break

    return position, length, metadata_extra


def build_chapter_annotations_payload(
    *,
    chapter: Chapter,
    analysis: Optional[PlotAnalysis],
    memories: Sequence[StoryMemory],
) -> Dict[str, Any]:
    chapter_content = str(chapter.content or '')
    annotations: List[Dict[str, Any]] = []

    for memory in memories:
        position, length, metadata_extra = _resolve_annotation_position_and_metadata(
            memory,
            analysis,
            chapter_content,
        )
        annotations.append(
            {
                'id': memory.id,
                'type': memory.memory_type,
                'title': memory.title,
                'content': memory.content,
                'importance': memory.importance_score or 0.5,
                'position': position,
                'length': length,
                'tags': memory.tags or [],
                'metadata': {
                    'is_foreshadow': memory.is_foreshadow,
                    'related_characters': memory.related_characters or [],
                    'related_locations': memory.related_locations or [],
                    **metadata_extra,
                },
            }
        )

    return {
        'chapter_id': chapter.id,
        'chapter_number': chapter.chapter_number,
        'title': chapter.title,
        'word_count': chapter.word_count or 0,
        'annotations': annotations,
        'has_analysis': analysis is not None,
        'summary': {
            'total_annotations': len(annotations),
            'hooks': sum(1 for item in annotations if item['type'] == 'hook'),
            'foreshadows': sum(1 for item in annotations if item['type'] == 'foreshadow'),
            'plot_points': sum(1 for item in annotations if item['type'] == 'plot_point'),
            'character_events': sum(1 for item in annotations if item['type'] == 'character_event'),
        },
    }
