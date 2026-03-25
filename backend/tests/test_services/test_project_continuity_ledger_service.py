from datetime import datetime, timedelta

import pytest
import pytest_asyncio
from sqlalchemy.ext.asyncio import async_sessionmaker, create_async_engine
from sqlalchemy.pool import StaticPool

from app.database import Base
from app.models.chapter import Chapter
from app.models.character import Character
from app.models.career import Career, CharacterCareer
from app.models.memory import PlotAnalysis, StoryMemory
from app.models.project import Project
from app.models.relationship import CharacterRelationship, Organization
from app.services.chapter_quality_context_service import (
    build_story_generation_packet_with_project_continuity,
)
from app.services.project_continuity_ledger_service import build_project_continuity_ledger

pytestmark = pytest.mark.asyncio


@pytest_asyncio.fixture
async def continuity_session_factory():
    engine = create_async_engine(
        "sqlite+aiosqlite://",
        connect_args={"check_same_thread": False},
        poolclass=StaticPool,
    )

    async with engine.begin() as conn:
        await conn.run_sync(Base.metadata.create_all)

    try:
        yield async_sessionmaker(engine, expire_on_commit=False)
    finally:
        await engine.dispose()


async def _seed_project(session_factory):
    project = Project(
        id="project-1",
        user_id="user-1",
        title="Continuity Test Project",
        chapter_count=12,
    )
    chapter = Chapter(
        id="chapter-7",
        project_id=project.id,
        chapter_number=7,
        title="Chapter 7",
    )
    lin = Character(
        id="char-lin",
        project_id=project.id,
        name="Lin",
        current_state="distrusts the council after the failed ambush",
        state_updated_chapter=8,
    )
    su = Character(
        id="char-su",
        project_id=project.id,
        name="Su",
        status="missing",
        status_changed_chapter=7,
    )
    rui = Character(
        id="char-rui",
        project_id=project.id,
        name="Rui",
    )
    shadow_guild = Character(
        id="org-shadow-guild",
        project_id=project.id,
        name="ShadowGuild",
        is_organization=True,
        current_state="internal command chain is unstable after the dock failure",
        state_updated_chapter=8,
    )
    organization = Organization(
        id="organization-1",
        character_id=shadow_guild.id,
        project_id=project.id,
        power_level=72,
        member_count=18,
        location="North Dock",
    )
    strategist = Career(
        id="career-strategist",
        project_id=project.id,
        name="Strategist",
        type="main",
        stages="[]",
        max_stage=9,
    )
    tactician_link = CharacterCareer(
        id="character-career-1",
        character_id=lin.id,
        career_id=strategist.id,
        career_type="main",
        current_stage=3,
        stage_progress=65,
        notes="promotion is blocked by the council vote",
        updated_at=datetime(2026, 3, 20, 12, 0, 0),
        created_at=datetime(2026, 3, 19, 12, 0, 0),
    )
    relationship = CharacterRelationship(
        id="rel-1",
        project_id=project.id,
        character_from_id=lin.id,
        character_to_id=su.id,
        relationship_name="alliance under strain",
        intimacy_level=58,
        updated_at=datetime(2026, 3, 20, 10, 0, 0),
        created_at=datetime(2026, 3, 18, 10, 0, 0),
    )
    foreshadow = StoryMemory(
        id="memory-1",
        project_id=project.id,
        chapter_id=chapter.id,
        memory_type="foreshadow",
        title="hidden key",
        content="The hidden key from the banquet still lacks a payoff.",
        story_timeline=6,
        importance_score=0.95,
        is_foreshadow=1,
        foreshadow_strength=0.9,
        updated_at=datetime(2026, 3, 20, 9, 0, 0),
        created_at=datetime(2026, 3, 20, 8, 0, 0),
    )
    analysis = PlotAnalysis(
        id="analysis-1",
        project_id=project.id,
        chapter_id=chapter.id,
        plot_stage="development",
        character_states=[
            {
                "character_name": "Rui",
                "state_after": "starts suspecting the archive keeper",
                "relationship_changes": {
                    "Lin": "trust begins to crack",
                },
            }
        ],
        foreshadows=[
            {
                "content": "archive cipher",
                "type": "planted",
            }
        ],
        created_at=datetime(2026, 3, 20, 11, 0, 0),
    )

    async with session_factory() as session:
        session.add_all([project, chapter, lin, su, rui, shadow_guild, organization, strategist, tactician_link, relationship, foreshadow, analysis])
        await session.commit()

    return project


