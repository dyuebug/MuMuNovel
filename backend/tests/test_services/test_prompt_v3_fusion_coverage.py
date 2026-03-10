import ast
import re
from pathlib import Path


ROOT_DIR = Path(__file__).resolve().parents[2]
PROMPT_SERVICE_PATH = ROOT_DIR / "app/services/prompt_service.py"
SYNC_RULES_PATH = ROOT_DIR / "app/services/prompt_template_sync_service.py"
V3_TAG = "rule_v3_fusion_20260303"


def _extract_template_block(source: str, template_key: str) -> str:
    module = ast.parse(source)
    namespace: dict[str, str] = {}

    for node in module.body:
        if isinstance(node, ast.Assign):
            for target in node.targets:
                if isinstance(target, ast.Name) and target.id in {
                    "CREATIVE_STORY_ENGINE_GUIDE",
                    "CREATIVE_LOW_AI_GUARD",
                }:
                    exec(  # noqa: S102 - test-only evaluation of constant assignments
                        compile(ast.Module(body=[node], type_ignores=[]), str(PROMPT_SERVICE_PATH), "exec"),
                        namespace,
                        namespace,
                    )
        if isinstance(node, ast.ClassDef) and node.name == "PromptService":
            for item in node.body:
                if isinstance(item, ast.Assign):
                    for target in item.targets:
                        if isinstance(target, ast.Name) and target.id == template_key:
                            return eval(  # noqa: S307 - test-only evaluation of template constants
                                compile(ast.Expression(body=item.value), str(PROMPT_SERVICE_PATH), "eval"),
                                namespace,
                                namespace,
                            )
    raise AssertionError(f"模板未找到: {template_key}")


def _extract_sync_rule_keys(source: str) -> set[str]:
    return set(re.findall(r'"([A-Z0-9_]+)"\s*:\s*TemplateSyncRule', source))


def test_should_keep_v3_tag_in_key_templates():
    source = PROMPT_SERVICE_PATH.read_text(encoding="utf-8-sig")
    required_templates = [
        "OUTLINE_CREATE",
        "OUTLINE_CONTINUE",
        "CHAPTER_GENERATION_ONE_TO_MANY",
        "CHAPTER_GENERATION_ONE_TO_MANY_NEXT",
        "CHAPTER_GENERATION_ONE_TO_ONE",
        "CHAPTER_GENERATION_ONE_TO_ONE_NEXT",
        "PARTIAL_REGENERATE",
        "INSPIRATION_TITLE_SYSTEM",
        "INSPIRATION_DESCRIPTION_SYSTEM",
        "INSPIRATION_THEME_SYSTEM",
        "INSPIRATION_GENRE_SYSTEM",
        "INSPIRATION_QUICK_COMPLETE",
        "AI_DENOISING",
        "PLOT_ANALYSIS",
        "OUTLINE_EXPAND_SINGLE",
        "OUTLINE_EXPAND_MULTI",
        "AUTO_CHARACTER_ANALYSIS",
        "AUTO_ORGANIZATION_ANALYSIS",
    ]

    for template_key in required_templates:
        block = _extract_template_block(source, template_key)
        assert V3_TAG in block, f"{template_key} 缺少第三版追踪标签"


def test_should_cover_v3_templates_in_sync_rules():
    source = SYNC_RULES_PATH.read_text(encoding="utf-8")
    sync_keys = _extract_sync_rule_keys(source)

    required_sync_keys = {
        "AI_DENOISING",
        "OUTLINE_CREATE",
        "OUTLINE_CONTINUE",
        "CHAPTER_GENERATION_ONE_TO_MANY",
        "CHAPTER_GENERATION_ONE_TO_MANY_NEXT",
        "CHAPTER_GENERATION_ONE_TO_ONE",
        "CHAPTER_GENERATION_ONE_TO_ONE_NEXT",
        "PARTIAL_REGENERATE",
        "PLOT_ANALYSIS",
        "OUTLINE_EXPAND_SINGLE",
        "OUTLINE_EXPAND_MULTI",
        "AUTO_CHARACTER_ANALYSIS",
        "AUTO_ORGANIZATION_ANALYSIS",
        "INSPIRATION_TITLE_SYSTEM",
        "INSPIRATION_DESCRIPTION_SYSTEM",
        "INSPIRATION_THEME_SYSTEM",
        "INSPIRATION_GENRE_SYSTEM",
        "INSPIRATION_QUICK_COMPLETE",
    }

    missing = sorted(required_sync_keys - sync_keys)
    assert not missing, f"同步规则缺少第三版关键模板: {missing}"


def test_should_keep_anti_ai_guards_in_chapter_templates_and_checker():
    source = PROMPT_SERVICE_PATH.read_text(encoding="utf-8-sig")

    chapter_block = _extract_template_block(source, "CHAPTER_GENERATION_ONE_TO_ONE")
    checker_block = _extract_template_block(source, "CHAPTER_TEXT_CHECKER")

    assert "比喻要省着用" in chapter_block
    assert "句法去模板" in chapter_block
    assert "高密度重复“像……/仿佛/像……一样”比喻" in chapter_block
    assert "模板腔与AI味" in checker_block
    assert "模板腔AI味" in checker_block
