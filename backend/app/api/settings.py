"""
设置管理 API
"""
from fastapi import APIRouter, HTTPException, Request, Depends
from sqlalchemy.ext.asyncio import AsyncSession
from sqlalchemy import select
from typing import Dict, Any, List, Optional
from pathlib import Path
from pydantic import BaseModel
from datetime import datetime
import hashlib
import httpx
import json
import os
import time
from urllib.parse import urlsplit, urlunsplit

from app.database import get_db
from app.models.settings import Settings
from app.schemas.settings import (
    SettingsCreate, SettingsUpdate, SettingsResponse,
    APIKeyPreset, APIKeyPresetConfig, PresetCreateRequest,
    PresetUpdateRequest, PresetResponse, PresetListResponse,
    FetchModelsRequest, FetchModelsResponse, FetchedModel
)
from app.user_manager import User
from app.api.common import require_request_user
from app.logger import get_logger
from app.config import settings as app_settings, PROJECT_ROOT
from app.services.ai_service import AIService, create_user_ai_service, create_user_ai_service_with_mcp
from app.services.ai_config import AIClientConfig, HTTPClientConfig, RetryConfig, RateLimitConfig
from app.services.chapter_web_research_service import chapter_web_research_service

logger = get_logger(__name__)

router = APIRouter(prefix="/settings", tags=["设置管理"])

PLACEHOLDER_API_KEYS = {
    "your_openai_api_key_here",
    "your_anthropic_api_key_here",
    "your_gemini_api_key_here",
    "your_api_key_here",
}

WEB_RESEARCH_PREF_KEY = "web_research"
WEB_RESEARCH_DEFAULTS = {
    "web_research_enabled": False,
    "web_research_exa_enabled": True,
    "web_research_grok_enabled": True,
    "web_research_exa_api_key": "",
    "web_research_exa_base_url": "",
    "web_research_grok_api_key": "",
    "web_research_grok_base_url": "",
    "web_research_grok_model": "grok-4.1-fast",
    "web_research_grok_search_enabled": False,
}


PROBE_CACHE_TTL_SECONDS = 90
DEFAULT_PROBE_READ_TIMEOUT_SECONDS = 10.0
_probe_result_cache: Dict[str, Dict[str, Any]] = {}
RESPONSES_TEXT_PROBE_PROVIDERS = {'sub2api', 'openai_responses'}
OPENAI_COMPATIBLE_V1_PROBE_PROVIDERS = {'openai', 'openai_responses', 'newapi', 'custom', 'sub2api'}


def clear_probe_result_cache() -> None:
    _probe_result_cache.clear()


def _hash_probe_secret(value: str) -> str:
    normalized = str(value or "").strip()
    if not normalized:
        return ""
    return hashlib.sha256(normalized.encode("utf-8")).hexdigest()[:16]


def _build_probe_cache_key(
    probe_kind: str,
    *,
    api_key: str,
    api_base_url: str,
    provider: str,
    llm_model: str,
    temperature: Optional[float] = None,
    max_tokens: Optional[int] = None,
    backup_urls: Optional[List[str]] = None,
    fallback_strategy: Optional[str] = None,
) -> str:
    payload = {
        "kind": probe_kind,
        "api_key_hash": _hash_probe_secret(api_key),
        "api_base_url": str(api_base_url or "").strip().rstrip("/"),
        "provider": str(provider or "").strip().lower(),
        "llm_model": str(llm_model or "").strip(),
        "temperature": temperature,
        "max_tokens": max_tokens,
        "backup_urls": [str(url or "").strip().rstrip("/") for url in (backup_urls or []) if str(url or "").strip()],
        "fallback_strategy": str(fallback_strategy or "auto").strip().lower() or "auto",
    }
    serialized = json.dumps(payload, ensure_ascii=False, sort_keys=True, separators=(",", ":"))
    return hashlib.sha256(serialized.encode("utf-8")).hexdigest()


def _clone_probe_result(result: Dict[str, Any]) -> Dict[str, Any]:
    return json.loads(json.dumps(result, ensure_ascii=False))


def _mark_probe_result_cached(result: Dict[str, Any], age_seconds: float) -> Dict[str, Any]:
    cloned = _clone_probe_result(result)
    cloned["cached"] = True
    cloned["cache_age_ms"] = round(max(age_seconds, 0.0) * 1000, 2)
    details = cloned.get("details")
    if isinstance(details, dict):
        details["cached"] = True
        details["cache_age_ms"] = cloned["cache_age_ms"]
    return cloned


def _get_cached_probe_result(cache_key: str) -> Optional[Dict[str, Any]]:
    record = _probe_result_cache.get(cache_key)
    if not isinstance(record, dict):
        return None
    cached_at = float(record.get("cached_at") or 0.0)
    age_seconds = time.time() - cached_at
    if age_seconds > PROBE_CACHE_TTL_SECONDS:
        _probe_result_cache.pop(cache_key, None)
        return None
    payload = record.get("payload")
    if not isinstance(payload, dict):
        _probe_result_cache.pop(cache_key, None)
        return None
    return _mark_probe_result_cached(payload, age_seconds)


def _should_cache_probe_result(payload: Dict[str, Any]) -> bool:
    if not isinstance(payload, dict):
        return False

    if payload.get("success") is True:
        return True

    if payload.get("supported") is None:
        return False

    error_type = str(payload.get("error_type") or "").strip()
    http_status_raw = payload.get("http_status")
    try:
        http_status = int(http_status_raw) if http_status_raw is not None else None
    except (TypeError, ValueError):
        http_status = None

    if error_type in {"TimeoutError", "ReadTimeout", "ConnectTimeout", "PoolTimeout"}:
        return False

    if error_type == "HTTPStatusError" and http_status in {429}:
        return False

    if error_type == "HTTPStatusError" and http_status is not None and http_status >= 500:
        return False

    return True


def _store_probe_result(cache_key: str, payload: Dict[str, Any]) -> Dict[str, Any]:
    cloned = _clone_probe_result(payload)
    cloned["cached"] = False
    details = cloned.get("details")
    if isinstance(details, dict):
        details.setdefault("cached", False)

    if _should_cache_probe_result(cloned):
        _probe_result_cache[cache_key] = {"cached_at": time.time(), "payload": cloned}
    else:
        _probe_result_cache.pop(cache_key, None)

    return _clone_probe_result(cloned)


def normalize_env_api_key(api_key: Optional[str]) -> str:
    """Treat example API keys as empty values."""
    if not api_key:
        return ""

    normalized = api_key.strip()
    if normalized.lower() in PLACEHOLDER_API_KEYS:
        return ""

    return normalized


def read_env_defaults() -> Dict[str, Any]:
    """从.env文件读取默认配置（仅读取，不修改）"""
    return {
        "api_provider": app_settings.default_ai_provider,
        "api_key": (
            normalize_env_api_key(app_settings.openai_api_key)
            or normalize_env_api_key(app_settings.anthropic_api_key)
            or normalize_env_api_key(app_settings.gemini_api_key)
            or ""
        ),
        "api_base_url": app_settings.openai_base_url or app_settings.anthropic_base_url or "",
        "llm_model": app_settings.default_model,
        "temperature": app_settings.default_temperature,
        "max_tokens": app_settings.default_max_tokens,
    }



async def _load_user_settings_or_none(user: User, db: AsyncSession) -> Optional[Settings]:
    result = await db.execute(
        select(Settings).where(Settings.user_id == user.user_id)
    )
    return result.scalar_one_or_none()


def _provider_default_model(provider: Optional[str]) -> str:
    normalized = str(provider or '').strip().lower()
    if normalized == 'anthropic':
        return 'claude-3-5-sonnet-latest'
    if normalized == 'gemini':
        return 'gemini-2.5-pro'
    return str(app_settings.default_model or 'gpt-4o-mini').strip() or 'gpt-4o-mini'


def _normalize_settings_write_payload(payload: Dict[str, Any], existing_settings: Optional[Settings] = None) -> Dict[str, Any]:
    normalized = dict(payload)

    provider = str(normalized.get('api_provider') or normalized.get('provider_type') or getattr(existing_settings, 'api_provider', '') or 'openai').strip().lower()
    normalized['api_provider'] = provider
    normalized['provider_type'] = provider

    if 'api_key' in normalized:
        incoming_api_key = str(normalized.get('api_key') or '').strip()
        if incoming_api_key:
            normalized['api_key'] = incoming_api_key
        else:
            # 前端常会把已保存的 key 以空值或掩码回填到表单里；
            # 更新设置时若没有提交新 key，应保留数据库中的真实值。
            if existing_settings is not None and str(existing_settings.api_key or '').strip():
                normalized['api_key'] = str(existing_settings.api_key or '').strip()
            else:
                normalized.pop('api_key', None)

    if 'api_base_url' in normalized:
        normalized['api_base_url'] = str(normalized.get('api_base_url') or '').strip()

    if 'llm_model' in normalized:
        llm_model = str(normalized.get('llm_model') or '').strip()
        normalized['llm_model'] = llm_model or _provider_default_model(provider)

    return normalized


async def _resolve_probe_credentials(
    *,
    user: User,
    db: AsyncSession,
    api_key: Optional[str],
    api_base_url: Optional[str],
    provider: Optional[str],
    llm_model: Optional[str],
) -> Dict[str, str]:
    resolved_api_key = str(api_key or '').strip()
    resolved_api_base_url = str(api_base_url or '').strip()
    resolved_provider = str(provider or '').strip().lower() or 'openai'
    resolved_llm_model = str(llm_model or '').strip()

    stored_settings = None
    if not resolved_api_key or not resolved_api_base_url or not resolved_llm_model:
        stored_settings = await _load_user_settings_or_none(user, db)

    if stored_settings is not None:
        if not resolved_api_key:
            resolved_api_key = str(stored_settings.api_key or '').strip()
        if not resolved_api_base_url:
            resolved_api_base_url = str(stored_settings.api_base_url or '').strip()
        if not resolved_llm_model:
            resolved_llm_model = str(stored_settings.llm_model or '').strip()
        if not provider:
            resolved_provider = str(stored_settings.api_provider or resolved_provider or 'openai').strip().lower() or 'openai'

    if not resolved_llm_model:
        resolved_llm_model = _provider_default_model(resolved_provider)

    return {
        'api_key': resolved_api_key,
        'api_base_url': resolved_api_base_url,
        'provider': resolved_provider,
        'llm_model': resolved_llm_model,
    }
