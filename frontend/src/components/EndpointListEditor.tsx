import React from 'react';
import { Input, Button, Space, Badge, Tooltip } from 'antd';
import { PlusOutlined, DeleteOutlined, CheckCircleOutlined, CloseCircleOutlined, LoadingOutlined, QuestionCircleOutlined } from '@ant-design/icons';

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

const EndpointListEditor: React.FC<EndpointListEditorProps> = ({
  endpoints,
  onChange,
  onTest,
  loading = false,
  disabled = false,
}) => {
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

  return (
    <div>
      <Space direction="vertical" style={{ width: '100%' }} size="middle">
        {endpoints.map((endpoint, index) => (
          <div
            key={index}
            style={{
              border: '1px solid #d9d9d9',
              borderRadius: 4,
              padding: 12,
              backgroundColor: endpoint.type === 'primary' ? '#f0f5ff' : '#fff',
            }}
          >
            <div style={{ marginBottom: 8 }}>
              <Space>
                <span style={{ fontWeight: 'bold' }}>
                  {endpoint.type === 'primary' ? '主端点 (Primary)' : `备端点 ${index} (Fallback)`}
                </span>
                {endpoint.type === 'primary' && (
                  <span style={{ color: '#ff4d4f', fontSize: 12 }}>* 必填</span>
                )}
              </Space>
            </div>

            <Space style={{ width: '100%' }} size="middle">
              <Input
                placeholder={endpoint.type === 'primary' ? 'https://api.openai.com/v1' : 'https://api-backup.openai.com/v1'}
                value={endpoint.url}
                onChange={(e) => handleUrlChange(index, e.target.value)}
                disabled={disabled || loading}
                style={{ width: 400 }}
                prefix={getStatusIcon(endpoint.status)}
              />

              <Tooltip title={endpoint.status === 'success' ? `响应时间: ${endpoint.responseTime}ms` : endpoint.error || '测试端点连接'}>
                <Button
                  size="small"
                  onClick={() => handleTest(index)}
                  disabled={!endpoint.url || disabled || loading}
                  loading={endpoint.status === 'pending'}
                >
                  测试
                </Button>
              </Tooltip>

              {(endpoint.type === 'fallback' || endpoints.length > 1) && (
                <Button
                  size="small"
                  danger
                  icon={<DeleteOutlined />}
                  onClick={() => handleRemove(index)}
                  disabled={disabled || loading}
                >
                  删除
                </Button>
              )}
            </Space>

            {endpoint.status && endpoint.status !== 'untested' && (
              <div style={{ marginTop: 8, fontSize: 12 }}>
                {getStatusBadge(endpoint.status)}
                {endpoint.lastTestTime && (
                  <span style={{ marginLeft: 8, color: '#999' }}>
                    最后测试: {endpoint.lastTestTime}
                  </span>
                )}
                {endpoint.error && (
                  <div style={{ color: '#ff4d4f', marginTop: 4 }}>
                    错误: {endpoint.error}
                  </div>
                )}
              </div>
            )}
          </div>
        ))}

        <Button
          type="dashed"
          icon={<PlusOutlined />}
          onClick={handleAdd}
          disabled={disabled || loading}
          style={{ width: '100%' }}
        >
          添加备端点
        </Button>
      </Space>

      {endpoints.length === 0 && (
        <div style={{ textAlign: 'center', padding: 20, color: '#999' }}>
          请添加至少一个主端点
        </div>
      )}
    </div>
  );
};

export default EndpointListEditor;
