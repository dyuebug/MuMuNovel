import React, { useMemo } from 'react';
import { CheckCircleOutlined } from '@ant-design/icons';
import { Select, Space, Tag, Typography, theme } from 'antd';
import { designDisplayFont } from '../theme/themeConfig';

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
    label: 'OpenAI 兼容接口',
    shortLabel: 'OpenAI 兼容',
    description: '统一使用 OpenAI-compatible 基础接入，适合官方接口、NewAPI、OpenRouter、硅基流动、DeepSeek、Kimi、GLM 与私有中转。',
    features: ['基础接入', '多模型', '兼容网关'],
    group: '推荐协议',
    recommended: true,
  },
  {
    value: 'anthropic',
    label: 'Claude（Anthropic）',
    shortLabel: 'Claude',
    description: 'Anthropic 官方 Messages API，适合直接使用 Claude 系列模型。',
    features: ['Claude 官方', 'Messages API', '长上下文'],
    group: '官方直连',
  },
  {
    value: 'gemini',
    label: 'Google Gemini',
    shortLabel: 'Gemini',
    description: 'Google 官方 Generative Language API，适合直接使用 Gemini 系列模型。',
    features: ['Gemini 官方', '多模态', '模型列表'],
    group: '官方直连',
  },
];

const PROVIDER_GUIDE = [
  '如果你正在使用中转站、NewAPI 或 OpenAI-compatible 网关，优先选择 OpenAI 兼容接口。',
  '如果你希望直接走 Anthropic 官方链路，再选择 Claude（Anthropic）。',
  '如果你需要 Gemini 官方 API 或多模态模型列表，再切换到 Google Gemini。',
];

const getProviderTone = (option: ProviderOption, token: ReturnType<typeof theme.useToken>['token']) => {
  switch (option.value) {
    case 'anthropic':
      return {
        accent: token.colorInfo,
        tagColor: 'cyan',
        featureFill: alphaColor(token.colorInfo, 0.1),
      };
    case 'gemini':
      return {
        accent: token.colorSuccess,
        tagColor: 'green',
        featureFill: alphaColor(token.colorSuccess, 0.1),
      };
    default:
      return {
        accent: token.colorPrimary,
        tagColor: 'processing',
        featureFill: alphaColor(token.colorPrimary, 0.1),
      };
  }
};

const alphaColor = (color: string, alpha: number) =>
  `color-mix(in srgb, ${color} ${(alpha * 100).toFixed(0)}%, transparent)`;

