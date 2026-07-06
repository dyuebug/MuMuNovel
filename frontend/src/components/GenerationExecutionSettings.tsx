/* eslint-disable react-refresh/only-export-components */
import { useCallback, useEffect, useRef, useState } from 'react';
import { Alert, Card, Select, Space, Switch, Typography, theme } from 'antd';

import { settingsApi } from '../services/modularApi';
import { renderCompactSettingHint } from './storyCreationCommonUi';

const { Text } = Typography;

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
  const mountedRef = useRef(true);
  const requestIdRef = useRef(0);

  useEffect(() => {
    mountedRef.current = true;
    return () => {
      mountedRef.current = false;
      requestIdRef.current += 1;
    };
  }, []);

  const loadDefaults = useCallback(async () => {
    requestIdRef.current += 1;
    const requestId = requestIdRef.current;
    setFetchingModels(true);
    try {
      const { settings, storedApiKey } = await settingsApi.getSettingsWithStoredApiKey();
      if (!mountedRef.current || requestIdRef.current !== requestId) {
        return { provider: undefined, model: undefined, webResearchEnabled: false };
      }
      const provider = (settings.provider_type || settings.api_provider || '').trim() || undefined;
      const model = settings.llm_model?.trim() || undefined;
      const webResearchEnabled = Boolean(settings.web_research_enabled);

      setRuntimeProvider(provider);
      setCurrentSettingsModel(model);

      if (!provider || !storedApiKey || !settings.api_base_url) {
        setAvailableModels([]);
        return { provider, model, webResearchEnabled };
      }

      const modelsResponse = await settingsApi.getAvailableModels({
        api_key: storedApiKey,
        api_base_url: settings.api_base_url,
        provider,
      });
      if (!mountedRef.current || requestIdRef.current !== requestId) {
        return { provider, model, webResearchEnabled };
      }

      setAvailableModels(normalizeModelOptions(modelsResponse.models));
      return { provider, model, webResearchEnabled };
    } catch (error) {
      if (mountedRef.current && requestIdRef.current === requestId) {
        setAvailableModels([]);
      }
      throw error;
    } finally {
      if (mountedRef.current && requestIdRef.current === requestId) {
        setFetchingModels(false);
      }
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
}: Omit<GenerationExecutionSettingsPanelProps, 'title' | 'card'>) => {
  const { token } = theme.useToken();
  const alphaColor = (color: string, alpha: number) => `color-mix(in srgb, ${color} ${(alpha * 100).toFixed(0)}%, transparent)`;
  const panelStyle = {
    padding: '14px 14px',
    borderRadius: 16,
    border: `1px solid ${alphaColor(token.colorBorderSecondary, 0.9)}`,
    background: `linear-gradient(180deg, ${alphaColor(token.colorBgElevated, 0.98)} 0%, ${alphaColor(token.colorFillQuaternary, 0.44)} 100%)`,
  };
  const eyebrowStyle = {
    display: 'block',
    fontSize: 11,
    letterSpacing: '0.08em',
    textTransform: 'uppercase' as const,
    color: token.colorTextTertiary,
    marginBottom: 6,
  };
  const renderModelStatusHint = (
    title: string,
    detail: string,
    tone: 'info' | 'warning' = 'info',
  ) => (
    <div style={{ padding: '10px 12px' }}>
      {renderCompactSettingHint(title, detail, {
        tone,
        style: {
          marginBottom: 0,
          padding: '10px 12px',
          borderRadius: 16,
          boxShadow: 'none',
        },
      })}
    </div>
  );

  return (
    <Space data-testid="generation-execution-settings-panel" direction="vertical" style={{ width: '100%' }} size={14}>
      <div>
        <Text style={eyebrowStyle}>Execution Controls</Text>
        <Text strong style={{ display: 'block', fontSize: 17, marginBottom: 6 }}>
          生成执行设置
        </Text>
        <Text type="secondary" style={{ display: 'block', lineHeight: 1.7, marginBottom: 12 }}>
          默认沿用当前用户设置的提供商与模型。这里只保留最常用的执行开关，适合在开始生成前快速确认运行策略。
        </Text>
        <Space wrap size={[8, 8]}>
          <Text
            style={{
              padding: '4px 10px',
              borderRadius: 999,
              background: alphaColor(token.colorPrimary, 0.08),
              border: `1px solid ${alphaColor(token.colorPrimary, 0.12)}`,
            }}
          >
            默认提供商：{runtimeProvider || '未读取到'}
          </Text>
          <Text
            style={{
              padding: '4px 10px',
              borderRadius: 999,
              background: alphaColor(token.colorInfo, 0.08),
              border: `1px solid ${alphaColor(token.colorInfo, 0.12)}`,
            }}
          >
            默认模型：{currentSettingsModel || '未读取到'}
          </Text>
        </Space>
      </div>

      <Alert
        data-testid="generation-execution-settings-info"
        type="info"
        showIcon
        style={{
          borderRadius: 14,
          border: `1px solid ${alphaColor(token.colorInfo, 0.12)}`,
          background: `linear-gradient(135deg, ${alphaColor(token.colorInfoBg, 0.9)} 0%, ${alphaColor(token.colorBgContainer, 0.98)} 100%)`,
        }}
        message="如果当前页面已经开启联网搜索或研究增强，也会继续沿用页面侧配置；留空模型覆盖时，系统将继续使用全局默认模型。"
      />

      <div
        style={{
          display: 'grid',
          gridTemplateColumns: 'repeat(auto-fit, minmax(240px, 1fr))',
          gap: 12,
        }}
      >
        <div style={panelStyle}>
          <Text style={eyebrowStyle}>Tooling</Text>
          <Text strong style={{ display: 'block', marginBottom: 6 }}>
            启用 MCP 工具增强
          </Text>
          <Text type="secondary" style={{ display: 'block', lineHeight: 1.7, marginBottom: 14 }}>
            决定这次生成是否启用额外工具能力，适合需要检索、辅助分析或更复杂推理时开启。
          </Text>
          <Space align="center" size={12}>
            <Switch
              checked={enableMcp}
              onChange={onEnableMcpChange}
              checkedChildren="开启"
              unCheckedChildren="关闭"
            />
            <Text type={enableMcp ? undefined : 'secondary'}>
              {enableMcp ? '本次任务将允许调用 MCP 工具' : '本次任务仅使用默认执行能力'}
            </Text>
          </Space>
        </div>

        <div style={panelStyle}>
          <Text style={eyebrowStyle}>Model Override</Text>
          <Text strong style={{ display: 'block', marginBottom: 6 }}>
            模型覆盖
          </Text>
          <Text type="secondary" style={{ display: 'block', lineHeight: 1.7, marginBottom: 14 }}>
            只有当你明确想切换这次任务的模型时再手动指定，否则优先沿用系统默认模型组合。
          </Text>
          <Select
            allowClear
            showSearch
            value={model}
            onChange={onModelChange}
            loading={fetchingModels}
            placeholder="留空则使用当前默认模型"
            optionFilterProp="label"
            notFoundContent={
              fetchingModels
                ? renderModelStatusHint(
                    '模型候选正在返回',
                    '这次只是在做单次任务覆盖，稍等候选列表返回即可；如果不想覆盖，也可以继续保持留空。',
                  )
                : renderModelStatusHint(
                    '暂时没有可选模型',
                    '当前仍可保持留空，继续沿用默认模型完成这次任务，不会改写全局设置。',
                    'warning',
                  )
            }
            options={availableModels.map((item) => ({
              value: item.value,
              label: item.label,
              title: item.description,
            }))}
          />
          <Text type="secondary" style={{ display: 'block', marginTop: 10, fontSize: 12, lineHeight: 1.6 }}>
            当前页面只做单次任务覆盖，不会改写全局设置。
          </Text>
        </div>
      </div>
    </Space>
  );
};

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
    <Card
      size="small"
      title={title}
      style={{ marginBottom: 24, borderRadius: 20 }}
      styles={{ body: { padding: 16 } }}
    >
      {content}
    </Card>
  );
};
