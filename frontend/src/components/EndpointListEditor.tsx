import React from 'react';
import { Input, Button, Space, Badge, Tooltip, Typography, theme } from 'antd';
import { PlusOutlined, DeleteOutlined, CheckCircleOutlined, CloseCircleOutlined, LoadingOutlined, QuestionCircleOutlined } from '@ant-design/icons';
import { designDisplayFont } from '../theme/themeConfig';

interface Endpoint {
  url: string;
  type: 'primary' | 'fallback';
  status?: 'success' | 'error' | 'pending' | 'untested';
  lastTestTime?: string;
  responseTime?: number;
  error?: string;
}

interface EndpointListEditorProps {
  endpoints: Endpoint[];
  onChange: (endpoints: Endpoint[]) => void;
  onTest?: (endpoint: string, index: number) => Promise<void>;
  loading?: boolean;
  disabled?: boolean;
}

const { Text } = Typography;

const EndpointListEditor: React.FC<EndpointListEditorProps> = ({
  endpoints,
  onChange,
  onTest,
  loading = false,
  disabled = false,
}) => {
  const { token } = theme.useToken();
  const alphaColor = (color: string, alpha: number) => `color-mix(in srgb, ${color} ${(alpha * 100).toFixed(0)}%, transparent)`;

  const handleAdd = () => {
    const newEndpoint: Endpoint = {
      url: '',
      type: endpoints.length === 0 ? 'primary' : 'fallback',
      status: 'untested',
    };
    onChange([...endpoints, newEndpoint]);
  };

  const handleRemove = (index: number) => {
    const newEndpoints = endpoints.filter((_, i) => i !== index);
    // 如果删除的是主端点，将第一个备端点升级为主端点
    if (index === 0 && newEndpoints.length > 0) {
      newEndpoints[0].type = 'primary';
    }
    onChange(newEndpoints);
  };

  const handleUrlChange = (index: number, url: string) => {
    const newEndpoints = [...endpoints];
    newEndpoints[index].url = url;
    newEndpoints[index].status = 'untested';
    onChange(newEndpoints);
  };

  const handleTest = async (index: number) => {
    if (onTest && endpoints[index].url) {
      await onTest(endpoints[index].url, index);
    }
  };

  const getStatusBadge = (status?: string) => {
    switch (status) {
      case 'success':
        return <Badge status="success" text="正常" />;
      case 'error':
        return <Badge status="error" text="错误" />;
      case 'pending':
        return <Badge status="processing" text="测试中" />;
      case 'untested':
      default:
        return <Badge status="default" text="未测试" />;
    }
  };

  const getStatusIcon = (status?: string) => {
    switch (status) {
      case 'success':
        return <CheckCircleOutlined style={{ color: '#52c41a' }} />;
      case 'error':
        return <CloseCircleOutlined style={{ color: '#ff4d4f' }} />;
      case 'pending':
        return <LoadingOutlined style={{ color: '#1890ff' }} />;
      case 'untested':
      default:
        return <QuestionCircleOutlined style={{ color: '#d9d9d9' }} />;
    }
  };

  const getEndpointPalette = (endpoint: Endpoint) => {
    if (endpoint.status === 'success') {
      return {
        border: alphaColor(token.colorSuccess, 0.22),
        shell: `linear-gradient(135deg, ${alphaColor(token.colorSuccessBg, 0.9)} 0%, ${alphaColor(token.colorBgElevated, 0.98)} 100%)`,
        panel: alphaColor(token.colorSuccessBg, 0.78),
      };
    }
    if (endpoint.status === 'error') {
      return {
        border: alphaColor(token.colorError, 0.22),
        shell: `linear-gradient(135deg, ${alphaColor(token.colorErrorBg, 0.92)} 0%, ${alphaColor(token.colorBgElevated, 0.98)} 100%)`,
        panel: alphaColor(token.colorErrorBg, 0.72),
      };
    }
    if (endpoint.type === 'primary') {
      return {
        border: alphaColor(token.colorPrimary, 0.22),
        shell: `linear-gradient(135deg, ${alphaColor(token.colorPrimaryBg, 0.9)} 0%, ${alphaColor(token.colorBgElevated, 0.98)} 100%)`,
        panel: alphaColor(token.colorPrimaryBg, 0.72),
      };
    }
    return {
      border: alphaColor(token.colorBorderSecondary, 0.9),
      shell: `linear-gradient(180deg, ${alphaColor(token.colorBgContainer, 0.98)} 0%, ${alphaColor(token.colorFillAlter, 0.4)} 100%)`,
      panel: alphaColor(token.colorFillQuaternary, 0.72),
    };
  };

  return (
    <div>
      <Space direction="vertical" style={{ width: '100%' }} size="middle">
        <div
          style={{
            padding: '16px 18px',
            borderRadius: 20,
            background: `linear-gradient(135deg, ${alphaColor(token.colorPrimaryBg, 0.88)} 0%, ${alphaColor(token.colorBgElevated, 0.98)} 100%)`,
            border: `1px solid ${alphaColor(token.colorPrimary, 0.14)}`,
            boxShadow: `0 16px 32px ${alphaColor(token.colorText, 0.05)}`,
          }}
        >
          <Text style={{ display: 'block', fontSize: 11, letterSpacing: '0.08em', textTransform: 'uppercase', color: token.colorTextTertiary, marginBottom: 6 }}>
            Endpoint Workspace
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
            端点编排与连通性检查
          </Text>
          <Text type="secondary" style={{ display: 'block', lineHeight: 1.75 }}>
            先确认主端点和备端点顺序，再执行测试或删除。这里只整理阅读顺序和卡面层级，不改变端点测试、主备切换或状态回写逻辑。
          </Text>
        </div>

        {endpoints.map((endpoint, index) => (
          (() => {
            const palette = getEndpointPalette(endpoint);
            return (
              <div
                key={index}
                style={{
                  border: `1px solid ${palette.border}`,
                  borderRadius: 22,
                  padding: 18,
                  background: palette.shell,
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
                        Endpoint Dossier
                      </Text>
                      <Space wrap size={[8, 8]}>
                        <Text
                          strong
                          style={{
                            fontSize: 16,
                            fontFamily: designDisplayFont,
                            letterSpacing: '-0.02em',
                          }}
                        >
                          {endpoint.type === 'primary' ? '主端点 (Primary)' : `备端点 ${index} (Fallback)`}
                        </Text>
                        {endpoint.type === 'primary' ? (
                          <Text style={{ color: token.colorError, fontSize: 12 }}>* 必填</Text>
                        ) : null}
                      </Space>
                      <Text type="secondary" style={{ display: 'block', lineHeight: 1.75, marginTop: 6 }}>
                        {endpoint.type === 'primary'
                          ? '主端点决定默认请求入口，建议优先保证可用性与正确协议路径。'
                          : '备端点用于失败回退或切换验证，适合在主端点之外补充稳定候选。'}
                      </Text>
                    </div>
                    <div
                      style={{
                        padding: '10px 12px',
                        borderRadius: 16,
                        background: alphaColor(token.colorBgElevated, 0.95),
                        border: `1px solid ${palette.border}`,
                        minWidth: 120,
                      }}
                    >
                      <Text style={{ display: 'block', fontSize: 11, letterSpacing: '0.08em', textTransform: 'uppercase', color: token.colorTextTertiary, marginBottom: 4 }}>
                        Status
                      </Text>
                      <div>{getStatusBadge(endpoint.status)}</div>
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
                      Connection Target
                    </Text>
                    <div style={{ display: 'flex', flexWrap: 'wrap', gap: 10, alignItems: 'center' }}>
                      <div style={{ minWidth: 0, flex: 1 }}>
                        <Input
                          placeholder={endpoint.type === 'primary' ? 'https://api.openai.com/v1' : 'https://api-backup.openai.com/v1'}
                          value={endpoint.url}
                          onChange={(e) => handleUrlChange(index, e.target.value)}
                          disabled={disabled || loading}
                          style={{ width: '100%' }}
                          prefix={getStatusIcon(endpoint.status)}
                        />
                      </div>

                      <Tooltip title={endpoint.status === 'success' ? `响应时间: ${endpoint.responseTime}ms` : endpoint.error || '测试端点连接'}>
                        <Button
                          onClick={() => handleTest(index)}
                          disabled={!endpoint.url || disabled || loading}
                          loading={endpoint.status === 'pending'}
                        >
                          测试
                        </Button>
                      </Tooltip>

                      {(endpoint.type === 'fallback' || endpoints.length > 1) && (
                        <Button
                          danger
                          icon={<DeleteOutlined />}
                          onClick={() => handleRemove(index)}
                          disabled={disabled || loading}
                        >
                          删除
                        </Button>
                      )}
                    </div>
                  </div>

                  {endpoint.status && endpoint.status !== 'untested' ? (
                    <div
                      style={{
                        padding: '12px 14px',
                        borderRadius: 16,
                        background: palette.panel,
                      }}
                    >
                      <Text style={{ display: 'block', fontSize: 11, letterSpacing: '0.08em', textTransform: 'uppercase', color: token.colorTextTertiary, marginBottom: 8 }}>
                        Runtime Feedback
                      </Text>
                      {endpoint.lastTestTime ? (
                        <Text type="secondary" style={{ display: 'block', fontSize: 12, lineHeight: 1.7 }}>
                          最后测试：{endpoint.lastTestTime}
                        </Text>
                      ) : null}
                      {endpoint.error ? (
                        <Text style={{ display: 'block', color: token.colorError, fontSize: 12, lineHeight: 1.7, marginTop: endpoint.lastTestTime ? 4 : 0 }}>
                          错误：{endpoint.error}
                        </Text>
                      ) : null}
                    </div>
                  ) : null}
                </div>
              </div>
            );
          })()
        ))}

        <Button
          type="dashed"
          icon={<PlusOutlined />}
          onClick={handleAdd}
          disabled={disabled || loading}
          style={{ width: '100%', borderRadius: 16, minHeight: 44 }}
        >
          添加备端点
        </Button>
      </Space>

      {endpoints.length === 0 && (
        <div
          style={{
            textAlign: 'center',
            padding: 24,
            color: token.colorTextTertiary,
            borderRadius: 18,
            border: `1px dashed ${alphaColor(token.colorBorderSecondary, 0.86)}`,
            marginTop: 12,
          }}
        >
          请添加至少一个主端点
        </div>
      )}
    </div>
  );
};

export default EndpointListEditor;
