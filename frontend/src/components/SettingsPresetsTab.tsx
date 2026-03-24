import { Button, Empty, List, Popconfirm, Space, Spin, Tag, Typography } from 'antd';
import { CheckCircleOutlined, CopyOutlined, DeleteOutlined, EditOutlined, PlusOutlined, ThunderboltOutlined } from '@ant-design/icons';
import type { APIKeyPreset } from '../types';

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
  return (
    <Spin spinning={presetsLoading}>
      <Space direction="vertical" size="middle" style={{ width: '100%' }}>
        <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
          <Text type="secondary">???? API ?????????????</Text>
          <Space>
            <Button icon={<CopyOutlined />} onClick={onCreateFromCurrent}>
              ?????
            </Button>
            <Button type="primary" icon={<PlusOutlined />} onClick={onCreatePreset}>
              ????
            </Button>
          </Space>
        </div>

        {presets.length === 0 ? (
          <Empty description="??????" image={Empty.PRESENTED_IMAGE_SIMPLE} style={{ margin: '40px 0' }}>
            <Button type="primary" icon={<PlusOutlined />} onClick={onCreatePreset}>
              ???????
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
                    background: isActive ? '#f0f5ff' : 'transparent',
                    padding: '16px',
                    marginBottom: '8px',
                    border: isActive ? '2px solid #1890ff' : '1px solid #f0f0f0',
                    borderRadius: '8px',
                  }}
                  actions={[
                    !isActive ? (
                      <Button key="activate" type="link" onClick={() => onActivatePreset(preset.id, preset.name)}>
                        ??
                      </Button>
                    ) : null,
                    <Button
                      key="test"
                      type="link"
                      icon={<ThunderboltOutlined />}
                      loading={testingPresetId === preset.id}
                      onClick={() => onTestPreset(preset.id)}
                    >
                      ??
                    </Button>,
                    <Button key="edit" type="link" icon={<EditOutlined />} onClick={() => onEditPreset(preset)}>
                      ??
                    </Button>,
                    <Popconfirm
                      key="delete"
                      title="?????????"
                      onConfirm={() => onDeletePreset(preset.id)}
                      disabled={isActive}
                      okText="??"
                      cancelText="??"
                    >
                      <Button type="link" danger icon={<DeleteOutlined />} disabled={isActive}>
                        ??
                      </Button>
                    </Popconfirm>,
                  ].filter(Boolean)}
                >
                  <List.Item.Meta
                    avatar={
                      isActive ? <CheckCircleOutlined style={{ fontSize: '24px', color: '#52c41a' }} /> : null
                    }
                    title={
                      <Space>
                        <span style={{ fontWeight: 'bold' }}>{preset.name}</span>
                        {isActive ? <Tag color="success">???</Tag> : null}
                      </Space>
                    }
                    description={
                      <Space direction="vertical" size="small" style={{ width: '100%' }}>
                        {preset.description ? <div style={{ color: '#666' }}>{preset.description}</div> : null}
                        <Space wrap>
                          <Tag color={getProviderColor(preset.config.api_provider)}>
                            {preset.config.api_provider.toUpperCase()}
                          </Tag>
                          <Tag>{preset.config.llm_model}</Tag>
                          <Tag>??: {preset.config.temperature}</Tag>
                          <Tag>Tokens: {preset.config.max_tokens}</Tag>
                        </Space>
                        <div style={{ fontSize: '12px', color: '#999' }}>
                          ???: {new Date(preset.created_at).toLocaleString()}
                        </div>
                      </Space>
                    }
                  />
                </List.Item>
              );
            }}
          />
        )}
      </Space>
    </Spin>
  );
}
