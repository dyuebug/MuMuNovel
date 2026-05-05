# API 模型获取功能 - 快速参考

## 更新说明

### 2026-05-05 更新
- ✅ 移除预设模型列表（Anthropic、Gemini）
- ✅ 实现智能路径剥离，支持动态获取
- ✅ 支持 DeepSeek、GLM、Kimi 等国内提供商
- ✅ 支持硅基流动、OpenRouter 等聚合站

## 核心功能

### 1. 智能路径剥离

自动识别并剥离已知的 Anthropic 协议兼容子路径：

**支持的子路径**（按长度降序匹配）：
```
/api/claudecode
/api/anthropic
/apps/anthropic
/api/coding
/claudecode
/anthropic
/step_plan
/coding
/claude
```

**示例**：
```
输入: https://api.deepseek.com/anthropic
候选端点:
  1. https://api.deepseek.com/anthropic/v1/models
  2. https://api.deepseek.com/v1/models
  3. https://api.deepseek.com/models
```

### 2. 支持的提供商

#### 国内提供商
- **DeepSeek**: `https://api.deepseek.com` 或 `https://api.deepseek.com/anthropic`
- **智谱 GLM**: `https://open.bigmodel.cn/api/anthropic`
- **Kimi (月之暗面)**: 支持标准端点
- **阶跃星辰**: `https://api.stepfun.com/step_plan`
- **豆包**: `https://ark.cn-beijing.volces.com/api/coding`
- **百炼**: `https://dashscope.aliyuncs.com/apps/anthropic`

#### 聚合站
- **硅基流动**: `https://api.siliconflow.cn`
- **OpenRouter**: `https://openrouter.ai/api`
- **其他 OpenAI 兼容代理**

#### 国际提供商
- **OpenAI**: `https://api.openai.com/v1`
- **Azure OpenAI**: 自定义端点
- **Anthropic**: `https://api.anthropic.com`（通过代理）

### 3. 工作流程

```
用户点击"获取模型"
    ↓
检查是否有自定义 models_url
    ↓
是 → 直接使用自定义 URL
否 → 构建候选端点列表
    ↓
尝试主端点 (base_url/v1/models)
    ↓
失败 → 尝试路径剥离后的端点
    ↓
成功 → 返回模型列表
失败 → 返回错误提示
```

## 使用方式

### 后端 API

**端点**：`POST /api/settings/fetch-models`

**请求示例 1 - DeepSeek**：
```json
{
  "api_key": "sk-xxx",
  "api_base_url": "https://api.deepseek.com/anthropic",
  "provider": "deepseek"
}
```

**响应**：
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

**请求示例 2 - 智谱 GLM**：
```json
{
  "api_key": "xxx.yyy",
  "api_base_url": "https://open.bigmodel.cn/api/anthropic",
  "provider": "glm"
}
```

**请求示例 3 - 硅基流动**：
```json
{
  "api_key": "sk-xxx",
  "api_base_url": "https://api.siliconflow.cn",
  "provider": "siliconflow"
}
```

### 前端组件

```tsx
import ModelInputWithFetch from './ModelInputWithFetch';

<ModelInputWithFetch
  value={modelName}
  onChange={setModelName}
  apiKey={apiKey}
  apiBaseUrl={apiBaseUrl}
  provider="deepseek"  // 或 "glm", "siliconflow" 等
/>
```

## 测试验证

```bash
cd backend
python test_fetch_models.py
```

**测试覆盖**：
- ✅ OpenAI 标准端点
- ✅ 自定义 models_url
- ✅ 响应模型解析
- ✅ DeepSeek 路径剥离
- ✅ GLM 路径剥离

## 技术优势

1. **智能路径剥离**：自动识别并处理兼容子路径
2. **多候选端点**：失败自动尝试下一个端点
3. **广泛兼容**：支持国内外主流提供商
4. **动态获取**：实时获取最新模型列表
5. **错误友好**：清晰的错误提示和建议

## 路径剥离示例

### DeepSeek
```
输入: https://api.deepseek.com/anthropic
候选:
  1. https://api.deepseek.com/anthropic/v1/models
  2. https://api.deepseek.com/v1/models
  3. https://api.deepseek.com/models
```

### 智谱 GLM
```
输入: https://open.bigmodel.cn/api/anthropic
候选:
  1. https://open.bigmodel.cn/api/anthropic/v1/models
  2. https://open.bigmodel.cn/v1/models
  3. https://open.bigmodel.cn/models
```

### 阶跃星辰
```
输入: https://api.stepfun.com/step_plan
候选:
  1. https://api.stepfun.com/step_plan/v1/models
  2. https://api.stepfun.com/v1/models
  3. https://api.stepfun.com/models
```

## 相关文件

- `backend/app/api/settings.py` - 预设模型定义 + API 端点
- `backend/test_fetch_models.py` - 测试脚本
- `INTEGRATION_GUIDE.md` - 完整集成指南
- `IMPLEMENTATION_SUMMARY.md` - 实施总结

## 下一步

1. ✅ 后端预设模型已添加
2. ✅ 测试验证通过
3. ⏳ 前端集成到 Settings 页面
4. ⏳ 用户测试不同提供商

---

**更新日期**：2026-05-05  
**状态**：预设模型功能已完成 ✅
