import { Button, Col, Form, Input, InputNumber, Modal, Row, Select, Space, Spin } from 'antd';
import { InfoCircleOutlined, ReloadOutlined } from '@ant-design/icons';
import type { FormInstance } from 'antd';
import type { APIKeyPreset, APIKeyPresetConfig } from '../types';

type ModelOption = {
  value: string;
  label: string;
  description: string;
};

type PresetFormValues = Partial<APIKeyPresetConfig> & {
  name?: string;
  description?: string;
  models_url?: string;
};

type SettingsPresetModalProps = {
  open: boolean;
  isMobile: boolean;
  editingPreset: APIKeyPreset | null;
  form: FormInstance<PresetFormValues>;
  fetchingPresetModels: boolean;
  presetModelsFetched: boolean;
  mergedPresetModelOptions: ModelOption[];
  onOk: () => void;
  onCancel: () => void;
  onProviderChange: (value: string) => void;
  onModelSelectFocus: () => void;
  onModelSearchChange: (value: string) => void;
  onModelChange: () => void;
  onModelCommit: () => void;
  onModelReload: () => void;
};

const { TextArea } = Input;

export default function SettingsPresetModal({
  open,
  isMobile,
  editingPreset,
  form,
  fetchingPresetModels,
  presetModelsFetched,
  mergedPresetModelOptions,
  onOk,
  onCancel,
  onProviderChange,
  onModelSelectFocus,
  onModelSearchChange,
  onModelChange,
  onModelCommit,
  onModelReload,
}: SettingsPresetModalProps) {
  return (
    <Modal
      title={editingPreset ? '编辑预设' : '创建预设'}
      open={open}
      onOk={onOk}
      onCancel={onCancel}
      width={isMobile ? '95%' : 640}
      centered
      okText="保存"
      cancelText="取消"
      styles={{
        body: {
          padding: isMobile ? '16px' : '20px 24px',
        },
      }}
    >
      <Form form={form} layout="vertical" size={isMobile ? 'middle' : 'large'}>
        <Row gutter={16}>
          <Col xs={24} sm={16}>
            <Form.Item
              name="name"
              label="预设名称"
              rules={[
                { required: true, message: '请输入预设名称' },
                { max: 50, message: '名称不能超过 50 个字符' },
              ]}
              style={{ marginBottom: 16 }}
            >
              <Input placeholder="例如：工作账号-GPT4" />
            </Form.Item>
          </Col>
          <Col xs={24} sm={8}>
            <Form.Item
              name="api_provider"
              label="API 提供商"
              rules={[{ required: true, message: '请选择提供商' }]}
              style={{ marginBottom: 16 }}
            >
              <Select placeholder="选择提供商" onChange={onProviderChange}>
                <Select.Option value="openai">OpenAI 兼容接口</Select.Option>
                <Select.Option value="anthropic">Claude（Anthropic）</Select.Option>
                <Select.Option value="gemini">Google Gemini</Select.Option>
              </Select>
            </Form.Item>
          </Col>
        </Row>

        <Form.Item
          name="description"
          label="预设描述"
          rules={[{ max: 200, message: '描述不能超过 200 个字符' }]}
          style={{ marginBottom: 16 }}
        >
          <Input placeholder="例如：用于日常写作任务（可选）" />
        </Form.Item>

        <Row gutter={16}>
          <Col xs={24} sm={12}>
            <Form.Item
              name="api_key"
              label="API Key"
              rules={[{ required: true, message: '请输入 API Key' }]}
              style={{ marginBottom: 16 }}
            >
              <Input.Password placeholder="sk-..." />
            </Form.Item>
          </Col>
          <Col xs={24} sm={12}>
            <Form.Item
              name="api_base_url"
              label="API Base URL"
              rules={[{ type: 'url', message: '请输入有效的 URL' }]}
              style={{ marginBottom: 16 }}
            >
              <Input placeholder="https://api.openai.com/v1" />
            </Form.Item>
          </Col>
        </Row>

        <Form.Item
          name="models_url"
          label="模型列表地址（可选）"
          rules={[{ type: 'url', message: '请输入有效的 URL' }]}
          style={{ marginBottom: 16 }}
        >
          <Input placeholder="留空自动探测，或填写 https://.../v1/models" />
        </Form.Item>

        <Row gutter={16}>
          <Col xs={24} sm={12}>
            <Form.Item
              name="llm_model"
              label={
                <Space size={4}>
                  <span>{"模型名称"}</span>
                  <InfoCircleOutlined
                    title="AI 模型名称，点击下拉框可自动获取可用模型"
                    style={{ color: 'var(--color-text-secondary)', fontSize: '12px' }}
                  />
                </Space>
              }
              rules={[{ required: true, message: '请选择或输入模型名称' }]}
              style={{ marginBottom: 16 }}
            >
              <Select
                showSearch
                placeholder="点击获取模型列表或直接输入"
                optionFilterProp="label"
                loading={fetchingPresetModels}
                onFocus={onModelSelectFocus}
                onSearch={onModelSearchChange}
                onChange={onModelChange}
                onBlur={onModelCommit}
                onInputKeyDown={(event) => {
                  if (event.key === 'Enter') {
                    onModelCommit();
                  }
                }}
                filterOption={(input, option) =>
                  (option?.label ?? '').toLowerCase().includes(input.toLowerCase())
                }
                dropdownRender={(menu) => (
                  <>
                    {menu}
                    {fetchingPresetModels ? (
                      <div style={{ padding: '8px 12px', color: 'var(--color-text-secondary)', textAlign: 'center', fontSize: '12px' }}>
                        <Spin size="small" /> {"正在获取模型列表..."}
                      </div>
                    ) : null}
                    {!fetchingPresetModels && mergedPresetModelOptions.length === 0 && presetModelsFetched ? (
                      <div style={{ padding: '8px 12px', color: '#ff4d4f', textAlign: 'center', fontSize: '12px' }}>
                        {"未能获取到模型列表，请检查 API 配置"}
                      </div>
                    ) : null}
                    {!fetchingPresetModels && mergedPresetModelOptions.length === 0 && !presetModelsFetched ? (
                      <div style={{ padding: '8px 12px', color: 'var(--color-text-secondary)', textAlign: 'center', fontSize: '12px' }}>
                        {"点击输入框自动获取模型列表"}
                      </div>
                    ) : null}
                  </>
                )}
                notFoundContent={
                  fetchingPresetModels ? (
                    <div style={{ padding: '8px 12px', textAlign: 'center', fontSize: '12px' }}>
                      <Spin size="small" /> {"加载中..."}
                    </div>
                  ) : (
                    <div style={{ padding: '8px 12px', color: 'var(--color-text-secondary)', textAlign: 'center', fontSize: '12px' }}>
                      {"未找到匹配的模型，可直接输入后按回车"}
                    </div>
                  )
                }
                suffixIcon={
                  <div
                    onClick={(event) => {
                      event.stopPropagation();
                      onModelReload();
                    }}
                    style={{
                      cursor: fetchingPresetModels ? 'not-allowed' : 'pointer',
                      display: 'flex',
                      alignItems: 'center',
                      padding: '0 4px',
                      height: '100%',
                      marginRight: -8,
                    }}
                    title="获取模型列表"
                  >
                    <Button
                      type="text"
                      size="small"
                      icon={<ReloadOutlined />}
                      loading={fetchingPresetModels}
                      style={{ pointerEvents: 'none' }}
                    >
                      {"获取"}
                    </Button>
                  </div>
                }
                options={mergedPresetModelOptions.map((model) => ({
                  value: model.value,
                  label: model.label,
                  description: model.description,
                }))}
                optionRender={(option) => (
                  <div>
                    <div style={{ fontWeight: 500, fontSize: '13px' }}>{option.data.label}</div>
                    {option.data.description ? (
                      <div style={{ fontSize: '11px', color: '#8c8c8c', marginTop: '2px' }}>
                        {option.data.description}
                      </div>
                    ) : null}
                  </div>
                )}
              />
            </Form.Item>
          </Col>
          <Col xs={12} sm={6}>
            <Form.Item
              name="temperature"
              label="温度"
              rules={[{ required: true, message: '必填' }]}
              style={{ marginBottom: 16 }}
            >
              <InputNumber min={0} max={2} step={0.1} style={{ width: '100%' }} placeholder="0.7" />
            </Form.Item>
          </Col>
          <Col xs={12} sm={6}>
            <Form.Item
              name="max_tokens"
              label="最大 Tokens"
              rules={[{ required: true, message: '必填' }]}
              style={{ marginBottom: 16 }}
            >
              <InputNumber min={1} max={200000} step={1000} style={{ width: '100%' }} placeholder="32000" />
            </Form.Item>
          </Col>
        </Row>

        <Form.Item name="system_prompt" label="系统提示词" style={{ marginBottom: 0 }}>
          <TextArea
            rows={isMobile ? 2 : 3}
            placeholder="例如：你是一个专业的小说创作助手...（可选）"
            maxLength={10000}
            showCount
          />
        </Form.Item>
      </Form>
    </Modal>
  );
}
