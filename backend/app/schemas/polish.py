"""AI去味相关的 Pydantic 模型"""

from typing import Literal, Optional

from pydantic import AliasChoices, BaseModel, ConfigDict, Field, model_validator


PolishFocusMode = Literal["balanced", "dialogue", "pacing", "emotion", "hook"]


class PolishRequest(BaseModel):
    """AI去味请求模型。"""

    model_config = ConfigDict(populate_by_name=True, protected_namespaces=())

    original_text: Optional[str] = Field(
        default=None,
        validation_alias=AliasChoices("original_text", "text"),
        description="原始文本，兼容 original_text / text 两种字段名",
    )
    project_id: Optional[int] = Field(None, description="项目ID（可选，用于记录历史）")
    provider: Optional[str] = Field(None, description="AI提供商")
    model: Optional[str] = Field(None, description="AI模型")
    temperature: Optional[float] = Field(0.8, description="温度参数，建议 0.7-0.9")
    style: Optional[str] = Field(None, description="额外风格要求")
    focus_mode: PolishFocusMode = Field("balanced", description="润色侧重点")
    preserve_paragraphs: bool = Field(True, description="尽量保留原段落结构")
    retain_hooks: bool = Field(True, description="尽量保留段尾或章尾钩子")

    @model_validator(mode="after")
    def validate_original_text(self) -> "PolishRequest":
        self.original_text = (self.original_text or "").strip()
        if not self.original_text:
            raise ValueError("original_text 或 text 不能为空")
        return self


class PolishBatchRequest(BaseModel):
    """批量AI去味请求模型。"""

    texts: list[str] = Field(..., min_length=1, description="待批量润色的文本列表")
    project_id: Optional[int] = Field(None, description="项目ID（可选，用于记录历史）")
    provider: Optional[str] = Field(None, description="AI提供商")
    model: Optional[str] = Field(None, description="AI模型")
    temperature: Optional[float] = Field(0.8, description="温度参数，建议 0.7-0.9")
    style: Optional[str] = Field(None, description="额外风格要求")
    focus_mode: PolishFocusMode = Field("balanced", description="润色侧重点")
    preserve_paragraphs: bool = Field(True, description="尽量保留原段落结构")
    retain_hooks: bool = Field(True, description="尽量保留段尾或章尾钩子")

    @model_validator(mode="after")
    def validate_texts(self) -> "PolishBatchRequest":
        normalized = [text.strip() for text in self.texts if isinstance(text, str) and text.strip()]
        if not normalized:
            raise ValueError("texts 不能为空")
        self.texts = normalized
        return self


class PolishResponse(BaseModel):
    """AI去味响应模型。"""

    original_text: str = Field(..., description="原始文本")
    polished_text: str = Field(..., description="去味后的文本")
    word_count_before: int = Field(..., description="处理前字数")
    word_count_after: int = Field(..., description="处理后字数")