const ProviderSelector: React.FC<ProviderSelectorProps> = ({
  value,
  onChange,
  disabled = false,
}) => {
  const { token } = theme.useToken();

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

  const selectedTone = getProviderTone(selectedOption, token);

  return (
    <Space direction="vertical" size={14} style={{ width: '100%' }}>
      <div
        style={{
          padding: '16px 18px',
          borderRadius: 20,
          background: `linear-gradient(135deg, ${alphaColor(token.colorPrimaryBg, 0.88)} 0%, ${alphaColor(token.colorBgElevated, 0.98)} 100%)`,
          border: `1px solid ${alphaColor(token.colorPrimary, 0.14)}`,
          boxShadow: `0 16px 32px ${alphaColor(token.colorText, 0.05)}`,
        }}
      >
        <div
          style={{
            display: 'grid',
            gridTemplateColumns: 'minmax(0, 1fr) auto',
            gap: 16,
            alignItems: 'start',
          }}
        >
          <div style={{ minWidth: 0 }}>
            <Text style={{ display: 'block', fontSize: 11, letterSpacing: '0.08em', textTransform: 'uppercase', color: token.colorTextTertiary, marginBottom: 6 }}>
              Provider Workspace
            </Text>
            <Text
              strong
              style={{
                display: 'block',
                fontSize: 18,
                marginBottom: 8,
                fontFamily: designDisplayFont,
                letterSpacing: '-0.03em',
              }}
            >
              选择最合适的 API 接入协议
            </Text>
            <Text type="secondary" style={{ display: 'block', lineHeight: 1.75 }}>
              先根据你是否使用官方直连或兼容网关选择 provider，再继续配置模型、密钥与端点。这里只调整阅读顺序和信息层级，不改变任何设置逻辑。
            </Text>
          </div>
          <Space wrap size={[8, 8]} style={{ justifyContent: 'flex-end' }}>
            <Tag color={selectedTone.tagColor} style={{ margin: 0, borderRadius: 999, paddingInline: 10 }}>
              当前 {selectedOption.shortLabel}
            </Tag>
            {selectedOption.recommended ? (
              <Tag color="success" style={{ margin: 0, borderRadius: 999, paddingInline: 10 }}>
                推荐默认路径
              </Tag>
            ) : (
              <Tag color="default" style={{ margin: 0, borderRadius: 999, paddingInline: 10 }}>
                官方专属链路
              </Tag>
            )}
          </Space>
        </div>
      </div>

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
          const tone = getProviderTone(rawOption, token);

          return (
            <div style={{ display: 'grid', gap: 8, paddingBlock: 4 }}>
              <Space wrap size={[8, 8]}>
                <Text strong>{rawOption.label}</Text>
                <Tag color={tone.tagColor} style={{ margin: 0, borderRadius: 999, paddingInline: 10 }}>
                  {rawOption.group}
                </Tag>
                {rawOption.recommended ? (
                  <Tag color="success" style={{ margin: 0, borderRadius: 999, paddingInline: 10 }}>
                    <CheckCircleOutlined /> 推荐
                  </Tag>
                ) : null}
              </Space>
              <Text style={{ fontSize: 12, color: token.colorTextSecondary, lineHeight: 1.7 }}>
                {rawOption.description}
              </Text>
              <Space size={[6, 6]} wrap>
                {rawOption.features.map((feature) => (
                  <Tag
                    key={feature}
                    style={{
                      margin: 0,
                      borderRadius: 999,
                      paddingInline: 10,
                      borderColor: alphaColor(tone.accent, 0.18),
                      background: tone.featureFill,
                      color: tone.accent,
                    }}
                  >
                    {feature}
                  </Tag>
                ))}
              </Space>
            </div>
          );
        }}
      />

      <div
        style={{
          display: 'grid',
          gridTemplateColumns: 'minmax(0, 1.45fr) minmax(260px, 0.95fr)',
          gap: 14,
        }}
      >
        <div
          style={{
            padding: 18,
            borderRadius: 22,
            border: `1px solid ${alphaColor(selectedTone.accent, 0.18)}`,
            background: `linear-gradient(135deg, ${alphaColor(selectedTone.accent, 0.12)} 0%, ${alphaColor(token.colorBgElevated, 0.98)} 100%)`,
            boxShadow: `0 16px 32px ${alphaColor(token.colorText, 0.05)}`,
          }}
        >
          <div
            style={{
              display: 'grid',
              gridTemplateColumns: 'minmax(0, 1fr) auto',
              gap: 14,
              alignItems: 'start',
            }}
          >
            <div style={{ minWidth: 0 }}>
              <Text style={{ display: 'block', fontSize: 11, letterSpacing: '0.08em', textTransform: 'uppercase', color: token.colorTextTertiary, marginBottom: 6 }}>
                Provider Dossier
              </Text>
              <Text
                strong
                style={{
                  display: 'block',
                  fontSize: 17,
                  marginBottom: 8,
                  fontFamily: designDisplayFont,
                  letterSpacing: '-0.03em',
                }}
              >
                {selectedOption.label}
              </Text>
              <Text type="secondary" style={{ display: 'block', lineHeight: 1.75 }}>
                {selectedOption.description}
              </Text>
            </div>
            <Space wrap size={[8, 8]} style={{ justifyContent: 'flex-end' }}>
              <Tag color={selectedTone.tagColor} style={{ margin: 0, borderRadius: 999, paddingInline: 10 }}>
                {selectedOption.group}
              </Tag>
              {selectedOption.recommended ? (
                <Tag color="success" style={{ margin: 0, borderRadius: 999, paddingInline: 10 }}>
                  <CheckCircleOutlined /> 默认推荐
                </Tag>
              ) : null}
            </Space>
          </div>

          <div
            style={{
              marginTop: 14,
              padding: '14px 16px',
              borderRadius: 18,
              background: alphaColor(token.colorBgElevated, 0.96),
              border: `1px solid ${alphaColor(token.colorBorderSecondary, 0.86)}`,
            }}
          >
            <Text style={{ display: 'block', fontSize: 11, letterSpacing: '0.08em', textTransform: 'uppercase', color: token.colorTextTertiary, marginBottom: 8 }}>
              Capability Snapshot
            </Text>
            <Space size={[8, 8]} wrap>
              {selectedOption.features.map((feature) => (
                <Tag
                  key={feature}
                  style={{
                    margin: 0,
                    borderRadius: 999,
                    paddingInline: 10,
                    borderColor: alphaColor(selectedTone.accent, 0.18),
                    background: selectedTone.featureFill,
                    color: selectedTone.accent,
                  }}
                >
                  {feature}
                </Tag>
              ))}
            </Space>
          </div>
        </div>

        <div
          style={{
            padding: 18,
            borderRadius: 22,
            background: alphaColor(token.colorFillQuaternary, 0.72),
            border: `1px solid ${alphaColor(token.colorBorderSecondary, 0.88)}`,
          }}
        >
          <Text style={{ display: 'block', fontSize: 11, letterSpacing: '0.08em', textTransform: 'uppercase', color: token.colorTextTertiary, marginBottom: 6 }}>
            Selection Guide
          </Text>
          <Text
            strong
            style={{
              display: 'block',
              fontSize: 16,
              marginBottom: 8,
              fontFamily: designDisplayFont,
              letterSpacing: '-0.02em',
            }}
          >
            如何快速判断该选哪一类
          </Text>
          <div style={{ display: 'grid', gap: 8 }}>
            {PROVIDER_GUIDE.map((item) => (
              <Text key={item} type="secondary" style={{ lineHeight: 1.75 }}>
                • {item}
              </Text>
            ))}
          </div>
          <Text style={{ display: 'block', fontSize: 12, lineHeight: 1.7, color: token.colorTextTertiary, marginTop: 12 }}>
            如果你连接的是中转、代理或聚合供应商，即使背后模型来自 Claude 或 Gemini，也通常仍然优先走 OpenAI 兼容入口。
          </Text>
        </div>
      </div>
    </Space>
  );
};

export default ProviderSelector;
