/* eslint-disable react-refresh/only-export-components */
import { useCallback, useState } from 'react';
import { Alert, Card, Col, Row, Select, Space, Switch } from 'antd';

import { settingsApi } from '../services/api';

export type ModelOption = {
  value: string;
  label: string;
  description?: string;
};

export const normalizeModelOptions = (rawModels: unknown): ModelOption[] => {
  if (!Array.isArray(rawModels)) {
    return [];
  }

  const seen = new Set<string>();
  const options: ModelOption[] = [];

  rawModels.forEach((item) => {
    let value = '';
    let label = '';
    let description: string | undefined;

    if (typeof item === 'string') {
      value = item.trim();
      label = value;
    } else if (item && typeof item === 'object') {
      const record = item as Record<string, unknown>;
      value = String(record.value ?? record.id ?? record.name ?? record.label ?? '').trim();
      label = String(record.label ?? record.name ?? record.value ?? record.id ?? '').trim();
      description = typeof record.description === 'string' ? record.description : undefined;
    }

    if (!value || seen.has(value)) {
      return;
    }

    seen.add(value);
    options.push({
      value,
      label: label || value,
      description,
    });
  });

  return options;
};

export const useGenerationExecutionSettings = () => {
  const [availableModels, setAvailableModels] = useState<ModelOption[]>([]);
  const [fetchingModels, setFetchingModels] = useState(false);
  const [runtimeProvider, setRuntimeProvider] = useState<string | undefined>();
  const [currentSettingsModel, setCurrentSettingsModel] = useState<string | undefined>();

  const loadDefaults = useCallback(async () => {
    setFetchingModels(true);
    try {
      const settings = await settingsApi.getSettings();
      const provider = (settings.provider_type || settings.api_provider || '').trim() || undefined;
      const model = settings.llm_model?.trim() || undefined;
      const webResearchEnabled = Boolean(settings.web_research_enabled);

      setRuntimeProvider(provider);
      setCurrentSettingsModel(model);

      if (!provider || !settings.api_key || !settings.api_base_url) {
        setAvailableModels([]);
        return { provider, model, webResearchEnabled };
      }

      const modelsResponse = await settingsApi.getAvailableModels({
        api_key: settings.api_key,
        api_base_url: settings.api_base_url,
        provider,
      });

      setAvailableModels(normalizeModelOptions(modelsResponse.models));
      return { provider, model, webResearchEnabled };
    } catch (error) {
      setAvailableModels([]);
      throw error;
    } finally {
      setFetchingModels(false);
    }
  }, []);

  return {
    availableModels,
    fetchingModels,
    runtimeProvider,
    currentSettingsModel,
    loadDefaults,
  };
};

type GenerationExecutionSettingsPanelProps = {
  enableMcp: boolean;
  onEnableMcpChange: (value: boolean) => void;
  model?: string;
  onModelChange: (value?: string) => void;
  fetchingModels: boolean;
  availableModels: ModelOption[];
  runtimeProvider?: string;
  currentSettingsModel?: string;
  title?: string;
  card?: boolean;
};

const ExecutionFields = ({
  enableMcp,
  onEnableMcpChange,
  model,
  onModelChange,
  fetchingModels,
  availableModels,
  runtimeProvider,
  currentSettingsModel,
}: Omit<GenerationExecutionSettingsPanelProps, 'title' | 'card'>) => (
  <Space data-testid="generation-execution-settings-panel" direction="vertical" style={{ width: '100%' }} size="middle">
    <Alert data-testid="generation-execution-settings-info"
      type="info"
      showIcon
      message="默认沿用当前用户设置的提供商与模型。这里仅保留常用执行开关；如果当前页面已开启联网搜索或研究增强，也会继续沿用页面侧配置；留空时继续使用系统默认模型。"
    />

    <div>
      <div style={{ marginBottom: 8, fontWeight: 500 }}>启用 MCP 工具增强</div>
      <Switch
        checked={enableMcp}
        onChange={onEnableMcpChange}
        checkedChildren="开启"
        unCheckedChildren="关闭"
      />
    </div>

    <div>
      <div style={{ marginBottom: 8, fontWeight: 500 }}>模型覆盖</div>
      <Select
        allowClear
        showSearch
        value={model}
        onChange={onModelChange}
        loading={fetchingModels}
        placeholder="留空则使用当前默认模型"
        optionFilterProp="label"
        notFoundContent={fetchingModels ? '正在加载模型列表...' : '未获取到模型列表，可继续使用默认模型'}
        options={availableModels.map((item) => ({
          value: item.value,
          label: item.label,
          title: item.description,
        }))}
      />
      <div style={{ marginTop: 8, color: 'var(--ant-color-text-secondary)', fontSize: 12 }}>
        当前默认提供商：{runtimeProvider || '未读取到'}
        {' · '}
        当前默认模型：{currentSettingsModel || '未读取到'}
      </div>
    </div>
  </Space>
);

export const GenerationExecutionSettingsPanel = ({
  enableMcp,
  onEnableMcpChange,
  model,
  onModelChange,
  fetchingModels,
  availableModels,
  runtimeProvider,
  currentSettingsModel,
  title = '执行设置',
  card = true,
}: GenerationExecutionSettingsPanelProps) => {
  const content = (
    <ExecutionFields
      enableMcp={enableMcp}
      onEnableMcpChange={onEnableMcpChange}
      model={model}
      onModelChange={onModelChange}
      fetchingModels={fetchingModels}
      availableModels={availableModels}
      runtimeProvider={runtimeProvider}
      currentSettingsModel={currentSettingsModel}
    />
  );

  if (!card) {
    return content;
  }

  return (
    <Card size="small" title={title} style={{ marginBottom: 24 }}>
      <Row gutter={16}>
        <Col span={24}>{content}</Col>
      </Row>
    </Card>
  );
};
