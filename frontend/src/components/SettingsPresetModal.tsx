import { Button, Card, Col, Form, Input, InputNumber, Modal, Row, Select, Space, Tag, Typography, theme } from 'antd';
import { InfoCircleOutlined, ReloadOutlined } from '@ant-design/icons';
import type { FormInstance } from 'antd';
import type { APIKeyPreset, APIKeyPresetConfig } from '../types';
import { designDisplayFont } from '../theme/themeConfig';
import { renderCompactSettingHint } from './storyCreationCommonUi';

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
  const { token } = theme.useToken();
  const alphaColor = (color: string, alpha: number) => `color-mix(in srgb, ${color} ${(alpha * 100).toFixed(0)}%, transparent)`;
  const editorialInk = '#f7f1e8';
  const heroBackground = `linear-gradient(135deg, #171411 0%, color-mix(in srgb, #171411 58%, ${token.colorPrimary} 42%) 100%)`;
  const quietPanelBackground = `linear-gradient(180deg, color-mix(in srgb, ${token.colorBgContainer} 95%, ${token.colorFillAlter} 5%) 0%, color-mix(in srgb, ${token.colorBgContainer} 87%, ${token.colorFillAlter} 13%) 100%)`;
  const { Paragraph, Text, Title } = Typography;
  const presetGuideSteps = [
    '先确定这次是在新建预设还是编辑已有预设，把表单当作模型接入信息的整理区，而不是业务设置页。',
    '再依次补齐 provider、API 入口和模型来源，优先让连接信息稳定，再回头微调温度与 token 上限。',
    '最后根据右侧焦点卡判断是等待模型列表返回，还是直接手动输入模型名称完成这次保存。',
  ];
  const presetWorkspaceFocus = fetchingPresetModels
    ? {
        title: '当前正在获取模型列表，先等待候选返回后再决定最终模型名',
        note: '这时最重要的是保持 provider 和接口配置稳定，不需要频繁重复触发刷新；候选返回后可以继续沿原有表单逻辑保存。',
      }
    : presetModelsFetched && mergedPresetModelOptions.length === 0
      ? {
          title: '当前未成功拿到模型列表，适合先检查接口地址或直接手动输入模型',
          note: '这里仍然保留原有的手动输入路径，不需要改变任何请求流程；重点是把可用的接入信息先保存下来。',
        }
      : editingPreset
        ? {
            title: `当前正在校对预设 ${editingPreset.name}，优先确认模型接入信息是否仍然匹配`,
            note: '这次更适合把它当成一次已有预设的修订动作，先检查 provider、API Base URL 和模型名，再更新参数细节。',
          }
        : {
            title: '当前适合先完成新的模型接入预设，再补齐生成参数细节',
            note: '先建立可用的 provider 与模型入口，后续温度、最大 token 和系统提示词都可以在同一张表单里继续微调。',
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
    <Modal
      title={
        <Space direction="vertical" size={6} style={{ width: '100%' }}>
          <Tag
            bordered={false}
            style={{
              alignSelf: 'center',
              borderRadius: 999,
              paddingInline: 12,
              lineHeight: '28px',
              background: alphaColor(token.colorPrimary, 0.12),
              color: token.colorPrimary,
            }}
          >
            Model Preset Studio
          </Tag>
          <Title level={4} style={{ margin: 0, textAlign: 'center', fontFamily: designDisplayFont, letterSpacing: '-0.03em' }}>
            {editingPreset ? '编辑预设' : '创建预设'}
          </Title>
        </Space>
      }
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
          background: quietPanelBackground,
        },
      }}
    >
      <Card
        bordered={false}
        style={{
          marginBottom: 16,
          borderRadius: 20,
          overflow: 'hidden',
          background: heroBackground,
        }}
        styles={{ body: { padding: 20 } }}
      >
        <Text style={{ color: alphaColor(token.colorWhite, 0.68), letterSpacing: '0.14em', textTransform: 'uppercase' }}>
          Preset Connection Brief
        </Text>
        <Title level={5} style={{ margin: '8px 0 10px', color: editorialInk, fontFamily: designDisplayFont, letterSpacing: '-0.03em' }}>
          先整理模型接入，再完成生成参数微调
        </Title>
        <Paragraph style={{ margin: 0, color: alphaColor(token.colorWhite, 0.82), lineHeight: 1.7 }}>
          这个弹窗现在只增强阅读顺序与当前焦点说明，不改变 provider 切换、模型获取、手动输入、保存提交或取消关闭逻辑。
        </Paragraph>
      </Card>

      <Card
        bordered={false}
        style={{
          marginBottom: 16,
          borderRadius: 18,
          background: quietPanelBackground,
          border: `1px solid ${alphaColor(token.colorPrimary, 0.08)}`,
        }}
        styles={{ body: { padding: 18 } }}
      >
        <div
          style={{
            display: 'grid',
            gridTemplateColumns: 'repeat(auto-fit, minmax(220px, 1fr))',
            gap: 16,
          }}
        >
          <div>
            <Text style={{ fontSize: 12, letterSpacing: '0.12em', textTransform: 'uppercase', color: token.colorTextTertiary }}>
              Preset Guide
            </Text>
            <Title level={5} style={{ margin: '6px 0 8px', fontFamily: designDisplayFont }}>
              预设编辑顺序
            </Title>
            <Paragraph style={{ margin: 0, color: token.colorTextSecondary, lineHeight: 1.75 }}>
              这里优先把接入链路信息排在前面，帮助先完成 provider、接口与模型名确认，再继续处理生成参数细节。
            </Paragraph>
            <div style={{ display: 'flex', flexWrap: 'wrap', gap: 8, marginTop: 12 }}>
              {presetGuideSteps.map((item, index) => (
                <span
                  key={item}
                  style={{
                    display: 'inline-flex',
                    alignItems: 'center',
                    gap: 8,
                    padding: '6px 12px',
                    borderRadius: 999,
                    background: token.colorBgContainer,
                    border: `1px solid ${token.colorBorderSecondary}`,
                    color: token.colorTextSecondary,
                    fontSize: 12,
                  }}
                >
                  <span style={{ color: token.colorPrimary, fontWeight: 700 }}>{index + 1}</span>
                  {item}
                </span>
              ))}
            </div>
          </div>
          <div
            style={{
              borderRadius: 16,
              padding: '16px 18px',
              background: token.colorBgContainer,
              border: `1px solid ${token.colorBorderSecondary}`,
            }}
          >
            <Text style={{ display: 'block', fontSize: 12, letterSpacing: '0.12em', textTransform: 'uppercase', color: token.colorTextTertiary }}>
              当前工作焦点
            </Text>
            <Title level={5} style={{ margin: '8px 0 6px', fontFamily: designDisplayFont }}>
              {presetWorkspaceFocus.title}
            </Title>
            <Paragraph style={{ margin: 0, color: token.colorTextSecondary, lineHeight: 1.75 }}>
              {presetWorkspaceFocus.note}
            </Paragraph>
            <Space wrap size={[8, 8]} style={{ marginTop: 12 }}>
              <Tag color={editingPreset ? 'processing' : 'blue'} style={{ margin: 0, borderRadius: 999, paddingInline: 10 }}>
                {editingPreset ? '编辑已有预设' : '创建新预设'}
              </Tag>
              <Tag color={fetchingPresetModels ? 'gold' : 'green'} style={{ margin: 0, borderRadius: 999, paddingInline: 10 }}>
                {fetchingPresetModels ? '模型列表获取中' : '模型列表可继续处理'}
              </Tag>
              <Tag color={presetModelsFetched && mergedPresetModelOptions.length === 0 ? 'default' : 'cyan'} style={{ margin: 0, borderRadius: 999, paddingInline: 10 }}>
                {presetModelsFetched && mergedPresetModelOptions.length === 0 ? '可手动输入模型' : '可选模型已接入'}
              </Tag>
            </Space>
          </div>
        </div>
      </Card>

      <Card
        bordered={false}
        style={{
          borderRadius: 18,
          background: token.colorBgContainer,
          border: `1px solid ${alphaColor(token.colorPrimary, 0.08)}`,
        }}
        styles={{ body: { padding: isMobile ? 16 : 20 } }}
      >
        <div style={{ marginBottom: 14 }}>
          <Text style={{ fontSize: 12, letterSpacing: '0.12em', textTransform: 'uppercase', color: token.colorTextTertiary }}>
            Preset Workspace
          </Text>
          <Title level={5} style={{ margin: '6px 0 0', fontFamily: designDisplayFont }}>
            填写接入信息与生成参数
          </Title>
        </div>

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
                        renderModelStatusHint(
                          '模型候选正在返回',
                          '保持当前 provider 与接口地址即可，候选列表返回后仍可沿原有流程手动选择或直接输入模型名。',
                        )
                      ) : null}
                      {!fetchingPresetModels && mergedPresetModelOptions.length === 0 && presetModelsFetched ? (
                        renderModelStatusHint(
                          '暂时未取回模型列表',
                          '可以先检查 API Base URL / 模型列表地址，也可以直接手动输入模型名称完成这次保存。',
                          'warning',
                        )
                      ) : null}
                      {!fetchingPresetModels && mergedPresetModelOptions.length === 0 && !presetModelsFetched ? (
                        renderModelStatusHint(
                          '点开后会自动拉取模型列表',
                          '如果你已经知道目标模型，也可以直接输入模型名称并按回车提交当前字段。',
                        )
                      ) : null}
                    </>
                  )}
                  notFoundContent={
                    fetchingPresetModels ? (
                      renderModelStatusHint(
                        '还在整理匹配候选',
                        '下拉内的模型结果正在返回，稍等片刻即可继续选择；已有输入内容不会丢失。',
                      )
                    ) : (
                      renderModelStatusHint(
                        '暂时没有匹配项',
                        '可以继续搜索，也可以直接输入模型名称后按回车，把这次配置先保存下来。',
                      )
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
      </Card>
    </Modal>
  );
}