def _should_route_probe_via_chat_completions(provider: str, api_base_url: str) -> bool:
    normalized_provider = str(provider or "").strip().lower()
    normalized_base_url = str(api_base_url or "").strip().rstrip("/")
    return bool(normalized_base_url) and normalized_provider in OPENAI_COMPATIBLE_V1_PROBE_PROVIDERS


def _should_prefer_normalized_v1_probe_candidate(provider: str, api_base_url: str) -> bool:
    normalized_provider = str(provider or "").strip().lower()
    normalized_base_url = str(api_base_url or "").strip().rstrip("/")
    return (
        bool(normalized_base_url)
        and normalized_provider in OPENAI_COMPATIBLE_V1_PROBE_PROVIDERS
        and not normalized_base_url.endswith("/v1")
    )


def _is_running_in_docker_environment() -> bool:
    return os.path.exists("/.dockerenv")


def _is_local_gateway_host(hostname: Optional[str]) -> bool:
    return hostname in {"127.0.0.1", "localhost", "host.docker.internal"}


def _replace_base_url_host(base_url: str, hostname: str) -> str:
    parsed = urlsplit(str(base_url or "").strip())

    auth = ""
    if parsed.username:
        auth = parsed.username
        if parsed.password:
            auth = f"{auth}:{parsed.password}"
        auth = f"{auth}@"

    port = f":{parsed.port}" if parsed.port is not None else ""
    netloc = f"{auth}{hostname}{port}"
    return urlunsplit(parsed._replace(netloc=netloc))


def _maybe_map_loopback_base_url_to_docker_host(base_url: str) -> Optional[str]:
    if not _is_running_in_docker_environment():
        return None

    parsed = urlsplit(str(base_url or "").strip())
    if parsed.hostname not in {"127.0.0.1", "localhost"}:
        return None

    return _replace_base_url_host(base_url, "host.docker.internal")


def _maybe_map_local_https_base_url_to_http(base_url: str) -> Optional[str]:
    parsed = urlsplit(str(base_url or "").strip())
    if parsed.scheme != "https" or not _is_local_gateway_host(parsed.hostname):
        return None
    return urlunsplit(parsed._replace(scheme="http"))


def _expand_openai_probe_base_url_candidates(base_urls: List[str]) -> List[str]:
    unique_urls: List[str] = []
    for base_url in base_urls:
        variants = [base_url]
        docker_variant = _maybe_map_loopback_base_url_to_docker_host(base_url)
        if docker_variant:
            variants.append(docker_variant)

        for variant in list(variants):
            http_variant = _maybe_map_local_https_base_url_to_http(variant)
            if http_variant:
                variants.append(http_variant)

        for candidate in variants:
            normalized = str(candidate or "").strip().rstrip("/")
            if normalized and normalized not in unique_urls:
                unique_urls.append(normalized)
    return unique_urls


def _build_api_connection_probe_request_options(provider: str, api_base_url: str) -> Optional[Dict[str, Any]]:
    request_options: Dict[str, Any] = {
        "transport_max_retries": 1,
        "read_timeout": DEFAULT_PROBE_READ_TIMEOUT_SECONDS,
    }
    if _should_route_probe_via_chat_completions(provider, api_base_url):
        request_options["prefer_chat_completions"] = True
    if _should_prefer_normalized_v1_probe_candidate(provider, api_base_url):
        request_options["prefer_normalized_v1_candidate"] = True
    return request_options


def _build_function_calling_probe_request_options(provider: str, api_base_url: str) -> Optional[Dict[str, Any]]:
    request_options: Dict[str, Any] = {
        "transport_max_retries": 1,
        "read_timeout": DEFAULT_PROBE_READ_TIMEOUT_SECONDS,
    }
    if _should_route_probe_via_chat_completions(provider, api_base_url):
        request_options["prefer_chat_completions"] = True
    if _should_prefer_normalized_v1_probe_candidate(provider, api_base_url):
        request_options["prefer_normalized_v1_candidate"] = True
    return request_options


def build_probe_ai_config() -> AIClientConfig:
    """Build a lightweight client config for fast settings probes."""
    return AIClientConfig(
        http=HTTPClientConfig(
            connect_timeout=8.0,
            read_timeout=20.0,
            write_timeout=8.0,
            pool_timeout=8.0,
            max_keepalive_connections=10,
            max_connections=20,
            keepalive_expiry=30.0,
        ),
        retry=RetryConfig(
            max_retries=1,
            base_delay=0.1,
            max_delay=0.5,
            exponential_base=2,
        ),
        rate_limit=RateLimitConfig(
            max_concurrent_requests=1,
            request_delay=0.0,
        ),
    )


def _normalize_probe_backup_urls(value: Optional[List[str]]) -> List[str]:
    normalized_urls: List[str] = []
    for item in value or []:
        normalized = str(item or "").strip().rstrip("/")
        if normalized and normalized not in normalized_urls:
            normalized_urls.append(normalized)
    return normalized_urls


def _build_probe_endpoint_diagnostics(
    *,
    api_base_url: str,
    backup_urls: Optional[List[str]],
    fallback_strategy: Optional[str],
) -> Dict[str, Any]:
    normalized_primary = str(api_base_url or "").strip().rstrip("/")
    normalized_backups = _normalize_probe_backup_urls(backup_urls)
    normalized_strategy = str(fallback_strategy or "auto").strip().lower() or "auto"
    return {
        "primary_endpoint": normalized_primary,
        "backup_endpoints": normalized_backups,
        "configured_endpoint_count": (1 if normalized_primary else 0) + len(normalized_backups),
        "fallback_strategy": normalized_strategy,
        "auto_failover_enabled": normalized_strategy == "auto" and bool(normalized_backups),
    }


def _extract_probe_transport_diagnostics(service: Optional[Any], provider: str) -> Optional[Dict[str, Any]]:
    if service is None or not hasattr(service, "get_transport_diagnostics"):
        return None
    try:
        diagnostics = service.get_transport_diagnostics(provider)
    except Exception as exc:
        logger.warning("Failed to collect probe transport diagnostics for provider %s: %s", provider, exc)
        return None
    return diagnostics if isinstance(diagnostics, dict) and diagnostics else None


def _build_probe_details(
    *,
    api_base_url: str,
    backup_urls: Optional[List[str]],
    fallback_strategy: Optional[str],
    transport_diagnostics: Optional[Dict[str, Any]] = None,
    extra: Optional[Dict[str, Any]] = None,
) -> Dict[str, Any]:
    details = dict(extra or {})
    details["endpoint_diagnostics"] = _build_probe_endpoint_diagnostics(
        api_base_url=api_base_url,
        backup_urls=backup_urls,
        fallback_strategy=fallback_strategy,
    )
    if transport_diagnostics:
        details["transport_diagnostics"] = transport_diagnostics
    return details

def _build_api_probe_exception_suggestions(
    exception: Exception,
    *,
    api_base_url: str,
    backup_urls: Optional[List[str]],
    fallback_strategy: Optional[str],
) -> List[str]:
    error_msg = str(exception)
    lowered = error_msg.lower()
    error_type = type(exception).__name__
    status_code = exception.response.status_code if isinstance(exception, httpx.HTTPStatusError) and exception.response is not None else None
    normalized_base_url = str(api_base_url or "").strip().rstrip("/")
    normalized_backups = _normalize_probe_backup_urls(backup_urls)
    normalized_strategy = str(fallback_strategy or "auto").strip().lower() or "auto"
    auto_failover_enabled = normalized_strategy == "auto" and bool(normalized_backups)
    is_local_gateway = (
        normalized_base_url.startswith("http://127.0.0.1")
        or normalized_base_url.startswith("http://localhost")
        or normalized_base_url.startswith("https://127.0.0.1")
        or normalized_base_url.startswith("https://localhost")
    )

    if "blocked" in lowered:
        return [
            "The upstream API request was blocked or rejected",
            "Check whether the API key has permission for the target model",
            "Confirm the API key is bound to the expected proxy or gateway",
            "Verify the API base URL and gateway policy are consistent",
        ]

    if "unauthorized" in lowered or "401" in error_msg:
        return [
            "API key authentication failed",
            "Check whether the API key is correct and active",
            "Confirm the API key has sufficient permission",
        ]

    if "not found" in lowered or "404" in error_msg:
        return [
            "The API endpoint or model could not be found",
            "Confirm the API base URL is correct",
            "Verify the target model exists on the current service",
        ]

    if "rate limit" in lowered or "429" in error_msg:
        return [
            "The API request hit a rate limit",
            "Retry later after the rate limit window resets",
            "Consider reducing concurrency or switching to a backup endpoint",
        ]

    if "insufficient" in lowered or "quota" in lowered:
        return [
            "The API quota appears to be exhausted",
            "Check the account balance or quota usage",
            "Confirm the current key is allowed to use this model",
        ]

    if status_code in {502, 503, 504} or any(code in error_msg for code in ["502", "503", "504"]):
        suggestions = []
        if is_local_gateway:
            suggestions.append("The local gateway or proxy is reachable, but it failed to forward the model request upstream")
            suggestions.append("Check the local gateway logs and verify its upstream provider configuration for /chat/completions or /responses")
        else:
            suggestions.append("The upstream gateway or proxy returned a server error while processing the request")
            suggestions.append("Check whether the current API gateway can reach its model provider and whether the target model is healthy")

        if auto_failover_enabled:
            suggestions.append("Retry the request and inspect transport diagnostics to confirm whether failover was attempted")
        else:
            suggestions.append("Configure at least one backup endpoint and keep fallback strategy as auto if you want automatic failover")

        return suggestions

    if "non-json" in lowered or "non json" in lowered or "doctype html" in lowered:
        suggestions = [
            "The configured Base URL returned an HTML page, not an API JSON response",
            "Use the provider's API root instead of its web console or homepage",
            "For DeepSeek-compatible Chat Completions, try a documented endpoint such as `https://api.deepseek.com/v1` or the gateway's exact `/v1` API base path",
            "If this gateway requires a vendor-specific path, copy the complete API Base URL from the gateway documentation",
        ]
        if normalized_base_url and not normalized_base_url.endswith("/v1"):
            suggestions.insert(1, "The current Base URL does not end with `/v1`; configure the exact API base path from the gateway instead of relying on the homepage root")
        return suggestions

    parsed_base_url = urlsplit(normalized_base_url) if normalized_base_url else None
    base_url_hostname = parsed_base_url.hostname if parsed_base_url else None
    if error_type in {"TimeoutError", "ReadTimeout", "ConnectTimeout", "PoolTimeout", "ConnectError"}:
        suggestions = [
            "The API endpoint did not respond in time or could not be reached",
            "Check the network path, API base URL, and gateway process status",
        ]

        if base_url_hostname == "host.docker.internal":
            if _is_running_in_docker_environment():
                suggestions.append("The current backend appears to run inside Docker; confirm the host machine is exposing the gateway on the configured port")
            else:
                suggestions.append("`host.docker.internal` usually only works from inside Docker Desktop containers; if this backend runs on the host OS, switch the API base URL to `http://127.0.0.1:<port>` or `http://localhost:<port>`")
        elif is_local_gateway:
            suggestions.append("If this is a local gateway, verify the gateway process is listening and can answer /chat/completions on the configured port")

        if auto_failover_enabled:
            suggestions.append("Retry after checking transport diagnostics to confirm whether backup endpoint failover was attempted")
        else:
            suggestions.append("Configure at least one backup endpoint and keep fallback strategy as auto if you want automatic failover")

        return suggestions

    return [
        "An unknown error occurred during the request",
        "Check the network and configuration parameters",
        "Review the detailed error message for more clues",
    ]


