# API 模型获取功能集成指南

## 概述

本次更新为 MuMuNovel 添加了从 AI 提供商自动获取可用模型列表的功能，参考了 cc-switch 项目的实现模式。

## 已完成的工作

### 1. 后端实现

#### Schema 定义 (`backend/app/schemas/settings.py`)

新增以下 Pydantic 模型：

```python
class FetchModelsRequest(BaseModel):
    """获取模型列表请求"""
    api_key: str
    api_base_url: str
    provider: str = "openai"
    models_url: Optional[str] = None

class FetchedModel(BaseModel):
    """获取到的模型信息"""
    id: str
    owned_by: Optional[str] = None

class FetchModelsResponse(BaseModel):
    """获取模型列表响应"""
    success: bool
    models: List[FetchedModel] = []
    message: Optional[str] = None
    error: Optional[str] = None
    error_type: Optional[str] = None
```

#### API 端点 (`backend/app/api/settings.py`)

新增端点：`POST /api/settings/fetch-models`

**功能特性**：
- 支持 OpenAI 兼容的 `/v1/models` 端点
- 智能路径剥离，支持 DeepSeek、GLM、Kimi 等国内提供商
- 自动尝试多个候选端点（含兼容路径剥离）
- 候选端点顺序：
  1. 自定义 `models_url`（如果提供）
  2. `{base_url}/v1/models` 或 `{base_url}/models`
  3. 剥离兼容子路径后重试（如 `/anthropic`、`/api/anthropic` 等）
- 错误处理：
  - 401/403：认证失败，立即返回
  - 404/405：端点不存在
  - Timeout：请求超时
  - 其他网络错误

**支持的兼容子路径**（按长度降序匹配）：
- `/api/claudecode`
- `/api/anthropic`
- `/apps/anthropic`
- `/api/coding`
- `/claudecode`
- `/anthropic`
- `/step_plan`
- `/coding`
- `/claude`

**请求示例**：
```json
{
  "api_key": "sk-xxx",
  "api_base_url": "https://api.deepseek.com/anthropic",
  "provider": "deepseek"
}
```

**响应示例**：
```json
{
  "success": true,
  "models": [
    {"id": "deepseek-chat", "owned_by": "deepseek"},
    {"id": "deepseek-coder", "owned_by": "deepseek"}
  ],
  "message": "成功获取 2 个可用模型"
}
```

### 2. 前端实现

#### API 服务 (`frontend/src/services/modules/settings.ts`)

新增方法：

```typescript
fetchModels: (params: {
  api_key: string;
  api_base_url: string;
  provider: string;
  models_url?: string;
}) => Promise<{
  success: boolean;
  models: Array<{ id: string; owned_by: string | null }>;
  message?: string;
  error?: string;
  error_type?: string;
}>
```

#### 组件 (`frontend/src/components/ModelInputWithFetch.tsx`)

新增组件 `ModelInputWithFetch`，提供：
- 输入框 + 获取按钮（初始状态）
- 输入框 + 下拉选择（获取成功后）
- 加载状态显示
- 按 `owned_by` 分组的模型列表
- 友好的错误提示

**Props**：
```typescript
interface ModelInputWithFetchProps {
  value: string;
  onChange: (value: string) => void;
  placeholder?: string;
  apiKey?: string;
  apiBaseUrl?: string;
  provider?: string;
  disabled?: boolean;
}
```

## 集成到 Settings 页面

### 方案 A：替换现有 Select（推荐）

在 `frontend/src/components/SettingsCurrentTab.tsx` 中：

```tsx
import ModelInputWithFetch from './ModelInputWithFetch';

// 在 Form.Item 中替换 Select
<Form.Item
  label="模型名称"
  name="llm_model"
  rules={[{ required: true, message: '请输入或选择模型名称' }]}
>
  <ModelInputWithFetch
    apiKey={form.getFieldValue('api_key')}
    apiBaseUrl={form.getFieldValue('api_base_url')}
    provider={form.getFieldValue('api_provider')}
  />
</Form.Item>
```

### 方案 B：保留现有 Select，添加获取按钮

如果希望保留现有的 Select 组件和模型选项逻辑，可以在 Select 旁边添加一个独立的获取按钮：

```tsx
<Space.Compact style={{ width: '100%' }}>
  <Form.Item
    name="llm_model"
    noStyle
    rules={[{ required: true, message: '请输入或选择模型名称' }]}
  >
    <Select
      showSearch
      placeholder="输入模型名称或点击获取"
      options={modelOptions}
      // ... 其他 props
    />
  </Form.Item>
  <Button
    icon={fetchingModels ? <LoadingOutlined /> : <DownloadOutlined />}
    onClick={handleFetchModels}
    loading={fetchingModels}
    title="从 API 提供商获取可用模型列表"
  >
    获取
  </Button>
</Space.Compact>
```

然后添加 `handleFetchModels` 函数：

