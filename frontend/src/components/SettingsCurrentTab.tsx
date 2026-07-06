/* eslint-disable @typescript-eslint/no-explicit-any */
import { Suspense } from 'react';
import { Alert, Button, Card, Col, Form, Input, InputNumber, Radio, Row, Segmented, Select, Slider, Space, Switch, Tag, Typography, theme } from 'antd';
import { CheckCircleOutlined, CloseCircleOutlined, DeleteOutlined, InfoCircleOutlined, ReloadOutlined, SaveOutlined, ThunderboltOutlined } from '@ant-design/icons';
import { designDisplayFont } from '../theme/themeConfig';
import InlineDeferredPanel from './InlineDeferredPanel';
import { renderCompactSettingHint } from './storyCreationCommonUi';

const { Text } = Typography;
const { TextArea } = Input;
const alphaColor = (color: string, alpha: number) =>
  `color-mix(in srgb, ${color} ${(alpha * 100).toFixed(0)}%, transparent)`;

export default function SettingsCurrentTab(props: any) {
  const { token } = theme.useToken();
  const {
  LazyEndpointListEditor,
  LazyProviderSelector,
  activeSettingsSection,
  activeSettingsSectionMeta,
  clipDisplayText,
  endpoints,
  fallbackStrategy,
  fetchingModels,
  fieldHintTextStyle,
  fieldPanelStyle,
  form,
  handleDelete,
  handleFetchModels,
  handleModelSelectFocus,
  handleProviderChange,
  handleReset,
  handleSave,
  handleTestConnection,
  handleTestWebResearch,
  hasSettings,
  initialLoading,
  isDefaultSettings,
  isMobile,
  loading,
  mergedModelOptions,
  modelOptions,
  modelSearchText,
  modelsFetched,
  renderSectionTitle,
  sectionCardStyle,
  sectionCardStyles,
  selectedProvider,
  setActiveSettingsSection,
  setEndpoints,
  setFallbackStrategy,
  setModelsFetched,
  setModelSearchText,
  setShowTestResult,
  settingsLazyFallback,
  settingsSectionItems,
  setWebResearchTestResult,
  showTestResult,
  testResult,
  testingApi,
  testingWebResearchProvider,
  watchedBaseUrl,
  watchedExaEnabled,
  watchedGrokEnabled,
  watchedGrokSearchEnabled,
  watchedMaxTokens,
  watchedModel,
  watchedProvider,
  watchedTemperature,
  watchedWebResearchEnabled,
  webResearchTestResult
  } = props;
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
          padding: isMobile ? '10px 12px' : '12px 14px',
          borderRadius: 16,
          boxShadow: 'none',
        },
      })}
    </div>
  );
  const providerHint = props.providerHint ?? {};


  const apiKeyInputPlaceholder = form.getFieldValue('api_key')
    ? 'sk-...'
    : '已保存密钥；留空表示保持不变，输入新值可覆盖';
  const watchedModelsEndpoint = form.getFieldValue('models_endpoint_url');

  const endpointDiagnostics = testResult?.details?.endpoint_diagnostics as {
    primary_endpoint?: string;
    backup_endpoints?: string[];
    configured_endpoint_count?: number;
    fallback_strategy?: string;
    auto_failover_enabled?: boolean;
  } | undefined;
  const backupEndpoints = Array.isArray(endpointDiagnostics?.backup_endpoints)
    ? endpointDiagnostics.backup_endpoints
    : [];
  const transportDiagnostics = testResult?.details?.transport_diagnostics as {
    summary?: {
      total_attempts?: number;
      successful_attempts?: number;
      api_modes_tried?: string[];
      backup_endpoint_used?: boolean;
      api_mode_fallback_used?: boolean;
      forced_chat_completions?: boolean;
      normalized_base_url_used?: boolean;
    };
    attempts?: Array<{
      api_mode?: string;
      endpoint_role?: string;
      base_url?: string;
      endpoint_path?: string;
      attempt_number?: number;
      max_attempts?: number;
      result?: string;
      status_code?: number;
      error_type?: string;
    }>;
  } | undefined;
  const transportAttempts = Array.isArray(transportDiagnostics?.attempts)
    ? transportDiagnostics.attempts.slice(-3)
    : [];
  const webResearchSearchStatus = webResearchTestResult?.search_status
    ?? (webResearchTestResult?.success
      ? ((webResearchTestResult?.source_count ?? 0) > 0 ? 'success_with_sources' : 'success_without_sources')
      : 'failed');
  const webResearchStatusNote = webResearchTestResult?.status_note
    ?? (webResearchSearchStatus === 'success_without_sources'
      ? '已联网检索（本次未返回可展示来源）'
      : webResearchTestResult?.sources_backfilled
        ? '已联网检索，来源已由 Exa 自动补全。'
        : undefined);

  return (
                    <Space direction="vertical" size={isMobile ? 'middle' : 'large'} style={{ width: '100%' }}>

                      {/* 默认配置提示 */}
                      {isDefaultSettings && (
                        <Alert
                          message="使用 .env 文件中的默认配置"
                          description={
                            <div style={{ fontSize: isMobile ? '12px' : '14px' }}>
                              <p style={{ margin: '8px 0' }}>
                                当前显示的是从服务器 <code>.env</code> 文件读取的默认配置。
                              </p>
                              <p style={{ margin: '8px 0 0 0' }}>
                                点击"保存设置"后，配置将保存到数据库并同步更新到 <code>.env</code> 文件。
                              </p>
                            </div>
                          }
                          type="info"
                          showIcon
                          style={{ marginBottom: isMobile ? 12 : 16 }}
                        />
                      )}

                      {/* 已保存配置提示 */}
                      {hasSettings && !isDefaultSettings && (
                        <Alert
                          message="使用已保存的个人配置"
                          type="success"
                          showIcon
                          style={{ marginBottom: isMobile ? 12 : 16 }}
                        />
                      )}

                      {/* 表单 */}
                      {initialLoading ? (
                        <InlineDeferredPanel
                          eyebrow="Current Settings"
                          title="正在恢复当前配置工作区"
                          message="系统正在准备供应商接入、模型参数与联网研究设置表单，原有读取、保存、测试与重置逻辑保持不变。"
                          minHeight={isMobile ? 320 : 360}
                          tags={[
                            { label: '当前配置同步中', color: 'processing' },
                            { label: '表单工作区待接管', color: 'gold' },
                            { label: '设置逻辑保持原样', color: 'green' },
                          ]}
                        />
                      ) : (
                        <Form
                          form={form}
                          layout="vertical"
                          onFinish={handleSave}
                          autoComplete="off"
                        >
                          <Card
                            size="small"
                            style={{
                              ...sectionCardStyle,
                              background: 'linear-gradient(135deg, rgba(77, 128, 136, 0.08) 0%, rgba(90, 155, 165, 0.04) 100%)',
                            }}
                            styles={{
                              body: {
                                padding: isMobile ? 14 : 18,
                              },
                            }}
                          >
                            <Row gutter={[12, 12]}>
                              {[
                                {
                                  label: '当前提供商',
                                  value: String(watchedProvider).toUpperCase(),
                                  hint: '决定协议与兼容行为',
                                },
                                {
                                  label: '当前模型',
                                  value: clipDisplayText(String(watchedModel)),
                                  hint: '生成与测试将复用',
                                },
                                {
                                  label: '主端点',
                                  value: clipDisplayText(String(watchedBaseUrl || providerHint?.baseUrl || ''), isMobile ? 18 : 28),
                                  hint: `已配置 ${Math.max(endpoints.length, watchedBaseUrl === '未设置' ? 0 : 1)} 个端点`,
                                },
                                {
                                  label: '联网检索',
                                  value: watchedWebResearchEnabled ? '已开启' : '已关闭',
                                  hint: `Exa ${watchedExaEnabled ? '开启' : '关闭'} / Grok ${watchedGrokEnabled ? '开启' : '关闭'}`,
                                },
                              ].map((item) => (
                                <Col xs={12} lg={6} key={item.label}>
                                  <div
                                    style={{
                                      height: '100%',
                                      padding: isMobile ? '10px 12px' : '12px 14px',
                                      borderRadius: 12,
                                      background: 'rgba(255,255,255,0.82)',
                                      border: '1px solid rgba(77, 128, 136, 0.10)',
                                    }}
                                  >
                                    <Text style={{ fontSize: isMobile ? 11 : 12, color: 'var(--color-text-secondary)' }}>
                                      {item.label}
                                    </Text>
                                    <div style={{ marginTop: 6, fontSize: isMobile ? 13 : 15, fontWeight: 600, color: '#22313f' }}>
                                      {item.value}
                                    </div>
                                    <Text style={{ fontSize: isMobile ? 11 : 12, color: '#8c8c8c' }}>
                                      {item.hint}
                                    </Text>
                                  </div>
                                </Col>
                              ))}
                            </Row>
                          </Card>

                          <Card
                            size="small"
                            style={{
                              ...sectionCardStyle,
                              background: 'linear-gradient(135deg, rgba(24, 144, 255, 0.08) 0%, rgba(255, 255, 255, 0.98) 100%)',
                            }}
                            styles={{
                              body: {
                                padding: isMobile ? 14 : 18,
                              },
                            }}
                          >
                            <Space direction="vertical" size={14} style={{ width: '100%' }}>
                              <Row gutter={[12, 12]} align="middle" justify="space-between">
                                <Col xs={24} md={16}>
                                  <Space direction="vertical" size={2}>
                                    <Text strong style={{ fontSize: isMobile ? 14 : 15 }}>配置分类菜单</Text>
                                    <Text style={{ fontSize: isMobile ? 12 : 13, color: 'var(--color-text-secondary)' }}>
                                      当前仅展示一个分区，减少长表单滚动；保存时仍会提交整张表单。
                                    </Text>
                                  </Space>
                                </Col>
                                <Col xs={24} md="auto">
                                  <Tag color="processing" style={{ marginInlineEnd: 0 }}>
                                    当前：{activeSettingsSectionMeta.label}
                                  </Tag>
                                </Col>
                              </Row>

                              <Segmented
                                block
                                size={isMobile ? 'middle' : 'large'}
                                value={activeSettingsSection}
                                onChange={(value: any) => setActiveSettingsSection(value)}
                                options={settingsSectionItems.map((item: any) => ({
                                  value: item.key,
                                  label: item.label,
                                }))}
                              />

                              <div
                                style={{
                                  padding: isMobile ? '12px 14px' : '14px 16px',
                                  borderRadius: 12,
                                  background: 'rgba(255, 255, 255, 0.88)',
                                  border: '1px solid rgba(24, 144, 255, 0.12)',
                                }}
                              >
                                <Text strong>{activeSettingsSectionMeta.label}</Text>
                                <div style={{ marginTop: 4 }}>
                                  <Text style={{ fontSize: isMobile ? 12 : 13, color: 'var(--color-text-secondary)' }}>
                                    {activeSettingsSectionMeta.description}
                                  </Text>
                                </div>
                                <Tag color="blue" style={{ marginTop: 10, marginInlineEnd: 0 }}>
                                  {activeSettingsSectionMeta.summary}
                                </Tag>
                              </div>
                            </Space>
                          </Card>

                          {activeSettingsSection === 'provider' ? (
                          <Card
                            size="small"
                            title={renderSectionTitle('供应商与凭证', '先确定服务提供商，再填写 API Key 与主地址。', '基础接入', 'blue')}
                            style={sectionCardStyle}
                            styles={sectionCardStyles}
                          >
                            <div
                              style={{
                                marginBottom: 18,
                                padding: isMobile ? 16 : 18,
                                borderRadius: 20,
                                background: `linear-gradient(135deg, ${alphaColor(token.colorPrimaryBg, 0.88)} 0%, ${alphaColor(token.colorBgElevated, 0.98)} 100%)`,
                                border: `1px solid ${alphaColor(token.colorPrimary, 0.14)}`,
                                boxShadow: `0 16px 32px ${alphaColor(token.colorText, 0.05)}`,
                              }}
                            >
                              <div
                                style={{
                                  display: 'grid',
                                  gridTemplateColumns: isMobile ? 'minmax(0, 1fr)' : 'minmax(0, 1fr) auto',
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
                                      fontSize: isMobile ? 17 : 18,
                                      marginBottom: 8,
                                      fontFamily: designDisplayFont,
                                      letterSpacing: '-0.03em',
                                    }}
                                  >
                                    先确认协议，再补齐密钥、主地址与模型列表入口
                                  </Text>
                                  <Text type="secondary" style={{ display: 'block', lineHeight: 1.75 }}>
                                    这里负责最基础的接入信息。若你使用 OpenAI 兼容中转站，建议把基础地址填写到完整的 <code>/v1</code> 路径；只有模型列表入口和主地址不一致时，再额外填写模型列表地址。
                                  </Text>
                                </div>
                                <Space wrap size={[8, 8]} style={{ justifyContent: 'flex-end' }}>
                                  <Tag color="processing" style={{ margin: 0, borderRadius: 999, paddingInline: 10 }}>
                                    Provider {String(selectedProvider || watchedProvider || 'openai').toUpperCase()}
                                  </Tag>
                                  <Tag color="blue" style={{ margin: 0, borderRadius: 999, paddingInline: 10 }}>
                                    端点 {Math.max(endpoints.length, watchedBaseUrl ? 1 : 0)} 个
                                  </Tag>
                                  <Tag color={watchedModelsEndpoint ? 'gold' : 'default'} style={{ margin: 0, borderRadius: 999, paddingInline: 10 }}>
                                    {watchedModelsEndpoint ? '自定义模型列表' : '自动推导模型列表'}
                                  </Tag>
                                </Space>
                              </div>
                            </div>

                            <Row gutter={[16, 16]}>
                              <Col xs={24}>
                                <div style={fieldPanelStyle}>
                                  <Form.Item
                                    label={
                                      <Space size={4}>
                                        <span>API 提供商</span>
                                        <InfoCircleOutlined
                                          title="选择你的AI服务提供商"
                                          style={{ color: 'var(--color-text-secondary)', fontSize: isMobile ? '12px' : '14px' }}
                                        />
                                      </Space>
                                    }
                                    name="api_provider"
                                    rules={[{ required: true, message: '请选择API提供商' }]}
                                    style={{ marginBottom: 0 }}
                                  >
                                    <Suspense fallback={settingsLazyFallback}>
                                      <LazyProviderSelector
                                        value={selectedProvider}
                                        onChange={(value: any) => {
                                          handleProviderChange(value);
                                          form.setFieldValue('api_provider', value);
                                        }}
                                      />
                                    </Suspense>
                                  </Form.Item>
                                </div>
                              </Col>

                              <Col xs={24} lg={10}>
                                <div style={fieldPanelStyle}>
                                  <Text style={fieldHintTextStyle}>
                                    仅用于接口鉴权，支持官方 Key、兼容网关 Key 与各类 NewAPI / 中转服务。
                                  </Text>
                                  <Form.Item
                                    label={
                                      <Space size={4}>
                                        <span>API 密钥</span>
                                        <InfoCircleOutlined
                                          title="你的API密钥，将加密存储"
                                          style={{ color: 'var(--color-text-secondary)', fontSize: isMobile ? '12px' : '14px' }}
                                        />
                                      </Space>
                                    }
                                    name="api_key"
                                    rules={[{ required: true, message: '请输入API密钥' }]}
                                    style={{ marginBottom: 0 }}
                                  >
                                    <Input.Password
                                      size={isMobile ? 'middle' : 'large'}
                                      placeholder={apiKeyInputPlaceholder}
                                      autoComplete="new-password"
                                    />
                                  </Form.Item>
                                </div>
                              </Col>

                              <Col xs={24} lg={14}>
                                <div style={fieldPanelStyle}>
                                  <Text style={fieldHintTextStyle}>
                                    建议填写 OpenAI 兼容基础路径；模型列表会自动尝试 <code>/v1/models</code>、<code>/models</code> 等常见端点。
                                  </Text>
                                  <Form.Item
                                    label={
                                      <Space size={4}>
                                        <span>API 地址</span>
                                        <InfoCircleOutlined
                                          title="API的基础URL地址"
                                          style={{ color: 'var(--color-text-secondary)', fontSize: isMobile ? '12px' : '14px' }}
                                        />
                                      </Space>
                                    }
                                    name="api_base_url"
                                    rules={[
                                      { required: true, message: '请输入API地址' },
                                      { type: 'url', message: '请输入有效的URL' }
                                    ]}
                                    style={{ marginBottom: 0 }}
                                  >
                                    <Input
                                      size={isMobile ? 'middle' : 'large'}
                                      placeholder="https://api.openai.com/v1"
                                      onChange={(e) => {
                                        const url = e.target.value;
                                        setEndpoints((prev: any) => {
                                          if (prev.length === 0) return [{ url, type: 'primary', status: 'untested' as const }];
                                          const updated = [...prev];
                                          updated[0] = { ...updated[0], url, status: 'untested' as const };
                                          return updated;
                                        });
                                      }}
                                    />
                                  </Form.Item>
                                </div>
                              </Col>

                              <Col xs={24}>
                                <div style={fieldPanelStyle}>
                                  <Text style={fieldHintTextStyle}>
                                    可选。仅当服务商模型列表地址与 API 地址不一致时填写，例如 <code>https://example.com/v1/models</code>。
                                  </Text>
                                  <Form.Item
                                    label={
                                      <Space size={4}>
                                        <span>模型列表地址</span>
                                        <InfoCircleOutlined
                                          title="自定义模型列表端点；留空时自动从 API 地址推导"
                                          style={{ color: 'var(--color-text-secondary)', fontSize: isMobile ? '12px' : '14px' }}
                                        />
                                      </Space>
                                    }
                                    name="models_url"
                                    rules={[{ type: 'url', message: '请输入有效的URL' }]}
                                    style={{ marginBottom: 0 }}
                                  >
                                    <Input
                                      size={isMobile ? 'middle' : 'large'}
                                      placeholder="留空自动探测，或填写 https://.../v1/models"
                                      onChange={() => setModelsFetched(false)}
                                    />
                                  </Form.Item>
                                </div>
                              </Col>
                            </Row>
                          </Card>
                          ) : null}

                          {activeSettingsSection === 'network' ? (
                          <Card
                            size="small"
                            title={renderSectionTitle('网络与容灾', '配置主备端点与切换策略，提升稳定性与可恢复性。', '高可用', 'cyan')}
                            style={sectionCardStyle}
                            styles={sectionCardStyles}
                          >
                            <Text style={{ ...fieldHintTextStyle, marginBottom: 16 }}>
                              主端点负责日常请求，备用端点用于降级。若你使用多个代理或网关，可以在这里统一维护主备链路。
                            </Text>

                            <Row gutter={[16, 16]}>
                              <Col xs={24} xl={16}>
                                <div style={fieldPanelStyle}>
                                  <Form.Item
                                    label={
                                      <Space size={4}>
                                        <span>端点配置</span>
                                        <InfoCircleOutlined
                                          title="配置主备端点，主端点失败时自动切换到备端点"
                                          style={{ color: 'var(--color-text-secondary)', fontSize: isMobile ? '12px' : '14px' }}
                                        />
                                      </Space>
                                    }
                                    style={{ marginBottom: 0 }}
                                  >
                                    {activeSettingsSection === 'network' ? (
                                      <Suspense fallback={settingsLazyFallback}>
                                        <LazyEndpointListEditor
                                          endpoints={endpoints}
                                          onChange={(nextEndpoints: any[]) => {
                                            setEndpoints(nextEndpoints);
                                            const primaryEndpoint = nextEndpoints.find((endpoint: any) => endpoint.type === 'primary');
                                            form.setFieldValue('api_base_url', primaryEndpoint?.url || '');
                                          }}
                                          loading={testingApi}
                                        />
                                      </Suspense>
                                    ) : null}
                                  </Form.Item>
                                </div>
                              </Col>

                              <Col xs={24} xl={8}>
                                <div
                                  style={{
                                    ...fieldPanelStyle,
                                    background: 'linear-gradient(180deg, #f6feff 0%, #ffffff 100%)',
                                  }}
                                >
                                  <Space direction="vertical" size={12} style={{ width: '100%' }}>
                                    <div
                                      style={{
                                        padding: 12,
                                        borderRadius: 12,
                                        background: 'rgba(24, 144, 255, 0.06)',
                                        border: '1px solid rgba(24, 144, 255, 0.12)',
                                      }}
                                    >
                                      <Text strong style={{ display: 'block', marginBottom: 4 }}>
                                        切换建议
                                      </Text>
                                      <Text style={{ fontSize: isMobile ? 12 : 13, color: 'var(--color-text-secondary)' }}>
                                        自动降级更适合日常使用；若你想固定单一端点并手动排查故障，再选择手动切换。
                                      </Text>
                                    </div>

                                    <Space wrap size={[8, 8]}>
                                      <Tag color="cyan">
                                        已配置端点：{Math.max(endpoints.length, watchedBaseUrl === '未设置' ? 0 : 1)}
                                      </Tag>
                                      <Tag color={fallbackStrategy === 'auto' ? 'success' : 'default'}>
                                        当前策略：{fallbackStrategy === 'auto' ? '自动降级' : '手动切换'}
                                      </Tag>
                                    </Space>

                                    <Form.Item label="端点切换策略" style={{ marginBottom: 0 }}>
                                      <Radio.Group value={fallbackStrategy} onChange={(e) => setFallbackStrategy(e.target.value)}>
                                        <Space direction="vertical" size={8}>
                                          <Radio value="auto">自动降级（主端点失败自动切换备端点）</Radio>
                                          <Radio value="manual">手动切换</Radio>
                                        </Space>
                                      </Radio.Group>
                                    </Form.Item>
                                  </Space>
                                </div>
                              </Col>
                            </Row>
                          </Card>
                          ) : null}

                          {activeSettingsSection === 'model' ? (
                          <Card
                            size="small"
                            title={renderSectionTitle('模型与生成参数', '调节模型、温度、Token 与系统提示词，控制输出风格与成本。', '生成策略', 'purple')}
                            style={sectionCardStyle}
                            styles={sectionCardStyles}
                          >
                            <Text style={{ ...fieldHintTextStyle, marginBottom: 16 }}>
                              这里控制模型能力、生成长度与文风。建议先确定模型，再微调 Token、温度和系统提示词。
                            </Text>

                            <Row gutter={[16, 16]}>
                              <Col xs={24} xl={16}>
                                <div style={fieldPanelStyle}>
                                  <Form.Item
                                    label={
                                      <Space size={4}>
                                        <span>模型名称</span>
                                        <InfoCircleOutlined
                                          title="AI模型的名称，如 gpt-4, gpt-3.5-turbo"
                                          style={{ color: 'var(--color-text-secondary)', fontSize: isMobile ? '12px' : '14px' }}
                                        />
                                      </Space>
                                    }
                                    name="llm_model"
                                    rules={[{ required: true, message: '请输入或选择模型名称' }]}
                                    style={{ marginBottom: 0 }}
                                  >
                                    <Select
                                      size={isMobile ? 'middle' : 'large'}
                                      showSearch
                                      placeholder={isMobile ? '选择模型' : '输入模型名称或点击获取'}
                                      optionFilterProp="label"
                                      loading={fetchingModels}
                                      onFocus={handleModelSelectFocus}
                                      onSearch={(value) => setModelSearchText(value)}
                                      onChange={() => setModelSearchText('')}
                                      onBlur={() => {
                                        const customModel = modelSearchText.trim();
                                        if (customModel) {
                                          form.setFieldValue('llm_model', customModel);
                                        }
                                      }}
                                      onInputKeyDown={(event) => {
                                        if (event.key === 'Enter') {
                                          const customModel = modelSearchText.trim();
                                          if (customModel) {
                                            form.setFieldValue('llm_model', customModel);
                                          }
                                        }
                                      }}
                                      filterOption={(input, option) =>
                                        String(option?.label ?? '').toLowerCase().includes(input.toLowerCase()) ||
                                        String(option?.description ?? '').toLowerCase().includes(input.toLowerCase())
                                      }
                                      dropdownRender={(menu) => (
                                        <>
                                          {menu}
                                          {fetchingModels && (
                                            renderModelStatusHint(
                                              '模型候选正在返回',
                                              '保持当前 provider 与接口地址即可，返回后仍可沿现有流程选择模型或直接输入名称。',
                                            )
                                          )}
                                          {!fetchingModels && modelOptions.length === 0 && modelsFetched && (
                                            renderModelStatusHint(
                                              '暂时未取回模型列表',
                                              '建议先检查 API 配置，或者直接手动输入模型名称，不会影响当前表单里的其他设置。',
                                              'warning',
                                            )
                                          )}
                                          {!fetchingModels && modelOptions.length === 0 && !modelsFetched && (
                                            renderModelStatusHint(
                                              '点开后会自动拉取模型列表',
                                              '如果已经知道目标模型，也可以直接输入名称并按回车，先把设置工作流推进下去。',
                                            )
                                          )}
                                        </>
                                      )}
                                      notFoundContent={
                                        fetchingModels ? (
                                          renderModelStatusHint(
                                            '还在整理匹配候选',
                                            '下拉里的模型结果正在返回，稍等片刻即可继续选择；当前输入不会丢失。',
                                          )
                                        ) : (
                                          renderModelStatusHint(
                                            '暂时没有匹配项',
                                            '可以继续搜索，也可以直接输入模型名称后按回车，把当前配置先保存下来。',
                                          )
                                        )
                                      }
                                      suffixIcon={
                                        !isMobile ? (
                                          <div
                                            onClick={(e) => {
                                              e.stopPropagation();
                                              if (!fetchingModels) {
                                                setModelsFetched(false);
                                                handleFetchModels(false);
                                              }
                                            }}
                                            style={{
                                              cursor: fetchingModels ? 'not-allowed' : 'pointer',
                                              display: 'flex',
                                              alignItems: 'center',
                                              padding: '0 4px',
                                              height: '100%',
                                              marginRight: -8
                                            }}
                                            title="重新获取模型列表"
                                          >
                                            <Button
                                              type="text"
                                              size="small"
                                              icon={<ReloadOutlined />}
                                              loading={fetchingModels}
                                              style={{ pointerEvents: 'none' }}
                                            >
                                              刷新
                                            </Button>
                                          </div>
                                        ) : undefined
                                      }
                                      options={mergedModelOptions.map((model: any) => ({
                                        value: model.value,
                                        label: model.label,
                                        description: model.description
                                      }))}
                                      optionRender={(option) => (
                                        <div>
                                          <div style={{ fontWeight: 500, fontSize: isMobile ? '13px' : '14px' }}>{option.data.label}</div>
                                          {option.data.description && (
                                            <div style={{ fontSize: isMobile ? '11px' : '12px', color: '#8c8c8c', marginTop: '2px' }}>
                                              {option.data.description}
                                            </div>
                                          )}
                                        </div>
                                      )}
                                    />
                                  </Form.Item>
                                </div>
                              </Col>

                              <Col xs={24} xl={8}>
                                <div
                                  style={{
                                    ...fieldPanelStyle,
                                    background: 'linear-gradient(180deg, #ffffff 0%, #fcfbff 100%)',
                                  }}
                                >
                                  <Text style={fieldHintTextStyle}>
                                    限制单次返回长度；长篇小说生成建议使用 32000 起步，按模型上下文能力调高或调低。
                                  </Text>
                                  <Form.Item
                                    label={
                                      <Space size={4}>
                                        <span>最大 Token 数</span>
                                        <InfoCircleOutlined
                                          title="单次请求的最大token数量"
                                          style={{ color: 'var(--color-text-secondary)', fontSize: isMobile ? '12px' : '14px' }}
                                        />
                                      </Space>
                                    }
                                    name="max_tokens"
                                    rules={[
                                      { required: true, message: '请输入最大token数' },
                                      { type: 'number', min: 1, message: '请输入大于0的数字' }
                                    ]}
                                    style={{ marginBottom: 0 }}
                                  >
                                    <InputNumber
                                      size={isMobile ? 'middle' : 'large'}
                                      style={{ width: '100%' }}
                                      min={1}
                                      max={200000}
                                      step={1000}
                                      placeholder="32000"
                                    />
                                  </Form.Item>
                                </div>
                              </Col>

                              <Col xs={24}>
                                <div style={fieldPanelStyle}>
                                  <Space wrap size={[8, 8]} style={{ marginBottom: 12 }}>
                                    <Tag color="purple">模型：{clipDisplayText(String(watchedModel), isMobile ? 18 : 26)}</Tag>
                                    <Tag color="geekblue">Token：{String(watchedMaxTokens)}</Tag>
                                    <Tag color="magenta">
                                      温度：{typeof watchedTemperature === 'number' ? watchedTemperature.toFixed(1) : String(watchedTemperature)}
                                    </Tag>
                                  </Space>
                                  <Form.Item
                                    label={
                                      <Space size={4}>
                                        <span>温度参数</span>
                                        <InfoCircleOutlined
                                          title="控制输出随机性：0.3 更稳定，0.7 平衡，1.0+ 更有创意"
                                          style={{ color: 'var(--color-text-secondary)', fontSize: isMobile ? '12px' : '14px' }}
                                        />
                                      </Space>
                                    }
                                    name="temperature"
                                    style={{ marginBottom: 0 }}
                                  >
                                    <Slider
                                      min={0}
                                      max={2}
                                      step={0.1}
                                      marks={{
                                        0: { style: { fontSize: isMobile ? '11px' : '12px' }, label: '0.0' },
                                        0.3: { style: { fontSize: isMobile ? '11px' : '12px' }, label: '0.3' },
                                        0.7: { style: { fontSize: isMobile ? '11px' : '12px' }, label: '0.7' },
                                        1: { style: { fontSize: isMobile ? '11px' : '12px' }, label: '1.0' },
                                        1.5: { style: { fontSize: isMobile ? '11px' : '12px' }, label: '1.5' },
                                        2: { style: { fontSize: isMobile ? '11px' : '12px' }, label: '2.0' }
                                      }}
                                    />
                                  </Form.Item>
                                </div>
                              </Col>

                              <Col xs={24}>
                                <div
                                  style={{
                                    ...fieldPanelStyle,
                                    background: 'linear-gradient(180deg, #ffffff 0%, #fcfbff 100%)',
                                  }}
                                >
                                  <Text style={fieldHintTextStyle}>
                                    用于统一设定角色、语气和输出边界，适合作为整站创作默认行为。
                                  </Text>
                                  <Form.Item
                                    label={
                                      <Space size={4}>
                                        <span>系统提示词</span>
                                        <InfoCircleOutlined
                                          title="设置全局系统提示词，每次AI调用时都会自动使用。可用于设定AI的角色、语言风格等"
                                          style={{ color: 'var(--color-text-secondary)', fontSize: isMobile ? '12px' : '14px' }}
                                        />
                                      </Space>
                                    }
                                    name="system_prompt"
                                    style={{ marginBottom: 0 }}
                                  >
                                    <TextArea
                                      rows={4}
                                      placeholder="例如：你是一个专业的小说创作助手，请用生动、细腻的文字进行创作..."
                                      maxLength={10000}
                                      showCount
                                      style={{ fontSize: isMobile ? '13px' : '14px' }}
                                    />
                                  </Form.Item>
                                </div>
                              </Col>
                            </Row>
                          </Card>
                          ) : null}

                          {activeSettingsSection === 'research' ? (
                          <Card
                            size="small"
                            title={renderSectionTitle('生成前网络检索', '将 Exa 与 Grok 的外部检索能力拆分管理，适合分别配置来源抓取与趋势摘要。', '增强信息', 'gold')}
                            style={sectionCardStyle}
                            styles={sectionCardStyles}
                          >
                            <div
                              style={{
                                padding: isMobile ? 16 : 18,
                                borderRadius: 20,
                                background: `linear-gradient(135deg, ${alphaColor(token.colorWarning, 0.12)} 0%, ${alphaColor(token.colorBgElevated, 0.98)} 100%)`,
                                border: `1px solid ${alphaColor(token.colorWarning, 0.18)}`,
                                boxShadow: `0 16px 32px ${alphaColor(token.colorText, 0.05)}`,
                                marginBottom: 18,
                              }}
                            >
                              <div
                                style={{
                                  display: 'grid',
                                  gridTemplateColumns: isMobile ? 'minmax(0, 1fr)' : 'minmax(0, 1fr) auto',
                                  gap: 16,
                                  alignItems: 'start',
                                  marginBottom: 14,
                                }}
                              >
                                <div style={{ minWidth: 0 }}>
                                  <Text style={{ display: 'block', fontSize: 11, letterSpacing: '0.08em', textTransform: 'uppercase', color: token.colorTextTertiary, marginBottom: 6 }}>
                                    Research Workspace
                                  </Text>
                                  <Text
                                    strong
                                    style={{
                                      display: 'block',
                                      fontSize: isMobile ? 17 : 18,
                                      marginBottom: 8,
                                      fontFamily: designDisplayFont,
                                      letterSpacing: '-0.03em',
                                    }}
                                  >
                                    先决定是否联网检索，再分别配置来源抓取与趋势摘要通道
                                  </Text>
                                  <Text type="secondary" style={{ display: 'block', lineHeight: 1.75 }}>
                                    用于章节、世界观、角色和大纲生成前，通过 Exa / Grok 抓取资料并沉淀摘要。这里只重排展示层与阅读顺序，不改变检索开关、测试逻辑或结果写回。
                                  </Text>
                                </div>
                                <Space wrap size={[8, 8]} style={{ justifyContent: 'flex-end' }}>
                                  <Tag color={watchedWebResearchEnabled ? 'success' : 'default'} style={{ margin: 0, borderRadius: 999, paddingInline: 10 }}>
                                    检索 {watchedWebResearchEnabled ? '已开启' : '已关闭'}
                                  </Tag>
                                  <Tag color={watchedExaEnabled ? 'blue' : 'default'} style={{ margin: 0, borderRadius: 999, paddingInline: 10 }}>
                                    Exa {watchedExaEnabled ? '来源抓取' : '未启用'}
                                  </Tag>
                                  <Tag color={watchedGrokEnabled ? 'purple' : 'default'} style={{ margin: 0, borderRadius: 999, paddingInline: 10 }}>
                                    Grok {watchedGrokEnabled ? '趋势摘要' : '未启用'}
                                  </Tag>
                                </Space>
                              </div>
                              <Row gutter={[16, 8]} align="middle">
                                <Col xs={24} md={8}>
                                  <Form.Item name="web_research_enabled" label="启用检索" valuePropName="checked" style={{ marginBottom: 8 }}>
                                    <Switch checkedChildren="开启" unCheckedChildren="关闭" />
                                  </Form.Item>
                                </Col>
                                <Col xs={12} md={8}>
                                  <Form.Item name="web_research_exa_enabled" label="启用 Exa" valuePropName="checked" style={{ marginBottom: 8 }}>
                                    <Switch checkedChildren="开启" unCheckedChildren="关闭" />
                                  </Form.Item>
                                </Col>
                                <Col xs={12} md={8}>
                                  <Form.Item name="web_research_grok_enabled" label="启用 Grok" valuePropName="checked" style={{ marginBottom: 8 }}>
                                    <Switch checkedChildren="开启" unCheckedChildren="关闭" />
                                  </Form.Item>
                                </Col>
                              </Row>
                              <Space wrap size={[8, 8]}>
                                <Tag color={watchedWebResearchEnabled ? 'success' : 'default'} style={{ margin: 0, borderRadius: 999, paddingInline: 10 }}>
                                  检索总开关：{watchedWebResearchEnabled ? '开启' : '关闭'}
                                </Tag>
                                <Tag color={watchedExaEnabled ? 'blue' : 'default'} style={{ margin: 0, borderRadius: 999, paddingInline: 10 }}>
                                  Exa：{watchedExaEnabled ? '已启用' : '未启用'}
                                </Tag>
                                <Tag color={watchedGrokEnabled ? 'purple' : 'default'} style={{ margin: 0, borderRadius: 999, paddingInline: 10 }}>
                                  Grok：{watchedGrokEnabled ? '已启用' : '未启用'}
                                </Tag>
                              </Space>
                            </div>

                            <Row gutter={[16, 16]}>
                              <Col xs={24} xl={12}>
                                <Card
                                  size="small"
                                  style={{
                                    height: '100%',
                                    borderRadius: 20,
                                    border: `1px solid ${alphaColor(token.colorInfo, 0.18)}`,
                                    background: `linear-gradient(180deg, ${alphaColor(token.colorBgContainer, 0.98)} 0%, ${alphaColor(token.colorInfo, 0.08)} 100%)`,
                                    boxShadow: `0 16px 32px ${alphaColor(token.colorText, 0.05)}`,
                                  }}
                                  styles={{ body: { padding: isMobile ? 16 : 18 } }}
                                >
                                  <div style={{ display: 'grid', gap: 14 }}>
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
                                          Research Dossier
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
                                          Exa 检索
                                        </Text>
                                        <Text type="secondary" style={{ display: 'block', lineHeight: 1.75 }}>
                                          更适合抓取可追溯来源、链接与事实型资料，适合把引用型信息提前整理进生成上下文。
                                        </Text>
                                      </div>
                                      <Tag color={watchedExaEnabled ? 'blue' : 'default'} style={{ margin: 0, borderRadius: 999, paddingInline: 10 }}>
                                        {watchedExaEnabled ? '来源抓取' : '已关闭'}
                                      </Tag>
                                    </div>
                                    <div
                                      style={{
                                        padding: '12px 14px',
                                        borderRadius: 16,
                                        background: alphaColor(token.colorBgElevated, 0.96),
                                        border: `1px solid ${alphaColor(token.colorBorderSecondary, 0.88)}`,
                                      }}
                                    >
                                      <Text style={{ display: 'block', fontSize: 11, letterSpacing: '0.08em', textTransform: 'uppercase', color: token.colorTextTertiary, marginBottom: 8 }}>
                                        Available Actions
                                      </Text>
                                  <Form.Item name="web_research_exa_api_key" label="Exa API Key">
                                    <Input.Password placeholder="填写 Exa API Key" autoComplete="new-password" />
                                  </Form.Item>
                                  <Form.Item
                                    name="web_research_exa_base_url"
                                    label="Exa Base URL"
                                    rules={[
                                      {
                                        validator: (_, value) => {
                                          if (!value) return Promise.resolve();
                                          try {
                                            new URL(value);
                                            return Promise.resolve();
                                          } catch {
                                            return Promise.reject(new Error('请输入有效的 URL'));
                                          }
                                        },
                                      },
                                    ]}
                                  >
                                    <Input placeholder="https://exa.chengtx.vip" />
                                  </Form.Item>
                                  <Button
                                    icon={<ThunderboltOutlined />}
                                    onClick={() => handleTestWebResearch('exa')}
                                    loading={testingWebResearchProvider === 'exa'}
                                    block={isMobile}
                                        style={{ borderRadius: 14, minHeight: 42 }}
                                  >
                                    测试 Exa
                                  </Button>
                                    </div>
                                  </div>
                                </Card>
                              </Col>
                              <Col xs={24} xl={12}>
                                <Card
                                  size="small"
                                  style={{
                                    height: '100%',
                                    borderRadius: 20,
                                    border: `1px solid ${alphaColor(token.colorPrimary, 0.18)}`,
                                    background: `linear-gradient(180deg, ${alphaColor(token.colorBgContainer, 0.98)} 0%, ${alphaColor(token.colorPrimary, 0.08)} 100%)`,
                                    boxShadow: `0 16px 32px ${alphaColor(token.colorText, 0.05)}`,
                                  }}
                                  styles={{ body: { padding: isMobile ? 16 : 18 } }}
                                >
                                  <div style={{ display: 'grid', gap: 14 }}>
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
                                          Research Dossier
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
                                          Grok 检索
                                        </Text>
                                        <Text type="secondary" style={{ display: 'block', lineHeight: 1.75 }}>
                                          更适合实时讨论、趋势摘要与表达参考；启用 GrokSearch 后会优先走当前项目内置的深度联网搜索逻辑。
                                        </Text>
                                      </div>
                                      <Tag color={watchedGrokEnabled ? 'purple' : 'default'} style={{ margin: 0, borderRadius: 999, paddingInline: 10 }}>
                                        {watchedGrokEnabled ? '摘要趋势' : '已关闭'}
                                      </Tag>
                                    </div>
                                    <div
                                      style={{
                                        padding: '12px 14px',
                                        borderRadius: 16,
                                        background: alphaColor(token.colorBgElevated, 0.96),
                                        border: `1px solid ${alphaColor(token.colorBorderSecondary, 0.88)}`,
                                      }}
                                    >
                                      <Text style={{ display: 'block', fontSize: 11, letterSpacing: '0.08em', textTransform: 'uppercase', color: token.colorTextTertiary, marginBottom: 8 }}>
                                        Available Actions
                                      </Text>
                                  <Form.Item name="web_research_grok_api_key" label="Grok API Key">
                                    <Input.Password placeholder="填写 Grok API Key" autoComplete="new-password" />
                                  </Form.Item>
                                  <Form.Item
                                    name="web_research_grok_base_url"
                                    label="Grok Base URL"
                                    rules={[
                                      {
                                        validator: (_, value) => {
                                          if (!value) return Promise.resolve();
                                          try {
                                            new URL(value);
                                            return Promise.resolve();
                                          } catch {
                                            return Promise.reject(new Error('请输入有效的 URL'));
                                          }
                                        },
                                      },
                                    ]}
                                  >
                                    <Input placeholder="https://your-grok-endpoint.example" />
                                  </Form.Item>
                                  <Form.Item name="web_research_grok_model" label="Grok 模型">
                                    <Input placeholder="grok-4.1-fast" />
                                  </Form.Item>
                                  <Form.Item name="web_research_grok_search_enabled" label="启用 GrokSearch 深搜" valuePropName="checked">
                                    <Switch checkedChildren="深搜" unCheckedChildren="普通" />
                                  </Form.Item>
                                  <Text style={{ display: 'block', color: 'var(--color-text-tertiary)', marginBottom: 14 }}>
                                    {watchedGrokSearchEnabled
                                      ? '当前已启用深搜：会优先使用当前项目内置的 GrokSearch 提示词与来源解析能力。'
                                      : '启用后会优先使用当前项目内置的 GrokSearch 提示词与来源解析能力。'}
                                  </Text>
                                  <Button
                                    icon={<ThunderboltOutlined />}
                                    onClick={() => handleTestWebResearch('grok')}
                                    loading={testingWebResearchProvider === 'grok'}
                                    block={isMobile}
                                        style={{ borderRadius: 14, minHeight: 42 }}
                                  >
                                    测试 Grok
                                  </Button>
                                    </div>
                                  </div>
                                </Card>
                              </Col>
                            </Row>

                            {webResearchTestResult && (
                              <Alert
                                style={{
                                  marginTop: 16,
                                  borderRadius: 18,
                                  border: `1px solid ${alphaColor(webResearchTestResult.success ? token.colorSuccess : token.colorError, 0.18)}`,
                                }}
                                type={webResearchTestResult.success ? 'success' : 'error'}
                                showIcon
                                closable
                                onClose={() => setWebResearchTestResult(null)}
                                message={`${webResearchTestResult.provider.toUpperCase()}：${webResearchTestResult.message}`}
                                description={
                                  <div style={{ display: 'grid', gap: 12, marginTop: 10 }}>
                                    <div
                                      style={{
                                        padding: isMobile ? '12px 14px' : '14px 16px',
                                        borderRadius: 18,
                                        background: `linear-gradient(135deg, ${alphaColor(webResearchTestResult.success ? token.colorSuccess : token.colorError, 0.12)} 0%, ${alphaColor(token.colorBgElevated, 0.98)} 100%)`,
                                        border: `1px solid ${alphaColor(webResearchTestResult.success ? token.colorSuccess : token.colorError, 0.16)}`,
                                      }}
                                    >
                                      <Text style={{ display: 'block', fontSize: 11, letterSpacing: '0.08em', textTransform: 'uppercase', color: token.colorTextTertiary, marginBottom: 8 }}>
                                        Research Snapshot
                                      </Text>
                                      <Space wrap size={[8, 8]} style={{ marginBottom: 10 }}>
                                        <Tag color={webResearchTestResult.success ? 'success' : 'error'} style={{ margin: 0, borderRadius: 999, paddingInline: 10 }}>
                                          {webResearchTestResult.provider.toUpperCase()}
                                        </Tag>
                                        {typeof webResearchTestResult.result_count === 'number' ? (
                                          <Tag color="blue" style={{ margin: 0, borderRadius: 999, paddingInline: 10 }}>
                                            结果 {webResearchTestResult.result_count}
                                          </Tag>
                                        ) : null}
                                        {typeof webResearchTestResult.source_count === 'number' ? (
                                          <Tag color={webResearchSearchStatus === 'success_with_sources' ? 'processing' : 'default'} style={{ margin: 0, borderRadius: 999, paddingInline: 10 }}>
                                            来源 {webResearchTestResult.source_count}
                                          </Tag>
                                        ) : null}
                                      </Space>
                                      {webResearchStatusNote ? (
                                        <Text style={{ display: 'block', color: token.colorTextSecondary, lineHeight: 1.75, marginBottom: webResearchTestResult.response_preview ? 10 : 0 }}>
                                          {webResearchStatusNote}
                                        </Text>
                                      ) : null}
                                      {webResearchTestResult.response_preview ? (
                                        <div
                                          style={{
                                            padding: '10px 12px',
                                            borderRadius: 14,
                                            background: alphaColor(token.colorBgElevated, 0.96),
                                            border: `1px solid ${alphaColor(webResearchTestResult.success ? token.colorSuccess : token.colorError, 0.12)}`,
                                          }}
                                        >
                                          <Text strong style={{ display: 'block', marginBottom: 4 }}>
                                            返回预览
                                          </Text>
                                          <Text style={{ color: token.colorTextSecondary, lineHeight: 1.7 }}>
                                            {webResearchTestResult.response_preview}
                                          </Text>
                                        </div>
                                      ) : null}
                                    </div>

                                    {(typeof webResearchTestResult.source_count === 'number' || webResearchSearchStatus === 'success_without_sources') && (
                                      <div
                                        style={{
                                          padding: isMobile ? '12px 14px' : '14px 16px',
                                          borderRadius: 18,
                                          background: alphaColor(token.colorBgElevated, 0.96),
                                          border: `1px solid ${alphaColor(token.colorBorderSecondary, 0.88)}`,
                                        }}
                                      >
                                        <Text style={{ display: 'block', fontSize: 11, letterSpacing: '0.08em', textTransform: 'uppercase', color: token.colorTextTertiary, marginBottom: 8 }}>
                                          Source Coverage
                                        </Text>
                                        <Space wrap size={[8, 8]} style={{ marginBottom: 8 }}>
                                          <Tag color={webResearchSearchStatus === 'success_with_sources' ? 'processing' : 'default'} style={{ margin: 0, borderRadius: 999, paddingInline: 10 }}>
                                            {webResearchSearchStatus === 'success_with_sources' ? '已返回来源' : '未返回来源'}
                                          </Tag>
                                          {webResearchTestResult.sources_backfilled ? (
                                            <Tag color="cyan" style={{ margin: 0, borderRadius: 999, paddingInline: 10 }}>
                                              Exa 自动补全来源
                                            </Tag>
                                          ) : null}
                                        </Space>
                                        <Text style={{ display: 'block', color: token.colorTextSecondary, lineHeight: 1.75 }}>
                                          {webResearchSearchStatus === 'success_with_sources'
                                            ? `本次检索返回 ${webResearchTestResult.source_count ?? 0} 个可展示来源，可作为后续摘要与记忆沉淀的参考依据。`
                                            : '本次联网检索已成功执行，但当前结果没有返回可展示来源，后续更适合结合返回摘要与状态说明一起判断。'}
                                        </Text>
                                      </div>
                                    )}

                                    {(webResearchTestResult.error || (webResearchTestResult.suggestions && webResearchTestResult.suggestions.length > 0)) && (
                                      <div
                                        style={{
                                          padding: isMobile ? '12px 14px' : '14px 16px',
                                          borderRadius: 18,
                                          background: alphaColor(token.colorFillQuaternary, 0.72),
                                          border: `1px solid ${alphaColor(token.colorBorderSecondary, 0.88)}`,
                                        }}
                                      >
                                        <Text style={{ display: 'block', fontSize: 11, letterSpacing: '0.08em', textTransform: 'uppercase', color: token.colorTextTertiary, marginBottom: 8 }}>
                                          Repair Notes
                                        </Text>
                                        {webResearchTestResult.error ? (
                                          <div
                                            style={{
                                              padding: '10px 12px',
                                              borderRadius: 14,
                                              background: alphaColor(token.colorBgElevated, 0.96),
                                              border: `1px solid ${alphaColor(token.colorError, 0.14)}`,
                                              color: token.colorError,
                                              marginBottom: webResearchTestResult.suggestions && webResearchTestResult.suggestions.length > 0 ? 10 : 0,
                                            }}
                                          >
                                            <Text strong style={{ display: 'block', color: token.colorError, marginBottom: 4 }}>
                                              错误信息
                                            </Text>
                                            <Text style={{ color: token.colorError, lineHeight: 1.7 }}>
                                              {webResearchTestResult.error}
                                            </Text>
                                          </div>
                                        ) : null}
                                        {webResearchTestResult.suggestions && webResearchTestResult.suggestions.length > 0 ? (
                                          <div style={{ display: 'grid', gap: 6 }}>
                                            {webResearchTestResult.suggestions.map((item: any, index: any) => (
                                              <Text key={index} style={{ color: token.colorTextSecondary, lineHeight: 1.75 }}>
                                                • {item}
                                              </Text>
                                            ))}
                                          </div>
                                        ) : null}
                                      </div>
                                    )}
                                  </div>
                                }
                              />
                            )}
                          </Card>
                          ) : null}

                          {/* 测试结果展示 */}
                          {showTestResult && testResult && (
                            <Alert
                              message={
                                <Space>
                                  {testResult.success ? (
                                    <CheckCircleOutlined style={{ color: 'var(--color-success)', fontSize: isMobile ? '16px' : '18px' }} />
                                  ) : (
                                    <CloseCircleOutlined style={{ color: 'var(--color-error)', fontSize: isMobile ? '16px' : '18px' }} />
                                  )}
                                  <span style={{ fontSize: isMobile ? '14px' : '16px', fontWeight: 500 }}>
                                    {testResult.message}
                                  </span>
                                </Space>
                              }
                              description={
                                <div style={{ display: 'grid', gap: 12, marginTop: 10 }}>
                                  {testResult.success ? (
                                    <div
                                      style={{
                                        padding: isMobile ? '12px 14px' : '14px 16px',
                                        borderRadius: 18,
                                        background: `linear-gradient(135deg, ${alphaColor(token.colorSuccess, 0.12)} 0%, ${alphaColor(token.colorBgElevated, 0.98)} 100%)`,
                                        border: `1px solid ${alphaColor(token.colorSuccess, 0.18)}`,
                                      }}
                                    >
                                      <Text style={{ display: 'block', fontSize: 11, letterSpacing: '0.08em', textTransform: 'uppercase', color: token.colorTextTertiary, marginBottom: 8 }}>
                                        Runtime Snapshot
                                      </Text>
                                      <Space wrap size={[8, 8]} style={{ marginBottom: testResult.response_preview ? 10 : 0 }}>
                                        <Tag color="success" style={{ margin: 0, borderRadius: 999, paddingInline: 10 }}>
                                          配置通过
                                        </Tag>
                                        {testResult.response_time_ms ? (
                                          <Tag color="blue" style={{ margin: 0, borderRadius: 999, paddingInline: 10 }}>
                                            响应 {testResult.response_time_ms} ms
                                          </Tag>
                                        ) : null}
                                      </Space>
                                      {testResult.response_preview ? (
                                        <div
                                          style={{
                                            padding: '10px 12px',
                                            borderRadius: 14,
                                            background: alphaColor(token.colorBgElevated, 0.96),
                                            border: `1px solid ${alphaColor(token.colorSuccess, 0.12)}`,
                                          }}
                                        >
                                          <Text strong style={{ display: 'block', marginBottom: 4 }}>
                                            AI 响应预览
                                          </Text>
                                          <Text style={{ color: token.colorTextSecondary, lineHeight: 1.7 }}>
                                            {testResult.response_preview}
                                          </Text>
                                        </div>
                                      ) : null}
                                      <Text style={{ display: 'block', color: token.colorSuccess, fontSize: isMobile ? '12px' : '13px', marginTop: 10 }}>
                                        当前 API 配置可正常连通，后续生成与测试会沿用这组基础接入信息。
                                      </Text>
                                    </div>
                                  ) : (
                                    <div
                                      style={{
                                        padding: isMobile ? '12px 14px' : '14px 16px',
                                        borderRadius: 18,
                                        background: `linear-gradient(135deg, ${alphaColor(token.colorError, 0.12)} 0%, ${alphaColor(token.colorBgElevated, 0.98)} 100%)`,
                                        border: `1px solid ${alphaColor(token.colorError, 0.18)}`,
                                      }}
                                    >
                                      <Text style={{ display: 'block', fontSize: 11, letterSpacing: '0.08em', textTransform: 'uppercase', color: token.colorTextTertiary, marginBottom: 8 }}>
                                        Repair Notes
                                      </Text>
                                      <Space wrap size={[8, 8]} style={{ marginBottom: 10 }}>
                                        <Tag color="error" style={{ margin: 0, borderRadius: 999, paddingInline: 10 }}>
                                          连接失败
                                        </Tag>
                                        {testResult.error_type ? (
                                          <Tag color="default" style={{ margin: 0, borderRadius: 999, paddingInline: 10 }}>
                                            {testResult.error_type}
                                          </Tag>
                                        ) : null}
                                      </Space>
                                      {testResult.error ? (
                                        <div
                                          style={{
                                            padding: '10px 12px',
                                            borderRadius: 14,
                                            background: alphaColor(token.colorBgElevated, 0.96),
                                            border: `1px solid ${alphaColor(token.colorError, 0.14)}`,
                                            color: token.colorError,
                                          }}
                                        >
                                          <Text strong style={{ display: 'block', color: token.colorError, marginBottom: 4 }}>
                                            错误信息
                                          </Text>
                                          <Text style={{ color: token.colorError, lineHeight: 1.7 }}>
                                            {testResult.error}
                                          </Text>
                                        </div>
                                      ) : null}
                                      {testResult.suggestions && testResult.suggestions.length > 0 ? (
                                        <div style={{ marginTop: 10 }}>
                                          <Text strong style={{ display: 'block', marginBottom: 6 }}>
                                            建议按下面顺序排查
                                          </Text>
                                          <div style={{ display: 'grid', gap: 6 }}>
                                            {testResult.suggestions.map((suggestion: any, index: any) => (
                                              <Text key={index} style={{ color: token.colorTextSecondary, lineHeight: 1.7 }}>
                                                • {suggestion}
                                              </Text>
                                            ))}
                                          </div>
                                        </div>
                                      ) : null}
                                    </div>
                                  )}
                                  {endpointDiagnostics && (
                                    <div
                                      style={{
                                        padding: isMobile ? '12px 14px' : '14px 16px',
                                        borderRadius: 18,
                                        background: alphaColor(token.colorBgElevated, 0.96),
                                        border: `1px solid ${alphaColor(token.colorBorderSecondary, 0.88)}`,
                                      }}
                                    >
                                      <Text style={{ display: 'block', fontSize: 11, letterSpacing: '0.08em', textTransform: 'uppercase', color: token.colorTextTertiary, marginBottom: 8 }}>
                                        Endpoint Diagnostics
                                      </Text>
                                      <Space wrap size={[8, 8]} style={{ marginBottom: 10 }}>
                                        <Tag color="processing" style={{ margin: 0, borderRadius: 999, paddingInline: 10 }}>
                                          备用端点 {backupEndpoints.length}
                                        </Tag>
                                        <Tag color="default" style={{ margin: 0, borderRadius: 999, paddingInline: 10 }}>
                                          回退 {endpointDiagnostics.fallback_strategy || 'auto'}
                                        </Tag>
                                        <Tag color={endpointDiagnostics.auto_failover_enabled ? 'success' : 'default'} style={{ margin: 0, borderRadius: 999, paddingInline: 10 }}>
                                          自动故障切换 {endpointDiagnostics.auto_failover_enabled ? '已启用' : '未启用'}
                                        </Tag>
                                      </Space>
                                      <Text style={{ display: 'block', lineHeight: 1.7, color: token.colorTextSecondary }}>
                                        主端点：<code style={{ wordBreak: 'break-all' }}>{endpointDiagnostics.primary_endpoint || '未设置'}</code>
                                      </Text>
                                      {backupEndpoints.length > 0 ? (
                                        <div style={{ marginTop: 10, display: 'grid', gap: 6 }}>
                                          {backupEndpoints.map((endpoint: string, index: number) => (
                                            <code
                                              key={`${endpoint}-${index}`}
                                              style={{
                                                whiteSpace: 'pre-wrap',
                                                wordBreak: 'break-all',
                                                padding: '8px 10px',
                                                borderRadius: 12,
                                                background: alphaColor(token.colorFillAlter, 0.82),
                                                border: `1px solid ${alphaColor(token.colorBorderSecondary, 0.88)}`,
                                              }}
                                            >
                                              {endpoint}
                                            </code>
                                          ))}
                                        </div>
                                      ) : null}
                                    </div>
                                  )}
                                  {transportDiagnostics && (
                                    <div
                                      style={{
                                        padding: isMobile ? '12px 14px' : '14px 16px',
                                        borderRadius: 18,
                                        background: alphaColor(token.colorFillQuaternary, 0.72),
                                        border: `1px solid ${alphaColor(token.colorBorderSecondary, 0.88)}`,
                                      }}
                                    >
                                      <Text style={{ display: 'block', fontSize: 11, letterSpacing: '0.08em', textTransform: 'uppercase', color: token.colorTextTertiary, marginBottom: 8 }}>
                                        Transport Diagnostics
                                      </Text>
                                      <Space wrap size={[8, 8]} style={{ marginBottom: 10 }}>
                                        <Tag color="blue" style={{ margin: 0, borderRadius: 999, paddingInline: 10 }}>
                                          总尝试 {transportDiagnostics.summary?.total_attempts ?? 0}
                                        </Tag>
                                        <Tag color="success" style={{ margin: 0, borderRadius: 999, paddingInline: 10 }}>
                                          成功 {transportDiagnostics.summary?.successful_attempts ?? 0}
                                        </Tag>
                                        <Tag color={transportDiagnostics.summary?.backup_endpoint_used ? 'processing' : 'default'} style={{ margin: 0, borderRadius: 999, paddingInline: 10 }}>
                                          备用端点 {transportDiagnostics.summary?.backup_endpoint_used ? '已介入' : '未介入'}
                                        </Tag>
                                      </Space>
                                      <div style={{ display: 'grid', gap: 6 }}>
                                        <Text style={{ color: token.colorTextSecondary, lineHeight: 1.7 }}>
                                          尝试过的 API 模式：{(transportDiagnostics.summary?.api_modes_tried || []).join(' -> ') || '未知'}
                                        </Text>
                                        <Text style={{ color: token.colorTextSecondary, lineHeight: 1.7 }}>
                                          API 模式回退：{transportDiagnostics.summary?.api_mode_fallback_used ? '已触发' : '未触发'}
                                        </Text>
                                        <Text style={{ color: token.colorTextSecondary, lineHeight: 1.7 }}>
                                          Chat Completions 强制模式：{transportDiagnostics.summary?.forced_chat_completions ? '是' : '否'}
                                        </Text>
                                        <Text style={{ color: token.colorTextSecondary, lineHeight: 1.7 }}>
                                          Base URL 规范化：{transportDiagnostics.summary?.normalized_base_url_used ? '已使用' : '未使用'}
                                        </Text>
                                      </div>
                                      {transportAttempts.length > 0 ? (
                                        <div style={{ marginTop: 10, display: 'grid', gap: 8 }}>
                                          {transportAttempts.map((attempt, index) => (
                                            <div
                                              key={`${attempt.base_url || 'attempt'}-${attempt.attempt_number || index}-${index}`}
                                              style={{
                                                padding: '10px 12px',
                                                borderRadius: 14,
                                                background: alphaColor(token.colorBgElevated, 0.96),
                                                border: `1px solid ${alphaColor(token.colorBorderSecondary, 0.88)}`,
                                              }}
                                            >
                                              <Text style={{ display: 'block', lineHeight: 1.7 }}>
                                                <code>{attempt.api_mode || '未知'}</code>
                                                {' / '}
                                                <strong>{attempt.result || '未知'}</strong>
                                                {' / '}
                                                {attempt.endpoint_role === 'backup' ? '备用端点' : '主端点'}
                                                {' / '}
                                                {attempt.attempt_number || 1}/{attempt.max_attempts || 1}
                                                {attempt.status_code ? ` / HTTP ${attempt.status_code}` : ''}
                                                {attempt.error_type ? ` / ${attempt.error_type}` : ''}
                                              </Text>
                                              <code style={{ display: 'block', marginTop: 4, whiteSpace: 'pre-wrap', wordBreak: 'break-all' }}>
                                                {`${attempt.base_url || ''}${attempt.endpoint_path || ''}` || '未知端点'}
                                              </code>
                                            </div>
                                          ))}
                                        </div>
                                      ) : null}
                                    </div>
                                  )}
                                </div>
                              }
                              type={testResult.success ? 'success' : 'error'}
                              closable
                              onClose={() => setShowTestResult(false)}
                              style={{ marginBottom: isMobile ? 16 : 24, borderRadius: 18, border: `1px solid ${alphaColor(testResult.success ? token.colorSuccess : token.colorError, 0.18)}` }}
                            />
                          )}

                          {/* 操作按钮 */}
                          <Form.Item style={{ marginBottom: 0, marginTop: isMobile ? 24 : 32 }}>
                            {isMobile ? (
                              // 移动端：垂直堆叠布局
                              <Space direction="vertical" size="middle" style={{ width: '100%' }}>
                                <Button
                                  type="primary"
                                  size="large"
                                  icon={<SaveOutlined />}
                                  htmlType="submit"
                                  loading={loading}
                                  block
                                  style={{
                                    background: 'var(--color-primary)',
                                    border: 'none',
                                    height: '44px'
                                  }}
                                >
                                  保存设置
                                </Button>
                                <Button
                                  size="large"
                                  icon={<ThunderboltOutlined />}
                                  onClick={handleTestConnection}
                                  loading={testingApi}
                                  block
                                  style={{
                                    borderColor: 'var(--color-success)',
                                    color: 'var(--color-success)',
                                    fontWeight: 500,
                                    height: '44px'
                                  }}
                                >
                                  {testingApi ? '测试中...' : '测试连接'}
                                </Button>
                                <Space size="middle" style={{ width: '100%' }}>
                                  <Button
                                    size="large"
                                    icon={<ReloadOutlined />}
                                    onClick={handleReset}
                                    style={{ flex: 1, height: '44px' }}
                                  >
                                    重置
                                  </Button>
                                  {hasSettings && (
                                    <Button
                                      danger
                                      size="large"
                                      icon={<DeleteOutlined />}
                                      onClick={handleDelete}
                                      loading={loading}
                                      style={{ flex: 1, height: '44px' }}
                                    >
                                      删除
                                    </Button>
                                  )}
                                </Space>
                              </Space>
                            ) : (
                              // 桌面端：删除在左边，测试、重置和保存在右边
                              <div style={{
                                display: 'flex',
                                justifyContent: 'space-between',
                                alignItems: 'center',
                                gap: '16px',
                                flexWrap: 'wrap'
                              }}>
                                {/* 左侧：删除按钮 */}
                                {hasSettings ? (
                                  <Button
                                    danger
                                    size="large"
                                    icon={<DeleteOutlined />}
                                    onClick={handleDelete}
                                    loading={loading}
                                    style={{
                                      minWidth: '100px'
                                    }}
                                  >
                                    删除配置
                                  </Button>
                                ) : (
                                  <div /> // 占位符，保持右侧按钮位置
                                )}

                                {/* 右侧：测试、重置和保存按钮组 */}
                                <Space size="middle">
                                  <Button
                                    size="large"
                                    icon={<ThunderboltOutlined />}
                                    onClick={handleTestConnection}
                                    loading={testingApi}
                                    style={{
                                      borderColor: 'var(--color-success)',
                                      color: 'var(--color-success)',
                                      fontWeight: 500,
                                      minWidth: '100px'
                                    }}
                                  >
                                    {testingApi ? '测试中...' : '测试'}
                                  </Button>
                                  <Button
                                    size="large"
                                    icon={<ReloadOutlined />}
                                    onClick={handleReset}
                                    style={{
                                      minWidth: '100px'
                                    }}
                                  >
                                    重置
                                  </Button>
                                  <Button
                                    type="primary"
                                    size="large"
                                    icon={<SaveOutlined />}
                                    htmlType="submit"
                                    loading={loading}
                                    style={{
                                      background: 'var(--color-primary)',
                                      border: 'none',
                                      minWidth: '120px',
                                      fontWeight: 500
                                    }}
                                  >
                                    保存
                                  </Button>
                                </Space>
                              </div>
                            )}
                          </Form.Item>
                        </Form>
                      )}
                    </Space>
  );
}