def build_default_web_research_settings() -> Dict[str, Any]:
    return {
        **WEB_RESEARCH_DEFAULTS,
        "web_research_enabled": bool(app_settings.pre_generation_web_research_enabled),
        "web_research_exa_enabled": bool(app_settings.pre_generation_web_research_exa_enabled),
        "web_research_grok_enabled": bool(app_settings.pre_generation_web_research_grok_enabled),
        "web_research_grok_search_enabled": bool(app_settings.pre_generation_web_research_grok_search_enabled),
    }


def load_settings_preferences(settings_obj: Optional[Settings]) -> Dict[str, Any]:
    if not settings_obj or not settings_obj.preferences:
        return {}
    try:
        value = json.loads(settings_obj.preferences)
        return value if isinstance(value, dict) else {}
    except json.JSONDecodeError:
        return {}


def extract_web_research_payload(preferences: Optional[Dict[str, Any]]) -> Dict[str, Any]:
    defaults = build_default_web_research_settings()
    source = preferences.get(WEB_RESEARCH_PREF_KEY) if isinstance(preferences, dict) else None
    if isinstance(source, dict):
        for key in list(defaults.keys()):
            if key in source and source[key] is not None:
                defaults[key] = source[key]
    return defaults


def pop_web_research_fields(payload: Dict[str, Any]) -> Dict[str, Any]:
    result: Dict[str, Any] = {}
    for key in list(WEB_RESEARCH_DEFAULTS.keys()):
        if key in payload:
            result[key] = payload.pop(key)
    return result


def merge_web_research_preferences(preferences: Dict[str, Any], values: Dict[str, Any]) -> Dict[str, Any]:
    merged = extract_web_research_payload(preferences)
    for key, value in values.items():
        if value is not None:
            merged[key] = value
    next_preferences = dict(preferences)
    next_preferences[WEB_RESEARCH_PREF_KEY] = merged
    return next_preferences


def serialize_settings_response(settings_obj: Settings) -> Dict[str, Any]:
    base = SettingsResponse.model_validate(settings_obj).model_dump()
    actual_api_key = str(getattr(settings_obj, "api_key", "") or "").strip()
    base["has_api_key"] = bool(actual_api_key)
    base["api_key"] = "********" if actual_api_key else ""
    base.update(extract_web_research_payload(load_settings_preferences(settings_obj)))
    return base

def require_login(request: Request) -> User:
    """依赖：要求用户已登录"""
    return require_request_user(request, "需要登录")


async def get_user_ai_service(
    user: User = Depends(require_login),
    db: AsyncSession = Depends(get_db)
) -> AIService:
    """
    依赖：获取当前用户的AI服务实例（支持MCP工具自动加载）
    
    从数据库读取用户设置并创建对应的AI服务。
    自动传递 user_id 和 db_session，使得 AIService 能够加载用户配置的MCP工具。
    根据用户的所有MCP插件状态决定是否启用MCP：如果有启用的插件则启用，否则禁用。
    """
    from app.models.mcp_plugin import MCPPlugin
    
    result = await db.execute(
        select(Settings).where(Settings.user_id == user.user_id)
    )
    settings = result.scalar_one_or_none()
    
    if not settings:
        # 如果用户没有设置，从.env读取并保存
        env_defaults = read_env_defaults()
        settings = Settings(
            user_id=user.user_id,
            **env_defaults
        )
        db.add(settings)
        await db.commit()
        await db.refresh(settings)
        logger.info(f"用户 {user.user_id} 首次使用AI服务，已从.env同步设置到数据库")
    
    # 查询用户的所有MCP插件状态
    mcp_result = await db.execute(
        select(MCPPlugin).where(MCPPlugin.user_id == user.user_id)
    )
    mcp_plugins = mcp_result.scalars().all()
    
    # 检查是否有启用的MCP插件
    enable_mcp = any(plugin.enabled for plugin in mcp_plugins) if mcp_plugins else False
    
    if mcp_plugins:
        enabled_count = sum(1 for p in mcp_plugins if p.enabled)
        logger.info(f"用户 {user.user_id} 有 {len(mcp_plugins)} 个MCP插件，{enabled_count} 个启用，{enable_mcp} 决定使用MCP")
    else:
        logger.debug(f"用户 {user.user_id} 没有配置MCP插件，禁用MCP")
    
    # 解析 backup_urls（数据库存储为 JSON 字符串）
    backup_urls = None
    if settings.api_backup_urls:
        try:
            backup_urls = json.loads(settings.api_backup_urls) if isinstance(settings.api_backup_urls, str) else settings.api_backup_urls
        except (json.JSONDecodeError, TypeError):
            logger.warning(f"用户 {user.user_id} 的 api_backup_urls 解析失败，忽略备用地址")

    # ✅ 使用支持MCP的工厂函数创建AI服务实例
    return create_user_ai_service_with_mcp(
        api_provider=settings.api_provider,
        api_key=settings.api_key,
        api_base_url=settings.api_base_url or "",
        model_name=settings.llm_model,
        temperature=settings.temperature,
        max_tokens=settings.max_tokens,
        user_id=user.user_id,
        db_session=db,
        system_prompt=settings.system_prompt,
        enable_mcp=enable_mcp,
        backup_urls=backup_urls,
        fallback_strategy=settings.fallback_strategy,
    )


@router.get("", response_model=SettingsResponse)
async def get_settings(
    user: User = Depends(require_login),
    db: AsyncSession = Depends(get_db)
):
    """
    获取当前用户的设置
    如果用户没有保存过设置，自动从.env创建并保存到数据库
    """
    result = await db.execute(
        select(Settings).where(Settings.user_id == user.user_id)
    )
    settings = result.scalar_one_or_none()
    
    if not settings:
        # 如果用户没有保存过设置，从.env读取默认配置并保存到数据库
        env_defaults = read_env_defaults()
        logger.info(f"用户 {user.user_id} 首次获取设置，自动从.env同步到数据库")
        
        # 创建新设置并保存到数据库
        settings = Settings(
            user_id=user.user_id,
            **env_defaults
        )
        db.add(settings)
        await db.commit()
        await db.refresh(settings)
        logger.info(f"用户 {user.user_id} 的设置已从.env同步到数据库")
    
    logger.info(f"用户 {user.user_id} 获取已保存的设置")
    return serialize_settings_response(settings)


@router.get("/api-key")
async def get_settings_api_key(
    user: User = Depends(require_login),
    db: AsyncSession = Depends(get_db)
):
    """?????????????? API Key?"""
    settings = await _load_user_settings_or_none(user, db)
    if not settings or not str(settings.api_key or "").strip():
        return {"api_key": "", "has_api_key": False}
    return {"api_key": str(settings.api_key or "").strip(), "has_api_key": True}


