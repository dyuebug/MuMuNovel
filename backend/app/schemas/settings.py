"""设置相关的Pydantic模型"""
from pydantic import BaseModel, Field, ConfigDict, field_validator
from typing import Optional, List
from datetime import datetime
import json


class SettingsBase(BaseModel):
    """设置基础模型"""
    model_config = ConfigDict(protected_namespaces=())
    
    api_provider: Optional[str] = Field(default="openai", description="API提供商")
    api_key: Optional[str] = Field(default=None, description="API密钥")
    api_base_url: Optional[str] = Field(default=None, description="自定义API地址")
    api_backup_urls: Optional[List[str]] = Field(default=None, description="备用API地址列表")
    provider_type: Optional[str] = Field(default="openai", description="Provider类型(openai/azure/newapi/custom)")
    fallback_strategy: Optional[str] = Field(default="auto", description="端点切换策略(auto/manual)")
    azure_api_version: Optional[str] = Field(default=None, description="Azure API版本")
    llm_model: Optional[str] = Field(default="gpt-4", description="模型名称")
    temperature: Optional[float] = Field(default=0.7, ge=0.0, le=2.0, description="温度参数")
    max_tokens: Optional[int] = Field(default=2000, ge=1, description="最大token数")
    system_prompt: Optional[str] = Field(default=None, description="系统级别提示词，每次AI调用都会使用")
    preferences: Optional[str] = Field(default=None, description="其他偏好设置(JSON)")


class SettingsCreate(SettingsBase):
    """创建设置请求模型"""
    pass


class SettingsUpdate(SettingsBase):
    """更新设置请求模型"""
    pass


class SettingsResponse(SettingsBase):
    """设置响应模型"""
    model_config = ConfigDict(from_attributes=True, protected_namespaces=())

    id: str
    user_id: str
    created_at: datetime
    updated_at: datetime

    @field_validator('api_backup_urls', mode='before')
    @classmethod
    def parse_backup_urls(cls, v):
        """数据库存储为 JSON 字符串，反序列化为 List[str]"""
        if v is None:
            return None
        if isinstance(v, str):
            try:
                parsed = json.loads(v)
                return parsed if isinstance(parsed, list) else None
            except (json.JSONDecodeError, TypeError):
                return None
        return v


# ========== API配置预设相关模型 ==========

class APIKeyPresetConfig(BaseModel):
    """预设配置内容"""
    model_config = ConfigDict(protected_namespaces=())
    
    api_provider: str = Field(..., description="API提供商")
    api_key: str = Field(..., description="API密钥")
    api_base_url: Optional[str] = Field(None, description="自定义API地址")
    api_backup_urls: Optional[List[str]] = Field(None, description="备用API地址列表")
    provider_type: Optional[str] = Field(default="openai", description="Provider类型(openai/azure/newapi/custom)")
    fallback_strategy: Optional[str] = Field(default="auto", description="端点切换策略(auto/manual)")
    azure_api_version: Optional[str] = Field(None, description="Azure API版本")
    llm_model: str = Field(..., description="模型名称")
    temperature: float = Field(default=0.7, ge=0.0, le=2.0, description="温度参数")
    max_tokens: int = Field(default=2000, ge=1, description="最大token数")
    system_prompt: Optional[str] = Field(default=None, description="系统级别提示词")


class APIKeyPreset(BaseModel):
    """API配置预设"""
    model_config = ConfigDict(protected_namespaces=())
    
    id: str = Field(..., description="预设ID")
    name: str = Field(..., min_length=1, max_length=50, description="预设名称")
    description: Optional[str] = Field(None, max_length=200, description="预设描述")
    is_active: bool = Field(default=False, description="是否激活")
    created_at: datetime = Field(..., description="创建时间")
    config: APIKeyPresetConfig = Field(..., description="配置内容")


class PresetCreateRequest(BaseModel):
    """创建预设请求"""
    model_config = ConfigDict(protected_namespaces=())
    
    name: str = Field(..., min_length=1, max_length=50, description="预设名称")
    description: Optional[str] = Field(None, max_length=200, description="预设描述")
    config: APIKeyPresetConfig = Field(..., description="配置内容")


class PresetUpdateRequest(BaseModel):
    """更新预设请求"""
    model_config = ConfigDict(protected_namespaces=())
    
    name: Optional[str] = Field(None, min_length=1, max_length=50, description="预设名称")
    description: Optional[str] = Field(None, max_length=200, description="预设描述")
    config: Optional[APIKeyPresetConfig] = Field(None, description="配置内容")


class PresetResponse(APIKeyPreset):
    """预设响应"""
    pass


class PresetListResponse(BaseModel):
    """预设列表响应"""
    model_config = ConfigDict(protected_namespaces=())
    
    presets: List[PresetResponse] = Field(..., description="预设列表")
    total: int = Field(..., description="总数")
    active_preset_id: Optional[str] = Field(None, description="当前激活的预设ID")