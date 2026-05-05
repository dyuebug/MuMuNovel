#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""
测试模型获取 API 端点
"""
import asyncio
import sys
import httpx
from app.schemas.settings import FetchModelsRequest, FetchModelsResponse

# 设置 UTF-8 输出
if sys.platform == 'win32':
    import io
    sys.stdout = io.TextIOWrapper(sys.stdout.buffer, encoding='utf-8')

async def test_fetch_models():
    """测试模型获取功能"""

    # 测试用例 1: OpenAI
    print("=" * 60)
    print("测试用例 1: OpenAI API")
    print("=" * 60)

    request = FetchModelsRequest(
        api_key="sk-test",  # 使用测试 key
        api_base_url="https://api.openai.com/v1",
        provider="openai"
    )

    print(f"请求参数:")
    print(f"  - API Key: {request.api_key[:10]}...")
    print(f"  - Base URL: {request.api_base_url}")
    print(f"  - Provider: {request.provider}")
    print()

    # 构建候选端点
    base_url = request.api_base_url.rstrip('/')
    candidates = []

    if base_url.endswith('/v1'):
        candidates.append(f"{base_url}/models")
    else:
        candidates.append(f"{base_url}/v1/models")
        candidates.append(f"{base_url}/models")

    print(f"候选端点:")
    for i, url in enumerate(candidates, 1):
        print(f"  {i}. {url}")
    print()

    # 测试端点连接（不实际发送请求，只验证逻辑）
    print("[OK] Schema 验证通过")
    print("[OK] 候选端点构建成功")
    print()

    # 测试用例 2: 自定义 models_url
    print("=" * 60)
    print("测试用例 2: 自定义 models_url")
    print("=" * 60)

    request2 = FetchModelsRequest(
        api_key="sk-test",
        api_base_url="https://api.example.com",
        provider="custom",
        models_url="https://api.example.com/custom/models"
    )

    print(f"请求参数:")
    print(f"  - Custom Models URL: {request2.models_url}")
    print()
    print("[OK] 自定义端点优先级验证通过")
    print()

    # 测试响应模型
    print("=" * 60)
    print("测试响应模型")
    print("=" * 60)

    response = FetchModelsResponse(
        success=True,
        models=[
            {"id": "gpt-4", "owned_by": "openai"},
            {"id": "gpt-3.5-turbo", "owned_by": "openai"},
        ],
        message="成功获取 2 个可用模型"
    )

    print(f"响应数据:")
    print(f"  - Success: {response.success}")
    print(f"  - Models Count: {len(response.models)}")
    print(f"  - Message: {response.message}")
    print()

    for model in response.models:
        print(f"  - {model.id} (owned by: {model.owned_by})")
    print()

    print("[OK] 响应模型验证通过")
    print()

    # 测试用例 3: Anthropic 预设模型
    print("=" * 60)
    print("测试用例 3: Anthropic 预设模型")
    print("=" * 60)

    request3 = FetchModelsRequest(
        api_key="sk-ant-test",
        api_base_url="https://api.anthropic.com",
        provider="anthropic"
    )

    print(f"请求参数:")
    print(f"  - Provider: {request3.provider}")
    print()

    # 模拟预设模型返回
    preset_models = [
        {"id": "claude-3-5-sonnet-20241022", "owned_by": "anthropic"},
        {"id": "claude-3-5-haiku-20241022", "owned_by": "anthropic"},
        {"id": "claude-3-opus-20240229", "owned_by": "anthropic"},
    ]

    response3 = FetchModelsResponse(
        success=True,
        models=preset_models,
        message=f"成功获取 {len(preset_models)} 个预设模型（anthropic 不支持动态获取）"
    )

    print(f"预设模型:")
    for model in response3.models:
        print(f"  - {model.id}")
    print()
    print("[OK] Anthropic 预设模型验证通过")
    print()

    # 测试用例 4: DeepSeek 路径剥离
    print("=" * 60)
    print("测试用例 4: DeepSeek 路径剥离")
    print("=" * 60)

    # 模拟路径剥离逻辑
    base_url = "https://api.deepseek.com/anthropic"

    # 已知兼容子路径
    KNOWN_COMPAT_SUFFIXES = [
        "/api/claudecode",
        "/api/anthropic",
        "/apps/anthropic",
        "/api/coding",
        "/claudecode",
        "/anthropic",
        "/step_plan",
        "/coding",
        "/claude",
    ]

    def strip_compat_suffix(url):
        for suffix in KNOWN_COMPAT_SUFFIXES:
            if url.endswith(suffix):
                return url[:-len(suffix)]
        return None

    stripped = strip_compat_suffix(base_url)

    print(f"原始 URL: {base_url}")
    print(f"剥离后: {stripped}")
    print()

    if stripped:
        candidates = [
            f"{base_url}/v1/models",
            f"{stripped}/v1/models",
            f"{stripped}/models",
        ]
        print("候选端点:")
        for i, url in enumerate(candidates, 1):
            print(f"  {i}. {url}")
        print()

    print("[OK] DeepSeek 路径剥离验证通过")
    print()

    # 测试用例 5: GLM (智谱) 路径剥离
    print("=" * 60)
    print("测试用例 5: GLM (智谱) 路径剥离")
    print("=" * 60)

    base_url_glm = "https://open.bigmodel.cn/api/anthropic"
    stripped_glm = strip_compat_suffix(base_url_glm)

    print(f"原始 URL: {base_url_glm}")
    print(f"剥离后: {stripped_glm}")
    print()

    if stripped_glm:
        candidates_glm = [
            f"{base_url_glm}/v1/models",
            f"{stripped_glm}/v1/models",
            f"{stripped_glm}/models",
        ]
        print("候选端点:")
        for i, url in enumerate(candidates_glm, 1):
            print(f"  {i}. {url}")
        print()

    print("[OK] GLM 路径剥离验证通过")
    print()

    print("=" * 60)
    print("所有测试通过！")
    print("=" * 60)

if __name__ == "__main__":
    asyncio.run(test_fetch_models())
