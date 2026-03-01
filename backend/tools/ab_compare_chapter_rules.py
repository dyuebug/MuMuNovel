# -*- coding: utf-8 -*-
"""A/B 实测：章节生成规则优化前后对比。

用途：
1. 从运行中的本地服务读取当前用户 API 配置（登录 admin/admin123）
2. 提取 HEAD 版本（优化前）与工作区版本（优化后）的章节模板
3. 用同一套章节输入分别生成两版正文
4. 输出样章与统计报告到 logs/ab_chapter_rules/<timestamp>/
"""

from __future__ import annotations

import argparse
import asyncio
import json
import re
import subprocess
import sys
from dataclasses import dataclass
from datetime import datetime
from pathlib import Path
from typing import Dict, List

import httpx

BACKEND_ROOT = Path(__file__).resolve().parent.parent
if str(BACKEND_ROOT) not in sys.path:
    sys.path.insert(0, str(BACKEND_ROOT))

from app.services.ai_service import AIService
from app.services.prompt_service import PromptService


API_BASE = "http://127.0.0.1:8003"
LOGIN_PAYLOAD = {"username": "admin", "password": "admin123"}

TERMS = ["门影折返", "镜面门", "雾层回灌", "相位锚", "判题偏好", "热区刷新"]
EXPLAIN_MARKERS = ["简单说", "说白了", "意思是", "换句话说", "你可以理解为", "打个比方", "就是"]
ACTION_MARKERS = ["冲", "拽", "按", "甩", "躲", "踩", "扯", "推", "砸", "扑", "抬", "踉跄", "后撤", "顶住"]
CHAR_NAMES = ["林昼", "许遥", "陈放", "小北"]


@dataclass
class RuntimeSettings:
    provider: str
    api_key: str
    api_base_url: str
    model: str
    models: List[str]


def fetch_runtime_settings() -> RuntimeSettings:
    with httpx.Client(base_url=API_BASE, timeout=30.0) as client:
        login = client.post("/api/auth/local/login", json=LOGIN_PAYLOAD)
        login.raise_for_status()

        settings_resp = client.get("/api/settings")
        settings_resp.raise_for_status()
        settings = settings_resp.json()

        provider = settings.get("api_provider") or "sub2api"
        api_key = settings.get("api_key") or ""
        api_base_url = settings.get("api_base_url") or "https://ai.qaq.al"
        preferred_model = settings.get("llm_model") or "gpt-5.3-codex"

        models: List[str] = []
        try:
            model_resp = client.get(
                "/api/settings/models",
                params={
                    "api_key": api_key,
                    "api_base_url": api_base_url,
                    "provider": provider,
                },
            )
            model_resp.raise_for_status()
            models = [m.get("value", "") for m in model_resp.json().get("models", []) if m.get("value")]
        except Exception:
            models = []

        if not api_key:
            raise RuntimeError("当前用户 api_key 为空，无法进行真实模型实测")

        chosen_model = choose_model(preferred_model, models)
        return RuntimeSettings(
            provider=provider,
            api_key=api_key,
            api_base_url=api_base_url,
            model=chosen_model,
            models=models,
        )


def choose_model(preferred: str, models: List[str]) -> str:
    if preferred and preferred in models:
        return preferred
    for candidate in models:
        if "gpt-5.3-codex" in candidate:
            return candidate
    for candidate in models:
        if "gpt-5" in candidate:
            return candidate
    return preferred or (models[0] if models else "gpt-5.3-codex")


def extract_old_template() -> str:
    old_file = subprocess.check_output(
        ["git", "show", "HEAD:backend/app/services/prompt_service.py"],
        text=True,
        encoding="utf-8",
    )
    match = re.search(r'CHAPTER_GENERATION_ONE_TO_ONE = """(.*?)"""', old_file, flags=re.S)
    if not match:
        raise RuntimeError("无法从 HEAD 版本提取 CHAPTER_GENERATION_ONE_TO_ONE")
    return match.group(1)


