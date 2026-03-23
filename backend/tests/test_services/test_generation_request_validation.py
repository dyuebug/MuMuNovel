import pytest
from pydantic import ValidationError

from app.schemas.chapter import BatchGenerateRequest, ChapterGenerateRequest
from app.schemas.outline import OutlineGenerateRequest
from app.schemas.project import ProjectCreate, ProjectUpdate


def test_should_validate_chapter_generate_request_quality_fields():
    request = ChapterGenerateRequest(
        quality_preset='plot_drive',
        quality_notes='  ??????????  ',
        story_creation_brief='  ????????????  ',
    )

    assert request.quality_preset == 'plot_drive'
    assert request.quality_notes == '??????????'
    assert request.story_creation_brief == '????????????'


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
        theme='????',
        chapter_count=12,
        narrative_perspective='????',
        quality_preset='immersive',
        quality_notes='  ????????????  ',
    )

    assert request.quality_preset == 'immersive'
    assert request.quality_notes == '????????????'


def test_should_forbid_unknown_fields_in_outline_generate_request():
    with pytest.raises(ValidationError):
        OutlineGenerateRequest(
            project_id='project-1',
            theme='????',
            chapter_count=12,
            narrative_perspective='????',
            unexpected='bad-field',
        )


def test_should_validate_project_generation_defaults_and_trim_texts():
    payload = ProjectUpdate(
        default_quality_preset='clean_prose',
        default_story_creation_brief='  ??????????????  ',
        default_quality_notes='  ???????????  ',
    )

    assert payload.default_quality_preset == 'clean_prose'
    assert payload.default_story_creation_brief == '??????????????'
    assert payload.default_quality_notes == '???????????'


def test_should_reject_invalid_project_generation_defaults():
    with pytest.raises(ValidationError):
        ProjectUpdate(default_quality_preset='invalid-preset')


def test_should_forbid_unknown_fields_in_project_create_request():
    with pytest.raises(ValidationError):
        ProjectCreate(
            title='????',
            unknown_field='should-fail',
        )
