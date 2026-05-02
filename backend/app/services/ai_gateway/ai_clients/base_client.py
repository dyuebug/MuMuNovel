"""AI 客户端基类"""
import asyncio
import hashlib
import ipaddress
import json
import time
from abc import ABC, abstractmethod
from typing import Any, AsyncGenerator, Dict, Optional, List
from urllib.parse import urlparse

import httpx

from app.logger import get_logger
from app.services.ai_gateway.ai_config import AIClientConfig, default_config

logger = get_logger(__name__)

# Shared HTTP client pool and semaphore pool keyed by concurrency.
_http_client_pool: Dict[str, httpx.AsyncClient] = {}
_semaphore_pool: Dict[int, asyncio.Semaphore] = {}


def _get_semaphore(max_concurrent: int) -> asyncio.Semaphore:
    """Return a shared semaphore keyed by max concurrency."""
    normalized_max_concurrent = max(1, int(max_concurrent or 1))
    semaphore = _semaphore_pool.get(normalized_max_concurrent)
    if semaphore is None:
        semaphore = asyncio.Semaphore(normalized_max_concurrent)
        _semaphore_pool[normalized_max_concurrent] = semaphore
    return semaphore


def _clone_transport_payload(payload: Dict[str, Any]) -> Dict[str, Any]:
    return json.loads(json.dumps(payload, ensure_ascii=False))


def _detect_api_mode(endpoint: str) -> Optional[str]:
    if endpoint == "/responses":
        return "responses"
    if endpoint == "/chat/completions":
        return "chat_completions"
    return None


