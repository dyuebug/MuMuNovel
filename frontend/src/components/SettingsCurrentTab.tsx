/* eslint-disable @typescript-eslint/no-explicit-any */
import { Suspense } from 'react';
import { Alert, Button, Card, Col, Form, Input, InputNumber, Radio, Row, Segmented, Select, Slider, Space, Spin, Switch, Tag, Typography } from 'antd';
import { CheckCircleOutlined, CloseCircleOutlined, DeleteOutlined, InfoCircleOutlined, ReloadOutlined, SaveOutlined, ThunderboltOutlined } from '@ant-design/icons';

const { Text } = Typography;
const { TextArea } = Input;

export default function SettingsCurrentTab(props: any) {
  const {
  LazyAzureConfigGuide,
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
  watchedMaxTokens,
  watchedModel,
  watchedProvider,
  watchedTemperature,
  watchedWebResearchEnabled,
  webResearchTestResult
  } = props;

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
                      <Spin spinning={initialLoading}>
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
                                  value: clipDisplayText(String(watchedBaseUrl), isMobile ? 18 : 28),
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
                            <Text style={{ ...fieldHintTextStyle, marginBottom: 16 }}>
                              这里负责最基础的接入信息。若你使用 OpenAI 兼容中转站，建议把基础地址填写到完整的 <code>/v1</code> 路径。
                            </Text>

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

                              {selectedProvider === 'azure' && (
                                <Col xs={24}>
                                  <Suspense fallback={settingsLazyFallback}>
                                    <LazyAzureConfigGuide visible />
                                  </Suspense>
                                </Col>
                              )}

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
                                      placeholder="sk-..."
                                      autoComplete="new-password"
                                    />
                                  </Form.Item>
                                </div>
                              </Col>

                              <Col xs={24} lg={14}>
                                <div style={fieldPanelStyle}>
                                  <Text style={fieldHintTextStyle}>
                                    建议填写完整基础路径；若走代理或中转站，请优先确认是否需要显式追加 <code>/v1</code>。
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
                                          onChange={setEndpoints}
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
                                            <div style={{ padding: '8px 12px', color: 'var(--color-text-secondary)', textAlign: 'center', fontSize: isMobile ? '12px' : '14px' }}>
                                              <Spin size="small" /> 正在获取模型列表...
                                            </div>
                                          )}
                                          {!fetchingModels && modelOptions.length === 0 && modelsFetched && (
                                            <div style={{ padding: '8px 12px', color: '#ff4d4f', textAlign: 'center', fontSize: isMobile ? '12px' : '14px' }}>
                                              未能获取到模型列表，请检查 API 配置
                                            </div>
                                          )}
                                          {!fetchingModels && modelOptions.length === 0 && !modelsFetched && (
                                            <div style={{ padding: '8px 12px', color: 'var(--color-text-secondary)', textAlign: 'center', fontSize: isMobile ? '12px' : '14px' }}>
                                              点击输入框自动获取模型列表
                                            </div>
                                          )}
                                        </>
                                      )}
                                      notFoundContent={
                                        fetchingModels ? (
                                          <div style={{ padding: '8px 12px', textAlign: 'center', fontSize: isMobile ? '12px' : '14px' }}>
                                            <Spin size="small" /> 加载中...
                                          </div>
                                        ) : (
                                          <div style={{ padding: '8px 12px', color: 'var(--color-text-secondary)', textAlign: 'center', fontSize: isMobile ? '12px' : '14px' }}>
                                            未找到匹配的模型，可直接输入后按回车
                                          </div>
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
                                    限制单次返回长度，能更稳定地控制成本和响应大小。
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
                                      placeholder="2000"
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
                                          title="控制输出的随机性，值越高越随机（0.0-2.0）"
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
                                        0.7: { style: { fontSize: isMobile ? '11px' : '12px' }, label: '0.7' },
                                        1: { style: { fontSize: isMobile ? '11px' : '12px' }, label: '1.0' },
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
                            <Alert
                              type="info"
                              showIcon
                              message="用于章节 / 世界观 / 角色 / 大纲生成前，自动通过 Exa / Grok 检索资料，并把摘要保存到记忆中。"
                              style={{ marginBottom: 16, borderRadius: 12 }}
                            />

                            <div
                              style={{
                                padding: isMobile ? 14 : 16,
                                borderRadius: 14,
                                background: 'linear-gradient(135deg, rgba(250, 173, 20, 0.08) 0%, rgba(255, 255, 255, 0.96) 100%)',
                                border: '1px solid rgba(250, 173, 20, 0.14)',
                                marginBottom: 16,
                              }}
                            >
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
                                <Tag color={watchedWebResearchEnabled ? 'success' : 'default'}>
                                  检索总开关：{watchedWebResearchEnabled ? '开启' : '关闭'}
                                </Tag>
                                <Tag color={watchedExaEnabled ? 'blue' : 'default'}>
                                  Exa：{watchedExaEnabled ? '已启用' : '未启用'}
                                </Tag>
                                <Tag color={watchedGrokEnabled ? 'purple' : 'default'}>
                                  Grok：{watchedGrokEnabled ? '已启用' : '未启用'}
                                </Tag>
                              </Space>
                            </div>

                            <Row gutter={[16, 16]}>
                              <Col xs={24} xl={12}>
                                <Card
                                  size="small"
                                  title={
                                    <Space wrap size={8}>
                                      <span style={{ fontWeight: 600 }}>Exa 检索</span>
                                      <Tag color={watchedExaEnabled ? 'blue' : 'default'} style={{ marginInlineEnd: 0 }}>
                                        {watchedExaEnabled ? '来源抓取' : '已关闭'}
                                      </Tag>
                                    </Space>
                                  }
                                  style={{
                                    height: '100%',
                                    borderRadius: 14,
                                    border: '1px solid #e6f4ff',
                                    background: 'linear-gradient(180deg, #ffffff 0%, #f8fcff 100%)',
                                  }}
                                  styles={{ body: { padding: isMobile ? 14 : 16 } }}
                                >
                                  <Text style={{ display: 'block', color: 'var(--color-text-secondary)', marginBottom: 14 }}>
                                    更适合抓取可追溯来源、链接与事实型资料。
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
                                  >
                                    测试 Exa
                                  </Button>
                                </Card>
                              </Col>
                              <Col xs={24} xl={12}>
                                <Card
                                  size="small"
                                  title={
                                    <Space wrap size={8}>
                                      <span style={{ fontWeight: 600 }}>Grok 检索</span>
                                      <Tag color={watchedGrokEnabled ? 'purple' : 'default'} style={{ marginInlineEnd: 0 }}>
                                        {watchedGrokEnabled ? '摘要趋势' : '已关闭'}
                                      </Tag>
                                    </Space>
                                  }
                                  style={{
                                    height: '100%',
                                    borderRadius: 14,
                                    border: '1px solid #f0e6ff',
                                    background: 'linear-gradient(180deg, #ffffff 0%, #fcfaff 100%)',
                                  }}
                                  styles={{ body: { padding: isMobile ? 14 : 16 } }}
                                >
                                  <Text style={{ display: 'block', color: 'var(--color-text-secondary)', marginBottom: 14 }}>
                                    更适合实时讨论、趋势摘要与表达参考。
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
                                  <Button
                                    icon={<ThunderboltOutlined />}
                                    onClick={() => handleTestWebResearch('grok')}
                                    loading={testingWebResearchProvider === 'grok'}
                                    block={isMobile}
                                  >
                                    测试 Grok
                                  </Button>
                                </Card>
                              </Col>
                            </Row>

                            {webResearchTestResult && (
                              <Alert
                                style={{ marginTop: 16, borderRadius: 12 }}
                                type={webResearchTestResult.success ? 'success' : 'error'}
                                showIcon
                                closable
                                onClose={() => setWebResearchTestResult(null)}
                                message={`${webResearchTestResult.provider.toUpperCase()}：${webResearchTestResult.message}`}
                                description={
                                  <div>
                                    {webResearchTestResult.response_preview && (
                                      <div style={{ marginBottom: 8 }}>
                                        <strong>返回预览：</strong> {webResearchTestResult.response_preview}
                                      </div>
                                    )}
                                    {typeof webResearchTestResult.result_count === 'number' && (
                                      <div>结果数：{webResearchTestResult.result_count}</div>
                                    )}
                                    {typeof webResearchTestResult.source_count === 'number' && (
                                      <div>来源数：{webResearchTestResult.source_count}</div>
                                    )}
                                    {webResearchTestResult.error && (
                                      <div style={{ color: 'var(--color-error)', marginTop: 8 }}>
                                        <strong>错误：</strong> {webResearchTestResult.error}
                                      </div>
                                    )}
                                    {webResearchTestResult.suggestions && webResearchTestResult.suggestions.length > 0 && (
                                      <ul style={{ marginTop: 8, paddingLeft: 18 }}>
                                        {webResearchTestResult.suggestions.map((item: any, index: any) => (
                                          <li key={index}>{item}</li>
                                        ))}
                                      </ul>
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
                                <div style={{ marginTop: 8 }}>
                                  {testResult.success ? (
                                    <Space direction="vertical" size="small" style={{ width: '100%' }}>
                                      {testResult.response_time_ms && (
                                        <div style={{ fontSize: isMobile ? '12px' : '14px' }}>
                                          ⚡ 响应时间: <strong>{testResult.response_time_ms} ms</strong>
                                        </div>
                                      )}
                                      {testResult.response_preview && (
                                        <div style={{
                                          fontSize: isMobile ? '12px' : '13px',
                                          padding: '8px 12px',
                                          background: '#f6ffed',
                                          borderRadius: '4px',
                                          border: '1px solid #b7eb8f',
                                          marginTop: '8px'
                                        }}>
                                          <div style={{ marginBottom: '4px', fontWeight: 500 }}>AI 响应预览:</div>
                                          <div style={{ color: '#595959' }}>{testResult.response_preview}</div>
                                        </div>
                                      )}
                                      <div style={{ color: 'var(--color-success)', fontSize: isMobile ? '12px' : '13px', marginTop: '4px' }}>
                                        ✓ API 配置正确，可以正常使用
                                      </div>
                                    </Space>
                                  ) : (
                                    <Space direction="vertical" size="small" style={{ width: '100%' }}>
                                      {testResult.error && (
                                        <div style={{
                                          fontSize: isMobile ? '12px' : '13px',
                                          padding: '8px 12px',
                                          background: '#fff2e8',
                                          borderRadius: '4px',
                                          border: '1px solid #ffbb96',
                                          color: '#d4380d'
                                        }}>
                                          <strong>错误信息:</strong> {testResult.error}
                                        </div>
                                      )}
                                      {testResult.error_type && (
                                        <div style={{ fontSize: isMobile ? '11px' : '12px', color: 'var(--color-text-secondary)' }}>
                                          错误类型: {testResult.error_type}
                                        </div>
                                      )}
                                      {testResult.suggestions && testResult.suggestions.length > 0 && (
                                        <div style={{ marginTop: '8px' }}>
                                          <div style={{ fontSize: isMobile ? '12px' : '13px', fontWeight: 500, marginBottom: '4px' }}>
                                            💡 解决建议:
                                          </div>
                                          <ul style={{
                                            margin: 0,
                                            paddingLeft: isMobile ? '16px' : '20px',
                                            fontSize: isMobile ? '12px' : '13px',
                                            color: '#595959'
                                          }}>
                                            {testResult.suggestions.map((suggestion: any, index: any) => (
                                              <li key={index} style={{ marginBottom: '4px' }}>{suggestion}</li>
                                            ))}
                                          </ul>
                                        </div>
                                      )}
                                    </Space>
                                  )}
                                </div>
                              }
                              type={testResult.success ? 'success' : 'error'}
                              closable
                              onClose={() => setShowTestResult(false)}
                              style={{ marginBottom: isMobile ? 16 : 24 }}
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
                      </Spin>
                    </Space>
  );
}