def build_prompt(template: str) -> str:
    return PromptService.format_prompt(
        template,
        project_title="临海折返线",
        genre="都市异能",
        chapter_number=12,
        chapter_title="雨夜折返",
        chapter_outline=(
            "雨夜中，林昼带许遥母子和陈放撤离C-17。途中依次出现门影折返、镜面门、雾层回灌、相位锚失效。"
            "主角必须在90秒内重算路线，并把“判题偏好”和“热区刷新”解释给非战斗人员。"
            "中段遇到陌生求救者：救人会让队伍暴露10秒，不救会触发主角旧创伤。"
            "结尾队伍抵达旧维护口，小北手腕反光条突然变亮，暗示门影已锁定他们。"
        ),
        target_word_count=1200,
        narrative_perspective="第三人称",
        world_time_period="近未来雨季常态化时期（2042年）",
        world_location="临海市三环老居住带与废弃地铁维护区",
        world_atmosphere="潮湿压抑、停电频发、警报高密度播报",
        world_rules=(
            "门影会优先锁定回头者与拥堵路径；镜面门会替换原出口；"
            "雾层回灌区停留越久失联概率越高；相位锚可短时稳定路径但会衰减。"
        ),
        characters_info=(
            "林昼：特勤解题员，冷静算路快，但对“抛下人”有心理阴影。\n"
            "许遥：社区分拣员，非战斗人员，护子本能强，关键时刻敢冒险。\n"
            "陈放：外卖骑手，嘴硬，左膝旧伤，怕拖累队伍。\n"
            "许小北：7岁，发烧但观察力敏锐。"
        ),
        chapter_careers="特勤解题员、社区分拣员、外卖骑手",
        foreshadow_reminders="镜面门对回头者更活跃；反光条可能与门影标记同步。",
        relevant_memories="第10章中林昼曾因先救陌生人导致同伴重伤，此后更谨慎。",
    )


def build_system_prompt(version: str) -> str:
    base = (
        "【🎨 写作风格参考】\n\n"
        "语言生活化，读起来像真人在讲故事；长短句穿插，不要模板腔。"
        "角色对话要有口吻差异，情绪主要通过动作和场景细节呈现。"
    )
    if version == "new":
        return (
            base
            + "\n若单段出现连续术语，请在三句内用角色互动补一句通俗解释，"
            "并用“动作→反馈→后果”推进关键情节。"
            "至少让1名核心配角出现一次反预期行为，并在当场补一句动机解释。"
        )
    return base


async def generate_once(
    service: AIService,
    prompt: str,
    model: str,
    system_prompt: str,
    max_attempts: int = 3,
) -> str:
    last_error: Exception | None = None
    for attempt in range(1, max_attempts + 1):
        try:
            chunks: List[str] = []
            async for chunk in service.generate_text_stream(
                prompt=prompt,
                model=model,
                temperature=0.2,
                max_tokens=2600,
                system_prompt=system_prompt,
                tool_choice="none",
                auto_mcp=False,
            ):
                chunks.append(chunk)
            return "".join(chunks)
        except Exception as exc:
            last_error = exc
            if attempt >= max_attempts:
                break
            await asyncio.sleep(min(2 * attempt, 6))
    assert last_error is not None
    raise last_error


def calc_metrics(text: str) -> Dict[str, object]:
    paragraphs = [p for p in re.split(r"\n+", text) if p.strip()]
    term_hits = {term: text.count(term) for term in TERMS}
    explain_hits = {marker: text.count(marker) for marker in EXPLAIN_MARKERS}
    action_hits = {marker: text.count(marker) for marker in ACTION_MARKERS}
    name_hits = {name: text.count(name) for name in CHAR_NAMES}
    return {
        "chars": len(text),
        "paragraphs": len(paragraphs),
        "dialogue_quote_count": text.count("“") + text.count("”"),
        "term_hits": term_hits,
        "term_total": sum(term_hits.values()),
        "explain_marker_hits": explain_hits,
        "explain_marker_total": sum(explain_hits.values()),
        "action_marker_hits": action_hits,
        "action_marker_total": sum(action_hits.values()),
        "character_name_hits": name_hits,
        "active_characters_ge3": sum(1 for count in name_hits.values() if count >= 3),
    }