class _RetriableStreamContext:
    def __init__(
        self,
        client: "BaseAIClient",
        method: str,
        endpoint: str,
        payload: Dict[str, Any],
        headers: Dict[str, str],
        endpoints: List[str],
        transport_max_retries: int,
        request_timeout: Optional[httpx.Timeout],
        retry_cfg: Any,
        rate_cfg: Any,
    ):
        self.client = client
        self.method = method
        self.endpoint = endpoint
        self.payload = payload
        self.headers = headers
        self.endpoints = endpoints
        self.transport_max_retries = transport_max_retries
        self.request_timeout = request_timeout
        self.retry_cfg = retry_cfg
        self.rate_cfg = rate_cfg
        self.last_exception: Optional[Exception] = None
        self._semaphore: Optional[asyncio.Semaphore] = None
        self._stream_context = None

    async def __aenter__(self):
        self._semaphore = _get_semaphore(self.rate_cfg.max_concurrent_requests)
        await self._semaphore.acquire()
        api_mode = _detect_api_mode(self.endpoint)

        try:
            await asyncio.sleep(self.rate_cfg.request_delay)

            for endpoint_index, base_url in enumerate(self.endpoints):
                url = f"{base_url}{self.endpoint}"

                for attempt in range(self.transport_max_retries):
                    stream_context = None
                    try:
                        if attempt > 0:
                            delay = min(
                                self.retry_cfg.base_delay * (self.retry_cfg.exponential_base ** attempt),
                                self.retry_cfg.max_delay,
                            )
                            logger.warning(
                                "Stream request retry %s/%s on endpoint %s/%s after %.2fs",
                                attempt + 1,
                                self.transport_max_retries,
                                endpoint_index + 1,
                                len(self.endpoints),
                                delay,
                            )
                            await asyncio.sleep(delay)

                        attempt_started_at = time.perf_counter()
                        request_kwargs = {"headers": self.headers, "json": self.payload}
                        if self.request_timeout is not None:
                            request_kwargs["timeout"] = self.request_timeout

                        stream_context = self.client.http_client.stream(
                            self.method,
                            url,
                            **request_kwargs,
                        )
                        response = await stream_context.__aenter__()
                        response.raise_for_status()

                        self.client._record_transport_attempt(
                            request_kind="stream",
                            api_mode=api_mode,
                            endpoint_path=self.endpoint,
                            endpoint_index=endpoint_index + 1,
                            endpoint_role="primary" if endpoint_index == 0 else "backup",
                            base_url=base_url,
                            request_url=url,
                            attempt_number=attempt + 1,
                            max_attempts=self.transport_max_retries,
                            duration_ms=round((time.perf_counter() - attempt_started_at) * 1000, 2),
                            result="success",
                            status_code=getattr(response, "status_code", None),
                            response_kind="stream",
                        )

                        self._stream_context = stream_context
                        if endpoint_index > 0:
                            logger.info(
                                "Primary stream endpoint failed; switched to backup endpoint %s",
                                endpoint_index,
                            )
                        return response
                    except httpx.HTTPStatusError as exc:
                        self.last_exception = exc
                        if stream_context is not None:
                            await stream_context.__aexit__(None, None, None)

                        non_retryable = exc.response.status_code in self.retry_cfg.non_retryable_status_codes
                        will_retry = (not non_retryable) and attempt < self.transport_max_retries - 1
                        will_failover = (
                            (not non_retryable)
                            and (not will_retry)
                            and self.client._should_failover(exc)
                            and endpoint_index < len(self.endpoints) - 1
                        )
                        self.client._record_transport_attempt(
                            request_kind="stream",
                            api_mode=api_mode,
                            endpoint_path=self.endpoint,
                            endpoint_index=endpoint_index + 1,
                            endpoint_role="primary" if endpoint_index == 0 else "backup",
                            base_url=base_url,
                            request_url=url,
                            attempt_number=attempt + 1,
                            max_attempts=self.transport_max_retries,
                            duration_ms=round((time.perf_counter() - attempt_started_at) * 1000, 2),
                            result="http_error",
                            status_code=exc.response.status_code if exc.response is not None else None,
                            error_type=type(exc).__name__,
                            error_message=str(exc),
                            will_retry_same_endpoint=will_retry,
                            will_failover=will_failover,
                        )

                        if non_retryable:
                            logger.error(
                                "Stream endpoint %s returned non-retryable status %s",
                                endpoint_index + 1,
                                exc.response.status_code,
                            )
                            raise

                        if attempt == self.transport_max_retries - 1:
                            if will_failover:
                                logger.warning(
                                    "Stream endpoint %s failed; trying backup endpoint %s",
                                    endpoint_index + 1,
                                    endpoint_index + 2,
                                )
                                break
                            raise
                    except (httpx.ConnectError, httpx.TimeoutException) as exc:
                        self.last_exception = exc
                        will_retry = attempt < self.transport_max_retries - 1
                        will_failover = (
                            (not will_retry)
                            and self.client._should_failover(exc)
                            and endpoint_index < len(self.endpoints) - 1
                        )
                        self.client._record_transport_attempt(
                            request_kind="stream",
                            api_mode=api_mode,
                            endpoint_path=self.endpoint,
                            endpoint_index=endpoint_index + 1,
                            endpoint_role="primary" if endpoint_index == 0 else "backup",
                            base_url=base_url,
                            request_url=url,
                            attempt_number=attempt + 1,
                            max_attempts=self.transport_max_retries,
                            duration_ms=round((time.perf_counter() - attempt_started_at) * 1000, 2),
                            result="network_error",
                            error_type=type(exc).__name__,
                            error_message=str(exc),
                            will_retry_same_endpoint=will_retry,
                            will_failover=will_failover,
                        )
                        if attempt == self.transport_max_retries - 1:
                            if will_failover:
                                logger.warning(
                                    "Stream endpoint %s connection failed; trying backup endpoint %s",
                                    endpoint_index + 1,
                                    endpoint_index + 2,
                                )
                                break
                            raise

            logger.error("All stream endpoints failed (%s total)", len(self.endpoints))
            if self.last_exception is not None:
                raise self.last_exception
            raise RuntimeError("All stream endpoints failed without a captured exception")
        except Exception:
            self._release_semaphore()
            raise

    async def __aexit__(self, exc_type, exc, tb):
        try:
            if self._stream_context is not None:
                return await self._stream_context.__aexit__(exc_type, exc, tb)
            return False
        finally:
            self._release_semaphore()

    def _release_semaphore(self) -> None:
        if self._semaphore is not None:
            self._semaphore.release()
            self._semaphore = None


