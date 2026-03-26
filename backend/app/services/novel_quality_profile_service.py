"""小说质量画像编排服务。"""
from __future__ import annotations

from dataclasses import dataclass
from typing import Any, Dict, Iterable, Mapping, Optional, Sequence, Tuple

from app.services.novel_quality_rules import (
    CHECKER_ALLOWED_CATEGORIES,
    CHECKER_ASSESSMENT_SCALE,
    CHECKER_REVIEW_ORDER,
    CHECKER_SEVERITY_ORDER,
    CHECKER_SEVERITY_RULES,
    DEFAULT_TOMATO_BASELINE_RULES,
    EXTERNAL_ASSET_IGNORE_REASON_DUPLICATE,
    EXTERNAL_ASSET_IGNORE_REASON_LIMIT,
    EXTERNAL_ASSET_IGNORE_REASON_NO_SUMMARY,
    EXTERNAL_ASSET_IGNORE_REASON_RAW_ONLY,
    EXTERNAL_ASSET_RULES,
    EXTERNAL_ASSET_SUMMARY_ONLY_NOTICE,
    MAX_EXTERNAL_ASSET_COUNT,
    MAX_EXTERNAL_ASSET_SOURCE_LENGTH,
    MAX_EXTERNAL_ASSET_SUMMARY_LENGTH,
    MAX_EXTERNAL_ASSET_TITLE_LENGTH,
    MAX_EXTERNAL_ASSET_USAGE_HINT_LENGTH,
    MCP_GUARD_RULES,
    QUALITY_BASELINE_ID,
    QUALITY_BLOCK_ORDER,
    QUALITY_BLOCK_TITLES,
    QUALITY_DIMENSIONS,
    QUALITY_PROFILE_VERSION,
    REVISER_CORE_RULES,
    detect_genre_profiles,
    detect_style_profile,
    get_genre_relaxations,
    get_style_relaxation,
)


@dataclass(frozen=True)
class NovelQualityAssetInput:
    title: Optional[str] = None
    source: Optional[str] = None
    summary: Optional[str] = None
    usage_hint: Optional[str] = None
    asset_type: Optional[str] = None
    raw_content: Optional[str] = None

    @classmethod
    def from_mapping(cls, payload: Mapping[str, Any]) -> "NovelQualityAssetInput":
        return cls(
            title=_first_text(payload, "title", "name", "label"),
            source=_first_text(payload, "source", "url", "reference", "origin"),
            summary=_first_text(
                payload,
                "summary",
                "content_summary",
                "excerpt_summary",
                "abstract",
                "note_summary",
            ),
            usage_hint=_first_text(payload, "usage_hint", "focus", "reason", "hint", "usage"),
            asset_type=_first_text(payload, "asset_type", "type", "category"),
            raw_content=_first_text(payload, "raw_content", "content", "text", "body", "excerpt"),
        )


@dataclass(frozen=True)
class NovelQualityAssetSummary:
    title: str
    source: str
    summary: str
    usage_hint: str
    asset_type: str
    summary_only: bool = True

    def to_line(self) -> str:
        parts = [self.title]
        if self.asset_type:
            parts.append(f"类型：{self.asset_type}")
        if self.source:
            parts.append(f"来源：{self.source}")
        if self.usage_hint:
            parts.append(f"使用提醒：{self.usage_hint}")
        parts.append(f"摘要：{self.summary}")
        return "；".join(part for part in parts if part)

    def to_dict(self) -> Dict[str, Any]:
        return {
            "title": self.title,
            "source": self.source,
            "summary": self.summary,
            "usage_hint": self.usage_hint,
            "asset_type": self.asset_type,
            "summary_only": self.summary_only,
        }


@dataclass(frozen=True)
class NovelQualityIgnoredAsset:
    title: str
    reason: str

    def to_dict(self) -> Dict[str, str]:
        return {
            "title": self.title,
            "reason": self.reason,
        }


