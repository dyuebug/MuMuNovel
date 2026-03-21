"""生成前网络检索服务。"""

from __future__ import annotations

import asyncio
import json
import sys
import uuid
from dataclasses import dataclass
from datetime import datetime
from pathlib import Path
from typing import Any, Dict, List, Mapping, Optional

from sqlalchemy import select
from sqlalchemy.ext.asyncio import AsyncSession

from app.config import PROJECT_ROOT, settings
from app.logger import get_logger
from app.models.chapter import Chapter
from app.models.memory import StoryMemory
from app.models.outline import Outline
from app.models.project import Project
from app.models.settings import Settings
from app.services.memory_service import memory_service

logger = get_logger(__name__)

WEB_RESEARCH_PREF_KEY = "web_research"
DEFAULT_GROK_MODEL = "grok-4.1-fast"


@dataclass(frozen=True)
class WebResearchRuntimeConfig:
    enabled: Optional[bool] = None
    exa_enabled: bool = True
    grok_enabled: bool = True
    exa_api_key: str = ""
    grok_api_key: str = ""
    grok_base_url: str = ""
    grok_model: str = DEFAULT_GROK_MODEL
    timeout_seconds: int = 90
    max_assets: int = 4


class ChapterWebResearchService:
    MEMORY_TYPE = "research_reference"
    WORLD_MEMORY_TYPE = "research_world_building"
    CAREERS_MEMORY_TYPE = "research_careers"
    CHARACTERS_MEMORY_TYPE = "research_characters"
    OUTLINE_MEMORY_TYPE = "research_outline"
    MAX_SUMMARY_CHARS = 360
    MAX_RAW_CHARS = 4000

    def _default_runtime_config(self) -> WebResearchRuntimeConfig:
        return WebResearchRuntimeConfig(
            enabled=bool(settings.pre_generation_web_research_enabled),
            exa_enabled=bool(settings.pre_generation_web_research_exa_enabled),
            grok_enabled=bool(settings.pre_generation_web_research_grok_enabled),
            grok_model=DEFAULT_GROK_MODEL,
            timeout_seconds=max(15, int(settings.pre_generation_web_research_timeout_seconds)),
            max_assets=max(1, int(settings.pre_generation_web_research_max_assets)),
        )

    def build_runtime_config(
        self,
        *,
        preferences: Optional[Mapping[str, Any]] = None,
        overrides: Optional[Mapping[str, Any]] = None,
    ) -> WebResearchRuntimeConfig:
        default = self._default_runtime_config()
        pref_payload = {}
        if isinstance(preferences, Mapping):
            value = preferences.get(WEB_RESEARCH_PREF_KEY)
            if isinstance(value, Mapping):
                pref_payload = dict(value)
        payload = {
            "enabled": pref_payload.get("enabled", default.enabled),
            "exa_enabled": pref_payload.get("exa_enabled", default.exa_enabled),
            "grok_enabled": pref_payload.get("grok_enabled", default.grok_enabled),
            "exa_api_key": str(pref_payload.get("exa_api_key") or "").strip(),
            "grok_api_key": str(pref_payload.get("grok_api_key") or "").strip(),
            "grok_base_url": str(pref_payload.get("grok_base_url") or "").strip(),
            "grok_model": str(pref_payload.get("grok_model") or DEFAULT_GROK_MODEL).strip() or DEFAULT_GROK_MODEL,
            "timeout_seconds": pref_payload.get("timeout_seconds", default.timeout_seconds),
            "max_assets": pref_payload.get("max_assets", default.max_assets),
        }
        if overrides:
            for key, value in overrides.items():
                if key in payload and value is not None:
                    payload[key] = value
        return WebResearchRuntimeConfig(
            enabled=None if payload["enabled"] is None else bool(payload["enabled"]),
            exa_enabled=bool(payload["exa_enabled"]),
            grok_enabled=bool(payload["grok_enabled"]),
            exa_api_key=str(payload["exa_api_key"] or "").strip(),
            grok_api_key=str(payload["grok_api_key"] or "").strip(),
            grok_base_url=str(payload["grok_base_url"] or "").strip(),
            grok_model=str(payload["grok_model"] or DEFAULT_GROK_MODEL).strip() or DEFAULT_GROK_MODEL,
            timeout_seconds=max(15, int(float(payload["timeout_seconds"] or default.timeout_seconds))),
            max_assets=max(1, int(payload["max_assets"] or default.max_assets)),
        )

    async def get_runtime_config(self, *, user_id: Optional[str], db_session: Optional[AsyncSession]) -> WebResearchRuntimeConfig:
        if not user_id or db_session is None:
            return self._default_runtime_config()
        result = await db_session.execute(select(Settings).where(Settings.user_id == user_id))
        user_settings = result.scalar_one_or_none()
        if not user_settings:
            return self._default_runtime_config()
        try:
            preferences = json.loads(user_settings.preferences or "{}")
            if not isinstance(preferences, dict):
                preferences = {}
        except json.JSONDecodeError:
            preferences = {}
        return self.build_runtime_config(preferences=preferences)

    def is_enabled(self, requested: Optional[bool], runtime_config: Optional[WebResearchRuntimeConfig] = None) -> bool:
        if requested is not None:
            return bool(requested)
        if runtime_config and runtime_config.enabled is not None:
            return bool(runtime_config.enabled)
        return bool(settings.pre_generation_web_research_enabled)

    def skills_root(self) -> Path:
        skill_root = Path(settings.pre_generation_web_research_skill_repo_path).expanduser()
        if not skill_root.is_absolute():
            skill_root = (PROJECT_ROOT / skill_root).resolve()
        return skill_root

    @staticmethod
    def _clean_text(value: Optional[str]) -> str:
        if not value:
            return ""
        return " ".join(str(value).replace("\r", " ").replace("\n", " ").split()).strip()

    @classmethod
    def _clip_text(cls, value: Optional[str], limit: int) -> str:
        text = cls._clean_text(value)
        return text if len(text) <= limit else text[: limit - 3].rstrip() + "..."

    def _chapter_exa_query(self, project: Project, chapter: Chapter, outline: Optional[Outline], story_creation_brief: Optional[str], query_override: Optional[str]) -> str:
        if query_override:
            return self._clip_text(query_override, 320)
        parts = [project.genre, project.theme, chapter.title, getattr(outline, 'title', None), getattr(outline, 'summary', None), getattr(outline, 'content', None), story_creation_brief]
        cleaned = [self._clip_text(item, 140) for item in parts if self._clean_text(item)]
        return self._clip_text(" | ".join(cleaned[:4]), 320) if cleaned else ""

    def _chapter_grok_query(self, project: Project, chapter: Chapter, outline: Optional[Outline], story_creation_brief: Optional[str], query_override: Optional[str]) -> str:
        if query_override:
            return f"请围绕以下小说创作主题进行实时网络研究，并给出来源：{self._clip_text(query_override, 260)}"
        context = "；".join(part for part in [
            f"项目类型：{self._clip_text(project.genre, 40)}" if self._clean_text(project.genre) else "",
            f"主题：{self._clip_text(project.theme, 50)}" if self._clean_text(project.theme) else "",
            f"章节标题：{self._clip_text(chapter.title, 60)}" if self._clean_text(chapter.title) else "",
            f"章节大纲：{self._clip_text(getattr(outline, 'content', None) or getattr(outline, 'summary', None), 180)}" if self._clean_text(getattr(outline, 'content', None) or getattr(outline, 'summary', None)) else "",
            f"创作总控摘要：{self._clip_text(story_creation_brief, 120)}" if self._clean_text(story_creation_brief) else "",
        ] if part)
        return f"请为小说章节创作做实时网络研究，优先提炼事实、职业细节、社会情绪与可借鉴表达，并保留来源。背景：{context}" if context else ""
    async def _run_skill_script(self, *, skill_dir_name: str, script_name: str, args: List[str], timeout_seconds: int) -> Dict[str, Any]:
        skill_root = self.skills_root() / skill_dir_name
        script_path = skill_root / "scripts" / script_name
        if not script_path.exists():
            return {"error": "script_not_found", "detail": f"脚本不存在: {script_path}"}

        process = await asyncio.create_subprocess_exec(
            sys.executable,
            str(script_path),
            *args,
            cwd=str(skill_root),
            stdout=asyncio.subprocess.PIPE,
            stderr=asyncio.subprocess.PIPE,
        )
        try:
            stdout, stderr = await asyncio.wait_for(process.communicate(), timeout=timeout_seconds)
        except asyncio.TimeoutError:
            process.kill()
            await process.communicate()
            return {"error": "timeout", "detail": f"{skill_dir_name} 调用超时"}

        stdout_text = (stdout or b"").decode("utf-8", errors="replace").strip()
        stderr_text = (stderr or b"").decode("utf-8", errors="replace").strip()
        if not stdout_text:
            return {"error": "empty_stdout", "detail": stderr_text or f"{skill_dir_name} 未返回结果"}
        try:
            payload = json.loads(stdout_text)
        except json.JSONDecodeError:
            payload = {"error": "invalid_json", "detail": self._clip_text(stdout_text, 1200)}
        if process.returncode != 0 and not payload.get("error"):
            payload["error"] = f"exit_code_{process.returncode}"
            payload["detail"] = stderr_text or payload.get("detail") or "技能脚本执行失败"
        elif stderr_text and not payload.get("detail"):
            payload["detail"] = self._clip_text(stderr_text, 600)
        return payload

    async def _run_exa_search(self, query: str, runtime_config: WebResearchRuntimeConfig) -> Dict[str, Any]:
        args = ["search", "--query", query, "--num", "3", "--text"]
        if runtime_config.exa_api_key:
            args.extend(["--api-key", runtime_config.exa_api_key])
        return await self._run_skill_script(
            skill_dir_name="exa-search",
            script_name="exa_search.py",
            args=args,
            timeout_seconds=runtime_config.timeout_seconds,
        )

    async def _run_grok_search(self, query: str, runtime_config: WebResearchRuntimeConfig) -> Dict[str, Any]:
        args = ["--mode", "research", "--query", query]
        if runtime_config.grok_api_key:
            args.extend(["--api-key", runtime_config.grok_api_key])
        if runtime_config.grok_base_url:
            args.extend(["--base-url", runtime_config.grok_base_url])
        if runtime_config.grok_model:
            args.extend(["--model", runtime_config.grok_model])
        return await self._run_skill_script(
            skill_dir_name="grok-search",
            script_name="grok_search.py",
            args=args,
            timeout_seconds=runtime_config.timeout_seconds,
        )

    def _build_exa_assets(self, payload: Dict[str, Any]) -> List[Dict[str, str]]:
        if not isinstance(payload, dict) or payload.get("error"):
            return []
        assets: List[Dict[str, str]] = []
        for item in (payload.get("results") or [])[:2]:
            title = self._clip_text(item.get("title") or item.get("url") or "Exa 参考资料", 120)
            source = self._clip_text(item.get("url") or "exa-search", 300)
            highlights = item.get("highlights") or []
            summary = self._clip_text(" ".join(str(text) for text in highlights[:3]) or item.get("text") or title, self.MAX_SUMMARY_CHARS)
            if not summary:
                continue
            raw_content = self._clip_text(item.get("text") or "\n".join(str(text) for text in highlights[:5]), self.MAX_RAW_CHARS)
            assets.append({
                "title": title,
                "source": source,
                "summary": summary,
                "usage_hint": "用于补强真实设定、职业/地点/历史细节，吸收信息结构，不要直接照抄原文。",
                "asset_type": "exa_search_result",
                "raw_content": raw_content,
            })
        return assets

    def _build_grok_assets(self, payload: Dict[str, Any]) -> List[Dict[str, str]]:
        if not isinstance(payload, dict) or payload.get("error"):
            return []
        assets: List[Dict[str, str]] = []
        content = self._clip_text(payload.get("content"), self.MAX_SUMMARY_CHARS)
        raw_content = self._clip_text(payload.get("content"), self.MAX_RAW_CHARS)
        sources = payload.get("sources") or []
        primary_source = self._clip_text((sources[0] or {}).get("url") if sources else "grok-search", 300)
        if content:
            assets.append({
                "title": "Grok 实时综述",
                "source": primary_source or "grok-search",
                "summary": content,
                "usage_hint": "用于提炼当下语感、讨论热点和社会氛围，避免把观点原样写成正文。",
                "asset_type": "grok_search_summary",
                "raw_content": raw_content,
            })
        for item in sources[:2]:
            title = self._clip_text(item.get("title") or item.get("url") or "Grok 来源", 120)
            source = self._clip_text(item.get("url") or "grok-search", 300)
            summary = self._clip_text(item.get("snippet") or item.get("title") or source, 220)
            if not summary:
                continue
            assets.append({
                "title": title,
                "source": source,
                "summary": summary,
                "usage_hint": "作为外部讨论样本参考，用来优化用词、氛围与现实感。",
                "asset_type": "grok_search_source",
                "raw_content": summary,
            })
        return assets

    def _write_archive(self, *, archive_scope: str, archive_id: str, bundle: Dict[str, Any]) -> str:
        archive_dir = PROJECT_ROOT / "data" / "web_research" / (archive_scope or "misc")
        archive_dir.mkdir(parents=True, exist_ok=True)
        output_path = archive_dir / f"{archive_id}.json"
        with open(output_path, "w", encoding="utf-8") as file:
            json.dump(bundle, file, ensure_ascii=False, indent=2)
        return str(output_path)

    async def collect_assets(
        self,
        *,
        user_id: Optional[str],
        db_session: Optional[AsyncSession],
        exa_query: str,
        grok_query: str,
        enable_web_research: Optional[bool],
        archive_scope: str,
        archive_id: str,
        metadata: Optional[Dict[str, Any]] = None,
        runtime_config: Optional[WebResearchRuntimeConfig] = None,
    ) -> Dict[str, Any]:
        resolved_config = runtime_config or await self.get_runtime_config(user_id=user_id, db_session=db_session)
        if not self.is_enabled(enable_web_research, resolved_config):
            return {"enabled": False, "assets": [], "query": "", "archive_path": ""}
        if not self.skills_root().exists():
            logger.warning("⚠️ 外部检索技能目录不存在，跳过预生成检索: %s", self.skills_root())
            return {"enabled": True, "assets": [], "query": "", "archive_path": "", "skip_reason": "skills_root_missing"}
        if not exa_query and not grok_query:
            return {"enabled": True, "assets": [], "query": "", "archive_path": "", "skip_reason": "empty_query"}

        exa_payload: Dict[str, Any] = {}
        grok_payload: Dict[str, Any] = {}
        if resolved_config.exa_enabled and exa_query:
            exa_payload = await self._run_exa_search(exa_query, resolved_config)
        if resolved_config.grok_enabled and grok_query:
            grok_payload = await self._run_grok_search(grok_query, resolved_config)

        assets = (self._build_exa_assets(exa_payload) + self._build_grok_assets(grok_payload))[: resolved_config.max_assets]
        bundle = {
            "generated_at": datetime.now().isoformat(),
            "query": {"exa": exa_query, "grok": grok_query},
            "assets": assets,
            "exa": exa_payload,
            "grok": grok_payload,
        }
        if metadata:
            bundle.update(metadata)
        archive_path = self._write_archive(archive_scope=archive_scope, archive_id=archive_id, bundle=bundle)
        return {
            "enabled": True,
            "assets": assets,
            "query": exa_query or grok_query,
            "archive_path": archive_path,
            "exa": exa_payload,
            "grok": grok_payload,
        }
    async def collect_for_chapter(
        self,
        *,
        user_id: str,
        db_session: AsyncSession,
        project: Project,
        chapter: Chapter,
        outline: Optional[Outline],
        story_creation_brief: Optional[str],
        enable_web_research: Optional[bool],
        web_research_query: Optional[str],
    ) -> Dict[str, Any]:
        return await self.collect_assets(
            user_id=user_id,
            db_session=db_session,
            exa_query=self._chapter_exa_query(project, chapter, outline, story_creation_brief, web_research_query),
            grok_query=self._chapter_grok_query(project, chapter, outline, story_creation_brief, web_research_query),
            enable_web_research=enable_web_research,
            archive_scope=project.id,
            archive_id=chapter.id,
            metadata={"project_id": project.id, "chapter_id": chapter.id, "chapter_number": chapter.chapter_number},
        )

    async def replace_memories(
        self,
        *,
        db_session: AsyncSession,
        user_id: str,
        project_id: str,
        query: str,
        archive_path: str,
        assets: List[Dict[str, str]],
        memory_type: str,
        title_prefix: str,
        story_timeline: int,
        chapter_id: Optional[str] = None,
    ) -> List[str]:
        if not assets:
            return []
        where_conditions = [StoryMemory.project_id == project_id, StoryMemory.memory_type == memory_type]
        where_conditions.append(StoryMemory.chapter_id == chapter_id if chapter_id else StoryMemory.chapter_id.is_(None))
        existing_result = await db_session.execute(select(StoryMemory).where(*where_conditions))
        for item in list(existing_result.scalars().all()):
            await db_session.delete(item)
        await db_session.flush()
        await memory_service.delete_memories_by_types(
            user_id=user_id,
            project_id=project_id,
            chapter_id=chapter_id,
            memory_types=[memory_type],
        )

        saved_ids: List[str] = []
        for index, asset in enumerate(assets, start=1):
            memory_id = str(uuid.uuid4())
            title = self._clip_text(f"{title_prefix} {index}: {asset.get('title') or '未命名资料'}", 180)
            summary = self._clip_text(asset.get("summary"), 500)
            memory_content = self._clip_text(f"{title} 来源：{asset.get('source') or '未知来源'} 摘要：{summary}", 600)
            full_context = json.dumps({"query": query, "archive_path": archive_path, "asset": asset}, ensure_ascii=False)
            db_session.add(StoryMemory(
                id=memory_id,
                project_id=project_id,
                chapter_id=chapter_id,
                memory_type=memory_type,
                title=title,
                content=summary or memory_content,
                full_context=full_context,
                tags=["web_research", asset.get("asset_type") or "external_asset"],
                importance_score=0.62,
                story_timeline=story_timeline,
                chapter_position=0,
                text_length=len(summary or memory_content),
                vector_id=memory_id,
            ))
            await memory_service.add_memory(
                user_id=user_id,
                project_id=project_id,
                memory_id=memory_id,
                content=memory_content,
                memory_type=memory_type,
                metadata={
                    "chapter_id": chapter_id or "",
                    "chapter_number": story_timeline,
                    "importance_score": 0.62,
                    "tags": ["web_research", asset.get("asset_type") or "external_asset"],
                    "title": title,
                },
            )
            saved_ids.append(memory_id)
        await db_session.commit()
        return saved_ids

    async def replace_chapter_memories(self, *, db_session: AsyncSession, user_id: str, project: Project, chapter: Chapter, query: str, archive_path: str, assets: List[Dict[str, str]]) -> List[str]:
        return await self.replace_memories(
            db_session=db_session,
            user_id=user_id,
            project_id=project.id,
            query=query,
            archive_path=archive_path,
            assets=assets,
            memory_type=self.MEMORY_TYPE,
            title_prefix="外部资料",
            story_timeline=chapter.chapter_number,
            chapter_id=chapter.id,
        )

    async def test_provider_connection(self, *, provider: str, overrides: Mapping[str, Any], query: Optional[str] = None) -> Dict[str, Any]:
        runtime_config = self.build_runtime_config(overrides=overrides)
        provider_name = (provider or "").strip().lower()
        if provider_name == "exa":
            payload = await self._run_exa_search(query or "historical fiction writing details with reliable sources", runtime_config)
            results = payload.get("results") or []
            success = not payload.get("error") and bool(results)
            return {
                "success": success,
                "provider": "exa",
                "message": "Exa 连接测试成功" if success else "Exa 连接测试失败",
                "response_preview": self._clip_text(((results[0] or {}).get("text") if results else "") or ((results[0] or {}).get("title") if results else ""), 180),
                "result_count": len(results),
                "error": payload.get("detail") or payload.get("error"),
                "error_type": "SkillError" if payload.get("error") else None,
                "suggestions": [] if success else ["检查 Exa API Key 是否正确", "确认技能仓库路径和网络访问正常"],
            }
        payload = await self._run_grok_search(query or "Summarize current discussion around fiction writing trends with sources", runtime_config)
        sources = payload.get("sources") or []
        content = self._clip_text(payload.get("content"), 180)
        success = not payload.get("error") and bool(content)
        return {
            "success": success,
            "provider": "grok",
            "message": "Grok 连接测试成功" if success else "Grok 连接测试失败",
            "response_preview": content,
            "source_count": len(sources),
            "error": payload.get("detail") or payload.get("error"),
            "error_type": "SkillError" if payload.get("error") else None,
            "suggestions": [] if success else ["检查 Grok API Key 是否正确", "确认 Grok Base URL 可访问且兼容 OpenAI 格式"],
        }


chapter_web_research_service = ChapterWebResearchService()
