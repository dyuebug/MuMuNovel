import React from 'react';
import { Radio, Card, Space, Tag } from 'antd';
import { CheckCircleOutlined } from '@ant-design/icons';

interface ProviderOption {
  value: string;
  label: string;
  description: string;
  features: string[];
  recommended?: boolean;
}

interface ProviderSelectorProps {
  value: string;
  onChange: (value: string) => void;
  disabled?: boolean;
}

const PROVIDER_OPTIONS: ProviderOption[] = [
  {
    value: 'openai',
    label: 'OpenAI（官方格式）',
    description: 'OpenAI Chat Completions 标准格式',
    features: ['官方 API', '稳定可靠', '功能完整'],
    recommended: true,
  },
  {
    value: 'openai_responses',
    label: 'OpenAI（Responses）',
    description: 'OpenAI Responses API 官方格式',
    features: ['官方 API', 'Responses'],
  },
  {
    value: 'anthropic',
    label: 'Claude（Anthropic）',
    description: 'Claude 官方 API',
    features: ['Anthropic', 'Messages API'],
  },
  {
    value: 'gemini',
    label: 'Google Gemini',
    description: 'Gemini 官方 API',
    features: ['Google', 'Generative AI'],
  },
  {
    value: 'azure',
    label: 'Azure OpenAI',
    description: '微软 Azure 云服务',
    features: ['企业级', '高可用', '合规性强'],
  },
  {
    value: 'newapi',
    label: 'NewAPI',
    description: 'One API 分支，国内支持',
    features: ['国内支持', '低成本', '多模型'],
  },
  {
    value: 'custom',
    label: '自定义',
    description: '其他 OpenAI 兼容服务',
    features: ['灵活配置', '自定义端点'],
  },
  {
    value: 'sub2api',
    label: 'Sub2API',
    description: 'OpenAI 兼容网关（Responses API）',
    features: ['OpenAI 兼容', 'Responses API'],
  },
];

const ProviderSelector: React.FC<ProviderSelectorProps> = ({
  value,
  onChange,
  disabled = false,
}) => {
  return (
    <div>
      <Radio.Group
        value={value}
        onChange={(e) => onChange(e.target.value)}
        disabled={disabled}
        style={{ width: '100%' }}
      >
        <Space direction="vertical" style={{ width: '100%' }} size="middle">
          {PROVIDER_OPTIONS.map((option) => (
            <Radio key={option.value} value={option.value} style={{ width: '100%' }}>
              <Card
                size="small"
                hoverable={!disabled}
                style={{
                  marginLeft: 8,
                  border: value === option.value ? '2px solid #1890ff' : '1px solid #d9d9d9',
                  backgroundColor: value === option.value ? '#e6f7ff' : '#fff',
                }}
              >
                <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
                  <div>
                    <div style={{ fontWeight: 'bold', marginBottom: 4 }}>
                      {option.label}
                      {option.recommended && (
                        <Tag color="green" style={{ marginLeft: 8 }}>
                          <CheckCircleOutlined /> 推荐
                        </Tag>
                      )}
                    </div>
                    <div style={{ color: '#666', fontSize: 12, marginBottom: 8 }}>
                      {option.description}
                    </div>
                    <Space size={4} wrap>
                      {option.features.map((feature) => (
                        <Tag key={feature} color="blue" style={{ fontSize: 11 }}>
                          {feature}
                        </Tag>
                      ))}
                    </Space>
                  </div>
                </div>
              </Card>
            </Radio>
          ))}
        </Space>
      </Radio.Group>
    </div>
  );
};

export default ProviderSelector;
