from __future__ import annotations

from fastapi import FastAPI


def register_api_routers(app: FastAPI) -> None:
    from app.api import (
        admin,
        auth,
        background_tasks,
        book_import,
        careers,
        changelog,
        chapter_analysis_routes,
        chapter_analysis_task_routes,
        chapter_annotation_routes,
        chapter_batch_generation_routes,
        chapter_crud_routes,
        chapter_draft_routes,
        chapter_expansion_plan_routes,
        chapter_generation_routes,
        chapter_partial_regeneration_routes,
        chapter_quality_routes,
        chapter_regeneration_routes,
        characters,
        foreshadows,
        inspiration,
        memories,
        mcp_plugins,
        organizations,
        outlines,
        projects,
        prompt_templates,
        prompt_workshop,
        relationships,
        settings,
        users,
        wizard_stream,
        writing_styles,
    )

    app.include_router(auth.router, prefix="/api")
    app.include_router(users.router, prefix="/api")
    app.include_router(settings.router, prefix="/api")
    app.include_router(admin.router, prefix="/api")

    app.include_router(projects.router, prefix="/api")
    app.include_router(wizard_stream.router, prefix="/api")
    app.include_router(inspiration.router, prefix="/api")
    app.include_router(outlines.router, prefix="/api")
    app.include_router(characters.router, prefix="/api")
    app.include_router(careers.router, prefix="/api")
    app.include_router(chapter_crud_routes.router, prefix="/api")
    app.include_router(chapter_analysis_routes.router, prefix="/api")
    app.include_router(chapter_analysis_task_routes.router, prefix="/api")
    app.include_router(chapter_annotation_routes.router, prefix="/api")
    app.include_router(chapter_batch_generation_routes.router, prefix="/api")
    app.include_router(chapter_draft_routes.router, prefix="/api")
    app.include_router(chapter_expansion_plan_routes.router, prefix="/api")
    app.include_router(chapter_generation_routes.router, prefix="/api")
    app.include_router(chapter_quality_routes.router, prefix="/api")
    app.include_router(chapter_partial_regeneration_routes.router, prefix="/api")
    app.include_router(chapter_regeneration_routes.router, prefix="/api")
    app.include_router(relationships.router, prefix="/api")
    app.include_router(organizations.router, prefix="/api")
    app.include_router(writing_styles.router, prefix="/api")
    app.include_router(memories.router)
    app.include_router(foreshadows.router)
    app.include_router(mcp_plugins.router, prefix="/api")
    app.include_router(prompt_templates.router, prefix="/api")
    app.include_router(changelog.router, prefix="/api")
    app.include_router(prompt_workshop.router, prefix="/api")
    app.include_router(background_tasks.router, prefix="/api")
    app.include_router(book_import.router, prefix="/api")