@dataclass(frozen=True)
class NovelQualityProfileInput:
    genre: Optional[str] = None
    style_name: Optional[str] = None
    style_preset_id: Optional[str] = None
    style_content: Optional[str] = None
    external_assets: Tuple[NovelQualityAssetInput, ...] = ()

    @classmethod
    def from_payload(cls, payload: Optional[Mapping[str, Any]]) -> "NovelQualityProfileInput":
        if not payload:
            return cls()

        assets = payload.get("external_assets") or payload.get("reference_assets") or ()
        return cls(
            genre=_as_text(payload.get("genre")),
            style_name=_as_text(payload.get("style_name")),
            style_preset_id=_as_text(payload.get("style_preset_id")),
            style_content=_as_text(payload.get("style_content")),
            external_assets=_coerce_asset_inputs(assets),
        )


@dataclass(frozen=True)
class NovelQualityProfileBlock:
    key: str
    title: str
    lines: Tuple[str, ...]
    text: str

    @classmethod
    def build(cls, key: str, title: str, lines: Iterable[str]) -> "NovelQualityProfileBlock":
        cleaned = tuple(_unique_non_empty(lines))
        rendered_lines = [f"【{title}】"]
        rendered_lines.extend(f"- {line}" for line in cleaned)
        return cls(
            key=key,
            title=title,
            lines=cleaned,
            text="\n".join(rendered_lines),
        )

    def to_dict(self) -> Dict[str, Any]:
        return {
            "key": self.key,
            "title": self.title,
            "lines": list(self.lines),
            "text": self.text,
        }


@dataclass(frozen=True)
class NovelQualityPromptBlocks:
    generation: str
    checker: str
    reviser: str
    mcp_guard: str
    external_assets: str

    def to_dict(self) -> Dict[str, str]:
        return {
            "generation": self.generation,
            "checker": self.checker,
            "reviser": self.reviser,
            "mcp_guard": self.mcp_guard,
            "external_assets": self.external_assets,
        }


@dataclass(frozen=True)
class NovelQualityRelaxationSnapshot:
    scope: str
    key: str
    label: str
    generation_relaxations: Tuple[str, ...]
    checker_adjustments: Tuple[str, ...]
    reviser_adjustments: Tuple[str, ...]

    def to_dict(self) -> Dict[str, Any]:
        return {
            "scope": self.scope,
            "key": self.key,
            "label": self.label,
            "generation_relaxations": list(self.generation_relaxations),
            "checker_adjustments": list(self.checker_adjustments),
            "reviser_adjustments": list(self.reviser_adjustments),
        }


@dataclass(frozen=True)
class NovelQualityProfile:
    version: str
    baseline_id: str
    genre_profiles: Tuple[str, ...]
    style_profile: str
    quality_dimensions: Tuple[str, ...]
    active_relaxations: Tuple[NovelQualityRelaxationSnapshot, ...]
    external_assets: Tuple[NovelQualityAssetSummary, ...]
    ignored_external_assets: Tuple[NovelQualityIgnoredAsset, ...]
    generation: NovelQualityProfileBlock
    checker: NovelQualityProfileBlock
    reviser: NovelQualityProfileBlock
    mcp_guard: NovelQualityProfileBlock
    external_assets_block: NovelQualityProfileBlock
    blocks: Dict[str, NovelQualityProfileBlock]
    prompt_blocks: NovelQualityPromptBlocks
    policy: Dict[str, Any]

    def to_prompt_blocks(self) -> Dict[str, str]:
        return self.prompt_blocks.to_dict()

    def to_dict(self) -> Dict[str, Any]:
        return {
            "version": self.version,
            "baseline_id": self.baseline_id,
            "genre_profiles": list(self.genre_profiles),
            "style_profile": self.style_profile,
            "quality_dimensions": list(self.quality_dimensions),
            "active_relaxations": [item.to_dict() for item in self.active_relaxations],
            "external_assets": [item.to_dict() for item in self.external_assets],
            "ignored_external_assets": [item.to_dict() for item in self.ignored_external_assets],
            "generation": self.generation.to_dict(),
            "checker": self.checker.to_dict(),
            "reviser": self.reviser.to_dict(),
            "mcp_guard": self.mcp_guard.to_dict(),
            "external_assets_block": self.external_assets_block.to_dict(),
            "blocks": {key: block.to_dict() for key, block in self.blocks.items()},
            "policy": self.policy,
            "prompt_blocks": self.to_prompt_blocks(),
        }


