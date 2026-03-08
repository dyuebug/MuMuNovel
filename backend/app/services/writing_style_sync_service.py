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
        "description": "低AI感的生活化网文叙事，强调日常现场里的眼前麻烦、真人对白、动作反馈与带余波的柔和章尾",
        "prompt_content": """写作风格建议：
1. 开场可以更贴近日常现场，但前段必须让读者看见眼前麻烦、情绪摩擦、秘密失衡或局面变化，少用背景概述起手
2. 叙述像真人在讲亲历故事，优先写正在发生的动作、人物反应和场面变化，再补必要解释，不要写成说明文
3. 日常戏也要让“动作/试探→反馈→余波或代价”可见，哪怕没有大冲突，也要写出关系变化、情绪波动或下一步压力
4. 对话要有停顿、改口、打断、潜台词和角色声线差异，允许少量口语毛边，但不要写成轮流讲道理
5. 句式长短交替，保留生活噪声与嘴感；别整段一个节拍，也别把每句都打磨成金句、口号或过分工整的排比
6. 每个场景尽量给一个可视化抓手：动作、物件、身体反应、环境细节，少用空泛形容词堆情绪；情绪推进优先靠细节与反应，而不是作者替人物总结
7. 遇到设定、术语或背景信息时，在几句内用角色追问、吐槽或现场反应补一句人话解释，不要整段灌说明，也不要写成讲义
8. 章尾可以更柔和，但至少留下情绪余震、关系余波、秘密悬挂或下一步动作牵引，避免鸡汤总结、预告腔和模板化收束
9. 比喻要克制：同一自然段不要连着堆“像……/仿佛/像……一样”；能直接写动作、表情、声音和结果，就不要先做抽象比喻
10. 少用“下一秒/那一瞬/忽然/不是……而是……”这类固定推进句，允许出现朴素、直接、没那么漂亮的过渡句""",
        "order_index": 7,
    },
    "low_ai_serial": {
        "name": "低AI连载感",
        "description": "低AI感的番茄连载风格，强调快开场、目标阻力选择链、小爽点反馈与顺滑追更钩子",
        "prompt_content": """写作风格建议：
1. 开篇尽量在150-300字内落到异常、任务压力、关系摩擦、危险逼近或信息缺口，让读者迅速知道“这一章为什么要看”
2. 正文优先写当下正在发生的动作、人物反应和局面变化，再补必要解释，避免大段概述替代现场
3. 单章尽量形成“开场钩子→冲突推进→小爆发→章尾牵引”的节奏，至少让目标、阻力、选择和即时后果可见，别只报结果不写过程
4. 每章最好给一个可感知的小爽点或阶段回报，并写出“铺垫→爆发→反馈/余波”；哪怕不是打脸，也要让读者感到局面真的被推动
5. 句子可以更短、更口语、更有现场颗粒感，但对白仍要分角色声线，保留停顿、反问、改口和话里有话，不要所有人说成同一种腔调
6. 配角不必长篇输出，只要做出会改变局面的主动选择就算有效推进；关键推进尽量带出新麻烦、损失、筹码变化或关系变化
7. 遇到术语、设定或规则时，三句内补一句读者能听懂的人话解释，可借追问、吐槽或身体反馈带出，别写成讲义，也别让设定说明压过剧情
8. 章尾优先停在信息缺口、危险临门、身份反转或选择未决上，宁可留动作停顿，也别用总结腔、鸡汤句收束
9. 比喻要省着用：单段尽量只保留1个强比喻，别把疼痛、危险、异常都写成“像什么”；能直写动作后果就直写
10. 慎用“下一秒/那一瞬/忽然/不是……而是……”等模板句式，别让整章每段都像在卡点或凹质感""",
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
            select(WritingStyle).where(
                WritingStyle.preset_id.in_(preset_ids),
                (WritingStyle.user_id.is_(None)) | (WritingStyle.style_type == "preset"),
            )
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
