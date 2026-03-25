from app.models.project import Project
from app.services.project_generation_defaults import resolve_project_generation_defaults


def test_should_resolve_project_generation_defaults_from_project():
    project = Project(
        title="测试项目",
        user_id="user-1",
        default_creative_mode="hook",
        default_story_focus="advance_plot",
        default_plot_stage="development",
        default_story_creation_brief=" 保持推进效率和强钩子。 ",
        default_quality_preset="plot_drive",
        default_quality_notes=" 优先把关键桥段写在台前。 ",
    )

    resolved = resolve_project_generation_defaults(project)

    assert resolved == {
        "creative_mode": "hook",
        "story_focus": "advance_plot",
        "plot_stage": "development",
        "story_creation_brief": "保持推进效率和强钩子。",
        "quality_preset": "plot_drive",
        "quality_notes": "优先把关键桥段写在台前。",
    }


def test_should_prefer_explicit_request_values_over_project_generation_defaults():
    project = Project(
        title="测试项目",
        user_id="user-1",
        default_creative_mode="hook",
        default_story_focus="advance_plot",
        default_plot_stage="development",
        default_story_creation_brief="默认摘要",
        default_quality_preset="immersive",
        default_quality_notes="默认补充说明",
    )

    resolved = resolve_project_generation_defaults(
        project,
        creative_mode="payoff",
        story_focus="foreshadow_payoff",
        plot_stage="ending",
        story_creation_brief=" 以回收伏笔为先。 ",
        quality_preset="clean_prose",
        quality_notes=" 优先压缩重复提醒。 ",
    )

    assert resolved == {
        "creative_mode": "payoff",
        "story_focus": "foreshadow_payoff",
        "plot_stage": "ending",
        "story_creation_brief": "以回收伏笔为先。",
        "quality_preset": "clean_prose",
        "quality_notes": "优先压缩重复提醒。",
    }


def test_should_fallback_to_project_defaults_when_request_values_are_blank():
    project = Project(
        title="测试项目",
        user_id="user-1",
        default_creative_mode="hook",
        default_story_focus="advance_plot",
        default_plot_stage="development",
        default_story_creation_brief="默认摘要",
        default_quality_preset="immersive",
        default_quality_notes="默认补充说明",
    )

    resolved = resolve_project_generation_defaults(
        project,
        creative_mode="   ",
        story_focus="",
        plot_stage="  ",
        story_creation_brief="\n",
        quality_preset=" ",
        quality_notes="  ",
    )

    assert resolved == {
        "creative_mode": "hook",
        "story_focus": "advance_plot",
        "plot_stage": "development",
        "story_creation_brief": "默认摘要",
        "quality_preset": "immersive",
        "quality_notes": "默认补充说明",
    }