class NovelQualityProfileService:
    """根据题材、风格与摘要资产生成稳定的质量画像块。"""

    def build_profile(
        self,
        payload: Optional[Mapping[str, Any]] = None,
        profile_input: Optional[NovelQualityProfileInput] = None,
    ) -> NovelQualityProfile:
        normalized = profile_input or NovelQualityProfileInput.from_payload(payload)
        genre_profiles = detect_genre_profiles(normalized.genre)
        style_profile = detect_style_profile(
            style_name=normalized.style_name,
            style_preset_id=normalized.style_preset_id,
            style_content=normalized.style_content,
        )

        genre_relaxations = get_genre_relaxations(genre_profiles)
        style_relaxation = get_style_relaxation(style_profile)
        active_relaxations = self._build_relaxation_snapshots(genre_relaxations, style_relaxation)
        external_assets, ignored_assets = self._sanitize_external_assets(normalized.external_assets)

        generation = NovelQualityProfileBlock.build(
            key="generation",
            title=QUALITY_BLOCK_TITLES["generation"],
            lines=self._build_generation_lines(active_relaxations, external_assets),
        )
        checker = NovelQualityProfileBlock.build(
            key="checker",
            title=QUALITY_BLOCK_TITLES["checker"],
            lines=self._build_checker_lines(active_relaxations),
        )
        reviser = NovelQualityProfileBlock.build(
            key="reviser",
            title=QUALITY_BLOCK_TITLES["reviser"],
            lines=self._build_reviser_lines(active_relaxations),
        )
        mcp_guard = NovelQualityProfileBlock.build(
            key="mcp_guard",
            title=QUALITY_BLOCK_TITLES["mcp_guard"],
            lines=self._build_mcp_guard_lines(external_assets, ignored_assets),
        )
        external_assets_block = NovelQualityProfileBlock.build(
            key="external_assets",
            title=QUALITY_BLOCK_TITLES["external_assets"],
            lines=self._build_external_asset_lines(external_assets, ignored_assets),
        )
        blocks = self._build_block_map(
            generation=generation,
            checker=checker,
            reviser=reviser,
            mcp_guard=mcp_guard,
            external_assets=external_assets_block,
        )
        prompt_blocks = NovelQualityPromptBlocks(
            generation=blocks["generation"].text,
            checker=blocks["checker"].text,
            reviser=blocks["reviser"].text,
            mcp_guard=blocks["mcp_guard"].text,
            external_assets=blocks["external_assets"].text,
        )

        return NovelQualityProfile(
            version=QUALITY_PROFILE_VERSION,
            baseline_id=QUALITY_BASELINE_ID,
            genre_profiles=genre_profiles,
            style_profile=style_profile,
            quality_dimensions=tuple(item.key for item in QUALITY_DIMENSIONS),
            active_relaxations=active_relaxations,
            external_assets=external_assets,
            ignored_external_assets=ignored_assets,
            generation=generation,
            checker=checker,
            reviser=reviser,
            mcp_guard=mcp_guard,
            external_assets_block=external_assets_block,
            blocks=blocks,
            prompt_blocks=prompt_blocks,
            policy=self._build_policy(),
        )

    def build_profile_dict(
        self,
        payload: Optional[Mapping[str, Any]] = None,
        profile_input: Optional[NovelQualityProfileInput] = None,
    ) -> Dict[str, Any]:
        return self.build_profile(payload=payload, profile_input=profile_input).to_dict()

    def _build_block_map(self, **blocks: NovelQualityProfileBlock) -> Dict[str, NovelQualityProfileBlock]:
        return {
            key: blocks[key]
            for key in QUALITY_BLOCK_ORDER
            if key in blocks
        }

    def _build_relaxation_snapshots(
        self,
        genre_relaxations: Tuple[Any, ...],
        style_relaxation: Optional[Any],
    ) -> Tuple[NovelQualityRelaxationSnapshot, ...]:
        snapshots = []
        for rule in genre_relaxations:
            snapshots.append(
                NovelQualityRelaxationSnapshot(
                    scope="genre",
                    key=rule.key,
                    label=rule.label,
                    generation_relaxations=tuple(rule.generation_relaxations),
                    checker_adjustments=tuple(rule.checker_adjustments),
                    reviser_adjustments=tuple(rule.reviser_adjustments),
                )
            )
        if style_relaxation is not None:
            snapshots.append(
                NovelQualityRelaxationSnapshot(
                    scope="style",
                    key=style_relaxation.key,
                    label=style_relaxation.label,
                    generation_relaxations=tuple(style_relaxation.generation_relaxations),
                    checker_adjustments=tuple(style_relaxation.checker_adjustments),
                    reviser_adjustments=tuple(style_relaxation.reviser_adjustments),
                )
            )
        return tuple(snapshots)

    def _sanitize_external_assets(
        self,
        assets: Tuple[NovelQualityAssetInput, ...],
    ) -> Tuple[Tuple[NovelQualityAssetSummary, ...], Tuple[NovelQualityIgnoredAsset, ...]]:
        accepted = []
        ignored = []
        accepted_count = 0

        for index, asset in enumerate(assets, start=1):
            title = _clip_text(asset.title, MAX_EXTERNAL_ASSET_TITLE_LENGTH) or f"外部资产{index}"
            summary = _clip_text(asset.summary, MAX_EXTERNAL_ASSET_SUMMARY_LENGTH)
            usage_hint = _clip_text(asset.usage_hint, MAX_EXTERNAL_ASSET_USAGE_HINT_LENGTH)
            source = _clip_text(asset.source, MAX_EXTERNAL_ASSET_SOURCE_LENGTH)
            asset_type = _clip_text(asset.asset_type, 40)
            raw_content = _as_text(asset.raw_content)

            if not summary:
                reason = EXTERNAL_ASSET_IGNORE_REASON_NO_SUMMARY
                if raw_content:
                    reason = EXTERNAL_ASSET_IGNORE_REASON_RAW_ONLY
                ignored.append(NovelQualityIgnoredAsset(title=title, reason=reason))
                continue

            if accepted_count >= MAX_EXTERNAL_ASSET_COUNT:
                ignored.append(NovelQualityIgnoredAsset(title=title, reason=EXTERNAL_ASSET_IGNORE_REASON_LIMIT))
                continue

            accepted.append(
                NovelQualityAssetSummary(
                    title=title,
                    source=source,
                    summary=summary,
                    usage_hint=usage_hint,
                    asset_type=asset_type,
                )
            )
            accepted_count += 1

        deduped = []
        seen_signatures = set()
        for asset in accepted:
            signature = (asset.title, asset.summary)
            if signature in seen_signatures:
                ignored.append(NovelQualityIgnoredAsset(title=asset.title, reason=EXTERNAL_ASSET_IGNORE_REASON_DUPLICATE))
                continue
            seen_signatures.add(signature)
            deduped.append(asset)

        return tuple(deduped[:MAX_EXTERNAL_ASSET_COUNT]), tuple(ignored)

    def _build_generation_lines(
        self,
        active_relaxations: Tuple[NovelQualityRelaxationSnapshot, ...],
        external_assets: Tuple[NovelQualityAssetSummary, ...],
    ) -> Tuple[str, ...]:
        lines = [
            f"质量画像版本：{QUALITY_PROFILE_VERSION}；默认基线：{QUALITY_BASELINE_ID}。",
            *DEFAULT_TOMATO_BASELINE_RULES,
            "统一命中目标：",
        ]
        lines.extend(
            f"[{dimension.label}] {dimension.generation_goal}"
            for dimension in QUALITY_DIMENSIONS
        )
        lines.append("当前松绑策略：")
        if active_relaxations:
            for relaxation in active_relaxations:
                lines.append(f"[{relaxation.scope}:{relaxation.label}] {'；'.join(relaxation.generation_relaxations)}")
        else:
            lines.append("未命中特殊题材/风格松绑，使用默认番茄基线。")

        if external_assets:
            lines.append("当前可用外部摘要资产：")
            lines.extend(asset.to_line() for asset in external_assets)
        else:
            lines.append("当前无外部摘要资产，按项目内设定与章节上下文执行。")
        return tuple(lines)

    def _build_checker_lines(
        self,
        active_relaxations: Tuple[NovelQualityRelaxationSnapshot, ...],
    ) -> Tuple[str, ...]:
        lines = [
            "质检只做证据驱动判断，不杜撰问题，不输出流程化元文本。",
            *CHECKER_REVIEW_ORDER,
            "严重度定义：",
            *CHECKER_SEVERITY_RULES,
            f"允许分类：{'、'.join(CHECKER_ALLOWED_CATEGORIES)}。",
            f"总评枚举：{'、'.join(CHECKER_ASSESSMENT_SCALE)}。",
            "统一检查维度：",
        ]
        lines.extend(
            f"[{dimension.label}] {dimension.checker_focus}"
            for dimension in QUALITY_DIMENSIONS
        )
        lines.append("当前松绑口径：")
        if active_relaxations:
            for relaxation in active_relaxations:
                lines.append(f"[{relaxation.scope}:{relaxation.label}] {'；'.join(relaxation.checker_adjustments)}")
        else:
            lines.append("未命中特殊松绑规则，按默认连载质检口径执行。")
        return tuple(lines)

    def _build_reviser_lines(
        self,
        active_relaxations: Tuple[NovelQualityRelaxationSnapshot, ...],
    ) -> Tuple[str, ...]:
        lines = [
            "修订输出必须仍是可直接阅读的小说正文或可执行建议，不得夹带说明腔。",
            *REVISER_CORE_RULES,
            f"严重度处理顺序：{' > '.join(CHECKER_SEVERITY_ORDER)}。",
            "统一修补重点：",
        ]
        lines.extend(
            f"[{dimension.label}] {dimension.reviser_focus}"
            for dimension in QUALITY_DIMENSIONS
        )
        lines.append("当前松绑口径：")
        if active_relaxations:
            for relaxation in active_relaxations:
                lines.append(f"[{relaxation.scope}:{relaxation.label}] {'；'.join(relaxation.reviser_adjustments)}")
        else:
            lines.append("未命中特殊松绑规则，按默认最小改动修订策略执行。")
        return tuple(lines)

    def _build_mcp_guard_lines(
        self,
        external_assets: Tuple[NovelQualityAssetSummary, ...],
        ignored_assets: Tuple[NovelQualityIgnoredAsset, ...],
    ) -> Tuple[str, ...]:
        lines = [
            *MCP_GUARD_RULES,
            *EXTERNAL_ASSET_RULES,
            f"当前接入结果：accepted={len(external_assets)}，ignored={len(ignored_assets)}，summary_only=true。",
        ]
        if external_assets:
            lines.append("已接入的外部摘要资产：")
            lines.extend(asset.to_line() for asset in external_assets)
        if ignored_assets:
            lines.append("已忽略的外部资产：")
            lines.extend(f"{item.title}：{item.reason}" for item in ignored_assets)
        return tuple(lines)

    def _build_external_asset_lines(
        self,
        external_assets: Tuple[NovelQualityAssetSummary, ...],
        ignored_assets: Tuple[NovelQualityIgnoredAsset, ...],
    ) -> Tuple[str, ...]:
        if not external_assets:
            lines = [
                "未提供合规的外部摘要资产。",
                EXTERNAL_ASSET_SUMMARY_ONLY_NOTICE,
            ]
            if ignored_assets:
                lines.extend(f"{item.title}：{item.reason}" for item in ignored_assets)
            return tuple(lines)

        lines = [
            f"共接入 {len(external_assets)} 条摘要资产；所有资产均按 summary-only 策略注入。"
        ]
        lines.extend(asset.to_line() for asset in external_assets)
        if ignored_assets:
            lines.append("其余资产已忽略：")
            lines.extend(f"{item.title}：{item.reason}" for item in ignored_assets)
        return tuple(lines)

    def _build_policy(self) -> Dict[str, Any]:
        return {
            "quality_profile_version": QUALITY_PROFILE_VERSION,
            "baseline_id": QUALITY_BASELINE_ID,
            "block_order": list(QUALITY_BLOCK_ORDER),
            "block_titles": dict(QUALITY_BLOCK_TITLES),
            "checker_allowed_categories": list(CHECKER_ALLOWED_CATEGORIES),
            "checker_severity_order": list(CHECKER_SEVERITY_ORDER),
            "checker_assessment_scale": list(CHECKER_ASSESSMENT_SCALE),
            "external_assets": {
                "summary_only": True,
                "summary_only_notice": EXTERNAL_ASSET_SUMMARY_ONLY_NOTICE,
                "max_count": MAX_EXTERNAL_ASSET_COUNT,
                "max_summary_length": MAX_EXTERNAL_ASSET_SUMMARY_LENGTH,
                "max_title_length": MAX_EXTERNAL_ASSET_TITLE_LENGTH,
                "max_source_length": MAX_EXTERNAL_ASSET_SOURCE_LENGTH,
                "max_usage_hint_length": MAX_EXTERNAL_ASSET_USAGE_HINT_LENGTH,
                "ignore_reasons": {
                    "no_summary": EXTERNAL_ASSET_IGNORE_REASON_NO_SUMMARY,
                    "raw_only": EXTERNAL_ASSET_IGNORE_REASON_RAW_ONLY,
                    "limit": EXTERNAL_ASSET_IGNORE_REASON_LIMIT,
                    "duplicate": EXTERNAL_ASSET_IGNORE_REASON_DUPLICATE,
                },
            },
        }


