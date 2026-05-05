import { useState } from 'react';
import { Button, Input, Dropdown, message } from 'antd';
import { DownloadOutlined, DownOutlined, LoadingOutlined } from '@ant-design/icons';
import type { MenuProps } from 'antd';
import { settingsApi } from '../services/modularApi';

interface FetchedModel {
  id: string;
  owned_by: string | null;
}

interface ModelInputWithFetchProps {
  value: string;
  onChange: (value: string) => void;
  placeholder?: string;
  apiKey?: string;
  apiBaseUrl?: string;
  provider?: string;
  disabled?: boolean;
}

export default function ModelInputWithFetch({
  value,
  onChange,
  placeholder = '请输入模型名称',
  apiKey,
  apiBaseUrl,
  provider = 'openai',
  disabled = false,
}: ModelInputWithFetchProps) {
  const [fetchedModels, setFetchedModels] = useState<FetchedModel[]>([]);
  const [isLoading, setIsLoading] = useState(false);

  const handleFetchModels = async () => {
    if (!apiKey || !apiBaseUrl) {
      message.warning('请先填写 API Key 和 Base URL');
      return;
    }

    setIsLoading(true);
    try {
      const response = await settingsApi.fetchModels({
        api_key: apiKey,
        api_base_url: apiBaseUrl,
        provider: provider,
      });

      if (response.success && response.models && response.models.length > 0) {
        setFetchedModels(response.models);
        message.success(response.message || `成功获取 ${response.models.length} 个模型`);
      } else {
        message.error(response.message || response.error || '获取模型列表失败');
      }
    } catch (error) {
      console.error('获取模型列表失败:', error);
      message.error('获取模型列表失败，请检查网络连接');
    } finally {
      setIsLoading(false);
    }
  };

  // 按 owned_by 分组
  const groupedModels: Record<string, FetchedModel[]> = {};
  for (const model of fetchedModels) {
    const vendor = model.owned_by || 'Other';
    if (!groupedModels[vendor]) {
      groupedModels[vendor] = [];
    }
    groupedModels[vendor].push(model);
  }

  const vendors = Object.keys(groupedModels).sort();

  // 构建下拉菜单项
  const menuItems: MenuProps['items'] = vendors.flatMap((vendor, vendorIndex) => {
    const items: MenuProps['items'] = [];

    // 添加分组标签
    if (vendorIndex > 0) {
      items.push({ type: 'divider' });
    }

    items.push({
      key: `vendor-${vendor}`,
      label: vendor,
      disabled: true,
      style: { fontWeight: 'bold', color: 'rgba(0, 0, 0, 0.45)' },
    });

    // 添加该分组下的模型
    groupedModels[vendor].forEach((model) => {
      items.push({
        key: model.id,
        label: model.id,
        onClick: () => onChange(model.id),
      });
    });

    return items;
  });

  return (
    <Input.Group compact style={{ display: 'flex' }}>
      <Input
        value={value}
        onChange={(e) => onChange(e.target.value)}
        placeholder={placeholder}
        disabled={disabled}
        style={{ flex: 1 }}
      />
      {fetchedModels.length > 0 ? (
        <Dropdown menu={{ items: menuItems }} trigger={['click']}>
          <Button icon={<DownOutlined />} disabled={disabled}>
            选择
          </Button>
        </Dropdown>
      ) : (
        <Button
          icon={isLoading ? <LoadingOutlined /> : <DownloadOutlined />}
          onClick={handleFetchModels}
          loading={isLoading}
          disabled={disabled || isLoading}
          title="从 API 提供商获取可用模型列表"
        >
          获取
        </Button>
      )}
    </Input.Group>
  );
}
