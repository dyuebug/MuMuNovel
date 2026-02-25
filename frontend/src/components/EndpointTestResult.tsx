import React from 'react';
import { Card, Space, Timeline, Statistic, Alert, Tag } from 'antd';
import { CheckCircleOutlined, CloseCircleOutlined, ClockCircleOutlined, ThunderboltOutlined } from '@ant-design/icons';

interface EndpointTestResult {
  endpoint: string;
  type: 'primary' | 'fallback';
  success: boolean;
  responseTime?: number;
  error?: string;
  suggestions?: string[];
}

interface FallbackDemonstration {
  primaryFailed: boolean;
  fallbackUsed: boolean;
  totalTime: number;
  steps: Array<{
    endpoint: string;
    success: boolean;
    time: number;
    message: string;
  }>;
}

interface EndpointTestResultProps {
  results: EndpointTestResult[];
  fallbackDemonstration?: FallbackDemonstration;
}

const EndpointTestResult: React.FC<EndpointTestResultProps> = ({
  results,
  fallbackDemonstration,
}) => {
  if (results.length === 0 && !fallbackDemonstration) {
    return null;
  }

  const getStatusIcon = (success: boolean) => {
    return success ? (
      <CheckCircleOutlined style={{ color: '#52c41a', fontSize: 20 }} />
    ) : (
      <CloseCircleOutlined style={{ color: '#ff4d4f', fontSize: 20 }} />
    );
  };

  const getStatusTag = (success: boolean) => {
    return success ? (
      <Tag color="success">连接成功</Tag>
    ) : (
      <Tag color="error">连接失败</Tag>
    );
  };

  return (
    <Space direction="vertical" style={{ width: '100%' }} size="large">
      {/* 端点测试结果 */}
      {results.length > 0 && (
        <Card title="端点测试结果" size="small">
          <Space direction="vertical" style={{ width: '100%' }} size="middle">
            {results.map((result, index) => (
              <Card
                key={index}
                type="inner"
                size="small"
                title={
                  <Space>
                    {getStatusIcon(result.success)}
                    <span>
                      {result.type === 'primary' ? '主端点' : `备端点 ${index}`}
                    </span>
                  </Space>
                }
                extra={getStatusTag(result.success)}
              >
                <Space direction="vertical" style={{ width: '100%' }}>
                  <div>
                    <strong>端点地址：</strong>
                    <code>{result.endpoint}</code>
                  </div>

                  {result.success && result.responseTime !== undefined && (
                    <Statistic
                      title="响应时间"
                      value={result.responseTime}
                      suffix="ms"
                      prefix={<ThunderboltOutlined />}
                      valueStyle={{ fontSize: 16 }}
                    />
                  )}

                  {!result.success && result.error && (
                    <Alert
                      message="错误信息"
                      description={result.error}
                      type="error"
                      showIcon
                    />
                  )}

                  {result.suggestions && result.suggestions.length > 0 && (
                    <Alert
                      message="建议"
                      description={
                        <ul style={{ marginBottom: 0, paddingLeft: 20 }}>
                          {result.suggestions.map((suggestion, i) => (
                            <li key={i}>{suggestion}</li>
                          ))}
                        </ul>
                      }
                      type="info"
                      showIcon
                    />
                  )}
                </Space>
              </Card>
            ))}
          </Space>
        </Card>
      )}

      {/* 降级演示 */}
      {fallbackDemonstration && (
        <Card title="自动降级演示" size="small">
          <Space direction="vertical" style={{ width: '100%' }} size="middle">
            <Timeline
              items={fallbackDemonstration.steps.map((step, _index) => ({
                color: step.success ? 'green' : 'red',
                dot: step.success ? (
                  <CheckCircleOutlined style={{ fontSize: 16 }} />
                ) : (
                  <CloseCircleOutlined style={{ fontSize: 16 }} />
                ),
                children: (
                  <div>
                    <div style={{ fontWeight: 'bold', marginBottom: 4 }}>
                      {step.message}
                    </div>
                    <div style={{ fontSize: 12, color: '#666' }}>
                      <code>{step.endpoint}</code> - 耗时: {step.time}ms
                    </div>
                  </div>
                ),
              }))}
            />

            <Card type="inner" size="small">
              <Space size="large">
                <Statistic
                  title="总耗时"
                  value={fallbackDemonstration.totalTime}
                  suffix="ms"
                  prefix={<ClockCircleOutlined />}
                />
                <Statistic
                  title="主端点状态"
                  value={fallbackDemonstration.primaryFailed ? '失败' : '成功'}
                  valueStyle={{
                    color: fallbackDemonstration.primaryFailed ? '#ff4d4f' : '#52c41a',
                  }}
                />
                <Statistic
                  title="备端点使用"
                  value={fallbackDemonstration.fallbackUsed ? '是' : '否'}
                  valueStyle={{
                    color: fallbackDemonstration.fallbackUsed ? '#1890ff' : '#999',
                  }}
                />
              </Space>
            </Card>

            {fallbackDemonstration.fallbackUsed && (
              <Alert
                message="自动降级成功"
                description="主端点失败时，系统自动切换到备端点并成功完成请求。这确保了服务的高可用性。"
                type="success"
                showIcon
              />
            )}
          </Space>
        </Card>
      )}
    </Space>
  );
};

export default EndpointTestResult;