QUALITY_FOCUS_LABELS: Dict[str, str] = {
    "opening": "开篇抓力",
    "conflict": "冲突升级",
    "outline": "大纲推进",
    "pacing": "节奏控制",
    "payoff": "兑现回收",
    "cliffhanger": "章尾牵引",
    "dialogue": "对白质感",
    "rule_grounding": "设定落地",
    "foreshadow_continuity": "伏笔接力",
    "relationship_continuity": "关系接力",
    "character_continuity": "人物接力",
    "organization_continuity": "势力接力",
    "career_continuity": "成长接力",
}

QUALITY_PROFILE_STYLE_LABELS: Dict[str, str] = {
    "low_ai_serial": "低 AI 连载感",
    "low_ai_life": "低 AI 生活化",
    "urban_finance": "都市金融",
    "tech_xianxia": "科技仙侠",
    "light_humor": "轻喜节奏",
    "era_plain": "年代朴素风",
}

QUALITY_PROFILE_GENRE_LABELS: Dict[str, str] = {
    "romance_slice_of_life": "言情 / 生活流",
    "suspense_mystery": "悬疑 / 谜案",
    "xianxia_fantasy": "仙侠 / 奇幻",
    "science_fiction_tech": "科幻 / 科技流",
    "history_power": "历史 / 权谋",
}

