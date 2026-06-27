from __future__ import annotations

import json
from typing import AsyncGenerator

from sqlalchemy import select
from sqlalchemy.ext.asyncio import AsyncSession

from tests.test_support.retired_runtime_test_support import get_logger
from migrator_app.models import Career
from migrator_app.models.project import Project
from tests.test_support.ai_gateway.ai_service import AIService
from tests.test_support.utils.sse_response import SSEResponse, WizardProgressTracker

logger = get_logger(__name__)


def _build_generation_request_options(ai_service: AIService) -> dict[str, object]:
    retry_cfg = getattr(getattr(ai_service, "config", None), "retry", None)
    configured_retry_budget = int(getattr(retry_cfg, "max_retries", 2) or 2)
    provider = str(getattr(ai_service, "api_provider", "") or "").strip().lower()
    request_options: dict[str, object] = {
        "transport_max_retries": max(1, min(configured_retry_budget, 2)),
    }
    if provider in {"sub2api", "openai_responses"}:
        request_options.update(
            {
                "prefer_chat_completions": True,
                "prefer_normalized_v1_candidate": True,
                "first_chunk_timeout": 20.0,
                "allow_non_stream_fallback": False,
            }
        )
    return request_options


async def create_career_system_stream(
    *,
    project_id: str,
    main_career_count: int,
    sub_career_count: int,
    enable_mcp: bool,
    db: AsyncSession,
    user_ai_service: AIService,
) -> AsyncGenerator[str, None]:
    """Generate and persist career-system additions as a test-only SSE seam."""
    _ = enable_mcp
    tracker = WizardProgressTracker("职业体系")

    try:
        yield await tracker.start()

        yield await tracker.loading("分析已有职业...", 0.3)
        existing_careers_result = await db.execute(
            select(Career).where(Career.project_id == project_id)
        )
        existing_careers = existing_careers_result.scalars().all()

        existing_main_careers = []
        existing_sub_careers = []
        for career in existing_careers:
            career_summary = f"- {career.name}（{career.category or '未分类'}，{career.max_stage}阶）"
            if career.description:
                career_summary += f": {career.description[:50]}"

            if career.type == "main":
                existing_main_careers.append(career_summary)
            else:
                existing_sub_careers.append(career_summary)

        existing_careers_text = ""
        if existing_main_careers:
            existing_careers_text += (
                f"\n已有主职业（{len(existing_main_careers)}个）：\n"
                + "\n".join(existing_main_careers)
            )
        if existing_sub_careers:
            existing_careers_text += (
                f"\n\n已有副职业（{len(existing_sub_careers)}个）：\n"
                + "\n".join(existing_sub_careers)
            )

        if not existing_careers_text:
            existing_careers_text = "\n当前还没有任何职业，这是第一次创建职业体系。"

        yield await tracker.loading("分析项目世界观...", 0.6)
        project_result = await db.execute(
            select(Project).where(Project.id == project_id)
        )
        project = project_result.scalar_one_or_none()
        if project is None:
            yield await tracker.error("项目不存在", 404)
            return

        project_context = f"""
项目信息：
- 书名：{project.title}
- 类型：{project.genre or '未设定'}
- 主题：{project.theme or '未设定'}
- 时间背景：{project.world_time_period or '未设定'}
- 地理位置：{project.world_location or '未设定'}
- 氛围基调：{project.world_atmosphere or '未设定'}
- 世界规则：{project.world_rules or '未设定'}
"""

        user_requirements = f"""
已有职业情况：{existing_careers_text}

生成要求（增量式）：
- 本次新增主职业：{main_career_count}个
- 本次新增副职业：{sub_career_count}个
- 重要：请生成与已有职业不重复的新职业，形成互补体系
- 新职业应填补已有职业体系的空缺，丰富职业多样性
- 主职业必须严格符合世界观规则，体现核心能力体系
- 副职业可以更加自由灵活，包含生产、辅助、特殊类型
"""

        yield await tracker.preparing("构建AI提示词...")
        prompt = f"""{project_context}

{user_requirements}

请为这个小说项目生成新的补充职业（增量式）。要求：
1. 仔细分析已有职业，避免生成重复或相似的职业
2. 填补职业体系的空缺，让职业体系更加完善和多样化
3. 如果已有职业较少，可以生成核心基础职业
4. 如果已有职业较多，可以生成特色化、专精化的职业

返回JSON格式，结构如下：

{{
  "main_careers": [
    {{
      "name": "职业名称",
      "description": "职业描述",
      "category": "职业分类（如：战斗系、法术系等）",
      "stages": [
        {{"level": 1, "name": "阶段名称", "description": "阶段描述"}},
        {{"level": 2, "name": "阶段名称", "description": "阶段描述"}}
      ],
      "max_stage": 10,
      "requirements": "职业要求",
      "special_abilities": "特殊能力",
      "worldview_rules": "世界观规则关联",
      "attribute_bonuses": {{"strength": "+10%", "intelligence": "+5%"}}
    }}
  ],
  "sub_careers": [
    {{
      "name": "副职业名称",
      "description": "职业描述",
      "category": "生产系/辅助系/特殊系",
      "stages": [],
      "max_stage": 5,
      "requirements": "职业要求",
      "special_abilities": "特殊能力"
    }}
  ]
}}

注意事项：
1. 避免重复：生成的职业名称和定位不能与已有职业重复
2. 互补性：新职业应与已有职业形成互补，丰富职业体系
3. 主职业的阶段设定要详细，体现明确的成长路径
4. 阶段名称要符合世界观特色
5. 副职业可以相对简化，但要有独特性
6. 所有职业都要符合项目的整体世界观设定
7. 只返回纯JSON，不要添加任何解释文字
"""

        estimated_total = max(3000, len(prompt) * 8)
        yield await tracker.generating(0, estimated_total, "调用AI生成新职业...")
        logger.info(
            "开始为项目 %s 生成新职业（增量式，已有%s个职业）",
            project_id,
            len(existing_careers),
        )

        try:
            ai_response = ""
            chunk_count = 0
            request_options = _build_generation_request_options(user_ai_service)
            async for chunk in user_ai_service.generate_text_stream(
                prompt=prompt,
                request_options=request_options,
            ):
                chunk_count += 1
                ai_response += chunk
                yield await SSEResponse.send_chunk(chunk)
                if chunk_count % 10 == 0:
                    yield await tracker.generating(len(ai_response), estimated_total)
                if chunk_count % 20 == 0:
                    yield await tracker.heartbeat()
        except Exception as ai_error:
            logger.error("AI服务调用异常：%s", ai_error)
            yield await tracker.error(f"AI服务调用失败：{str(ai_error)}")
            return

        if not ai_response or not ai_response.strip():
            yield await tracker.error("AI服务返回空响应")
            return

        yield await tracker.parsing("解析AI响应...", 0.5)
        try:
            cleaned_response = user_ai_service._clean_json_response(ai_response)
            career_data = json.loads(cleaned_response)
            logger.info("职业体系JSON解析成功")
        except json.JSONDecodeError as error:
            logger.error("职业体系JSON解析失败: %s", error)
            logger.error("原始响应预览: %s", ai_response[:200])
            yield await tracker.error(f"AI返回的内容无法解析为JSON：{str(error)}")
            return

        yield await tracker.saving("保存主职业到数据库...", 0.3)
        main_careers_created = []
        for idx, career_info in enumerate(career_data.get("main_careers", [])):
            try:
                stages_json = json.dumps(career_info.get("stages", []), ensure_ascii=False)
                attribute_bonuses = career_info.get("attribute_bonuses")
                attribute_bonuses_json = (
                    json.dumps(attribute_bonuses, ensure_ascii=False)
                    if attribute_bonuses
                    else None
                )

                career = Career(
                    project_id=project_id,
                    name=career_info.get("name", f"未命名主职业{idx + 1}"),
                    type="main",
                    description=career_info.get("description"),
                    category=career_info.get("category"),
                    stages=stages_json,
                    max_stage=career_info.get("max_stage", 10),
                    requirements=career_info.get("requirements"),
                    special_abilities=career_info.get("special_abilities"),
                    worldview_rules=career_info.get("worldview_rules"),
                    attribute_bonuses=attribute_bonuses_json,
                    source="ai",
                )
                db.add(career)
                await db.flush()
                main_careers_created.append(career.name)
                logger.info("创建主职业成功：%s", career.name)
            except Exception as error:
                logger.error("创建主职业失败：%s", error)
                continue

        yield await tracker.saving("保存副职业到数据库...", 0.6)
        sub_careers_created = []
        for idx, career_info in enumerate(career_data.get("sub_careers", [])):
            try:
                stages_json = json.dumps(career_info.get("stages", []), ensure_ascii=False)
                attribute_bonuses = career_info.get("attribute_bonuses")
                attribute_bonuses_json = (
                    json.dumps(attribute_bonuses, ensure_ascii=False)
                    if attribute_bonuses
                    else None
                )

                career = Career(
                    project_id=project_id,
                    name=career_info.get("name", f"未命名副职业{idx + 1}"),
                    type="sub",
                    description=career_info.get("description"),
                    category=career_info.get("category"),
                    stages=stages_json,
                    max_stage=career_info.get("max_stage", 5),
                    requirements=career_info.get("requirements"),
                    special_abilities=career_info.get("special_abilities"),
                    worldview_rules=career_info.get("worldview_rules"),
                    attribute_bonuses=attribute_bonuses_json,
                    source="ai",
                )
                db.add(career)
                await db.flush()
                sub_careers_created.append(career.name)
                logger.info("创建副职业成功：%s", career.name)
            except Exception as error:
                logger.error("创建副职业失败：%s", error)
                continue

        await db.commit()

        total_main = len(existing_main_careers) + len(main_careers_created)
        total_sub = len(existing_sub_careers) + len(sub_careers_created)

        logger.info(
            "新职业生成完成：新增主职业%s个，新增副职业%s个；体系总数主职业%s个，副职业%s个",
            len(main_careers_created),
            len(sub_careers_created),
            total_main,
            total_sub,
        )

        yield await tracker.complete(f"新职业生成完成！（主职业{total_main}个，副职业{total_sub}个）")
        yield await tracker.result(
            {
                "main_careers_count": len(main_careers_created),
                "sub_careers_count": len(sub_careers_created),
                "main_careers": main_careers_created,
                "sub_careers": sub_careers_created,
            }
        )
        yield await tracker.done()

    except Exception as error:
        logger.error("生成职业体系失败: %s", error)
        yield await tracker.error(f"生成新职业失败: {str(error)}")