async def test_should_build_project_continuity_ledger_from_persisted_story_state(
    continuity_session_factory,
):
    project = await _seed_project(continuity_session_factory)

    async with continuity_session_factory() as session:
        ledger = await build_project_continuity_ledger(session, project.id)

    assert any(item.startswith("Lin:") for item in ledger.character_state_ledger)
    assert any(item.startswith("Su:") and "missing" in item for item in ledger.character_state_ledger)
    assert any(item.startswith("Rui:") for item in ledger.character_state_ledger)
    assert any(item.startswith("Lin/Su:") for item in ledger.relationship_state_ledger)
    assert any(
        item.startswith("Rui/Lin:") or item.startswith("Lin/Rui:")
        for item in ledger.relationship_state_ledger
    )
    assert any(item.startswith("hidden key") for item in ledger.foreshadow_state_ledger)
    assert any(item.startswith("archive cipher") for item in ledger.foreshadow_state_ledger)
    assert any(item.startswith("ShadowGuild:") for item in ledger.organization_state_ledger)
    assert any(item.startswith("Lin/Strategist:") for item in ledger.career_state_ledger)


async def test_should_fill_story_packet_with_project_continuity_without_overriding_explicit_ledgers(
    continuity_session_factory,
):
    project = await _seed_project(continuity_session_factory)

    async with continuity_session_factory() as session:
        packet = await build_story_generation_packet_with_project_continuity(
            session,
            project,
            source_label="continuity-defaults",
        )
        override_packet = await build_story_generation_packet_with_project_continuity(
            session,
            project,
            source={
                "story_character_state_ledger": [
                    "CustomHero: keep the oath visible in every confrontation",
                ],
            },
            source_label="continuity-override",
        )

    assert any(item.startswith("Lin:") for item in packet.blueprint.character_state_ledger)
    assert any(item.startswith("Lin/Su:") for item in packet.blueprint.relationship_state_ledger)
    assert any(item.startswith("hidden key") for item in packet.blueprint.foreshadow_state_ledger)
    assert any(item.startswith("ShadowGuild:") for item in packet.blueprint.organization_state_ledger)
    assert any(item.startswith("Lin/Strategist:") for item in packet.blueprint.career_state_ledger)

    assert override_packet.blueprint.character_state_ledger == (
        "CustomHero: keep the oath visible in every confrontation",
    )
    assert any(item.startswith("Lin/Su:") for item in override_packet.blueprint.relationship_state_ledger)
    assert any(item.startswith("hidden key") for item in override_packet.blueprint.foreshadow_state_ledger)
    assert any(item.startswith("ShadowGuild:") for item in override_packet.blueprint.organization_state_ledger)
    assert any(item.startswith("Lin/Strategist:") for item in override_packet.blueprint.career_state_ledger)


async def test_should_reuse_project_continuity_ledger_within_same_transaction(
    continuity_session_factory,
    monkeypatch,
):
    project = await _seed_project(continuity_session_factory)

    async with continuity_session_factory() as session:
        original_execute = session.execute
        call_count = 0

        async def counted_execute(*args, **kwargs):
            nonlocal call_count
            call_count += 1
            return await original_execute(*args, **kwargs)

        monkeypatch.setattr(session, "execute", counted_execute)

        first_ledger = await build_project_continuity_ledger(session, project.id)
        first_call_count = call_count
        second_ledger = await build_project_continuity_ledger(session, project.id)

        assert second_ledger == first_ledger
        assert call_count == first_call_count

        await session.commit()
        refreshed_ledger = await build_project_continuity_ledger(session, project.id)

    assert refreshed_ledger == first_ledger
    assert call_count > first_call_count