QUALITY_PROFILE_PRESET_LABELS: Dict[str, str] = {
    "plot_drive": "剧情推进",
    "immersive": "沉浸细节",
    "emotion_drama": "情绪戏剧",
    "clean_prose": "干净文风",
}


def _normalize_profile_token(value: Any) -> str:
    return _as_text(value).lower()


def _normalize_profile_token_sequence(values: Any, *, limit: int = 4) -> list[str]:
    if isinstance(values, (list, tuple, set)):
        iterable: Sequence[Any] = list(values)
    elif values in (None, ""):
        iterable = ()
    else:
        iterable = (values,)

    items: list[str] = []
    seen: set[str] = set()
    for value in iterable:
        token = _normalize_profile_token(value)
        if not token or token in seen:
            continue
        seen.add(token)
        items.append(token)
        if len(items) >= limit:
            break
    return items


def resolve_runtime_quality_profile(runtime_context: Mapping[str, Any]) -> Dict[str, Any]:
    genre = _as_text(runtime_context.get("genre"))
    style_name = _as_text(runtime_context.get("style_name"))
    style_preset_id = _as_text(runtime_context.get("style_preset_id"))
    style_profile = _normalize_profile_token(runtime_context.get("style_profile"))
    if not style_profile:
        style_profile = detect_style_profile(
            style_name=style_name,
            style_preset_id=style_preset_id,
        )

    genre_profiles = _normalize_profile_token_sequence(runtime_context.get("genre_profiles"), limit=4)
    if not genre_profiles or genre_profiles == ["default"]:
        genre_profiles = list(detect_genre_profiles(genre))

    return {
        "genre": genre,
        "genre_profiles": genre_profiles,
        "style_name": style_name,
        "style_preset_id": style_preset_id,
        "style_profile": style_profile,
        "quality_preset": _normalize_profile_token(runtime_context.get("quality_preset")),
    }