@router.post("", response_model=SettingsResponse)
async def save_settings(
    data: SettingsCreate,
    user: User = Depends(require_login),
    db: AsyncSession = Depends(get_db)
):
    """
    创建或更新当前用户的设置（Upsert）
    如果设置已存在则更新，否则创建新设置
    仅保存到数据库
    
    注意：手动保存配置后会自动取消之前激活的预设状态，
    因为手动修改的配置可能与预设不一致
    """
    # 查找现有设置
    result = await db.execute(
        select(Settings).where(Settings.user_id == user.user_id)
    )
    settings = result.scalar_one_or_none()
    
    # 准备数据
    settings_dict = _normalize_settings_write_payload(data.model_dump(exclude_unset=True), settings)
    web_research_values = pop_web_research_fields(settings_dict)

    # api_backup_urls 需要序列化为 JSON 字符串存储
    if 'api_backup_urls' in settings_dict and settings_dict['api_backup_urls'] is not None:
        settings_dict['api_backup_urls'] = json.dumps(settings_dict['api_backup_urls'], ensure_ascii=False)

    if settings:
        current_preferences = load_settings_preferences(settings)
        if web_research_values:
            settings.preferences = json.dumps(
                merge_web_research_preferences(current_preferences, web_research_values),
                ensure_ascii=False,
            )
        # 更新现有设置
        for key, value in settings_dict.items():
            setattr(settings, key, value)
        
        # 检查并取消预设激活状态
        # 因为用户手动修改了配置，可能与之前激活的预设不一致
        try:
            prefs = json.loads(settings.preferences or '{}')
            api_presets = prefs.get('api_presets', {'presets': [], 'version': '1.0'})
            presets = api_presets.get('presets', [])
            
            # 找到激活的预设并检查是否与当前保存的配置一致
            active_preset = next((p for p in presets if p.get('is_active')), None)
            if active_preset:
                preset_config = active_preset.get('config', {})
                # 检查配置是否发生变化
                config_changed = (
                    preset_config.get('api_provider') != settings_dict.get('api_provider', settings.api_provider) or
                    preset_config.get('api_key') != settings_dict.get('api_key', settings.api_key) or
                    preset_config.get('api_base_url') != settings_dict.get('api_base_url', settings.api_base_url) or
                    preset_config.get('llm_model') != settings_dict.get('llm_model', settings.llm_model) or
                    preset_config.get('temperature') != settings_dict.get('temperature', settings.temperature) or
                    preset_config.get('max_tokens') != settings_dict.get('max_tokens', settings.max_tokens)
                )
                
                if config_changed:
                    # 取消激活状态
                    active_preset['is_active'] = False
                    prefs['api_presets'] = api_presets
                    settings.preferences = json.dumps(prefs, ensure_ascii=False)
                    logger.info(f"用户 {user.user_id} 手动修改配置，已取消预设 {active_preset.get('name')} 的激活状态")
        except (json.JSONDecodeError, TypeError) as e:
            logger.warning(f"解析用户 {user.user_id} 的preferences失败: {e}")
        
        await db.commit()
        await db.refresh(settings)
        logger.info(f"用户 {user.user_id} 更新设置")
    else:
        # 创建新设置
        preferences = {}
        if web_research_values:
            preferences = merge_web_research_preferences(preferences, web_research_values)
        settings = Settings(
            user_id=user.user_id,
            preferences=json.dumps(preferences, ensure_ascii=False) if preferences else None,
            **settings_dict
        )
        db.add(settings)
        await db.commit()
        await db.refresh(settings)
        logger.info(f"用户 {user.user_id} 创建设置")
    
    return serialize_settings_response(settings)


@router.put("", response_model=SettingsResponse)
async def update_settings(
    data: SettingsUpdate,
    user: User = Depends(require_login),
    db: AsyncSession = Depends(get_db)
):
    """
    更新当前用户的设置
    仅保存到数据库
    """
    result = await db.execute(
        select(Settings).where(Settings.user_id == user.user_id)
    )
    settings = result.scalar_one_or_none()
    
    if not settings:
        raise HTTPException(status_code=404, detail="设置不存在，请先创建设置")
    
    # 更新设置
    update_data = _normalize_settings_write_payload(data.model_dump(exclude_unset=True), settings)
    web_research_values = pop_web_research_fields(update_data)

    # api_backup_urls 需要序列化为 JSON 字符串存储
    if 'api_backup_urls' in update_data and update_data['api_backup_urls'] is not None:
        update_data['api_backup_urls'] = json.dumps(update_data['api_backup_urls'], ensure_ascii=False)

    if web_research_values:
        settings.preferences = json.dumps(
            merge_web_research_preferences(load_settings_preferences(settings), web_research_values),
            ensure_ascii=False,
        )

    for key, value in update_data.items():
        setattr(settings, key, value)
    
    await db.commit()
    await db.refresh(settings)
    logger.info(f"用户 {user.user_id} 更新设置")
    
    return serialize_settings_response(settings)


@router.delete("")
async def delete_settings(
    user: User = Depends(require_login),
    db: AsyncSession = Depends(get_db)
):
    """
    删除当前用户的设置
    """
    result = await db.execute(
        select(Settings).where(Settings.user_id == user.user_id)
    )
    settings = result.scalar_one_or_none()
    
    if not settings:
        raise HTTPException(status_code=404, detail="设置不存在")
    
    await db.delete(settings)
    await db.commit()
    logger.info(f"用户 {user.user_id} 删除设置")
    
    return {"message": "设置已删除", "user_id": user.user_id}


@router.get("/models")
async def get_available_models(
    api_key: str,
    api_base_url: str,
    provider: str = "openai"
):
    """
    从配置的 API 获取可用的模型列表

    Args:
        api_key: API 密钥
        api_base_url: API 基础 URL
        provider: API 提供商 (openai, anthropic, azure, newapi, custom)

    Returns:
        模型列表
    """
    api_key = (api_key or "").strip()
    api_base_url = (api_base_url or "").strip()
    provider = (provider or "openai").strip().lower()

    if not api_key:
        raise HTTPException(status_code=400, detail="请先填写 API Key，再获取模型列表")

    if not api_base_url:
        raise HTTPException(status_code=400, detail="请先填写 API Base URL，再获取模型列表")

    try:
        async with httpx.AsyncClient(timeout=10.0) as client:
            # OpenAI 兼容接口（包括 openai/openai_responses/azure/newapi/custom/sub2api）
            openai_compatible_providers = {"openai", "openai_responses", "azure", "newapi", "custom", "sub2api"}
            if provider in openai_compatible_providers:
                base_url = api_base_url.rstrip("/")
                candidate_base_urls = [base_url]
                if base_url.endswith("/v1"):
                    root_base = base_url[:-3].rstrip("/")
                    if root_base:
                        candidate_base_urls.append(root_base)
                else:
                    candidate_base_urls.append(f"{base_url}/v1")

                unique_urls = [
                    f"{candidate_base_url}/models"
                    for candidate_base_url in _expand_openai_probe_base_url_candidates(candidate_base_urls)
                ]
                # Azure 使用 api-key 头，其他使用 Bearer
                if provider == "azure":
                    headers = {
                        "api-key": api_key,
                        "Content-Type": "application/json"
                    }
                else:
                    headers = {
                        "Authorization": f"Bearer {api_key}",
                        "Content-Type": "application/json"
                    }

                logger.info(
                    f"正在获取模型列表 (provider: {provider}, candidates: {unique_urls})"
                )
                last_http_error: Optional[httpx.HTTPStatusError] = None
                last_network_error: Optional[Exception] = None

                for index, url in enumerate(unique_urls):
                    try:
                        response = await client.get(url, headers=headers)
                        response.raise_for_status()
                    except httpx.HTTPStatusError as e:
                        last_http_error = e
                        if provider == "azure" and e.response.status_code in [404, 403]:
                            # Azure 端点可能不支持 /models，返回友好提示
                            return {
                                "provider": provider,
                                "models": [],
                                "count": 0,
                                "message": "Azure OpenAI 无法自动获取模型列表，请手动填写部署名称到模型字段"
                            }
                        if e.response.status_code == 404 and index < len(unique_urls) - 1:
                            logger.warning(
                                f"模型列表端点不存在，尝试下一个候选地址: {url}"
                            )
                            continue
                        raise
                    except (httpx.ConnectError, httpx.TimeoutException) as e:
                        last_network_error = e
                        if index < len(unique_urls) - 1:
                            logger.warning(
                                f"模型列表候选地址连接失败，尝试下一个候选地址: {url}"
                            )
                            continue
                        raise


                    data = response.json()
                    models = []
                    raw_models = data.get("data", []) if isinstance(data, dict) else []
                    if not raw_models and isinstance(data, dict):
                        raw_models = data.get("models", [])

                    if isinstance(raw_models, list):
                        for model in raw_models:
                            if isinstance(model, str):
                                model_id = model
                                model_desc = ""
                            else:
                                model_id = (
                                    model.get("id")
                                    or model.get("name", "").replace("models/", "")
                                )
                                model_desc = (
                                    model.get("description", "")
                                    or model.get("display_name", "")
                                    or f"Created: {model.get('created', 'N/A')}"
                                )
                            if model_id:
                                models.append({
                                    "value": model_id,
                                    "label": model_id,
                                    "description": model_desc
                                })

                    if models:
                        logger.info(f"成功获取 {len(models)} 个模型")
                        return {
                            "provider": provider,
                            "models": models,
                            "count": len(models)
                        }

                    if index < len(unique_urls) - 1:
                        logger.warning(
                            f"当前端点未返回模型列表，尝试下一个候选地址: {url}"
                        )
                        continue

                if provider == "azure":
                    return {
                        "provider": provider,
                        "models": [],
                        "count": 0,
                        "message": "Azure OpenAI 无法自动获取模型列表，请手动填写部署名称到模型字段"
                    }

                if last_http_error:
                    raise last_http_error
                if last_network_error:
                    raise last_network_error

                raise HTTPException(
                    status_code=404,
                    detail="未能从 API 获取到可用的模型列表"
                )
                
            elif provider == "anthropic":
                # Anthropic models API
                url = f"{api_base_url.rstrip('/')}/v1/models"
                headers = {"x-api-key": api_key, "anthropic-version": "2023-06-01"}
                response = await client.get(url, headers=headers)
                response.raise_for_status()
                data = response.json()
                models = [{"value": m["id"], "label": m["id"], "description": m.get("display_name", "")} for m in data.get("data", [])]
                return {"provider": provider, "models": models, "count": len(models)}
            
            elif provider == "gemini":
                # Gemini models API
                url = f"{api_base_url.rstrip('/')}/models?key={api_key}"
                response = await client.get(url)
                response.raise_for_status()
                data = response.json()
                models = []
                for m in data.get("models", []):
                    if "generateContent" in m.get("supportedGenerationMethods", []):
                        mid = m.get("name", "").replace("models/", "")
                        models.append({"value": mid, "label": m.get("displayName", mid), "description": ""})
                return {"provider": provider, "models": models, "count": len(models)}
            
            else:
                raise HTTPException(status_code=400, detail=f"不支持的提供商: {provider}")
            
    except httpx.HTTPStatusError as e:
        logger.error(f"获取模型列表失败 (HTTP {e.response.status_code}): {e.response.text}")
        if e.response.status_code == 404:
            raise HTTPException(
                status_code=400,
                detail=f"该 API 提供商不支持模型列表查询接口 (/models 返回 404)，请手动输入模型名称。当前请求地址: {api_base_url.rstrip('/')}/models"
            )
        raise HTTPException(
            status_code=400,
            detail=f"无法从 API 获取模型列表 (HTTP {e.response.status_code})"
        )
    except httpx.RequestError as e:
        logger.error(f"请求模型列表失败: {str(e)}")
        raise HTTPException(
            status_code=400,
            detail=f"无法连接到 API: {str(e)}"
        )
    except HTTPException:
        raise
    except Exception as e:
        logger.error(f"获取模型列表时发生错误: {str(e)}")
        raise HTTPException(
            status_code=500,
            detail=f"获取模型列表失败: {str(e)}"
        )


