import { Button, Empty, List, Popconfirm, Space, Tag, Typography, theme } from 'antd';
import { CheckCircleOutlined, CopyOutlined, DeleteOutlined, EditOutlined, PlusOutlined, ThunderboltOutlined } from '@ant-design/icons';
import type { APIKeyPreset } from '../types';
import { designDisplayFont } from '../theme/themeConfig';
import InlineDeferredPanel from './InlineDeferredPanel';

type SettingsPresetsTabProps = {
  presetsLoading: boolean;
  presets: APIKeyPreset[];
  activePresetId?: string;
  testingPresetId: string | null;
  onCreateFromCurrent: () => void;
  onCreatePreset: () => void;
  onActivatePreset: (presetId: string, presetName: string) => void;
  onTestPreset: (presetId: string) => void;
  onEditPreset: (preset: APIKeyPreset) => void;
  onDeletePreset: (presetId: string) => void;
};

const { Text } = Typography;

const getProviderColor = (provider: string) => {
  switch (provider) {
    case 'openai':
      return 'blue';
    case 'openai_responses':
      return 'geekblue';
    case 'anthropic':
      return 'volcano';
    case 'azure':
      return 'cyan';
    case 'newapi':
      return 'orange';
    case 'custom':
      return 'purple';
    case 'sub2api':
      return 'magenta';
    case 'gemini':
      return 'green';
    default:
      return 'default';
  }
};