def _apply_focus_weight(weights: Dict[str, float], focus_area: str, multiplier: float) -> None:
    if focus_area not in weights:
        return
    weights[focus_area] = round(max(0.78, min(1.45, weights[focus_area] * multiplier)), 4)


def resolve_quality_weight_profile(
    runtime_context: Mapping[str, Any],
    resolved_stage: Optional[str],
) -> Dict[str, Any]:
    weights: Dict[str, float] = {
        "opening": 1.0,
        "conflict": 1.0,
        "outline": 1.0,
        "pacing": 1.0,
        "payoff": 1.0,
        "cliffhanger": 1.0,
        "dialogue": 1.0,
        "rule_grounding": 1.0,
    }

    if resolved_stage == "opening":
        _apply_focus_weight(weights, "opening", 1.15)
        _apply_focus_weight(weights, "conflict", 1.08)
        _apply_focus_weight(weights, "outline", 1.05)
    elif resolved_stage == "ending":
        _apply_focus_weight(weights, "payoff", 1.18)
        _apply_focus_weight(weights, "cliffhanger", 1.10)
        _apply_focus_weight(weights, "outline", 1.08)
        _apply_focus_weight(weights, "conflict", 1.05)
    else:
        _apply_focus_weight(weights, "conflict", 1.12)
        _apply_focus_weight(weights, "pacing", 1.08)
        _apply_focus_weight(weights, "payoff", 1.05)

    profile = resolve_runtime_quality_profile(runtime_context)
    style_profile = profile["style_profile"]
    genre_profiles = profile["genre_profiles"]
    quality_preset = profile["quality_preset"]

    if style_profile == "low_ai_serial":
        _apply_focus_weight(weights, "conflict", 1.10)
        _apply_focus_weight(weights, "payoff", 1.08)
        _apply_focus_weight(weights, "cliffhanger", 1.12)
    elif style_profile == "low_ai_life":
        _apply_focus_weight(weights, "dialogue", 1.14)
        _apply_focus_weight(weights, "payoff", 1.08)
        _apply_focus_weight(weights, "cliffhanger", 0.94)
        _apply_focus_weight(weights, "outline", 1.04)
    elif style_profile == "urban_finance":
        _apply_focus_weight(weights, "rule_grounding", 1.10)
        _apply_focus_weight(weights, "dialogue", 1.08)
        _apply_focus_weight(weights, "conflict", 1.06)
    elif style_profile == "tech_xianxia":
        _apply_focus_weight(weights, "rule_grounding", 1.14)
        _apply_focus_weight(weights, "payoff", 1.08)
        _apply_focus_weight(weights, "outline", 1.06)
    elif style_profile == "light_humor":
        _apply_focus_weight(weights, "dialogue", 1.12)
        _apply_focus_weight(weights, "cliffhanger", 0.95)
        _apply_focus_weight(weights, "payoff", 1.05)
    elif style_profile == "era_plain":
        _apply_focus_weight(weights, "rule_grounding", 1.08)
        _apply_focus_weight(weights, "outline", 1.05)
        _apply_focus_weight(weights, "dialogue", 1.05)

    if "romance_slice_of_life" in genre_profiles:
        _apply_focus_weight(weights, "dialogue", 1.10)
        _apply_focus_weight(weights, "payoff", 1.08)
        _apply_focus_weight(weights, "cliffhanger", 0.95)
    if "suspense_mystery" in genre_profiles:
        _apply_focus_weight(weights, "conflict", 1.08)
        _apply_focus_weight(weights, "cliffhanger", 1.12)
        _apply_focus_weight(weights, "outline", 1.06)
    if "xianxia_fantasy" in genre_profiles:
        _apply_focus_weight(weights, "rule_grounding", 1.12)
        _apply_focus_weight(weights, "payoff", 1.08)
        _apply_focus_weight(weights, "outline", 1.06)
    if "science_fiction_tech" in genre_profiles:
        _apply_focus_weight(weights, "rule_grounding", 1.12)
        _apply_focus_weight(weights, "outline", 1.08)
        _apply_focus_weight(weights, "opening", 1.04)
    if "history_power" in genre_profiles:
        _apply_focus_weight(weights, "outline", 1.10)
        _apply_focus_weight(weights, "rule_grounding", 1.08)
        _apply_focus_weight(weights, "conflict", 1.06)

    if quality_preset == "immersive":
        _apply_focus_weight(weights, "dialogue", 1.10)
        _apply_focus_weight(weights, "rule_grounding", 1.08)
        _apply_focus_weight(weights, "pacing", 1.05)
    elif quality_preset == "plot_drive":
        _apply_focus_weight(weights, "conflict", 1.12)
        _apply_focus_weight(weights, "cliffhanger", 1.08)
        _apply_focus_weight(weights, "outline", 1.06)
    elif quality_preset == "emotion_drama":
        _apply_focus_weight(weights, "dialogue", 1.12)
        _apply_focus_weight(weights, "payoff", 1.10)
        _apply_focus_weight(weights, "conflict", 1.04)
    elif quality_preset == "clean_prose":
        _apply_focus_weight(weights, "pacing", 1.08)
        _apply_focus_weight(weights, "dialogue", 1.06)
        _apply_focus_weight(weights, "rule_grounding", 1.04)

    ranked_weights = sorted(weights.items(), key=lambda item: item[1], reverse=True)
    emphasized_focuses = [focus_area for focus_area, weight in ranked_weights if weight >= 1.08][:3]
    focus_labels = [QUALITY_FOCUS_LABELS.get(focus_area, focus_area) for focus_area in emphasized_focuses]

    profile_parts: list[str] = []
    visible_genres = [
        QUALITY_PROFILE_GENRE_LABELS.get(item, item)
        for item in genre_profiles
        if item and item != "default"
    ]
    if visible_genres:
        profile_parts.append("题材：" + " / ".join(visible_genres[:2]))
    if style_profile and style_profile != "default":
        profile_parts.append("风格：" + QUALITY_PROFILE_STYLE_LABELS.get(style_profile, style_profile))
    if quality_preset and quality_preset != "balanced":
        profile_parts.append("预设：" + QUALITY_PROFILE_PRESET_LABELS.get(quality_preset, quality_preset))

    profile_summary = ""
    if focus_labels:
        focus_text = " / ".join(focus_labels)
        if profile_parts:
            profile_summary = f"{' / '.join(profile_parts)}，当前更看重 {focus_text}。"
        else:
            profile_summary = f"当前画像更看重 {focus_text}。"
    elif profile_parts:
        profile_summary = " / ".join(profile_parts)

    return {
        "weights": weights,
        "focus_areas": emphasized_focuses,
        "focus_labels": focus_labels,
        "summary": profile_summary,
        "genre_profiles": genre_profiles,
        "style_profile": style_profile,
        "quality_preset": quality_preset,
    }


