import { Button, Col, Form, Input, InputNumber, Modal, Row, Select, Space, Spin } from 'antd';
import { InfoCircleOutlined, ReloadOutlined } from '@ant-design/icons';
import type { FormInstance } from 'antd';
import type { APIKeyPreset, PresetCreateRequest } from '../types';

type ModelOption = {
  value: string;
  label: string;
  description: string;
};

type SettingsPresetModalProps = {
  open: boolean;
  isMobile: boolean;
  editingPreset: APIKeyPreset | null;
  form: FormInstance<PresetCreateRequest>;
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
      title={editingPreset ? '\u7f16\u8f91\u9884\u8bbe' : '\u521b\u5efa\u9884\u8bbe'}
      open={open}
      onOk={onOk}
      onCancel={onCancel}
      width={isMobile ? '95%' : 640}
      centered
      okText="\u4fdd\u5b58"
      cancelText="\u53d6\u6d88"
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
              label="\u9884\u8bbe\u540d\u79f0"
              rules={[
                { required: true, message: '\u8bf7\u8f93\u5165\u9884\u8bbe\u540d\u79f0' },
                { max: 50, message: '\u540d\u79f0\u4e0d\u80fd\u8d85\u8fc7 50 \u4e2a\u5b57\u7b26' },
              ]}
              style={{ marginBottom: 16 }}
            >
              <Input placeholder="\u4f8b\u5982\uff1a\u5de5\u4f5c\u8d26\u53f7-GPT4" />
            </Form.Item>
          </Col>
          <Col xs={24} sm={8}>
            <Form.Item
              name="api_provider"
              label="API \u63d0\u4f9b\u5546"
              rules={[{ required: true, message: '\u8bf7\u9009\u62e9\u63d0\u4f9b\u5546' }]}
              style={{ marginBottom: 16 }}
            >
              <Select placeholder="\u9009\u62e9\u63d0\u4f9b\u5546" onChange={onProviderChange}>
                <Select.Option value="openai">OpenAI</Select.Option>
                <Select.Option value="openai_responses">OpenAI Responses</Select.Option>
                <Select.Option value="anthropic">Claude (Anthropic)</Select.Option>
                <Select.Option value="azure">Azure OpenAI</Select.Option>
                <Select.Option value="newapi">NewAPI</Select.Option>
                <Select.Option value="custom">\u81ea\u5b9a\u4e49</Select.Option>
                <Select.Option value="sub2api">Sub2API</Select.Option>
                <Select.Option value="gemini">Google Gemini</Select.Option>
              </Select>
            </Form.Item>
          </Col>
        </Row>

        <Form.Item
          name="description"
          label="\u9884\u8bbe\u63cf\u8ff0"
          rules={[{ max: 200, message: '\u63cf\u8ff0\u4e0d\u80fd\u8d85\u8fc7 200 \u4e2a\u5b57\u7b26' }]}
          style={{ marginBottom: 16 }}
        >
          <Input placeholder="\u4f8b\u5982\uff1a\u7528\u4e8e\u65e5\u5e38\u5199\u4f5c\u4efb\u52a1\uff08\u53ef\u9009\uff09" />
        </Form.Item>

        <Row gutter={16}>
          <Col xs={24} sm={12}>
            <Form.Item
              name="api_key"
              label="API Key"
              rules={[{ required: true, message: '\u8bf7\u8f93\u5165 API Key' }]}
              style={{ marginBottom: 16 }}
            >
              <Input.Password placeholder="sk-..." />
            </Form.Item>
          </Col>
          <Col xs={24} sm={12}>
            <Form.Item name="api_base_url" label="API Base URL" style={{ marginBottom: 16 }}>
              <Input placeholder="https://api.openai.com/v1" />
            </Form.Item>
          </Col>
        </Row>

        <Row gutter={16}>
          <Col xs={24} sm={12}>
            <Form.Item
              name="llm_model"
              label={
                <Space size={4}>
                  <span>{"\u6a21\u578b\u540d\u79f0"}</span>
                  <InfoCircleOutlined
                    title="AI \u6a21\u578b\u540d\u79f0\uff0c\u70b9\u51fb\u4e0b\u62c9\u6846\u53ef\u81ea\u52a8\u83b7\u53d6\u53ef\u7528\u6a21\u578b"
                    style={{ color: 'var(--color-text-secondary)', fontSize: '12px' }}
                  />
                </Space>
              }
              rules={[{ required: true, message: '\u8bf7\u9009\u62e9\u6216\u8f93\u5165\u6a21\u578b\u540d\u79f0' }]}
              style={{ marginBottom: 16 }}
            >
              <Select
                showSearch
                placeholder="\u70b9\u51fb\u83b7\u53d6\u6a21\u578b\u5217\u8868\u6216\u76f4\u63a5\u8f93\u5165"
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
                        <Spin size="small" /> {"\u6b63\u5728\u83b7\u53d6\u6a21\u578b\u5217\u8868..."}
                      </div>
                    ) : null}
                    {!fetchingPresetModels && mergedPresetModelOptions.length === 0 && presetModelsFetched ? (
                      <div style={{ padding: '8px 12px', color: '#ff4d4f', textAlign: 'center', fontSize: '12px' }}>
                        {"\u672a\u80fd\u83b7\u53d6\u5230\u6a21\u578b\u5217\u8868\uff0c\u8bf7\u68c0\u67e5 API \u914d\u7f6e"}
                      </div>
                    ) : null}
                    {!fetchingPresetModels && mergedPresetModelOptions.length === 0 && !presetModelsFetched ? (
                      <div style={{ padding: '8px 12px', color: 'var(--color-text-secondary)', textAlign: 'center', fontSize: '12px' }}>
                        {"\u70b9\u51fb\u8f93\u5165\u6846\u81ea\u52a8\u83b7\u53d6\u6a21\u578b\u5217\u8868"}
                      </div>
                    ) : null}
                  </>
                )}
                notFoundContent={
                  fetchingPresetModels ? (
                    <div style={{ padding: '8px 12px', textAlign: 'center', fontSize: '12px' }}>
                      <Spin size="small" /> {"\u52a0\u8f7d\u4e2d..."}
                    </div>
                  ) : (
                    <div style={{ padding: '8px 12px', color: 'var(--color-text-secondary)', textAlign: 'center', fontSize: '12px' }}>
                      {"\u672a\u627e\u5230\u5339\u914d\u7684\u6a21\u578b\uff0c\u53ef\u76f4\u63a5\u8f93\u5165\u540e\u6309\u56de\u8f66"}
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
                    title="\u83b7\u53d6\u6a21\u578b\u5217\u8868"
                  >
                    <Button
                      type="text"
                      size="small"
                      icon={<ReloadOutlined />}
                      loading={fetchingPresetModels}
                      style={{ pointerEvents: 'none' }}
                    >
                      {"\u83b7\u53d6"}
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
              label="\u6e29\u5ea6"
              rules={[{ required: true, message: '\u5fc5\u586b' }]}
              style={{ marginBottom: 16 }}
            >
              <InputNumber min={0} max={2} step={0.1} style={{ width: '100%' }} placeholder="0.7" />
            </Form.Item>
          </Col>
          <Col xs={12} sm={6}>
            <Form.Item
              name="max_tokens"
              label="\u6700\u5927 Tokens"
              rules={[{ required: true, message: '\u5fc5\u586b' }]}
              style={{ marginBottom: 16 }}
            >
              <InputNumber min={1} max={100000} style={{ width: '100%' }} placeholder="2000" />
            </Form.Item>
          </Col>
        </Row>

        <Form.Item name="system_prompt" label="\u7cfb\u7edf\u63d0\u793a\u8bcd" style={{ marginBottom: 0 }}>
          <TextArea
            rows={isMobile ? 2 : 3}
            placeholder="\u4f8b\u5982\uff1a\u4f60\u662f\u4e00\u4e2a\u4e13\u4e1a\u7684\u5c0f\u8bf4\u521b\u4f5c\u52a9\u624b...\uff08\u53ef\u9009\uff09"
            maxLength={10000}
            showCount
          />
        </Form.Item>
      </Form>
    </Modal>
  );
}
