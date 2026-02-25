import React from 'react';
import { Alert, Space, Typography } from 'antd';
import { InfoCircleOutlined, LinkOutlined } from '@ant-design/icons';

const { Text, Link } = Typography;

interface AzureConfigGuideProps {
  visible: boolean;
}

const AzureConfigGuide: React.FC<AzureConfigGuideProps> = ({ visible }) => {
  if (!visible) {
    return null;
  }

  return (
    <Alert
      message="Azure OpenAI 配置说明"
      description={
        <Space direction="vertical" size="small" style={{ width: '100%' }}>
          <Text>
            Azure OpenAI 的部署名称需要通过 <Text strong>"模型名称"</Text> 字段传递。
          </Text>

          <div style={{ marginTop: 8 }}>
            <Text strong>配置示例：</Text>
            <ul style={{ marginTop: 4, marginBottom: 0 }}>
              <li>
                <Text code>API Base URL</Text>: https://your-resource.openai.azure.com
              </li>
              <li>
                <Text code>模型名称</Text>: your-deployment-name
              </li>
              <li>
                <Text code>API 版本</Text>: 2024-02-15-preview（可选）
              </li>
            </ul>
          </div>

          <div style={{ marginTop: 8 }}>
            <Text type="secondary" style={{ fontSize: 12 }}>
              <InfoCircleOutlined /> 注意：Azure OpenAI 使用 <Text code>api-key</Text> 认证头，而非标准的 <Text code>Authorization: Bearer</Text>
            </Text>
          </div>

          <div style={{ marginTop: 8 }}>
            <Link href="https://learn.microsoft.com/zh-cn/azure/ai-services/openai/quickstart" target="_blank">
              <LinkOutlined /> 查看 Azure OpenAI 配置文档
            </Link>
          </div>
        </Space>
      }
      type="info"
      showIcon
      icon={<InfoCircleOutlined />}
      style={{ marginTop: 16, marginBottom: 16 }}
    />
  );
};

export default AzureConfigGuide;