def aggregate_round_metrics(rounds: List[Dict[str, object]]) -> Dict[str, object]:
    metric_keys = [
        "chars",
        "paragraphs",
        "dialogue_quote_count",
        "term_total",
        "explain_marker_total",
        "action_marker_total",
        "active_characters_ge3",
    ]
    aggregate = {}
    for key in metric_keys:
        old_vals = [r["metrics"]["old"][key] for r in rounds]
        new_vals = [r["metrics"]["new"][key] for r in rounds]
        old_avg = sum(old_vals) / len(old_vals)
        new_avg = sum(new_vals) / len(new_vals)
        aggregate[key] = {
            "old_avg": round(old_avg, 2),
            "new_avg": round(new_avg, 2),
            "delta": round(new_avg - old_avg, 2),
        }

    old_explain_rounds = sum(1 for r in rounds if r["metrics"]["old"]["explain_marker_total"] > 0)
    new_explain_rounds = sum(1 for r in rounds if r["metrics"]["new"]["explain_marker_total"] > 0)
    old_action_density = [
        (r["metrics"]["old"]["action_marker_total"] / max(r["metrics"]["old"]["chars"], 1)) * 1000
        for r in rounds
    ]
    new_action_density = [
        (r["metrics"]["new"]["action_marker_total"] / max(r["metrics"]["new"]["chars"], 1)) * 1000
        for r in rounds
    ]

    aggregate["stability"] = {
        "rounds": len(rounds),
        "old_explain_hit_ratio": round(old_explain_rounds / len(rounds), 2),
        "new_explain_hit_ratio": round(new_explain_rounds / len(rounds), 2),
        "old_action_density_per_1k_chars_avg": round(sum(old_action_density) / len(old_action_density), 2),
        "new_action_density_per_1k_chars_avg": round(sum(new_action_density) / len(new_action_density), 2),
    }
    return aggregate


async def main(rounds: int) -> None:
    runtime = fetch_runtime_settings()
    old_template = extract_old_template()
    new_template = PromptService.CHAPTER_GENERATION_ONE_TO_ONE

    old_prompt = build_prompt(old_template)
    new_prompt = build_prompt(new_template)

    ai = AIService(
        api_provider=runtime.provider,
        api_key=runtime.api_key,
        api_base_url=runtime.api_base_url,
        default_model=runtime.model,
        default_temperature=0.2,
        default_max_tokens=2600,
        enable_mcp=False,
    )

    timestamp = datetime.now().strftime("%Y%m%d_%H%M%S")
    output_dir = Path("../logs/ab_chapter_rules") / timestamp
    output_dir.mkdir(parents=True, exist_ok=True)
    (output_dir / "old_prompt.txt").write_text(old_prompt, encoding="utf-8")
    (output_dir / "new_prompt.txt").write_text(new_prompt, encoding="utf-8")

    rounds_report: List[Dict[str, object]] = []
    for idx in range(1, rounds + 1):
        old_text = await generate_once(ai, old_prompt, runtime.model, build_system_prompt("old"))
        new_text = await generate_once(ai, new_prompt, runtime.model, build_system_prompt("new"))

        (output_dir / f"round_{idx}_old.txt").write_text(old_text, encoding="utf-8")
        (output_dir / f"round_{idx}_new.txt").write_text(new_text, encoding="utf-8")

        rounds_report.append(
            {
                "round": idx,
                "metrics": {
                    "old": calc_metrics(old_text),
                    "new": calc_metrics(new_text),
                },
            }
        )

    report = {
        "provider": runtime.provider,
        "api_base_url": runtime.api_base_url,
        "chosen_model": runtime.model,
        "models_count": len(runtime.models),
        "rounds": rounds,
        "prompt_length": {"old": len(old_prompt), "new": len(new_prompt)},
        "per_round": rounds_report,
        "aggregate": aggregate_round_metrics(rounds_report),
        "output_dir": str(output_dir.resolve()),
    }
    (output_dir / "report.json").write_text(json.dumps(report, ensure_ascii=False, indent=2), encoding="utf-8")

    print(json.dumps(report, ensure_ascii=False, indent=2))


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description="章节生成规则 A/B 对比")
    parser.add_argument("--rounds", type=int, default=5, help="A/B 轮次（1-10）")
    args = parser.parse_args()
    rounds = max(1, min(args.rounds, 10))
    asyncio.run(main(rounds))
