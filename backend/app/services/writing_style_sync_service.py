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
        "description": "低AI感的生活化网文叙事，强调真人口语、可视化动作和番茄式冲突切入",
        "prompt_content": """写作风格建议：
1. 叙述像真人在讲亲历故事，口语自然，不要写成说明文或提纲摘要
2. 开场尽量先落动作、异常或眼前麻烦，少平铺背景，前几句就让读者知道“出了什么事”
3. 句式长短交替，快节奏处用短句，情绪段落允许稍慢一点，但别整段一个节拍
4. 每个场景尽量给一个可视化抓手：动作、物件、身体反应、环境细节，少空泛形容词
5. 对话允许停顿、打断、半句收口和话里有话，贴近中国人真实聊天节奏
6. 控制修辞密度，每段最多一个明显比喻，别连环堆意象，也别把每句都写成“金句”
7. 去掉模板化总结和口号腔，少用“总之/事实上/值得注意的是/在这个过程中”
8. 保留少量不完美表达和情绪毛边，让人物声音有区分度；热梗只能偶发点缀且必须贴场景""",
        "order_index": 7,
    },
    "low_ai_serial": {
        "name": "低AI连载感",
        "description": "低AI感的番茄连载风格，强调快开场、小爽点、章尾钩子与顺滑追更感",
        "prompt_content": """写作风格建议：
1. 叙述要有“当下正在发生”的现场感，先写动作和反应，再补解释，开篇尽量在150-300字内进入冲突或异常变化
2. 单章尽量形成“开场钩子→对抗推进→小爆发→章尾牵引”的四拍节奏，至少落地其中三拍
3. 每章最好有一个可感知的小爽点（反转/收获/打脸/突破其一），并写出“铺垫→爆发→反馈”
4. 章尾留自然钩子：优先使用信息缺口、危险临门、身份反转、选择未决等类型，并注意与上一章错开
5. 人物说话要分声线：词汇习惯、语气强弱、停顿方式都要有差别，别让所有角色像一个人
6. 情绪别一步到位，至少走出“触发→压住/回避→外露”中的两个阶段；配角也要有自己的立场和选择
7. 冲突一定要有代价：角色每次选择都尽量带来损失、新麻烦或关系变化，避免一路顺风
8. 术语或设定密集时，用角色追问、吐槽或现场反应补一句人话解释，降低阅读门槛；避免结尾用鸡汤总结句""",
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
