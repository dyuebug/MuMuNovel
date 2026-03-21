"""AI 客户端基类"""
import asyncio
import hashlib
import json
from abc import ABC, abstractmethod
from typing import Any, AsyncGenerator, Dict, Optional, List

import httpx

from app.logger import get_logger
from app.services.ai_config import AIClientConfig, default_config

logger = get_logger(__name__)

# 全局 HTTP 客户端池
_http_client_pool: Dict[str, httpx.AsyncClient] = {}
_global_semaphore: Optional[asyncio.Semaphore] = None


def _get_semaphore(max_concurrent: int) -> asyncio.Semaphore:
    """获取全局信号量"""
    global _global_semaphore
    if _global_semaphore is None:
        _global_semaphore = asyncio.Semaphore(max_concurrent)
    return _global_semaphore


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

    def _get_client_key(self) -> str:
        """生成客户端唯一键"""
        key_hash = hashlib.md5(self.api_key.encode()).hexdigest()[:8]
        return f"{self.__class__.__name__}_{self.base_url}_{key_hash}"

    def _get_or_create_client(self) -> httpx.AsyncClient:
        """获取或创建 HTTP 客户端"""
        client_key = self._get_client_key()

        if client_key in _http_client_pool:
            client = _http_client_pool[client_key]
            if not client.is_closed:
                return client
            del _http_client_pool[client_key]

        http_cfg = self.config.http
        client = httpx.AsyncClient(
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
        logger.info(f"✅ 创建 HTTP 客户端: {client_key}")
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

    async def _request_with_retry(
        self,
        method: str,
        endpoint: str,
        payload: Dict[str, Any],
        stream: bool = False,
    ) -> Any:
        """
        带重试和主备降级的 HTTP 请求

        流式请求不支持自动重试和降级
        """
        # 流式请求：单端点单次尝试
        if stream:
            url = f"{self.base_url}{endpoint}"
            headers = self._build_headers()
            return self.http_client.stream(method, url, headers=headers, json=payload)

        # 非流式请求：支持重试和主备降级
        headers = self._build_headers()
        retry_cfg = self.config.retry
        rate_cfg = self.config.rate_limit

        semaphore = _get_semaphore(rate_cfg.max_concurrent_requests)

        # 构建端点池：主端点 + 备端点
        endpoints = [self.base_url] + self.backup_urls
        last_exception = None

        async with semaphore:
            await asyncio.sleep(rate_cfg.request_delay)

            # 遍历端点池
            for endpoint_index, base_url in enumerate(endpoints):
                url = f"{base_url}{endpoint}"

                # 对每个端点进行重试
                for attempt in range(retry_cfg.max_retries):
                    try:
                        if attempt > 0:
                            delay = min(
                                retry_cfg.base_delay * (retry_cfg.exponential_base ** attempt),
                                retry_cfg.max_delay,
                            )
                            logger.warning(f"⚠️ 端点 {endpoint_index + 1}/{len(endpoints)} 重试 {attempt + 1}/{retry_cfg.max_retries}，等待 {delay}s")
                            await asyncio.sleep(delay)

                        response = await self.http_client.request(method, url, headers=headers, json=payload)
                        response.raise_for_status()

                        # 成功时记录降级信息
                        if endpoint_index > 0:
                            logger.info(f"✅ 主端点失败，已自动切换到备端点 {endpoint_index}，响应成功")

                        try:
                            return response.json()
                        except json.JSONDecodeError as e:
                            raw_text = (response.text or "").strip()
                            if raw_text.startswith("data:"):
                                return {
                                    "_raw_sse_text": raw_text,
                                    "_raw_response_status_code": response.status_code,
                                }

                            body_preview = raw_text.replace("\r", " ").replace("\n", " ")[:200]
                            raise RuntimeError(
                                f"API 返回了非 JSON 内容，可能是 Base URL 路径不正确（例如缺少 /v1）。HTTP {response.status_code}，响应片段: {body_preview}"
                            ) from e

                    except httpx.HTTPStatusError as e:
                        last_exception = e
                        # 不可重试的错误直接抛出
                        if e.response.status_code in retry_cfg.non_retryable_status_codes:
                            logger.error(f"❌ 端点 {endpoint_index + 1} 返回不可重试错误 {e.response.status_code}")
                            raise
                        # 最后一次重试失败，判断是否降级
                        if attempt == retry_cfg.max_retries - 1:
                            if self._should_failover(e) and endpoint_index < len(endpoints) - 1:
                                logger.warning(f"⚠️ 端点 {endpoint_index + 1} 失败，尝试切换到备端点 {endpoint_index + 2}")
                                break  # 跳出重试循环，尝试下一个端点
                            else:
                                raise
                    except (httpx.ConnectError, httpx.TimeoutException) as e:
                        last_exception = e
                        # 最后一次重试失败，判断是否降级
                        if attempt == retry_cfg.max_retries - 1:
                            if self._should_failover(e) and endpoint_index < len(endpoints) - 1:
                                logger.warning(f"⚠️ 端点 {endpoint_index + 1} 网络错误，尝试切换到备端点 {endpoint_index + 2}")
                                break  # 跳出重试循环，尝试下一个端点
                            else:
                                raise

        # 所有端点都失败
        logger.error(f"❌ 所有端点 ({len(endpoints)} 个) 都失败")
        if last_exception:
            raise last_exception
        raise Exception("所有端点都失败，且没有捕获到异常")

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
