import pytest
from pydantic import ValidationError

from app.schemas.chapter import BatchGenerateRequest, ChapterGenerateRequest
from app.schemas.outline import OutlineGenerateRequest
from app.schemas.project import ProjectCreate, ProjectUpdate


def test_should_validate_chapter_generate_request_quality_fields():
    request = ChapterGenerateRequest(
        quality_preset='plot_drive',
        quality_notes='  保持冲突张力  ',
        story_creation_brief='  强化开篇悬念  ',
    )

    assert request.quality_preset == 'plot_drive'
    assert request.quality_notes == '保持冲突张力'
    assert request.story_creation_brief == '强化开篇悬念'


def test_should_reject_invalid_chapter_generate_request_quality_preset():
    with pytest.raises(ValidationError):
        ChapterGenerateRequest(quality_preset='invalid')


def test_should_forbid_unknown_fields_in_batch_generate_request():
    with pytest.raises(ValidationError):
        BatchGenerateRequest(
            start_chapter_number=1,
            count=2,
            unknown_field='should-fail',
        )


def test_should_validate_outline_generate_request_quality_fields():
    request = OutlineGenerateRequest(
        project_id='project-1',
        theme='悬疑冒险',
        chapter_count=12,
        narrative_perspective='third_person',
        quality_preset='immersive',
        quality_notes='  突出人物代价  ',
    )

    assert request.quality_preset == 'immersive'
    assert request.quality_notes == '突出人物代价'


def test_should_forbid_unknown_fields_in_outline_generate_request():
    with pytest.raises(ValidationError):
        OutlineGenerateRequest(
            project_id='project-1',
            theme='悬疑冒险',
            chapter_count=12,
            narrative_perspective='third_person',
            unexpected='bad-field',
        )


def test_should_validate_project_generation_defaults_and_trim_texts():
    payload = ProjectUpdate(
        default_quality_preset='clean_prose',
        default_story_creation_brief='  强化双线冲突推进  ',
        default_quality_notes='  保持电影感节奏  ',
    )

    assert payload.default_quality_preset == 'clean_prose'
    assert payload.default_story_creation_brief == '强化双线冲突推进'
    assert payload.default_quality_notes == '保持电影感节奏'


def test_should_reject_invalid_project_generation_defaults():
    with pytest.raises(ValidationError):
        ProjectUpdate(default_quality_preset='invalid-preset')


def test_should_forbid_unknown_fields_in_project_create_request():
    with pytest.raises(ValidationError):
        ProjectCreate(
            title='测试项目',
            unknown_field='should-fail',
        )