export default function SettingsPresetsTab({
  presetsLoading,
  presets,
  activePresetId,
  testingPresetId,
  onCreateFromCurrent,
  onCreatePreset,
  onActivatePreset,
  onTestPreset,
  onEditPreset,
  onDeletePreset,
}: SettingsPresetsTabProps) {
  const { token } = theme.useToken();
  const alphaColor = (color: string, alpha: number) => `color-mix(in srgb, ${color} ${(alpha * 100).toFixed(0)}%, transparent)`;

  return (
    presetsLoading ? (
      <InlineDeferredPanel
        eyebrow="Preset Workspace"
        title="正在恢复 API 预设管理台"
        message="系统正在准备预设列表、激活状态与创建入口，原有复制、切换、测试、编辑和删除逻辑保持不变。"
        minHeight={280}
        tags={[
          { label: '预设列表同步中', color: 'processing' },
          { label: '激活状态待恢复', color: activePresetId ? 'green' : 'default' },
          { label: '预设逻辑保持原样', color: 'green' },
        ]}
      />
    ) : (
      <Space direction="vertical" size="middle" style={{ width: '100%' }}>
        <div
          style={{
            padding: '18px 20px',
            borderRadius: 20,
            background: `linear-gradient(135deg, ${alphaColor(token.colorPrimaryBg, 0.9)} 0%, ${alphaColor(token.colorBgElevated, 0.98)} 100%)`,
            border: `1px solid ${alphaColor(token.colorPrimary, 0.14)}`,
            boxShadow: `0 18px 36px ${alphaColor(token.colorText, 0.05)}`,
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
                Preset Workspace
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
                API 预设管理台
              </Text>
              <Text type="secondary" style={{ display: 'block', lineHeight: 1.75 }}>
                先看当前激活状态和供应商组合，再决定复制、创建、测试或切换预设。这里只调整阅读顺序，不改变任何预设逻辑。
              </Text>
            </div>
            <Space wrap size={[8, 8]} style={{ justifyContent: 'flex-end' }}>
              <Tag color="blue" style={{ margin: 0, borderRadius: 999, paddingInline: 10 }}>
                预设数 {presets.length}
              </Tag>
              {activePresetId ? (
                <Tag color="success" style={{ margin: 0, borderRadius: 999, paddingInline: 10 }}>
                  已有激活预设
                </Tag>
              ) : (
                <Tag color="default" style={{ margin: 0, borderRadius: 999, paddingInline: 10 }}>
                  当前未激活预设
                </Tag>
              )}
            </Space>
          </div>
          <div style={{ display: 'flex', flexWrap: 'wrap', gap: 8, marginTop: 14 }}>
            <Button icon={<CopyOutlined />} onClick={onCreateFromCurrent}>
              复制当前配置
            </Button>
            <Button type="primary" icon={<PlusOutlined />} onClick={onCreatePreset}>
              新建预设
            </Button>
          </div>
        </div>

        {presets.length === 0 ? (
          <Empty description="暂无配置预设" image={Empty.PRESENTED_IMAGE_SIMPLE} style={{ margin: '40px 0' }}>
            <Button type="primary" icon={<PlusOutlined />} onClick={onCreatePreset}>
              创建第一个预设
            </Button>
          </Empty>
        ) : (
          <List
            dataSource={presets}
            renderItem={(preset) => {
              const isActive = preset.id === activePresetId;
              return (
                <List.Item
                  key={preset.id}
                  style={{
                    background: isActive
                      ? `linear-gradient(135deg, ${alphaColor(token.colorPrimaryBg, 0.84)} 0%, ${alphaColor(token.colorBgElevated, 0.98)} 100%)`
                      : `linear-gradient(180deg, ${alphaColor(token.colorBgContainer, 0.98)} 0%, ${alphaColor(token.colorFillAlter, 0.4)} 100%)`,
                    padding: '18px',
                    marginBottom: '12px',
                    border: isActive
                      ? `1px solid ${alphaColor(token.colorPrimary, 0.24)}`
                      : `1px solid ${alphaColor(token.colorBorderSecondary, 0.88)}`,
                    borderRadius: '20px',
                    display: 'block',
                    boxShadow: `0 16px 32px ${alphaColor(token.colorText, 0.05)}`,
                  }}
                >
                  <div style={{ display: 'grid', gap: 14 }}>
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
                          Preset Dossier
                        </Text>
                        <Space wrap size={[8, 8]} style={{ marginBottom: 8 }}>
                          <Text
                            strong
                            style={{
                              fontSize: 17,
                              fontFamily: designDisplayFont,
                              letterSpacing: '-0.02em',
                            }}
                          >
                            {preset.name}
                          </Text>
                          {isActive ? (
                            <Tag color="success" style={{ margin: 0, borderRadius: 999, paddingInline: 10 }}>
                              已激活
                            </Tag>
                          ) : null}
                        </Space>
                        {preset.description ? (
                          <Text type="secondary" style={{ display: 'block', lineHeight: 1.75 }}>
                            {preset.description}
                          </Text>
                        ) : (
                          <Text type="secondary" style={{ display: 'block', lineHeight: 1.75 }}>
                            这个预设当前没有补充说明，可通过编辑补上使用场景或切换策略说明。
                          </Text>
                        )}
                      </div>
                      <div style={{ display: 'flex', alignItems: 'center', gap: 10 }}>
                        {isActive ? <CheckCircleOutlined style={{ fontSize: 24, color: token.colorSuccess }} /> : null}
                        <div
                          style={{
                            padding: '10px 12px',
                            borderRadius: 16,
                            background: alphaColor(token.colorBgElevated, 0.95),
                            border: `1px solid ${alphaColor(token.colorBorderSecondary, 0.86)}`,
                            minWidth: 112,
                          }}
                        >
                          <Text style={{ display: 'block', fontSize: 11, letterSpacing: '0.08em', textTransform: 'uppercase', color: token.colorTextTertiary, marginBottom: 4 }}>
                            Provider
                          </Text>
                          <Text strong style={{ display: 'block', fontSize: 14 }}>
                            {preset.config.api_provider.toUpperCase()}
                          </Text>
                        </div>
                      </div>
                    </div>

                    <div
                      style={{
                        padding: '14px 16px',
                        borderRadius: 18,
                        background: alphaColor(token.colorBgElevated, 0.96),
                        border: `1px solid ${alphaColor(token.colorBorderSecondary, 0.86)}`,
                      }}
                    >
                      <Text style={{ display: 'block', fontSize: 11, letterSpacing: '0.08em', textTransform: 'uppercase', color: token.colorTextTertiary, marginBottom: 8 }}>
                        Runtime Snapshot
                      </Text>
                      <Space wrap size={[8, 8]}>
                        <Tag color={getProviderColor(preset.config.api_provider)} style={{ margin: 0, borderRadius: 999, paddingInline: 10 }}>
                          {preset.config.api_provider.toUpperCase()}
                        </Tag>
                        <Tag style={{ margin: 0, borderRadius: 999, paddingInline: 10 }}>{preset.config.llm_model}</Tag>
                        <Tag style={{ margin: 0, borderRadius: 999, paddingInline: 10 }}>温度 {preset.config.temperature}</Tag>
                        <Tag style={{ margin: 0, borderRadius: 999, paddingInline: 10 }}>Tokens {preset.config.max_tokens}</Tag>
                      </Space>
                      <Text type="secondary" style={{ display: 'block', fontSize: 12, lineHeight: 1.7, marginTop: 10 }}>
                        创建时间：{new Date(preset.created_at).toLocaleString()}
                      </Text>
                    </div>

                    <div
                      style={{
                        padding: '14px 16px',
                        borderRadius: 18,
                        background: alphaColor(token.colorFillQuaternary, 0.72),
                      }}
                    >
                      <Text style={{ display: 'block', fontSize: 11, letterSpacing: '0.08em', textTransform: 'uppercase', color: token.colorTextTertiary, marginBottom: 8 }}>
                        Available Actions
                      </Text>
                      <div style={{ display: 'flex', flexWrap: 'wrap', gap: 8 }}>
                        {!isActive ? (
                          <Button key="activate" type="primary" onClick={() => onActivatePreset(preset.id, preset.name)}>
                            激活
                          </Button>
                        ) : null}
                        <Button
                          key="test"
                          icon={<ThunderboltOutlined />}
                          loading={testingPresetId === preset.id}
                          onClick={() => onTestPreset(preset.id)}
                        >
                          测试
                        </Button>
                        <Button key="edit" icon={<EditOutlined />} onClick={() => onEditPreset(preset)}>
                          编辑
                        </Button>
                        <Popconfirm
                          key="delete"
                          title="确定要删除这个预设吗？"
                          onConfirm={() => onDeletePreset(preset.id)}
                          disabled={isActive}
                          okText="确定"
                          cancelText="取消"
                        >
                          <Button danger icon={<DeleteOutlined />} disabled={isActive}>
                            删除
                          </Button>
                        </Popconfirm>
                      </div>
                    </div>
                  </div>
                </List.Item>
              );
            }}
          />
        )}
      </Space>
    )
  );
}
