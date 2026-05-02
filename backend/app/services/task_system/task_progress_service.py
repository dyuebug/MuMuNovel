from __future__ import annotations

from typing import Mapping, Optional


PROGRESS_PHASE_ORDER: dict[str, int] = {
    "init": 0,
    "loading": 1,
    "preparing": 2,
    "generating": 3,
    "parsing": 4,
    "saving": 5,
    "complete": 6,
}

TASK_STAGE_ROOTS: dict[str, str] = {
    "wizard_world_building": "0.creative",
    "wizard_characters": "1.outline",
    "wizard_outline": "1.outline",
    "wizard_career_system": "1.outline",
    "world_regenerate": "0.creative",
    "outline_generate": "1.outline",
    "outline_expand": "4.group",
    "outline_batch_expand": "4.group",
    "careers_generate_system": "1.outline",
    "character_generate": "1.outline",
    "organization_generate": "1.outline",
    "chapters_batch_generate": "6.writing",
    "chapter_single_generate": "6.writing",
}

PHASE_KEYWORDS: dict[str, tuple[str, ...]] = {
    "init": ("开始", "启动", "初始化", "start", "init"),
    "loading": ("加载", "读取", "获取", "检索", "loading", "load", "fetch"),
    "preparing": ("准备", "预处理", "提示词", "prompt", "prepare", "preparing"),
    "generating": ("生成", "创作", "推理", "草稿", "rewrite", "generate", "generating"),
    "parsing": ("解析", "校验", "提取", "parsing", "parse", "validate"),
    "saving": ("保存", "写入", "入库", "提交", "持久化", "saving", "save", "persist"),
    "complete": ("完成", "结束", "done", "complete", "success"),
}


def split_stage_code(
    stage_code: Optional[str],
    *,
    phase_order: Mapping[str, int] = PROGRESS_PHASE_ORDER,
) -> tuple[Optional[str], Optional[str]]:
    raw = (stage_code or "").strip()
    if not raw:
        return None, None
    base, sep, suffix = raw.rpartition(".")
    if sep and suffix in phase_order:
        return base, suffix
    return raw, None


def contains_retry_hint(message: Optional[str]) -> bool:
    if not message:
        return False
    text = message.lower()
    return "重试" in text or "retry" in text


def detect_phase_by_message(
    message: Optional[str],
    *,
    phase_keywords: Mapping[str, tuple[str, ...]] = PHASE_KEYWORDS,
) -> Optional[str]:
    if not message:
        return None
    text = message.strip().lower()
    if not text:
        return None

    for phase in (
        "complete",
        "saving",
        "parsing",
        "generating",
        "preparing",
        "loading",
        "init",
    ):
        if any(keyword in text for keyword in phase_keywords[phase]):
            return phase
    return None


def detect_phase_by_progress(progress: Optional[int]) -> Optional[str]:
    if progress is None:
        return None
    normalized = max(0, min(int(progress), 100))
    if normalized >= 100:
        return "complete"
    if normalized >= 93:
        return "saving"
    if normalized >= 86:
        return "parsing"
    if normalized >= 21:
        return "generating"
    if normalized >= 16:
        return "preparing"
    if normalized >= 6:
        return "loading"
    return "init"


def resolve_progress_phase(
    *,
    message: Optional[str],
    progress: Optional[int],
    stage_code: Optional[str],
    phase_order: Mapping[str, int] = PROGRESS_PHASE_ORDER,
) -> Optional[str]:
    detected = detect_phase_by_message(message) or detect_phase_by_progress(progress)
    if not detected:
        return None

    _, current_phase = split_stage_code(stage_code, phase_order=phase_order)
    if not current_phase:
        return detected

    if (
        phase_order.get(detected, -1) < phase_order.get(current_phase, -1)
        and not contains_retry_hint(message)
    ):
        return current_phase
    return detected


def resolve_stage_code_for_phase(
    *,
    task_type: str,
    stage_code: Optional[str],
    phase: Optional[str],
    stage_roots: Mapping[str, str] = TASK_STAGE_ROOTS,
    phase_order: Mapping[str, int] = PROGRESS_PHASE_ORDER,
) -> Optional[str]:
    base, _ = split_stage_code(stage_code, phase_order=phase_order)
    if not base:
        base = stage_roots.get(task_type)
    if not base:
        return stage_code
    if not phase or phase == "init":
        return base
    return f"{base}.{phase}"


def infer_workflow_phase(
    *,
    event_type: str,
    progress: Optional[int],
    message: Optional[str],
) -> Optional[str]:
    normalized_event = (event_type or "").strip().lower()
    text = (message or "").strip().lower()

    if normalized_event == "error":
        return "failed"
    if normalized_event == "done":
        return "complete"
    if normalized_event in {"chunk", "chapter_start"}:
        return "generating"
    if normalized_event == "analysis_started":
        return "parsing"

    if "取消" in text or "cancel" in text:
        return "cancelled"
    if "完成" in text or "complete" in text or "done" in text:
        return "complete"
    if "保存" in text or "save" in text:
        return "saving"
    if "分析" in text or "analysis" in text or "解析" in text or "parse" in text:
        return "parsing"
    if "重试" in text or "retry" in text:
        return "generating"
    if "生成" in text or "写作" in text or "generate" in text:
        return "generating"
    if "准备" in text or "prepare" in text:
        return "preparing"
    if "加载" in text or "load" in text:
        return "loading"

    if progress is None:
        return None
    if progress >= 100:
        return "complete"
    if progress >= 93:
        return "saving"
    if progress >= 85:
        return "parsing"
    if progress >= 20:
        return "generating"
    if progress >= 10:
        return "preparing"
    if progress > 0:
        return "loading"
    return "init"