```tsx
const handleFetchModels = async () => {
  const apiKey = form.getFieldValue('api_key');
  const apiBaseUrl = form.getFieldValue('api_base_url');
  const provider = form.getFieldValue('api_provider');

  if (!apiKey || !apiBaseUrl) {
    message.warning('请先填写 API Key 和 Base URL');
    return;
  }

  setFetchingModels(true);
  try {
    const response = await settingsApi.fetchModels({
      api_key: apiKey,
      api_base_url: apiBaseUrl,
      provider: provider,
    });

    if (response.success && response.models) {
      const options = response.models.map(model => ({
        value: model.id,
        label: model.id,
        description: model.owned_by || 'Unknown',
      }));
      setModelOptions(options);
      message.success(response.message || `成功获取 ${response.models.length} 个模型`);
    } else {
      message.error(response.message || response.error || '获取模型列表失败');
    }
  } catch (error) {
    console.error('获取模型列表失败:', error);
    message.error('获取模型列表失败，请检查网络连接');
  } finally {
    setFetchingModels(false);
  }
};
```

## 测试

### 后端测试

```bash
cd backend
python -c "from app.schemas.settings import FetchModelsRequest, FetchModelsResponse; print('Schema OK')"
```

### 前端测试

1. 启动前端开发服务器：
```bash
cd frontend
npm run dev
```

2. 访问设置页面
3. 填写 API Key 和 Base URL
4. 点击"获取"按钮
5. 验证模型列表是否正确显示

### API 测试（使用 curl）

```bash
curl -X POST http://localhost:8000/api/settings/fetch-models \
  -H "Content-Type: application/json" \
  -H "Cookie: token=YOUR_TOKEN" \
  -d '{
    "api_key": "sk-xxx",
    "api_base_url": "https://api.openai.com/v1",
    "provider": "openai"
  }'
```

## 支持的提供商

### 动态获取模型列表

支持所有 OpenAI 兼容的提供商，包括：

**国内提供商**：
- **DeepSeek**: `https://api.deepseek.com` 或 `https://api.deepseek.com/anthropic`
- **智谱 GLM**: `https://open.bigmodel.cn/api/anthropic`
- **Kimi (月之暗面)**: 支持标准端点
- **阶跃星辰**: `https://api.stepfun.com/step_plan`
- **豆包**: `https://ark.cn-beijing.volces.com/api/coding`
- **百炼**: `https://dashscope.aliyuncs.com/apps/anthropic`

**聚合站**：
- **硅基流动**: `https://api.siliconflow.cn`
- **OpenRouter**: `https://openrouter.ai/api`
- **其他 OpenAI 兼容代理**

**国际提供商**：
- **OpenAI**: `https://api.openai.com/v1`
- **Azure OpenAI**: 自定义端点
- **Anthropic**: `https://api.anthropic.com`（通过代理）
- **Google Gemini**: 通过兼容代理

### 路径剥离示例

**DeepSeek**：
```
输入: https://api.deepseek.com/anthropic
候选端点:
  1. https://api.deepseek.com/anthropic/v1/models
  2. https://api.deepseek.com/v1/models
  3. https://api.deepseek.com/models
```

**智谱 GLM**：
```
输入: https://open.bigmodel.cn/api/anthropic
候选端点:
  1. https://open.bigmodel.cn/api/anthropic/v1/models
  2. https://open.bigmodel.cn/v1/models
  3. https://open.bigmodel.cn/models
```

用户仍然可以手动输入其他模型名称。

## 错误处理

前端会根据后端返回的 `error_type` 显示相应的错误提示：

- `ValidationError`: "请先填写 API Key 和 Base URL"
- `AuthenticationError`: "API Key 认证失败"
- `EndpointNotFound`: "该提供商可能不支持模型列表接口"
- `TimeoutError`: "请求超时，请检查网络连接"
- `NetworkError`: "获取模型列表失败"

## 后续优化建议

1. **缓存机制**：对相同的 `api_key + api_base_url + provider` 组合缓存结果
2. **预设集成**：在预设管理中也添加模型获取功能
3. **模型过滤**：允许用户按模型类型（如 GPT-4、GPT-3.5）过滤
4. **模型详情**：显示模型的更多信息（如上下文长度、价格等）
5. **批量测试**：允许用户测试多个模型的可用性

## 相关文件

### 后端
- `backend/app/schemas/settings.py` - Schema 定义
- `backend/app/api/settings.py` - API 端点实现

### 前端
- `frontend/src/services/modules/settings.ts` - API 服务
- `frontend/src/components/ModelInputWithFetch.tsx` - UI 组件
- `frontend/src/components/SettingsCurrentTab.tsx` - 集成位置（待修改）

## 参考

- cc-switch 项目：`E:\Code\ProjectsCode\WorkSpace\Codex\NovelAi\cc-switch`
  - `src/lib/api/model-fetch.ts` - 模型获取逻辑
  - `src/components/providers/forms/shared/ModelInputWithFetch.tsx` - UI 组件
