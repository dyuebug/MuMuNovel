"""
认证中间件 - 从 Cookie 中提取用户信息并注入到 request.state
支持来自其他实例的代理请求（提示词工坊功能）
"""
import secrets

from fastapi import Request
from sqlalchemy.exc import OperationalError
from starlette.middleware.base import BaseHTTPMiddleware
from app.user_manager import user_manager
from app.config import settings, is_workshop_server
from app.logger import get_logger

logger = get_logger(__name__)


class AuthMiddleware(BaseHTTPMiddleware):
    """认证中间件"""

    @staticmethod
    def _is_trusted_workshop_proxy_request(request: Request) -> bool:
        if not is_workshop_server():
            return False

        configured_secret = settings.WORKSHOP_PROXY_SHARED_SECRET
        if not configured_secret:
            logger.warning("提示词工坊代理请求未启用共享密钥校验，已拒绝代理身份透传")
            return False

        header_secret = request.headers.get("X-Workshop-Secret")
        if not header_secret:
            logger.warning("提示词工坊代理请求缺少共享密钥，已拒绝代理身份透传")
            return False

        if not secrets.compare_digest(header_secret, configured_secret):
            logger.warning("提示词工坊代理请求共享密钥校验失败，已拒绝代理身份透传")
            return False

        return True
    
    async def dispatch(self, request: Request, call_next):
        """
        处理请求，从 Cookie 或 Header 中提取用户 ID 并注入到 request.state
        
        对于提示词工坊相关的代理请求（带有 X-Instance-ID Header），
        从 Header 中读取用户标识而不是 Cookie。
        """
        request.state.is_proxy_request = False
        request.state.proxy_instance_id = None
        request.state.auth_backend_unavailable = False
        request.state.auth_backend_unavailable_message = None

        # 检查是否为来自其他实例的代理请求（提示词工坊）
        instance_id = request.headers.get("X-Instance-ID")
        is_workshop_path = request.url.path.startswith("/api/prompt-workshop")
        
        if instance_id and is_workshop_path and self._is_trusted_workshop_proxy_request(request):
            # 来自其他实例的代理请求
            header_user_id = request.headers.get("X-User-ID")
            
            request.state.is_proxy_request = True
            request.state.proxy_instance_id = instance_id
            
            if header_user_id and ":" in header_user_id:
                # 有用户标识，使用代理的用户信息
                request.state.user_id = header_user_id  # 这是 "instance:user_id" 格式
                request.state.user = None  # 代理请求没有实际的 User 对象
                request.state.is_admin = False
            else:
                # 没有用户标识，匿名访问
                request.state.user_id = None
                request.state.user = None
                request.state.is_admin = False
        else:
            # 本地请求或非工坊路径，使用 Cookie 认证
            # 从 Cookie 中获取用户 ID
            user_id = request.cookies.get("user_id")
            
            if user_id:
                try:
                    user = await user_manager.get_user(user_id)
                except (OperationalError, ConnectionRefusedError, ConnectionError, TimeoutError, OSError) as exc:
                    logger.warning(f"鉴权用户加载失败，已降级为匿名请求: {user_id}, error={type(exc).__name__}: {exc}")
                    request.state.user_id = user_id
                    request.state.user = None
                    request.state.is_admin = False
                    request.state.auth_backend_unavailable = True
                    request.state.auth_backend_unavailable_message = '认证服务暂时不可用，请确认 PostgreSQL 已启动后重试'
                else:
                    if user:
                        # 检查用户是否被禁用 (trust_level = -1)
                        if user.trust_level == -1:
                            logger.warning(f"禁用用户尝试访问: {user_id} ({user.username})")
                            # 清除用户状态，视为未登录
                            request.state.user_id = None
                            request.state.user = None
                            request.state.is_admin = False
                        else:
                            # 用户正常，注入状态
                            request.state.user_id = user_id
                            request.state.user = user
                            request.state.is_admin = user.is_admin
                    else:
                        # 用户正常，注入状态?
                        request.state.user_id = None
                        request.state.user = None
                        request.state.is_admin = False
            else:
                # 未登录
                request.state.user_id = None
                request.state.user = None
                request.state.is_admin = False
        
        # 继续处理请求
        response = await call_next(request)
        return response
