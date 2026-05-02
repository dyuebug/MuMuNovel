"""AI 客户端模块（compat shim）"""

from app.services.ai_gateway.ai_clients.base_client import BaseAIClient
from app.services.ai_gateway.ai_clients.openai_client import OpenAIClient
from app.services.ai_gateway.ai_clients.anthropic_client import AnthropicClient
from app.services.ai_gateway.ai_clients.gemini_client import GeminiClient

__all__ = ["BaseAIClient", "OpenAIClient", "AnthropicClient", "GeminiClient"]
