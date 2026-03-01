"""写作风格预设同步服务。

用途：
1. 兼容未及时执行 Alembic 迁移的存量数据库。
2. 统一修正 low_ai 预设文案，避免“代码改了但数据库没变”。
"""
from __future__ import annotations

import asyncio
from typing import Dict, Any

from sqlalchemy import select, func
from sqlalchemy.ext.asyncio import AsyncSession

from app.models.writing_style import WritingStyle
from app.logger import get_logger

logger = get_logger(__name__)


LOW_AI_PRESET_DEFINITIONS: Dict[str, Dict[str, Any]] = {
    "low_ai_life": {
        "name": "低AI生活化",
        "description": "低AI感的生活化叙事，强调口语自然、节奏起伏与去工整感",
        "prompt_content": """写作风格建议：
1. 叙述像真人在讲亲历故事，口语自然，不要写成说明文
2. 句式长短交替，快节奏处用短句，情绪段落允许稍慢一点
3. 控制修辞密度，每段最多一个明显比喻，别连环堆意象
4. 别把每句话都写成“金句”，保留普通过渡句和生活化连接词
5. 对话允许停顿、打断和半句收口，贴近中国人真实聊天节奏
6. 去掉模板化总结和口号腔，少用“总之/事实上/值得注意的是”
7. 保留少量不完美表达和情绪毛边，让人物声音有区分度
8. 网络热梗仅可偶发点缀，优先给配角使用，必须贴场景且不过量""",
        "order_index": 7,
    },
    "low_ai_serial": {
        "name": "低AI连载感",
        "description": "低AI感的网文连载风格，强调现场感、自然口语和非工整节奏",
        "prompt_content": """写作风格建议：
1. 叙述要有“现场正在发生”的感觉，少解释，多让动作和反应自己说话
2. 句式长短交替，关键推进处用短句，情绪过渡用中句，别整段同长度
3. 段落允许有轻微粗粝感，不追求句句漂亮，优先保证可读和代入
4. 别把每句话都打磨成金句，保留自然的过渡句和口头连接词
5. 对话贴近日常中文，允许停顿、打断、欲言又止，角色语气要分开
6. 修辞要克制，每段最多一个明显比喻，避免连续堆意象
7. 热梗仅可偶发点缀，优先放在配角台词，必须贴场景且不过量
8. 章末可留轻钩子，但不要生硬反转，保持“下一章想看”的顺滑感""",
        "order_index": 8,
    },
}


_sync_lock = asyncio.Lock()
_sync_done = False


async def sync_low_ai_presets(db: AsyncSession, force: bool = False) -> bool:
    """同步 low_ai 预设到最新定义。

    返回：
    - True: 本次有数据库写入
    - False: 无变更
    """
    global _sync_done
    if _sync_done and not force:
        return False

    async with _sync_lock:
        if _sync_done and not force:
            return False

        preset_ids = tuple(LOW_AI_PRESET_DEFINITIONS.keys())
        result = await db.execute(
            select(WritingStyle).where(WritingStyle.preset_id.in_(preset_ids))
        )
        styles = list(result.scalars().all())

        modified = False
        has_global = {preset_id: False for preset_id in preset_ids}

        for style in styles:
            definition = LOW_AI_PRESET_DEFINITIONS.get(style.preset_id)
            if not definition:
                continue

            if style.user_id is None:
                has_global[style.preset_id] = True

            if style.name != definition["name"]:
                style.name = definition["name"]
                modified = True
            if style.style_type != "preset":
                style.style_type = "preset"
                modified = True
            if style.description != definition["description"]:
                style.description = definition["description"]
                modified = True
            if style.prompt_content != definition["prompt_content"]:
                style.prompt_content = definition["prompt_content"]
                modified = True

            # 只固定全局预设的顺序，避免打乱用户自定义排序
            if style.user_id is None and style.order_index != definition["order_index"]:
                style.order_index = definition["order_index"]
                modified = True

        if not all(has_global.values()):
            count_result = await db.execute(
                select(func.coalesce(func.max(WritingStyle.order_index), 0)).where(
                    WritingStyle.user_id.is_(None)
                )
            )
            max_order = int(count_result.scalar_one() or 0)

            for preset_id, definition in LOW_AI_PRESET_DEFINITIONS.items():
                if has_global[preset_id]:
                    continue

                db.add(
                    WritingStyle(
                        user_id=None,
                        name=definition["name"],
                        style_type="preset",
                        preset_id=preset_id,
                        description=definition["description"],
                        prompt_content=definition["prompt_content"],
                        order_index=max(definition["order_index"], max_order + 1),
                    )
                )
                max_order += 1
                modified = True

        if modified:
            await db.commit()
            logger.info("✅ 已同步 low_ai 预设到最新文案定义")

        _sync_done = True
        return modified