class ApiTestRequest(BaseModel):
    """API 测试请求模型"""
    api_key: str
    api_base_url: str
    provider: str
    llm_model: str
    temperature: Optional[float] = None
    max_tokens: Optional[int] = None
    api_backup_urls: Optional[List[str]] = None
    fallback_strategy: Optional[str] = "auto"


class WebResearchTestRequest(BaseModel):
    """生成前网络检索测试请求模型"""

    provider: str
    exa_api_key: Optional[str] = None
    exa_base_url: Optional[str] = None
    grok_api_key: Optional[str] = None
    grok_base_url: Optional[str] = None
    grok_model: Optional[str] = None
    grok_search_enabled: Optional[bool] = None
    query: Optional[str] = None


@router.post("/check-function-calling")
async def check_function_calling_support(data: ApiTestRequest, user: User = Depends(require_login), db: AsyncSession = Depends(get_db)):
    """
    检查模型是否支持 Function Calling（工具调用）
    
    基于业界最佳实践的测试方法：
    1. 发送包含工具定义的请求
    2. 检查响应的 finish_reason 是否为 "tool_calls"
    3. 验证响应中是否包含有效的 tool_calls 数据
    
    Args:
        data: 包含 API 配置的请求数据
    
    Returns:
        检测结果包含支持状态、详细信息和建议
    """
    resolved = await _resolve_probe_credentials(
        user=user,
        db=db,
        api_key=data.api_key,
        api_base_url=data.api_base_url,
        provider=data.provider,
        llm_model=data.llm_model,
    )
    api_key = resolved["api_key"]
    api_base_url = resolved["api_base_url"]
    provider = resolved["provider"]
    llm_model = resolved["llm_model"]
    api_backup_urls = _normalize_probe_backup_urls(data.api_backup_urls)
    fallback_strategy = str(data.fallback_strategy or "auto").strip().lower() or "auto"
    cache_key = _build_probe_cache_key(
        "function_calling",
        api_key=api_key,
        api_base_url=api_base_url,
        provider=provider,
        llm_model=llm_model,
        backup_urls=api_backup_urls,
        fallback_strategy=fallback_strategy,
    )
    cached_result = _get_cached_probe_result(cache_key)
    if cached_result is not None:
        logger.info("使用缓存的 Function Calling 探测结果")
        return cached_result
    
    try:
        start_time = time.time()
        
        # 定义一个简单的测试工具（天气查询）
        test_tools = [{
            "type": "function",
            "function": {
                "name": "get_weather",
                "description": "获取指定城市的当前天气信息",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "city": {
                            "type": "string",
                            "description": "城市名称，例如：北京、上海、深圳"
                        },
                        "unit": {
                            "type": "string",
                            "enum": ["celsius", "fahrenheit"],
                            "description": "温度单位"
                        }
                    },
                    "required": ["city"]
                }
            }
        }]
        
        # Force a tool call instead of allowing a plain-text fallback.
        test_prompt = (
            "Do not explain or answer directly. "
            "Call the get_weather tool immediately for city=Beijing "
            "and unit=celsius."
        )
        
        logger.info("开始 Function Calling 探测")
        logger.info(f"  - 提供商: {provider}")
        logger.info(f"  - 模型: {llm_model}")
        logger.info("  - 测试工具: get_weather")
        
        probe_config = build_probe_ai_config()
        probe_max_tokens = 64
        probe_request_options = _build_function_calling_probe_request_options(provider, api_base_url)

        # Build a lightweight AI service for probing
        test_service = AIService(
            api_provider=provider,
            api_key=api_key,
            api_base_url=api_base_url,
            default_model=llm_model,
            default_temperature=0.3,
            default_max_tokens=probe_max_tokens,
            config=probe_config,
            backup_urls=api_backup_urls,
            fallback_strategy=fallback_strategy,
        )
        
        # Execute the probe with the real tool definition.
        response = await test_service.generate_text(
            prompt=test_prompt,
            provider=provider,
            model=llm_model,
            temperature=0.3,
            max_tokens=probe_max_tokens,
            tools=test_tools,
            tool_choice="required",
            auto_mcp=False,
            request_options=probe_request_options,
        )
        
        end_time = time.time()
        response_time = round((end_time - start_time) * 1000, 2)
        
        # 分析响应以确定是否支持 Function Calling
        supported = False
        finish_reason = None
        tool_calls = None
        response_content = None
        
        if isinstance(response, dict):
            # 检查 finish_reason（OpenAI 标准）
            finish_reason = response.get("finish_reason")
            
            # 检查是否有 tool_calls
            if "tool_calls" in response and response["tool_calls"]:
                supported = True
                tool_calls = response["tool_calls"]
                logger.info(f"✅ 检测到工具调用: {len(tool_calls)} 个")
            
            # 记录返回的内容（如果有）
            if "content" in response:
                response_content = response["content"]
        elif isinstance(response, str):
            # 如果只返回字符串，说明不支持工具调用
            response_content = response
        
        logger.info(f"  - 响应时间: {response_time}ms")
        logger.info(f"  - finish_reason: {finish_reason}")
        logger.info(f"  - 支持状态: {'✅ 支持' if supported else '❌ 不支持'}")
        
        transport_diagnostics = _extract_probe_transport_diagnostics(test_service, provider)

        # 构建检测结果
        result = {
            "success": True,
            "supported": supported,
            "message": "✅ 支持 Function Calling" if supported else "❌ 不支持 Function Calling",
            "response_time_ms": response_time,
            "provider": provider,
            "model": llm_model,
            "details": _build_probe_details(
                api_base_url=api_base_url,
                backup_urls=api_backup_urls,
                fallback_strategy=fallback_strategy,
                transport_diagnostics=transport_diagnostics,
                extra={
                    "finish_reason": finish_reason,
                    "has_tool_calls": bool(tool_calls),
                    "tool_call_count": len(tool_calls) if tool_calls else 0,
                    "test_tool": "get_weather",
                    "test_prompt": test_prompt,
                    "response_type": "tool_calls" if supported else "text",
                },
            )
        }
        
        # 添加工具调用详情
        if tool_calls:
            result["tool_calls"] = tool_calls
            result["suggestions"] = [
                "✅ 该模型支持 Function Calling，可以正常使用 MCP 插件",
                "建议：启用需要的 MCP 插件以扩展 AI 能力",
                "提示：测试成功检测到工具调用，模型能够正确解析和使用外部工具"
            ]
        else:
            result["response_preview"] = response_content[:200] if response_content else None
            result["suggestions"] = [
                "❌ 该模型不支持 Function Calling，无法使用 MCP 插件功能",
                "建议：更换支持工具调用的模型",
                "推荐模型：GPT-4 系列、GPT-4-turbo、Claude 3 Opus/Sonnet、Gemini 1.5 Pro 等",
                "说明：模型返回了文本回复而非工具调用，表明不支持该功能"
            ]
        
        return _store_probe_result(cache_key, result)
        
    except ValueError as e:
        error_msg = str(e)
        logger.error(f"❌ Function Calling 检测配置错误: {error_msg}")
        result = {
            "success": False,
            "supported": None,
            "message": "配置错误，暂时无法确认模型能力",
            "error": error_msg,
            "error_type": "ConfigurationError",
            "suggestions": [
                "请检查 API Key 是否正确",
                "请确认 API Base URL 格式是否正确",
                "请验证所选提供商与配置是否匹配"
            ],
            "details": {
                "endpoint_diagnostics": _build_probe_endpoint_diagnostics(
                    api_base_url=api_base_url,
                    backup_urls=api_backup_urls,
                    fallback_strategy=fallback_strategy,
                ),
            },
        }
        return _store_probe_result(cache_key, result)

    except httpx.HTTPStatusError as e:
        error_msg = str(e)
        status_code = e.response.status_code if e.response is not None else None

        logger.error(f"❌ Function Calling 检测遇到 HTTP 错误: {error_msg}")
        logger.error(f"  - HTTP 状态码: {status_code}")

        if status_code is not None and status_code >= 500:
            message = f"上游服务暂时不可用（HTTP {status_code}）"
            suggestions = [
                "检测请求已发出，但上游 API 服务返回了 5xx 错误",
                "建议：稍后重试，或检查代理/网关是否稳定",
                "提示：这类错误通常不能直接判定为模型不支持 Function Calling"
            ]
        elif status_code == 429:
            message = "请求过于频繁，暂时无法确认模型能力"
            suggestions = [
                "API 服务触发了限流，请稍后再试",
                "建议：检查当前账号配额、并发限制或代理限流策略",
                "提示：限流错误不能直接判定为模型不支持 Function Calling"
            ]
        elif status_code == 401:
            message = "认证失败，暂时无法确认模型能力"
            suggestions = [
                "API Key 认证失败",
                "请检查 API Key 是否正确且有效",
                "请确认 API Key 是否有足够的权限"
            ]
        elif status_code == 404:
            message = "接口地址或模型不可用，暂时无法确认模型能力"
            suggestions = [
                "请检查 API Base URL 是否正确",
                "请确认模型名称是否正确",
                "如果使用代理服务，请确认其支持当前接口路径"
            ]
        else:
            message = "检测失败，暂时无法确认模型能力"
            suggestions = [
                "请检查 API 服务返回的状态码和错误详情",
                "建议：确认代理、网关和模型路由配置是否正确",
                "提示：请求失败时不能直接判定为模型不支持 Function Calling"
            ]

        result = {
            "success": False,
            "supported": None,
            "message": message,
            "error": error_msg,
            "error_type": "HTTPStatusError",
            "http_status": status_code,
            "suggestions": suggestions,
            "details": {
                "endpoint_diagnostics": _build_probe_endpoint_diagnostics(
                    api_base_url=api_base_url,
                    backup_urls=api_backup_urls,
                    fallback_strategy=fallback_strategy,
                ),
            },
        }
        return _store_probe_result(cache_key, result)

    except TimeoutError as e:
        error_msg = str(e)
        logger.error(f"❌ Function Calling 检测超时: {error_msg}")
        result = {
            "success": False,
            "supported": None,
            "message": "检测超时",
            "error": error_msg,
            "error_type": "TimeoutError",
            "suggestions": [
                "请检查网络连接是否正常",
                "请确认 API 服务是否可访问",
                "建议：稍后重试或使用其他网络环境"
            ],
            "details": {
                "endpoint_diagnostics": _build_probe_endpoint_diagnostics(
                    api_base_url=api_base_url,
                    backup_urls=api_backup_urls,
                    fallback_strategy=fallback_strategy,
                ),
            },
        }
        return _store_probe_result(cache_key, result)
        
    except Exception as e:
        error_msg = str(e)
        error_type = type(e).__name__
        
        logger.error(f"❌ Function Calling 检测失败: {error_msg}")
        logger.error(f"  - 错误类型: {error_type}")
        
        # 智能分析错误原因
        suggestions = []
        if "tool" in error_msg.lower() or "function" in error_msg.lower():
            suggestions = [
                "该模型可能不支持 Function Calling 功能",
                "API 返回了与工具调用相关的错误",
                "建议：更换支持工具调用的模型或联系 API 提供商"
            ]
        elif "unauthorized" in error_msg.lower() or "401" in error_msg:
            suggestions = [
                "API Key 认证失败",
                "请检查 API Key 是否正确且有效",
                "请确认 API Key 是否有足够的权限"
            ]
        elif "not found" in error_msg.lower() or "404" in error_msg:
            suggestions = [
                "模型不存在或不可用",
                "请检查模型名称是否正确",
                "请确认该模型在当前 API 中是否可用"
            ]
        else:
            suggestions = [
                "检测过程中遇到未知错误",
                "建议：检查所有配置参数是否正确",
                "提示：查看详细错误信息以获取更多线索"
            ]
        
        result = {
            "success": False,
            "supported": None,
            "message": "Function Calling 检测失败，暂时无法确认模型能力",
            "error": error_msg,
            "error_type": error_type,
            "suggestions": suggestions,
            "details": {
                "endpoint_diagnostics": _build_probe_endpoint_diagnostics(
                    api_base_url=api_base_url,
                    backup_urls=api_backup_urls,
                    fallback_strategy=fallback_strategy,
                ),
            },
        }
        return _store_probe_result(cache_key, result)