novel_quality_profile_service = NovelQualityProfileService()


def _as_text(value: Any) -> str:
    if value is None:
        return ""
    return str(value).strip()


def _first_text(payload: Mapping[str, Any], *keys: str) -> str:
    for key in keys:
        value = payload.get(key)
        text = _as_text(value)
        if text:
            return text
    return ""


def _clip_text(value: Optional[str], limit: int) -> str:
    text = _as_text(value)
    if not text:
        return ""
    compact = " ".join(text.replace("\r", " ").replace("\n", " ").split())
    return compact[:limit]


def _coerce_asset_inputs(raw_assets: Any) -> Tuple[NovelQualityAssetInput, ...]:
    assets = []
    if isinstance(raw_assets, Mapping):
        raw_assets = [raw_assets]
    if not isinstance(raw_assets, (list, tuple)):
        return ()

    for item in raw_assets:
        if isinstance(item, NovelQualityAssetInput):
            assets.append(item)
        elif isinstance(item, Mapping):
            assets.append(NovelQualityAssetInput.from_mapping(item))
    return tuple(assets)


def _unique_non_empty(lines: Iterable[str]) -> Tuple[str, ...]:
    seen = set()
    ordered = []
    for line in lines:
        text = _as_text(line)
        if not text or text in seen:
            continue
        seen.add(text)
        ordered.append(text)
    return tuple(ordered)
