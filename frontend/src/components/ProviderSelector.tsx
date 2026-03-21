import React, { useMemo } from 'react';
import { Select, Space, Tag, Typography } from 'antd';
import { CheckCircleOutlined } from '@ant-design/icons';

const { Text } = Typography;

interface ProviderOption {
  value: string;
  label: string;
  shortLabel: string;
  description: string;
  features: string[];
  group: string;
  recommended?: boolean;
}

interface ProviderSelectorProps {
  value: string;
  onChange: (value: string) => void;
  disabled?: boolean;
}

interface ProviderSelectOption extends ProviderOption {
  searchText: string;
}

const PROVIDER_OPTIONS: ProviderOption[] = [
  {
    value: 'openai',
    label: 'OpenAI（官方格式）',
    shortLabel: 'OpenAI 官方',
    description: '标准 Chat Completions 格式，适合官方接口与大多数兼容服务。',
    features: ['官方 API', '稳定可靠', '功能完整'],
    group: '官方与标准协议',
    recommended: true,
  },
  {
    value: 'openai_responses',
    label: 'OpenAI（Responses）',
    shortLabel: 'OpenAI Responses',
    description: '使用 OpenAI Responses API，适合新版官方能力。',
    features: ['官方 API', 'Responses'],
    group: '官方与标准协议',
  },
  {
    value: 'anthropic',
    label: 'Claude（Anthropic）',
    shortLabel: 'Claude',
    description: 'Anthropic 官方 Messages API。',
    features: ['Anthropic', 'Messages API'],
    group: '官方与标准协议',
  },
  {
    value: 'gemini',
    label: 'Google Gemini',
    shortLabel: 'Gemini',
    description: 'Google 官方 Generative AI 接口。',
    features: ['Google', 'Generative AI'],
    group: '官方与标准协议',
  },
  {
    value: 'azure',
    label: 'Azure OpenAI',
    shortLabel: 'Azure OpenAI',
    description: 'Azure 企业级托管接口，适合合规和企业部署。',
    features: ['企业级', '高可用', '合规性强'],
    group: '官方与标准协议',
  },
  {
    value: 'newapi',
    label: 'NewAPI',
    shortLabel: 'NewAPI',
    description: '常见的 OpenAI 兼容中转或聚合接口。',
    features: ['国内支持', '低成本', '多模型'],
    group: '兼容网关与中转',
  },
  {
    value: 'sub2api',
    label: 'Sub2API',
    shortLabel: 'Sub2API',
    description: 'OpenAI 兼容网关，偏向 Responses API 兼容。',
    features: ['OpenAI 兼容', 'Responses API'],
    group: '兼容网关与中转',
  },
  {
    value: 'custom',
    label: '自定义',
    shortLabel: '自定义兼容服务',
    description: '其他 OpenAI 兼容服务或私有网关。',
    features: ['灵活配置', '自定义端点'],
    group: '兼容网关与中转',
  },
];

const ProviderSelector: React.FC<ProviderSelectorProps> = ({
  value,
  onChange,
  disabled = false,
}) => {
  const selectOptions = useMemo<ProviderSelectOption[]>(() => (
    PROVIDER_OPTIONS.map((option) => ({
      ...option,
      searchText: [option.label, option.shortLabel, option.description, option.group, ...option.features].join(' ').toLowerCase(),
    }))
  ), []);

  const selectedOption = useMemo(
    () => selectOptions.find((option) => option.value === value) || selectOptions[0],
    [selectOptions, value]
  );

  return (
    <Space direction="vertical" size={12} style={{ width: '100%' }}>
      <Select
        value={value}
        onChange={onChange}
        disabled={disabled}
        size="large"
        showSearch
        optionFilterProp="label"
        placeholder="选择 API 提供商"
        popupMatchSelectWidth={false}
        style={{ width: '100%' }}
        options={selectOptions}
        filterOption={(input, option) => {
          const rawOption = option as ProviderSelectOption | undefined;
          return Boolean(rawOption?.searchText.includes(input.toLowerCase()));
        }}
        optionRender={(option) => {
          const rawOption = option.data as ProviderSelectOption;
          return (
            <Space direction="vertical" size={6} style={{ width: '100%' }}>
              <Space wrap size={8}>
                <Text strong>{rawOption.label}</Text>
                <Tag color="processing" style={{ marginInlineEnd: 0 }}>{rawOption.group}</Tag>
                {rawOption.recommended && (
                  <Tag color="green" style={{ marginInlineEnd: 0 }}>
                    <CheckCircleOutlined /> 推荐
                  </Tag>
                )}
              </Space>
              <Text style={{ fontSize: 12, color: '#666' }}>{rawOption.description}</Text>
              <Space size={4} wrap>
                {rawOption.features.map((feature) => (
                  <Tag key={feature} color="blue" style={{ fontSize: 11, marginInlineEnd: 0 }}>
                    {feature}
                  </Tag>
                ))}
              </Space>
            </Space>
          );
        }}
      />

      <div
        style={{
          padding: '12px 14px',
          borderRadius: 12,
          border: '1px solid rgba(24, 144, 255, 0.15)',
          background: 'linear-gradient(135deg, rgba(24, 144, 255, 0.08) 0%, rgba(255, 255, 255, 0.98) 100%)',
        }}
      >
        <Space direction="vertical" size={6} style={{ width: '100%' }}>
          <Space wrap size={8}>
            <Text strong>{selectedOption.label}</Text>
            <Tag color="processing" style={{ marginInlineEnd: 0 }}>{selectedOption.group}</Tag>
            {selectedOption.recommended && (
              <Tag color="green" style={{ marginInlineEnd: 0 }}>
                <CheckCircleOutlined /> 推荐默认选择
              </Tag>
            )}
          </Space>
          <Text style={{ color: '#666', lineHeight: 1.7 }}>{selectedOption.description}</Text>
          <Space size={4} wrap>
            {selectedOption.features.map((feature) => (
              <Tag key={feature} color="blue" style={{ marginInlineEnd: 0 }}>
                {feature}
              </Tag>
            ))}
          </Space>
          <Text style={{ fontSize: 12, color: '#8c8c8c' }}>
            {"如果你使用中转站、NewAPI 或 OpenAI 兼容网关，通常优先选择“NewAPI”或“自定义”。"}
          </Text>
        </Space>
      </div>
    </Space>
  );
};

export default ProviderSelector;