@router.post("/test")
async def test_api_connection(data: ApiTestRequest, user: User = Depends(require_login), db: AsyncSession = Depends(get_db)):
    """
    Test API connectivity and basic text generation.

    Args:
        data: API probe payload with optional temperature and max_tokens.

    Returns:
        Probe result including latency, preview, and endpoint diagnostics.
    """
    resolved = await _resolve_probe_credentials(
        user=user,
        db=db,
        api_key=data.api_key,
        api_base_url=data.api_base_url,
        provider=data.provider,
        llm_model=data.llm_model,
    )
    api_key = resolved["api_key"]
    api_base_url = resolved["api_base_url"]
    provider = resolved["provider"]
    llm_model = resolved["llm_model"]
    api_backup_urls = _normalize_probe_backup_urls(data.api_backup_urls)
    fallback_strategy = str(data.fallback_strategy or "auto").strip().lower() or "auto"
    temperature = data.temperature if data.temperature is not None else 0.7
    max_tokens = data.max_tokens if data.max_tokens is not None else 2000
    probe_max_tokens = min(max_tokens, 64)
    cache_key = _build_probe_cache_key(
        "api_connection",
        api_key=api_key,
        api_base_url=api_base_url,
        provider=provider,
        llm_model=llm_model,
        temperature=temperature,
        max_tokens=probe_max_tokens,
        backup_urls=api_backup_urls,
        fallback_strategy=fallback_strategy,
    )
    cached_result = _get_cached_probe_result(cache_key)
    if cached_result is not None:
        logger.info("使用缓存的 API 连接探测结果")
        return cached_result

    test_service: Optional[AIService] = None

    try:
        start_time = time.time()

        probe_config = build_probe_ai_config()

        test_service = AIService(
            api_provider=provider,
            api_key=api_key,
            api_base_url=api_base_url,
            default_model=llm_model,
            default_temperature=temperature,
            default_max_tokens=probe_max_tokens,
            config=probe_config,
            backup_urls=api_backup_urls,
            fallback_strategy=fallback_strategy,
        )

        test_prompt = "Reply with exactly: TEST_OK"

        logger.info("开始 API 连接探测")
        logger.info(f"  - 提供商: {provider}")
        logger.info(f"  - 模型: {llm_model}")
        logger.info(f"  - Base URL: {api_base_url}")
        logger.info(f"  - Temperature: {temperature}")
        logger.info(f"  - Max Tokens: {max_tokens}")
        logger.info(f"  - 探测 Max Tokens: {probe_max_tokens}")

        probe_request_options = _build_api_connection_probe_request_options(provider, api_base_url)

        response = await test_service.generate_text(
            prompt=test_prompt,
            provider=provider,
            model=llm_model,
            temperature=temperature,
            max_tokens=probe_max_tokens,
            auto_mcp=False,
            request_options=probe_request_options,
        )

        end_time = time.time()
        response_time = round((end_time - start_time) * 1000, 2)

        logger.info(f"API 连接探测成功，耗时 {response_time}ms")

        response_str = str(response) if response else "N/A"
        logger.info(f"  - 响应预览: {response_str[:100]}")

        transport_diagnostics = _extract_probe_transport_diagnostics(test_service, provider)
        result = {
            "success": True,
            "message": "API 连接测试成功",
            "response_time_ms": response_time,
            "provider": provider,
            "model": llm_model,
            "response_preview": response_str[:100] if len(response_str) > 100 else response_str,
            "details": _build_probe_details(
                api_base_url=api_base_url,
                backup_urls=api_backup_urls,
                fallback_strategy=fallback_strategy,
                transport_diagnostics=transport_diagnostics,
                extra={
                    "api_available": True,
                    "model_accessible": True,
                    "response_valid": bool(response),
                    "temperature": temperature,
                    "max_tokens": max_tokens,
                    "probe_max_tokens": probe_max_tokens,
                },
            ),
        }
        return _store_probe_result(cache_key, result)

    except ValueError as e:
        error_msg = str(e)
        logger.error(f"API 配置错误: {error_msg}")
        transport_diagnostics = _extract_probe_transport_diagnostics(test_service, provider)
        result = {
            "success": False,
            "message": "API 配置错误",
            "error": error_msg,
            "error_type": "ConfigurationError",
            "suggestions": [
                "请检查 API Key 是否正确",
                "请确认 API Base URL 格式是否有效",
                "请验证所选提供商与当前配置是否匹配",
            ],
            "details": _build_probe_details(
                api_base_url=api_base_url,
                backup_urls=api_backup_urls,
                fallback_strategy=fallback_strategy,
                transport_diagnostics=transport_diagnostics,
            ),
        }
        return _store_probe_result(cache_key, result)

    except TimeoutError as e:
        error_msg = str(e)
        logger.error(f"API 请求超时: {error_msg}")
        transport_diagnostics = _extract_probe_transport_diagnostics(test_service, provider)
        result = {
            "success": False,
            "message": "API 请求超时",
            "error": error_msg,
            "error_type": "TimeoutError",
            "suggestions": [
                "请检查网络连接是否稳定",
                "请确认 API Base URL 可以正常访问",
                "如果代理较慢，请稍后重试或切换备用端点",
            ],
            "details": _build_probe_details(
                api_base_url=api_base_url,
                backup_urls=api_backup_urls,
                fallback_strategy=fallback_strategy,
                transport_diagnostics=transport_diagnostics,
            ),
        }
        return _store_probe_result(cache_key, result)

    except Exception as e:
        error_msg = str(e)
        error_type = type(e).__name__
        transport_diagnostics = _extract_probe_transport_diagnostics(test_service, provider)
        status_code = e.response.status_code if isinstance(e, httpx.HTTPStatusError) and e.response is not None else None

        logger.error(f"API 探测失败: {error_msg}")
        logger.error(f"  - 错误类型: {error_type}")

        result = {
            "success": False,
            "message": "API 测试失败",
            "error": error_msg,
            "error_type": error_type,
            "suggestions": _build_api_probe_exception_suggestions(
                e,
                api_base_url=api_base_url,
                backup_urls=api_backup_urls,
                fallback_strategy=fallback_strategy,
            ),
            "details": _build_probe_details(
                api_base_url=api_base_url,
                backup_urls=api_backup_urls,
                fallback_strategy=fallback_strategy,
                transport_diagnostics=transport_diagnostics,
                extra={
                    "http_status_code": status_code,
                } if status_code is not None else None,
            ),
        }
        return _store_probe_result(cache_key, result)