class BaseAIClient(ABC):
    """AI HTTP 客户端基类"""

    def __init__(
        self,
        api_key: str,
        base_url: str,
        config: Optional[AIClientConfig] = None,
        backup_urls: Optional[List[str]] = None,
    ):
        self.api_key = api_key
        self.base_url = base_url.rstrip("/")
        self.backup_urls = [url.rstrip("/") for url in (backup_urls or [])]
        self.config = config or default_config
        self.http_client = self._get_or_create_client()
        self._transport_diagnostics: Dict[str, Any] = {}

    def _get_client_key(self) -> str:
        """生成客户端池键"""
        key_hash = hashlib.md5(self.api_key.encode()).hexdigest()[:8]
        return f"{self.__class__.__name__}_{self.base_url}_{key_hash}"

    @staticmethod
    def _should_disable_env_proxy_for_base_url(base_url: str) -> bool:
        parsed = urlparse(base_url)
        hostname = (parsed.hostname or "").strip().lower()
        if not hostname:
            return False
        if hostname == "localhost":
            return True
        try:
            return ipaddress.ip_address(hostname).is_loopback
        except ValueError:
            return False

    def _get_or_create_client(self) -> httpx.AsyncClient:
        """获取或创建 HTTP 客户端"""
        client_key = self._get_client_key()

        if client_key in _http_client_pool:
            client = _http_client_pool[client_key]
            if not client.is_closed:
                return client
            del _http_client_pool[client_key]

        http_cfg = self.config.http
        trust_env = not self._should_disable_env_proxy_for_base_url(self.base_url)
        client = httpx.AsyncClient(
            trust_env=trust_env,
            timeout=httpx.Timeout(
                connect=http_cfg.connect_timeout,
                read=http_cfg.read_timeout,
                write=http_cfg.write_timeout,
                pool=http_cfg.pool_timeout,
            ),
            limits=httpx.Limits(
                max_keepalive_connections=http_cfg.max_keepalive_connections,
                max_connections=http_cfg.max_connections,
                keepalive_expiry=http_cfg.keepalive_expiry,
            ),
        )
        _http_client_pool[client_key] = client
        logger.info(f"✅ 创建 HTTP 客户端: {client_key} (trust_env={trust_env})")
        return client

    @abstractmethod
    def _build_headers(self) -> Dict[str, str]:
        """构建请求头"""
        pass

    def _should_failover(self, exception: Exception) -> bool:
        """
        判断是否应该触发降级

        仅对网络错误/5xx/429 触发降级
        401/403/404 不降级
        """
        if isinstance(exception, httpx.HTTPStatusError):
            status_code = exception.response.status_code
            # 仅对 5xx 和 429 触发降级
            return status_code >= 500 or status_code == 429
        # 网络错误触发降级
        if isinstance(exception, (httpx.ConnectError, httpx.TimeoutException)):
            return True
        return False

    def reset_transport_diagnostics(self, metadata: Optional[Dict[str, Any]] = None) -> None:
        self._transport_diagnostics = {
            "client_class": self.__class__.__name__,
            "events": [],
            "attempts": [],
        }
        if metadata:
            self._transport_diagnostics.update(metadata)

    def _ensure_transport_diagnostics(self) -> Dict[str, Any]:
        if not isinstance(self._transport_diagnostics, dict) or not self._transport_diagnostics:
            self.reset_transport_diagnostics()
        self._transport_diagnostics.setdefault("events", [])
        self._transport_diagnostics.setdefault("attempts", [])
        return self._transport_diagnostics

    def _record_transport_event(self, event_type: str, **payload: Any) -> None:
        diagnostics = self._ensure_transport_diagnostics()
        event = {"type": event_type}
        for key, value in payload.items():
            if value is not None:
                event[key] = value
        diagnostics["events"].append(event)

    def _record_transport_attempt(self, **payload: Any) -> None:
        diagnostics = self._ensure_transport_diagnostics()
        attempt = {}
        for key, value in payload.items():
            if value is not None:
                attempt[key] = value
        diagnostics["attempts"].append(attempt)

    def _set_transport_diagnostic_values(self, **payload: Any) -> None:
        diagnostics = self._ensure_transport_diagnostics()
        for key, value in payload.items():
            if value is not None:
                diagnostics[key] = value

    def get_transport_diagnostics(self) -> Dict[str, Any]:
        if not isinstance(self._transport_diagnostics, dict) or not self._transport_diagnostics:
            return {}

        cloned = _clone_transport_payload(self._transport_diagnostics)
        cloned.pop("_request_started_perf_counter", None)

        attempts = cloned.get("attempts") or []
        events = cloned.get("events") or []

        api_modes_tried: List[str] = []
        for event in events:
            for key in ("api_mode", "from_api_mode", "to_api_mode"):
                value = event.get(key)
                if value and value not in api_modes_tried:
                    api_modes_tried.append(value)
        for attempt in attempts:
            value = attempt.get("api_mode")
            if value and value not in api_modes_tried:
                api_modes_tried.append(value)

        api_mode_fallback_count = sum(1 for event in events if event.get("type") == "api_mode_fallback")
        candidate_fallback_count = sum(
            1
            for event in events
            if event.get("type") == "chat_completions_candidate_selected"
            and event.get("candidate_base_url")
            and event.get("candidate_base_url") != event.get("original_base_url")
        )
        failover_count = sum(1 for attempt in attempts if attempt.get("will_failover"))
        attempt_durations = [
            float(attempt.get("duration_ms"))
            for attempt in attempts
            if isinstance(attempt.get("duration_ms"), (int, float))
        ]
        last_successful_attempt = next(
            (attempt for attempt in reversed(attempts) if attempt.get("result") == "success"),
            None,
        )
        request_completed_latency_ms = cloned.get("request_completed_latency_ms")
        stream_completed_latency_ms = cloned.get("stream_completed_latency_ms")
        first_chunk_latency_ms = cloned.get("first_chunk_latency_ms")

        cloned["summary"] = {
            "total_attempts": len(attempts),
            "successful_attempts": sum(1 for attempt in attempts if attempt.get("result") == "success"),
            "api_modes_tried": api_modes_tried,
            "backup_endpoint_used": any(attempt.get("endpoint_role") == "backup" for attempt in attempts),
            "api_mode_fallback_used": api_mode_fallback_count > 0,
            "api_mode_fallback_count": api_mode_fallback_count,
            "candidate_fallback_count": candidate_fallback_count,
            "fallback_count": api_mode_fallback_count + candidate_fallback_count,
            "failover_count": failover_count,
            "forced_chat_completions": any(event.get("type") == "api_mode_forced" for event in events),
            "normalized_base_url_used": candidate_fallback_count > 0,
            "first_chunk_latency_ms": first_chunk_latency_ms,
            "request_completed_latency_ms": request_completed_latency_ms,
            "stream_completed_latency_ms": stream_completed_latency_ms,
            "final_latency_ms": stream_completed_latency_ms or request_completed_latency_ms,
            "slowest_attempt_duration_ms": max(attempt_durations) if attempt_durations else None,
            "successful_base_url": last_successful_attempt.get("base_url") if last_successful_attempt else None,
            "successful_endpoint_path": last_successful_attempt.get("endpoint_path") if last_successful_attempt else None,
        }
        return cloned

    async def _request_with_retry(
        self,
        method: str,
        endpoint: str,
        payload: Dict[str, Any],
        stream: bool = False,
        request_options: Optional[Dict[str, Any]] = None,
        base_url_override: Optional[str] = None,
    ) -> Any:
        """
        Send an HTTP request with retry and endpoint failover support.

        Supports both JSON and streaming requests, with backup endpoint fallback.
        """
        headers = self._build_headers()
        retry_cfg = self.config.retry
        rate_cfg = self.config.rate_limit
        request_options = request_options or {}
        transport_max_retries = request_options.get("transport_max_retries")
        if transport_max_retries is None:
            transport_max_retries = retry_cfg.max_retries
        transport_max_retries = max(int(transport_max_retries), 1)
        read_timeout_override = request_options.get("read_timeout")

        request_timeout = None
        if read_timeout_override is not None:
            http_cfg = self.config.http
            request_timeout = httpx.Timeout(
                connect=http_cfg.connect_timeout,
                read=float(read_timeout_override),
                write=http_cfg.write_timeout,
                pool=http_cfg.pool_timeout,
            )

        primary_base_url = str(base_url_override or self.base_url).rstrip("/")
        endpoints = [primary_base_url] + self.backup_urls
        diagnostics = self._ensure_transport_diagnostics()
        diagnostics["last_endpoint_path"] = endpoint
        diagnostics["transport_max_retries"] = transport_max_retries
        if read_timeout_override is not None:
            diagnostics["read_timeout_override"] = float(read_timeout_override)

        if stream:
            return _RetriableStreamContext(
                client=self,
                method=method,
                endpoint=endpoint,
                payload=payload,
                headers=headers,
                endpoints=endpoints,
                transport_max_retries=transport_max_retries,
                request_timeout=request_timeout,
                retry_cfg=retry_cfg,
                rate_cfg=rate_cfg,
            )

        semaphore = _get_semaphore(rate_cfg.max_concurrent_requests)
        last_exception = None
        api_mode = _detect_api_mode(endpoint)

        async with semaphore:
            await asyncio.sleep(rate_cfg.request_delay)

            for endpoint_index, base_url in enumerate(endpoints):
                url = f"{base_url}{endpoint}"

                for attempt in range(transport_max_retries):
                    try:
                        if attempt > 0:
                            delay = min(
                                retry_cfg.base_delay * (retry_cfg.exponential_base ** attempt),
                                retry_cfg.max_delay,
                            )
                            logger.warning(
                                "Request retry %s/%s on endpoint %s/%s after %.2fs",
                                attempt + 1,
                                transport_max_retries,
                                endpoint_index + 1,
                                len(endpoints),
                                delay,
                            )
                            await asyncio.sleep(delay)

                        attempt_started_at = time.perf_counter()
                        request_kwargs = {"headers": headers, "json": payload}
                        if request_timeout is not None:
                            request_kwargs["timeout"] = request_timeout
                        response = await self.http_client.request(method, url, **request_kwargs)
                        response.raise_for_status()

                        if endpoint_index > 0:
                            logger.info(
                                "Primary endpoint failed; switched to backup endpoint %s",
                                endpoint_index,
                            )

                        try:
                            data = response.json()
                            self._record_transport_attempt(
                                request_kind="json",
                                api_mode=api_mode,
                                endpoint_path=endpoint,
                                endpoint_index=endpoint_index + 1,
                                endpoint_role="primary" if endpoint_index == 0 else "backup",
                                base_url=base_url,
                                request_url=url,
                                attempt_number=attempt + 1,
                                max_attempts=transport_max_retries,
                                duration_ms=round((time.perf_counter() - attempt_started_at) * 1000, 2),
                                result="success",
                                status_code=response.status_code,
                                response_kind="json",
                            )
                            return data
                        except json.JSONDecodeError as exc:
                            raw_text = (response.text or "").strip()
                            if raw_text.startswith("data:"):
                                self._record_transport_attempt(
                                    request_kind="json",
                                    api_mode=api_mode,
                                    endpoint_path=endpoint,
                                    endpoint_index=endpoint_index + 1,
                                    endpoint_role="primary" if endpoint_index == 0 else "backup",
                                    base_url=base_url,
                                    request_url=url,
                                    attempt_number=attempt + 1,
                                    max_attempts=transport_max_retries,
                                    duration_ms=round((time.perf_counter() - attempt_started_at) * 1000, 2),
                                    result="success",
                                    status_code=response.status_code,
                                    response_kind="sse_text_passthrough",
                                )
                                return {
                                    "_raw_sse_text": raw_text,
                                    "_raw_response_status_code": response.status_code,
                                }

                            body_preview = raw_text.replace("\r", " ").replace("\n", " ")[:200]
                            runtime_error = RuntimeError(
                                f"API returned non-JSON content. The Base URL may be incorrect (for example, missing /v1). HTTP {response.status_code}, response preview: {body_preview}"
                            )
                            self._record_transport_attempt(
                                request_kind="json",
                                api_mode=api_mode,
                                endpoint_path=endpoint,
                                endpoint_index=endpoint_index + 1,
                                endpoint_role="primary" if endpoint_index == 0 else "backup",
                                base_url=base_url,
                                request_url=url,
                                attempt_number=attempt + 1,
                                max_attempts=transport_max_retries,
                                duration_ms=round((time.perf_counter() - attempt_started_at) * 1000, 2),
                                result="invalid_json",
                                status_code=response.status_code,
                                error_type=type(runtime_error).__name__,
                                error_message=str(runtime_error),
                            )
                            raise runtime_error from exc

                    except httpx.HTTPStatusError as exc:
                        last_exception = exc
                        non_retryable = exc.response.status_code in retry_cfg.non_retryable_status_codes
                        will_retry = (not non_retryable) and attempt < transport_max_retries - 1
                        will_failover = (
                            (not non_retryable)
                            and (not will_retry)
                            and self._should_failover(exc)
                            and endpoint_index < len(endpoints) - 1
                        )
                        self._record_transport_attempt(
                            request_kind="json",
                            api_mode=api_mode,
                            endpoint_path=endpoint,
                            endpoint_index=endpoint_index + 1,
                            endpoint_role="primary" if endpoint_index == 0 else "backup",
                            base_url=base_url,
                            request_url=url,
                            attempt_number=attempt + 1,
                            max_attempts=transport_max_retries,
                            duration_ms=round((time.perf_counter() - attempt_started_at) * 1000, 2),
                            result="http_error",
                            status_code=exc.response.status_code if exc.response is not None else None,
                            error_type=type(exc).__name__,
                            error_message=str(exc),
                            will_retry_same_endpoint=will_retry,
                            will_failover=will_failover,
                        )
                        if non_retryable:
                            logger.error(
                                "Endpoint %s returned non-retryable status %s",
                                endpoint_index + 1,
                                exc.response.status_code,
                            )
                            raise
                        if attempt == transport_max_retries - 1:
                            if will_failover:
                                logger.warning(
                                    "Endpoint %s failed; trying backup endpoint %s",
                                    endpoint_index + 1,
                                    endpoint_index + 2,
                                )
                                break
                            raise
                    except (httpx.ConnectError, httpx.TimeoutException) as exc:
                        last_exception = exc
                        will_retry = attempt < transport_max_retries - 1
                        will_failover = (
                            (not will_retry)
                            and self._should_failover(exc)
                            and endpoint_index < len(endpoints) - 1
                        )
                        self._record_transport_attempt(
                            request_kind="json",
                            api_mode=api_mode,
                            endpoint_path=endpoint,
                            endpoint_index=endpoint_index + 1,
                            endpoint_role="primary" if endpoint_index == 0 else "backup",
                            base_url=base_url,
                            request_url=url,
                            attempt_number=attempt + 1,
                            max_attempts=transport_max_retries,
                            duration_ms=round((time.perf_counter() - attempt_started_at) * 1000, 2),
                            result="network_error",
                            error_type=type(exc).__name__,
                            error_message=str(exc),
                            will_retry_same_endpoint=will_retry,
                            will_failover=will_failover,
                        )
                        if attempt == transport_max_retries - 1:
                            if will_failover:
                                logger.warning(
                                    "Endpoint %s connection failed; trying backup endpoint %s",
                                    endpoint_index + 1,
                                    endpoint_index + 2,
                                )
                                break
                            raise

        logger.error("All endpoints failed (%s total)", len(endpoints))
        if last_exception:
            raise last_exception
        raise RuntimeError("All endpoints failed without a captured exception")

    @abstractmethod
    async def chat_completion(
        self,
        messages: list,
        model: str,
        temperature: float,
        max_tokens: int,
        tools: Optional[list] = None,
        tool_choice: Optional[str] = None,
    ) -> Dict[str, Any]:
        """聊天补全"""
        pass

    @abstractmethod
    async def chat_completion_stream(
        self,
        messages: list,
        model: str,
        temperature: float,
        max_tokens: int,
    ) -> AsyncGenerator[str, None]:
        """流式聊天补全"""
        pass


async def cleanup_all_clients():
    """清理所有 HTTP 客户端"""
    for key, client in list(_http_client_pool.items()):
        if not client.is_closed:
            await client.aclose()
    _http_client_pool.clear()
    logger.info("✅ HTTP 客户端池已清理")
