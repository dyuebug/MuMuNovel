"""AI去味 API - 核心特色功能。"""

from typing import Any

from fastapi import APIRouter, Body, Depends, HTTPException, Request
from sqlalchemy.ext.asyncio import AsyncSession

from app.api.settings import get_user_ai_service
from app.database import get_db
from app.logger import get_logger
from app.models.generation_history import GenerationHistory
from app.schemas.polish import PolishBatchRequest, PolishRequest, PolishResponse
from app.services.ai_service import AIService
from app.services.prompt_service import PromptService

router = APIRouter(prefix="/polish", tags=["AI去味"])
logger = get_logger(__name__)


POLISH_FOCUS_INSTRUCTIONS = {
    "balanced": "- 平衡处理叙事、对话、情绪和节奏，整体降低模板腔。",
    "dialogue": "- 优先处理人物对白，让说话方式更像真人，保住角色区分度。",
    "pacing": "- 优先处理叙事节奏，减少拖沓解释，强化场面推进和段落落点。",
    "emotion": "- 优先处理情绪表达，让反应更具体，少空泛感慨和统一抒情。",
    "hook": "- 优先处理开场与结尾牵引，保住追读钩子和信息差。",
}


def _build_polish_runtime_blocks(
    *,
    style: str | None,
    focus_mode: str,
    preserve_paragraphs: bool,
    retain_hooks: bool,
) -> dict[str, str]:
    focus_instruction = POLISH_FOCUS_INSTRUCTIONS.get(
        focus_mode,
        POLISH_FOCUS_INSTRUCTIONS["balanced"],
    )

    structure_lines = [
        "- 尽量保留原文的情节顺序和信息密度，不要重写成另一种故事。",
        (
            "- 保留原段落边界和段间呼吸感，除非原文断段明显影响阅读。"
            if preserve_paragraphs
            else "- 允许按节奏重新切分段落，但不要打散原有事件顺序。"
        ),
        (
            "- 保留段尾和章尾的悬念、动作牵引或情绪悬置，不要抹平成总结句。"
            if retain_hooks
            else "- 可以适度重写尾句，但仍要保住阅读牵引力。"
        ),
    ]

    style_hint = (style or "").strip()
    style_hint_block = (
        f"【额外风格偏好】\n- {style_hint}"
        if style_hint
        else "【额外风格偏好】\n- 无额外补充，按自然中文网文表达处理。"
    )

    return {
        "focus_instruction": focus_instruction,
        "structure_instruction": "\n".join(structure_lines),
        "style_hint_block": style_hint_block,
    }


@router.post("", response_model=PolishResponse, summary="AI去味")
async def polish_text(
    request: PolishRequest,
    http_request: Request,
    db: AsyncSession = Depends(get_db),
    user_ai_service: AIService = Depends(get_user_ai_service),
):
    """将 AI 生成文本润色得更像真人创作。"""

    try:
        user_id = getattr(http_request.state, "user_id", None)
        runtime_blocks = _build_polish_runtime_blocks(
            style=request.style,
            focus_mode=request.focus_mode,
            preserve_paragraphs=request.preserve_paragraphs,
            retain_hooks=request.retain_hooks,
        )

        template = await PromptService.get_template("AI_DENOISING", user_id, db)
        prompt = PromptService.format_prompt(
            template,
            original_text=request.original_text,
            **runtime_blocks,
        )

        logger.info(f"开始AI去味处理，原文长度: {len(request.original_text)}")

        polished_text = await user_ai_service.generate_text(
            prompt=prompt,
            provider=request.provider,
            model=request.model,
            temperature=request.temperature,
            max_tokens=len(request.original_text) * 2,
        )

        word_count_before = len(request.original_text)
        word_count_after = len(polished_text)
        logger.info(f"AI去味完成，处理后长度: {word_count_after}")

        if request.project_id:
            history = GenerationHistory(
                project_id=request.project_id,
                generation_type="polish",
                prompt=f"原文: {request.original_text[:100]}...",
                result=polished_text,
                provider=request.provider or "default",
                model=request.model or "default",
            )
            db.add(history)
            await db.commit()

        return PolishResponse(
            original_text=request.original_text,
            polished_text=polished_text,
            word_count_before=word_count_before,
            word_count_after=word_count_after,
        )
    except Exception as e:
        logger.error(f"AI去味失败: {str(e)}")
        raise HTTPException(status_code=500, detail=f"AI去味失败: {str(e)}")


@router.post("/batch", summary="批量AI去味")
async def polish_batch(
    request_body: Any = Body(...),
    http_request: Request = None,
    db: AsyncSession = Depends(get_db),
    user_ai_service: AIService = Depends(get_user_ai_service),
):
    """批量处理多个文本的 AI 去味。"""

    try:
        user_id = getattr(http_request.state, "user_id", None) if http_request else None
        batch_request = (
            PolishBatchRequest.model_validate(request_body)
            if isinstance(request_body, dict)
            else PolishBatchRequest(texts=request_body)
        )

        template = await PromptService.get_template("AI_DENOISING", user_id, db)
        runtime_blocks = _build_polish_runtime_blocks(
            style=batch_request.style,
            focus_mode=batch_request.focus_mode,
            preserve_paragraphs=batch_request.preserve_paragraphs,
            retain_hooks=batch_request.retain_hooks,
        )

        results = []
        for idx, text in enumerate(batch_request.texts):
            logger.info(f"处理第{idx + 1}/{len(batch_request.texts)} 个文本")
            prompt = PromptService.format_prompt(
                template,
                original_text=text,
                **runtime_blocks,
            )

            polished_text = await user_ai_service.generate_text(
                prompt=prompt,
                provider=batch_request.provider,
                model=batch_request.model,
                temperature=batch_request.temperature,
                max_tokens=len(text) * 2,
            )

            results.append(
                {
                    "index": idx,
                    "original": text,
                    "polished": polished_text,
                    "word_count_before": len(text),
                    "word_count_after": len(polished_text),
                }
            )

        logger.info(f"批量AI去味完成，共处理 {len(results)} 个文本")
        return {"total": len(results), "results": results}
    except Exception as e:
        logger.error(f"批量AI去味失败: {str(e)}")
        raise HTTPException(status_code=500, detail=f"批量AI去味失败: {str(e)}")