@router.post("/test-web-research")
async def test_web_research_connection(data: WebResearchTestRequest):
    """测试 Exa / Grok 检索配置是否可用。"""
    provider = (data.provider or "").strip().lower()
    if provider not in {"exa", "grok"}:
        raise HTTPException(status_code=400, detail="provider 仅支持 exa 或 grok")

    if provider == "exa" and not (data.exa_api_key or "").strip():
        raise HTTPException(status_code=400, detail="请填写 Exa API Key")
    if provider == "grok":
        if not (data.grok_api_key or "").strip():
            raise HTTPException(status_code=400, detail="请填写 Grok API Key")
        if not (data.grok_base_url or "").strip():
            raise HTTPException(status_code=400, detail="请填写 Grok Base URL")

    return await chapter_web_research_service.test_provider_connection(
        provider=provider,
        overrides={
            "enabled": True,
            "exa_enabled": provider == "exa",
            "grok_enabled": provider == "grok",
            "exa_api_key": data.exa_api_key,
            "exa_base_url": data.exa_base_url,
            "grok_api_key": data.grok_api_key,
            "grok_base_url": data.grok_base_url,
            "grok_model": data.grok_model,
            "grok_search_enabled": data.grok_search_enabled,
        },
        query=data.query,
    )


# ========== API配置预设管理（零数据库改动方案）==========

async def get_user_settings(user_id: str, db: AsyncSession) -> Settings:
    """获取用户settings，如果不存在则创建"""
    result = await db.execute(
        select(Settings).where(Settings.user_id == user_id)
    )
    settings = result.scalar_one_or_none()
    
    if not settings:
        # 创建默认设置
        env_defaults = read_env_defaults()
        settings = Settings(
            user_id=user_id,
            **env_defaults,
            preferences='{}'  # 初始化为空JSON
        )
        db.add(settings)
        await db.commit()
        await db.refresh(settings)
        logger.info(f"用户 {user_id} 首次访问，已创建默认设置")
    
    return settings


@router.get("/presets", response_model=PresetListResponse)
async def get_presets(
    user: User = Depends(require_login),
    db: AsyncSession = Depends(get_db)
):
    """
    获取所有API配置预设
    
    从preferences字段读取预设列表
    """
    settings = await get_user_settings(user.user_id, db)
    
    # 解析preferences
    try:
        prefs = json.loads(settings.preferences or '{}')
    except json.JSONDecodeError:
        logger.warning(f"用户 {user.user_id} 的preferences字段JSON格式错误，重置为空")
        prefs = {}
    
    api_presets = prefs.get('api_presets', {'presets': [], 'version': '1.0'})
    presets = api_presets.get('presets', [])
    
    # 找到激活的预设
    active_preset_id = next(
        (p['id'] for p in presets if p.get('is_active')),
        None
    )
    
    logger.info(f"用户 {user.user_id} 获取预设列表，共 {len(presets)} 个")
    
    return {
        "presets": presets,
        "total": len(presets),
        "active_preset_id": active_preset_id
    }


@router.post("/presets", response_model=PresetResponse)
async def create_preset(
    data: PresetCreateRequest,
    user: User = Depends(require_login),
    db: AsyncSession = Depends(get_db)
):
    """
    创建新预设
    
    将预设添加到preferences字段的JSON中
    """
    settings = await get_user_settings(user.user_id, db)
    
    # 解析preferences
    try:
        prefs = json.loads(settings.preferences or '{}')
    except json.JSONDecodeError:
        prefs = {}
    
    api_presets = prefs.get('api_presets', {'presets': [], 'version': '1.0'})
    presets = api_presets.get('presets', [])
    
    # 创建新预设
    new_preset = {
        "id": f"preset_{int(datetime.now().timestamp() * 1000)}",
        "name": data.name,
        "description": data.description,
        "is_active": False,
        "created_at": datetime.now().isoformat(),
        "config": data.config.model_dump()
    }
    
    presets.append(new_preset)
    
    # 保存回preferences
    api_presets['presets'] = presets
    prefs['api_presets'] = api_presets
    settings.preferences = json.dumps(prefs, ensure_ascii=False)
    
    await db.commit()
    
    logger.info(f"用户 {user.user_id} 创建预设: {data.name}")
    return new_preset


@router.put("/presets/{preset_id}", response_model=PresetResponse)
async def update_preset(
    preset_id: str,
    data: PresetUpdateRequest,
    user: User = Depends(require_login),
    db: AsyncSession = Depends(get_db)
):
    """
    更新预设
    
    在preferences字段的JSON中更新指定预设
    """
    settings = await get_user_settings(user.user_id, db)
    
    # 解析preferences
    try:
        prefs = json.loads(settings.preferences or '{}')
    except json.JSONDecodeError:
        raise HTTPException(status_code=500, detail="配置数据格式错误")
    
    api_presets = prefs.get('api_presets', {'presets': [], 'version': '1.0'})
    presets = api_presets.get('presets', [])
    
    # 找到并更新预设
    target_preset = next((p for p in presets if p['id'] == preset_id), None)
    if not target_preset:
        raise HTTPException(status_code=404, detail="预设不存在")
    
    # 更新字段
    if data.name is not None:
        target_preset['name'] = data.name
    if data.description is not None:
        target_preset['description'] = data.description
    if data.config is not None:
        target_preset['config'] = data.config.model_dump()
    
    # 保存回preferences
    prefs['api_presets'] = api_presets
    settings.preferences = json.dumps(prefs, ensure_ascii=False)
    
    await db.commit()
    
    logger.info(f"用户 {user.user_id} 更新预设: {preset_id}")
    return target_preset


@router.delete("/presets/{preset_id}")
async def delete_preset(
    preset_id: str,
    user: User = Depends(require_login),
    db: AsyncSession = Depends(get_db)
):
    """
    删除预设
    
    从preferences字段的JSON中删除指定预设
    """
    settings = await get_user_settings(user.user_id, db)
    
    # 解析preferences
    try:
        prefs = json.loads(settings.preferences or '{}')
    except json.JSONDecodeError:
        raise HTTPException(status_code=500, detail="配置数据格式错误")
    
    api_presets = prefs.get('api_presets', {'presets': [], 'version': '1.0'})
    presets = api_presets.get('presets', [])
    
    # 找到预设
    target_preset = next((p for p in presets if p['id'] == preset_id), None)
    if not target_preset:
        raise HTTPException(status_code=404, detail="预设不存在")
    
    # 检查是否是激活的预设
    if target_preset.get('is_active'):
        raise HTTPException(status_code=400, detail="无法删除激活中的预设，请先激活其他预设")
    
    # 删除预设
    presets = [p for p in presets if p['id'] != preset_id]
    
    # 保存回preferences
    api_presets['presets'] = presets
    prefs['api_presets'] = api_presets
    settings.preferences = json.dumps(prefs, ensure_ascii=False)
    
    await db.commit()
    
    logger.info(f"用户 {user.user_id} 删除预设: {preset_id}")
    return {"message": "预设已删除", "preset_id": preset_id}


@router.post("/presets/{preset_id}/activate")
async def activate_preset(
    preset_id: str,
    user: User = Depends(require_login),
    db: AsyncSession = Depends(get_db)
):
    """
    激活预设
    
    将预设的配置应用到Settings主字段
    """
    settings = await get_user_settings(user.user_id, db)
    
    # 解析preferences
    try:
        prefs = json.loads(settings.preferences or '{}')
    except json.JSONDecodeError:
        raise HTTPException(status_code=500, detail="配置数据格式错误")
    
    api_presets = prefs.get('api_presets', {'presets': [], 'version': '1.0'})
    presets = api_presets.get('presets', [])
    
    # 找到目标预设
    target_preset = next((p for p in presets if p['id'] == preset_id), None)
    if not target_preset:
        raise HTTPException(status_code=404, detail="预设不存在")
    
    # 应用配置到Settings主字段
    config = target_preset['config']
    settings.api_provider = config['api_provider']
    settings.api_key = config['api_key']
    settings.api_base_url = config.get('api_base_url')
    settings.llm_model = config['llm_model']
    settings.temperature = config['temperature']
    settings.max_tokens = config['max_tokens']
    settings.system_prompt = config.get('system_prompt')
    
    # 更新所有预设的is_active状态
    for preset in presets:
        preset['is_active'] = (preset['id'] == preset_id)
    
    # 保存回preferences
    prefs['api_presets'] = api_presets
    settings.preferences = json.dumps(prefs, ensure_ascii=False)
    
    await db.commit()
    
    logger.info(f"用户 {user.user_id} 激活预设: {target_preset['name']}")
    return {
        "message": "预设已激活",
        "preset_id": preset_id,
        "preset_name": target_preset['name']
    }


@router.post("/presets/{preset_id}/test")
async def test_preset(
    preset_id: str,
    user: User = Depends(require_login),
    db: AsyncSession = Depends(get_db)
):
    """
    测试预设的API连接
    """
    settings = await get_user_settings(user.user_id, db)
    
    # 解析preferences
    try:
        prefs = json.loads(settings.preferences or '{}')
    except json.JSONDecodeError:
        raise HTTPException(status_code=500, detail="配置数据格式错误")
    
    api_presets = prefs.get('api_presets', {'presets': [], 'version': '1.0'})
    presets = api_presets.get('presets', [])
    
    # 找到预设
    target_preset = next((p for p in presets if p['id'] == preset_id), None)
    if not target_preset:
        raise HTTPException(status_code=404, detail="预设不存在")
    
    # 使用现有的test_api_connection逻辑
    # 确保传递完整参数，与当前配置测试保持一致
    config = target_preset['config']
    test_request = ApiTestRequest(
        api_key=config['api_key'],
        api_base_url=config.get('api_base_url', ''),
        provider=config['api_provider'],
        llm_model=config['llm_model'],
        temperature=config.get('temperature'),   # 使用预设中的温度参数
        max_tokens=config.get('max_tokens')      # 使用预设中的最大tokens参数
    )
    
    logger.info(f"用户 {user.user_id} 测试预设: {target_preset['name']}")
    return await test_api_connection(test_request)


@router.post("/presets/from-current", response_model=PresetResponse)
async def create_preset_from_current(
    name: str,
    description: Optional[str] = None,
    user: User = Depends(require_login),
    db: AsyncSession = Depends(get_db)
):
    """
    从当前配置创建新预设
    
    快捷方式：将当前激活的配置保存为新预设
    """
    settings = await get_user_settings(user.user_id, db)
    
    # 从当前Settings主字段读取配置
    current_config = APIKeyPresetConfig(
        api_provider=settings.api_provider,
        api_key=settings.api_key,
        api_base_url=settings.api_base_url,
        llm_model=settings.llm_model,
        temperature=settings.temperature,
        max_tokens=settings.max_tokens,
        system_prompt=settings.system_prompt
    )
    
    # 创建预设
    create_request = PresetCreateRequest(
        name=name,
        description=description,
        config=current_config
    )
    
    logger.info(f"用户 {user.user_id} 从当前配置创建预设: {name}")
    return await create_preset(create_request, user, db)


# 已知的 Anthropic 协议兼容子路径后缀（按长度降序，最长前缀优先匹配）
KNOWN_COMPAT_SUFFIXES = [
    "/v1/chat/completions",
    "/chat/completions",
    "/v1/responses",
    "/responses",
    "/api/claudecode",
    "/api/anthropic",
    "/api/openai",
    "/apps/anthropic",
    "/api/coding",
    "/claudecode",
    "/anthropic",
    "/openai",
    "/step_plan",
    "/coding",
    "/claude",
]


def strip_compat_suffix(base_url: str) -> Optional[str]:
    """
    若 baseURL 以任一已知兼容子路径结尾，返回剥离后的剩余部分；否则 None

    依赖 KNOWN_COMPAT_SUFFIXES 按长度降序排列，确保最长前缀优先命中
    """
    for suffix in KNOWN_COMPAT_SUFFIXES:
        if base_url.endswith(suffix):
            return base_url[:-len(suffix)]
    return None


def _append_model_url_candidates(candidates: List[str], base_url: str) -> None:
    """按 OpenAI 兼容模型列表的常见位置追加候选 URL。"""
    normalized = (base_url or "").strip().rstrip("/")
    if not normalized or "://" not in normalized:
        return

    if normalized.endswith("/v1/models") or normalized.endswith("/models"):
        candidates.append(normalized)
        return

    if normalized.endswith("/v1"):
        root = normalized[:-3].rstrip("/")
        candidates.append(f"{normalized}/models")
        if root:
            candidates.append(f"{root}/models")
        return

    candidates.append(f"{normalized}/v1/models")
    candidates.append(f"{normalized}/models")


def build_model_url_candidates(base_url: str, models_url: Optional[str] = None) -> List[str]:
    """生成模型列表端点候选，兼容 cc-switch 的 OpenAI-compatible 探测方式。"""
    candidates: List[str] = []

    if models_url:
        explicit_url = models_url.strip().rstrip("/")
        if explicit_url:
            candidates.append(explicit_url)

    base_url_normalized = (base_url or "").strip().rstrip("/")
    _append_model_url_candidates(candidates, base_url_normalized)

    stripped = strip_compat_suffix(base_url_normalized)
    if stripped:
        _append_model_url_candidates(candidates, stripped)

    seen = set()
    unique_candidates = []
    for url in candidates:
        if url not in seen:
            seen.add(url)
            unique_candidates.append(url)
    return unique_candidates


def build_official_model_url_candidates(provider: str, base_url: str, models_url: Optional[str] = None) -> List[str]:
    """生成官方协议模型列表端点，保留 models_url 作为最高优先级。"""
    if provider == "gemini":
        candidates = []
        if models_url:
            candidates.append(models_url.strip().rstrip("/"))
        normalized = (base_url or "").strip().rstrip("/")
        if normalized:
            if normalized.endswith("/models"):
                candidates.append(normalized)
            else:
                candidates.append(f"{normalized}/models")

        seen = set()
        return [url for url in candidates if url and not (url in seen or seen.add(url))]

    if provider == "anthropic":
        candidates = []
        if models_url:
            candidates.append(models_url.strip().rstrip("/"))
        normalized = (base_url or "").strip().rstrip("/")
        if normalized:
            if normalized.endswith("/v1/models") or normalized.endswith("/models"):
                candidates.append(normalized)
            elif normalized.endswith("/v1"):
                candidates.append(f"{normalized}/models")
            else:
                candidates.append(f"{normalized}/v1/models")

        seen = set()
        return [url for url in candidates if url and not (url in seen or seen.add(url))]

    return build_model_url_candidates(base_url, models_url)


def parse_fetched_models(data_json: Any) -> List[FetchedModel]:
    """解析 OpenAI-compatible 与常见聚合站的模型列表响应。"""
    if isinstance(data_json, dict):
        models_data = data_json.get("data") or data_json.get("models")
    elif isinstance(data_json, list):
        models_data = data_json
    else:
        models_data = None

    if not isinstance(models_data, list):
        return []

    models: List[FetchedModel] = []
    seen = set()
    for model_item in models_data:
        owned_by = None
        if isinstance(model_item, str):
            model_id = model_item
        elif isinstance(model_item, dict):
            model_id = model_item.get("id") or model_item.get("model") or model_item.get("name")
            owned_by = model_item.get("owned_by") or model_item.get("owner") or model_item.get("provider")
            if isinstance(model_id, str) and model_id.startswith("models/"):
                model_id = model_id.removeprefix("models/")
        else:
            continue

        if model_id and model_id not in seen:
            seen.add(model_id)
            models.append(FetchedModel(id=model_id, owned_by=owned_by))
    return models


def build_fetch_models_headers(provider: str, api_key: str) -> Dict[str, str]:
    if provider == "anthropic":
        return {
            "x-api-key": api_key,
            "anthropic-version": "2023-06-01",
        }
    return {
        "Authorization": f"Bearer {api_key}",
        "Content-Type": "application/json",
    }


@router.post("/fetch-models", response_model=FetchModelsResponse)
async def fetch_models(
    data: FetchModelsRequest,
    user: User = Depends(require_login),
    db: AsyncSession = Depends(get_db)
):
    """
    获取 AI 提供商可用模型列表。

    优先使用官方模型列表接口，并兼容 OpenAI 风格网关、DeepSeek、
    OpenRouter、Anthropic 和 Gemini 的模型列表响应格式。
    """
    resolved = await _resolve_probe_credentials(
        user=user,
        db=db,
        api_key=data.api_key,
        api_base_url=data.api_base_url,
        provider=data.provider,
        llm_model=None,
    )
    api_key = resolved["api_key"]
    base_url = resolved["api_base_url"]
    provider = resolved["provider"]

    if not api_key:
        return FetchModelsResponse(
            success=False,
            message="缺少 API Key",
            error="Missing API Key",
            error_type="ValidationError"
        )

    if not base_url:
        return FetchModelsResponse(
            success=False,
            message="缺少 API Base URL",
            error="Missing Base URL",
            error_type="ValidationError"
        )

    unique_candidates = build_official_model_url_candidates(provider, base_url, data.models_url)

    logger.info(f"用户 {user.user_id} 获取模型列表")
    logger.info(f"  - 提供商: {provider}")
    logger.info(f"  - Base URL: {base_url}")
    logger.info(f"  - 候选端点: {unique_candidates}")

    last_error = None
    last_status_code = None

    async with httpx.AsyncClient(timeout=10.0) as client:
        for candidate_url in unique_candidates:
            try:
                logger.info(f"尝试获取模型列表: {candidate_url}")

                headers = build_fetch_models_headers(provider, api_key)
                params = {"key": api_key} if provider == "gemini" else None
                response = await client.get(candidate_url, headers=headers, params=params)
                response.raise_for_status()

                models = parse_fetched_models(response.json())
                if not models:
                    logger.warning(f"端点 {candidate_url} 未返回可用模型")
                    continue

                return FetchModelsResponse(
                    success=True,
                    models=models,
                    message=f"已获取 {len(models)} 个模型"
                )
            except httpx.HTTPStatusError as e:
                last_status_code = e.response.status_code
                last_error = f"HTTP {last_status_code}"
                logger.warning(f"端点 {candidate_url} 返回 HTTP {last_status_code}")

                if last_status_code in (401, 403):
                    return FetchModelsResponse(
                        success=False,
                        message="API Key 无效或权限不足",
                        error=f"HTTP {last_status_code}: {e.response.text[:200]}",
                        error_type="AuthenticationError"
                    )
            except httpx.TimeoutException:
                last_error = "请求超时"
                logger.warning(f"端点 {candidate_url} 请求超时")
            except Exception as e:
                last_error = str(e)
                logger.warning(f"端点 {candidate_url} 获取模型失败: {last_error}")

    logger.error(f"所有候选模型端点均失败: {last_error}")

    if last_status_code in (404, 405):
        return FetchModelsResponse(
            success=False,
            message="模型列表端点不存在或不支持",
            error=f"端点返回 HTTP {last_status_code}",
            error_type="EndpointNotFound"
        )

    if "timeout" in str(last_error).lower():
        return FetchModelsResponse(
            success=False,
            message="模型列表请求超时",
            error=last_error,
            error_type="TimeoutError"
        )

    return FetchModelsResponse(
        success=False,
        message="获取模型列表失败",
        error=last_error or "未返回可用模型",
        error_type="NetworkError"
    )
