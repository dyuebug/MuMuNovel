import { useState, useEffect, useRef, useCallback } from 'react';
import {
  Card,
  Button,
  Space,
  Typography,
  Modal,
  Form,
  Input,
  Switch,
  Select,
  message,
  Tag,
  Empty,
  Alert,
  Row,
  Col,
  theme,
} from 'antd';
import {
  PlusOutlined,
  EditOutlined,
  DeleteOutlined,
  CheckCircleOutlined,
  CloseCircleOutlined,
  ThunderboltOutlined,
  InfoCircleOutlined,
  ToolOutlined,
  ApiOutlined,
  QuestionCircleOutlined,
  WarningOutlined,
} from '@ant-design/icons';
import { mcpPluginApi, settingsApi } from '../services/modularApi';
import type { MCPPlugin, MCPTool } from '../types';
import { designDisplayFont } from '../theme/themeConfig';
import InlineDeferredPanel from '../components/InlineDeferredPanel';

const { Paragraph, Text, Title } = Typography;
const { TextArea } = Input;
type ModelSupportStatus = 'unknown' | 'supported' | 'unsupported';

const resolveFunctionCallingStatus = (result: { success: boolean; supported?: boolean | null }): ModelSupportStatus => {
  if (!result.success || result.supported === null || result.supported === undefined) {
    return 'unknown';
  }

  return result.supported ? 'supported' : 'unsupported';
};

type EndpointDiagnostics = {
  primary_endpoint?: string;
  backup_endpoints?: string[];
  configured_endpoint_count?: number;
  fallback_strategy?: string;
  auto_failover_enabled?: boolean;
};

type TransportDiagnostics = {
  summary?: {
    total_attempts?: number;
    successful_attempts?: number;
    api_modes_tried?: string[];
    backup_endpoint_used?: boolean;
    api_mode_fallback_used?: boolean;
    forced_chat_completions?: boolean;
    normalized_base_url_used?: boolean;
  };
  attempts?: Array<{
    api_mode?: string;
    endpoint_role?: string;
    base_url?: string;
    endpoint_path?: string;
    attempt_number?: number;
    max_attempts?: number;
    result?: string;
    status_code?: number;
    error_type?: string;
    will_retry_same_endpoint?: boolean;
    will_failover?: boolean;
  }>;
};

const UNSUPPORTED_STATUS_CACHE_TTL_MS = 2 * 60 * 60 * 1000;

const isUnsupportedStatusCacheExpired = (verifiedConfig: { status?: string; testedAt?: string | null }) => {
  if (verifiedConfig.status !== 'unsupported') {
    return false;
  }

  const testedAt = typeof verifiedConfig.testedAt === 'string' ? verifiedConfig.testedAt.trim() : '';
  if (!testedAt) {
    return true;
  }

  const testedAtMs = Date.parse(testedAt);
  if (!Number.isFinite(testedAtMs)) {
    return true;
  }

  return Date.now() - testedAtMs >= UNSUPPORTED_STATUS_CACHE_TTL_MS;
};

const normalizeBackupUrls = (urls: unknown): string[] => {
  if (!Array.isArray(urls)) {
    return [];
  }

  return urls
    .map((url) => (typeof url === 'string' ? url.trim() : ''))
    .filter(Boolean);
};

const buildVerifiedConfig = (settings: {
  api_provider?: string | null;
  provider_type?: string | null;
  api_base_url?: string | null;
  llm_model?: string | null;
  api_backup_urls?: string[] | null;
  fallback_strategy?: string | null;
}) => ({
  provider: settings.provider_type || settings.api_provider || '',
  baseUrl: settings.api_base_url || '',
  model: settings.llm_model || '',
  backupUrls: normalizeBackupUrls(settings.api_backup_urls),
  fallbackStrategy: settings.fallback_strategy || 'auto',
});

const renderEndpointDiagnostics = (details: Record<string, unknown> | undefined, colorBgLayout: string) => {
  const diagnostics = details?.endpoint_diagnostics as EndpointDiagnostics | undefined;
  if (!diagnostics) {
    return null;
  }

  const backupEndpoints = normalizeBackupUrls(diagnostics.backup_endpoints);

  return (
    <div style={{ marginBottom: 12, padding: 12, background: colorBgLayout, borderRadius: 8 }}>
      <Text type="secondary" style={{ fontSize: 12, display: "block", marginBottom: 6 }}>端点诊断</Text>
      <div style={{ fontSize: 12, lineHeight: 1.8 }}>
        <div>主端点：<Text code>{diagnostics.primary_endpoint || '未设置'}</Text></div>
        <div>备用端点数：<Text strong>{backupEndpoints.length}</Text></div>
        <div>回退策略：<Text strong>{diagnostics.fallback_strategy || 'auto'}</Text></div>
        <div>自动故障切换：<Text strong>{diagnostics.auto_failover_enabled ? '已启用' : '已禁用'}</Text></div>
      </div>
      {backupEndpoints.length > 0 && (
        <div style={{ marginTop: 8 }}>
          <Text type="secondary" style={{ fontSize: 12, display: "block", marginBottom: 4 }}>备用端点列表</Text>
          <Space direction="vertical" size={4} style={{ width: '100%' }}>
            {backupEndpoints.map((url, index) => (
              <Text code key={`${url}-${index}`} style={{ width: '100%', whiteSpace: 'pre-wrap', wordBreak: 'break-all' }}>
                {url}
              </Text>
            ))}
          </Space>
        </div>
      )}
    </div>
  );
};

const renderTransportDiagnostics = (details: Record<string, unknown> | undefined, colorBgLayout: string) => {
  const diagnostics = details?.transport_diagnostics as TransportDiagnostics | undefined;
  if (!diagnostics) {
    return null;
  }

  const summary = diagnostics.summary ?? {};
  const attempts = Array.isArray(diagnostics.attempts) ? diagnostics.attempts.slice(-3) : [];
  const apiModes = Array.isArray(summary.api_modes_tried) ? summary.api_modes_tried : [];

  return (
    <div style={{ marginBottom: 12, padding: 12, background: colorBgLayout, borderRadius: 8 }}>
      <Text type="secondary" style={{ fontSize: 12, display: "block", marginBottom: 6 }}>传输诊断</Text>
      <div style={{ fontSize: 12, lineHeight: 1.8 }}>
        <div>总尝试次数：<Text strong>{summary.total_attempts ?? 0}</Text></div>
        <div>成功次数：<Text strong>{summary.successful_attempts ?? 0}</Text></div>
        <div>尝试过的 API 模式：<Text strong>{apiModes.length > 0 ? apiModes.join(' -> ') : '未知'}</Text></div>
        <div>是否使用备用端点：<Text strong>{summary.backup_endpoint_used ? '是' : '否'}</Text></div>
        <div>是否触发 API 模式回退：<Text strong>{summary.api_mode_fallback_used ? '是' : '否'}</Text></div>
        <div>是否强制使用 Chat Completions：<Text strong>{summary.forced_chat_completions ? '是' : '否'}</Text></div>
        <div>是否使用规范化 Base URL：<Text strong>{summary.normalized_base_url_used ? '是' : '否'}</Text></div>
      </div>
      {attempts.length > 0 && (
        <div style={{ marginTop: 8 }}>
          <Text type="secondary" style={{ fontSize: 12, display: "block", marginBottom: 4 }}>最近尝试</Text>
          <Space direction="vertical" size={4} style={{ width: '100%' }}>
            {attempts.map((attempt, index) => (
              <div key={`${attempt.base_url || 'attempt'}-${attempt.attempt_number || index}-${index}`} style={{ fontSize: 12, lineHeight: 1.6 }}>
                <Text code>{attempt.api_mode || '未知'}</Text>
                {' / '}
                <Text strong>{attempt.result || '未知'}</Text>
                {' / '}
                <Text>{attempt.endpoint_role === 'backup' ? '备用端点' : '主端点'}</Text>
                {' / '}
                <Text>{attempt.attempt_number || 1}/{attempt.max_attempts || 1}</Text>
                {attempt.status_code ? <> {' / '}<Text>HTTP {attempt.status_code}</Text></> : null}
                {attempt.error_type ? <> {' / '}<Text type="danger">{attempt.error_type}</Text></> : null}
                <div style={{ wordBreak: 'break-all' }}><Text code>{`${attempt.base_url || ''}${attempt.endpoint_path || ''}` || '未知端点'}</Text></div>
              </div>
            ))}
          </Space>
        </div>
      )}
    </div>
  );
};

export default function MCPPluginsPage() {
  const [isMobile, setIsMobile] = useState(window.innerWidth <= 768);
  const [form] = Form.useForm();
  const { token } = theme.useToken();
  const alphaColor = (color: string, alpha: number) => `color-mix(in srgb, ${color} ${(alpha * 100).toFixed(0)}%, transparent)`;
  const editorialInk = '#f7f1e8';
  const pageBackground = `linear-gradient(180deg, ${alphaColor(token.colorPrimary, 0.06)} 0%, ${token.colorBgLayout} 30%, ${token.colorBgLayout} 100%)`;
  const heroBackground = `linear-gradient(135deg, #171411 0%, color-mix(in srgb, #171411 60%, ${token.colorPrimary} 40%) 100%)`;
  const quietPanelBackground = `linear-gradient(180deg, color-mix(in srgb, ${token.colorBgContainer} 94%, ${token.colorFillAlter} 6%) 0%, color-mix(in srgb, ${token.colorBgContainer} 86%, ${token.colorFillAlter} 14%) 100%)`;
  const panelBorder = alphaColor(token.colorPrimary, 0.12);

  const statusStyles = {
    success: {
      bg: token.colorSuccessBg,
      border: token.colorSuccessBorder,
      text: token.colorSuccessText,
    },
    info: {
      bg: token.colorInfoBg,
      border: token.colorInfoBorder,
      text: token.colorInfoText,
    },
    warning: {
      bg: token.colorWarningBg,
      border: token.colorWarningBorder,
      text: token.colorWarningText,
    },
    error: {
      bg: token.colorErrorBg,
      border: token.colorErrorBorder,
      text: token.colorErrorText,
    },
  };

  // 响应式监听窗口大小变化
  useEffect(() => {
    const handleResize = () => {
      setIsMobile(window.innerWidth <= 768);
    };
    window.addEventListener('resize', handleResize);
    return () => window.removeEventListener('resize', handleResize);
  }, []);
  const [modal, contextHolder] = Modal.useModal();
  const [loading, setLoading] = useState(false);
  const [plugins, setPlugins] = useState<MCPPlugin[]>([]);
  const [modalVisible, setModalVisible] = useState(false);
  const [editingPlugin, setEditingPlugin] = useState<MCPPlugin | null>(null);
  const [testingPluginId, setTestingPluginId] = useState<string | null>(null);
  const [viewingTools, setViewingTools] = useState<{ pluginId: string; tools: MCPTool[] } | null>(null);
  const [checkingFunctionCalling, setCheckingFunctionCalling] = useState(false);
  const [modelSupportStatus, setModelSupportStatus] = useState<ModelSupportStatus>('unknown');
  const mountedRef = useRef(true);
  const initRequestIdRef = useRef(0);
  const pluginListRequestIdRef = useRef(0);
  const testPluginRequestIdRef = useRef(0);
  const toolsRequestIdRef = useRef(0);
  const functionCallingRequestIdRef = useRef(0);
  const submitRequestIdRef = useRef(0);

  useEffect(() => {
    mountedRef.current = true;
    return () => {
      mountedRef.current = false;
      initRequestIdRef.current += 1;
      pluginListRequestIdRef.current += 1;
      testPluginRequestIdRef.current += 1;
      toolsRequestIdRef.current += 1;
      functionCallingRequestIdRef.current += 1;
      submitRequestIdRef.current += 1;
    };
  }, []);

  const beginTrackedRequest = useCallback((ref: React.MutableRefObject<number>) => {
    ref.current += 1;
    return ref.current;
  }, []);

  const isTrackedRequestActive = useCallback((ref: React.MutableRefObject<number>, requestId: number) => {
    return mountedRef.current && ref.current === requestId;
  }, []);

  useEffect(() => {
    const initPage = async () => {
      const requestId = beginTrackedRequest(initRequestIdRef);
      setLoading(true);
      try {
        // 1. 并行获取插件列表和当前设置
        const [pluginsData, settings] = await Promise.all([
          mcpPluginApi.getPlugins(),
          settingsApi.getSettings()
        ]);
        if (!isTrackedRequestActive(initRequestIdRef, requestId)) {
          return;
        }
        
        setPlugins(pluginsData);

        // 2. 检查配置一致性
        const verifiedConfigStr = localStorage.getItem('mcp_verified_config');
        if (verifiedConfigStr) {
          try {
            const verifiedConfig = JSON.parse(verifiedConfigStr);
            const currentConfig = buildVerifiedConfig(settings);
            const cachedConfig = buildVerifiedConfig({
              api_provider: verifiedConfig.provider,
              api_base_url: verifiedConfig.baseUrl,
              llm_model: verifiedConfig.model,
              api_backup_urls: verifiedConfig.backupUrls,
              fallback_strategy: verifiedConfig.fallbackStrategy,
            });

            // 深度比较缓存配置与当前配置是否一致
            const isConfigChanged = JSON.stringify(cachedConfig) !== JSON.stringify(currentConfig);

            if (isConfigChanged) {
              if (!isTrackedRequestActive(initRequestIdRef, requestId)) {
                return;
              }
              // 配置已变更
              setModelSupportStatus('unknown');
              
              // 检查是否有正在运行的插件
              const activePlugins = pluginsData.filter(p => p.enabled);
              if (activePlugins.length > 0) {
                // 自动禁用所有插件
                message.loading({ content: '检测到模型配置变更，正在为了安全自动禁用插件...', key: 'auto_disable' });
                
                await Promise.all(activePlugins.map(p => mcpPluginApi.togglePlugin(p.id, false)));
                if (!isTrackedRequestActive(initRequestIdRef, requestId)) {
                  return;
                }
                
                // 重新加载插件列表状态
                const updatedPlugins = await mcpPluginApi.getPlugins();
                if (!isTrackedRequestActive(initRequestIdRef, requestId)) {
                  return;
                }
                setPlugins(updatedPlugins);
                
                message.success({ content: '已自动禁用所有插件，请重新检测模型能力', key: 'auto_disable' });
                
                modal.warning({
                  title: '配置变更提醒',
                  centered: true,
                  content: '检测到您更换了 AI 模型或接口地址。为了防止错误调用，系统已自动暂停所有 MCP 插件。请重新进行"模型能力检查"，确认新模型支持 Function Calling 后再启用插件。',
                  okText: '知道了',
                });
              } else {
                // 没有运行中的插件，仅提示
                message.info('检测到模型配置已变更，请重新检测模型能力');
              }
              
              // 清除旧的验证状态
              localStorage.removeItem('mcp_verified_config');
            } else {
              // 旧的 unsupported 检测缓存只保留短期有效，过期后提示重新检测
              if (isUnsupportedStatusCacheExpired(verifiedConfig)) {
                localStorage.removeItem('mcp_verified_config');
                setModelSupportStatus('unknown');
                message.info('模型能力检测缓存已过期，请重新检测');
              } else {
                const cachedStatus = verifiedConfig.status || 'supported';
                setModelSupportStatus(cachedStatus as ModelSupportStatus);
              }
            }
          } catch (e) {
            console.error('Failed to parse verified config:', e);
            localStorage.removeItem('mcp_verified_config');
          }
        }
      } catch (error) {
        if (!isTrackedRequestActive(initRequestIdRef, requestId)) {
          return;
        }
        console.error('Init page failed:', error);
        message.error('页面初始化失败');
      } finally {
        if (isTrackedRequestActive(initRequestIdRef, requestId)) {
          setLoading(false);
        }
      }
    };
    initPage();
  }, [beginTrackedRequest, isTrackedRequestActive, modal]);

  const loadPlugins = useCallback(async () => {
    const requestId = beginTrackedRequest(pluginListRequestIdRef);
    try {
      const data = await mcpPluginApi.getPlugins();
      if (!isTrackedRequestActive(pluginListRequestIdRef, requestId)) {
        return null;
      }
      setPlugins(data);
      return data;
    } catch (error) {
      if (!isTrackedRequestActive(pluginListRequestIdRef, requestId)) {
        return null;
      }
      console.error('Load plugins failed:', error);
      message.error('加载插件列表失败');
      return null;
    }
  }, [beginTrackedRequest, isTrackedRequestActive]);

  const handleCreate = () => {
    if (modelSupportStatus !== 'supported') {
      modal.confirm({
        title: '模型能力检查',
        centered: true,
        icon: <WarningOutlined />,
        content: '为了确保 MCP 插件正常工作，您当前使用的 AI 模型必须支持 Function Calling（工具调用）能力。请先进行模型支持检测。',
        okText: '去检测',
        cancelText: '取消',
        onOk: handleCheckFunctionCalling,
      });
      return;
    }
    setEditingPlugin(null);
    form.resetFields();
    form.setFieldsValue({
      enabled: true,
      category: 'search',
      config_json: `{
  "mcpServers": {
    "exa": {
      "type": "http",
      "url": "https://mcp.exa.ai/mcp?exaApiKey=YOUR_API_KEY",
      "headers": {}
    }
  }
}`
    });
    setModalVisible(true);
  };

  const handleEdit = (plugin: MCPPlugin) => {
    setEditingPlugin(plugin);

    // 重构为标准MCP配置格式
    const mcpConfig: Record<string, Record<string, Record<string, unknown>>> = {
      mcpServers: {
        [plugin.plugin_name]: {
          type: plugin.plugin_type || 'http'
        }
      }
    };

    if (plugin.plugin_type === 'http' || plugin.plugin_type === 'streamable_http' || plugin.plugin_type === 'sse') {
      mcpConfig.mcpServers[plugin.plugin_name].url = plugin.server_url;
      mcpConfig.mcpServers[plugin.plugin_name].headers = plugin.headers || {};
    } else {
      mcpConfig.mcpServers[plugin.plugin_name].command = plugin.command;
      mcpConfig.mcpServers[plugin.plugin_name].args = plugin.args || [];
      mcpConfig.mcpServers[plugin.plugin_name].env = plugin.env || {};
    }

    form.setFieldsValue({
      config_json: JSON.stringify(mcpConfig, null, 2),
      enabled: plugin.enabled,
      category: plugin.category || 'general',
    });
    setModalVisible(true);
  };

  const handleDelete = (plugin: MCPPlugin) => {
    modal.confirm({
      title: '删除插件',
      content: `确定要删除插件 "${plugin.display_name || plugin.plugin_name}" 吗？`,
      centered: true,
      okText: '确定',
      cancelText: '取消',
      okType: 'danger',
      onOk: async () => {
        try {
          await mcpPluginApi.deletePlugin(plugin.id);
          message.success('插件已删除');
          await loadPlugins();
        } catch (error) {
          console.error('Delete plugin failed:', error);
          message.error('删除插件失败');
        }
      },
    });
  };

  const handleToggle = async (plugin: MCPPlugin, enabled: boolean) => {
    try {
      await mcpPluginApi.togglePlugin(plugin.id, enabled);
      message.success(enabled ? '插件已启用' : '插件已禁用');
      await loadPlugins();
    } catch (error) {
      console.error('Toggle plugin failed:', error);
      message.error('切换插件状态失败');
    }
  };

  const handleTest = async (pluginId: string) => {
    const requestId = beginTrackedRequest(testPluginRequestIdRef);
    setTestingPluginId(pluginId);
    try {
      const result = await mcpPluginApi.testPlugin(pluginId);
      if (!isTrackedRequestActive(testPluginRequestIdRef, requestId)) {
        return;
      }

      // 测试完成后，无论成功失败都刷新插件列表以更新状态
      await loadPlugins();
      if (!isTrackedRequestActive(testPluginRequestIdRef, requestId)) {
        return;
      }

      if (result.success) {
        const suggestions = result.suggestions || [];
        const aiChoice = suggestions.find((s: string) => s.startsWith('🤖'))?.replace('🤖 AI选择: ', '') || '';
        const paramsStr = suggestions.find((s: string) => s.startsWith('📝'))?.replace('📝 参数: ', '') || '';
        const callTime = suggestions.find((s: string) => s.startsWith('⏱️'))?.replace('⏱️ 耗时: ', '') || '';
        const resultStr = suggestions.find((s: string) => s.startsWith('📊'))?.replace('📊 结果:\n', '') || '';

        modal.success({
          title: '🎉 测试成功',
          centered: true,
          width: isMobile ? '95%' : 700,
          content: (
            <div style={{ padding: '8px 0' }}>
              <div style={{ marginBottom: 16, padding: 12, background: statusStyles.success.bg, border: `1px solid ${statusStyles.success.border}`, borderRadius: 8 }}>
                <Typography.Text strong style={{ color: statusStyles.success.text, fontSize: 14 }}>
                  ✓ {result.message}
                </Typography.Text>
              </div>

              <div style={{ display: 'grid', gridTemplateColumns: isMobile ? '1fr' : '1fr 1fr', gap: 12, marginBottom: 16 }}>
                <div style={{ padding: 12, background: token.colorBgLayout, borderRadius: 8 }}>
                  <Text type="secondary" style={{ fontSize: 12 }}>可用工具数</Text>
                  <div><Text strong style={{ fontSize: 20 }}>{result.tools_count || 0}</Text></div>
                </div>
                <div style={{ padding: 12, background: token.colorBgLayout, borderRadius: 8 }}>
                  <Text type="secondary" style={{ fontSize: 12 }}>总响应时间</Text>
                  <div><Text strong style={{ fontSize: 20 }}>{result.response_time_ms?.toFixed(0) || 0}ms</Text></div>
                </div>
              </div>

              {aiChoice && (
                <div style={{ marginBottom: 12, padding: 12, background: statusStyles.info.bg, borderRadius: 8, border: `1px solid ${statusStyles.info.border}` }}>
                  <Text type="secondary" style={{ fontSize: 12, display: 'block', marginBottom: 4 }}>🤖 AI选择的工具</Text>
                  <Text code strong>{aiChoice}</Text>
                  {callTime && <Tag color="blue" style={{ marginLeft: 8 }}>{callTime}</Tag>}
                </div>
              )}

              {paramsStr && (
                <div style={{ marginBottom: 12 }}>
                  <Text type="secondary" style={{ fontSize: 12, display: 'block', marginBottom: 4 }}>📝 调用参数</Text>
                  <pre style={{ margin: 0, padding: 8, background: token.colorBgLayout, borderRadius: 4, fontSize: 12, overflow: 'auto', maxHeight: 100 }}>
                    {(() => { try { return JSON.stringify(JSON.parse(paramsStr), null, 2); } catch { return paramsStr; } })()}
                  </pre>
                </div>
              )}

              {resultStr && (
                <div style={{ marginBottom: 12 }}>
                  <Text type="secondary" style={{ fontSize: 12, display: 'block', marginBottom: 4 }}>📊 返回结果预览</Text>
                  <pre style={{ margin: 0, padding: 8, background: token.colorBgLayout, borderRadius: 4, fontSize: 11, overflow: 'auto', maxHeight: 150, whiteSpace: 'pre-wrap', wordBreak: 'break-word' }}>
                    {resultStr}
                  </pre>
                </div>
              )}

              <Alert message='插件状态已自动更新为"运行中"' type="success" showIcon />
            </div>
          ),
        });
      } else {
        modal.error({
          title: '测试失败',
          centered: true,
          width: isMobile ? '90%' : 600,
          content: (
            <div style={{ padding: '8px 0' }}>
              <div style={{ marginBottom: 16 }}>
                <Alert
                  message={result.message || 'MCP插件测试失败'}
                  type="error"
                  showIcon
                />
              </div>

              {result.error && (
                <div style={{
                  padding: 16,
                  background: statusStyles.error.bg,
                  border: `1px solid ${statusStyles.error.border}`,
                  borderRadius: 8,
                  marginBottom: 16
                }}>
                  <Text strong style={{ fontSize: 14, display: 'block', marginBottom: 8 }}>错误信息:</Text>
                  <Text style={{ fontSize: 13, color: statusStyles.error.text, fontFamily: 'monospace', whiteSpace: 'pre-wrap', wordBreak: 'break-word' }}>
                    {result.error}
                  </Text>
                </div>
              )}

              {result.suggestions && result.suggestions.length > 0 && (
                <div style={{
                  padding: 16,
                  background: statusStyles.warning.bg,
                  border: `1px solid ${statusStyles.warning.border}`,
                  borderRadius: 8,
                  marginBottom: 16
                }}>
                  <Text strong style={{ fontSize: 14, display: 'block', marginBottom: 8 }}>💡 建议:</Text>
                  <ul style={{ margin: 0, paddingLeft: 20, fontSize: 13 }}>
                    {result.suggestions.map((s: string, i: number) => (
                      <li key={i} style={{ marginBottom: 4 }}>{s}</li>
                    ))}
                  </ul>
                </div>
              )}

              <Alert
                message="插件状态已更新，请检查配置后重试"
                type="warning"
                showIcon
              />
            </div>
          ),
        });
      }
    } catch {
      if (!isTrackedRequestActive(testPluginRequestIdRef, requestId)) {
        return;
      }
      message.error('测试插件失败');
    } finally {
      if (isTrackedRequestActive(testPluginRequestIdRef, requestId)) {
        setTestingPluginId(null);
      }
    }
  };

  const handleViewTools = async (pluginId: string) => {
    const requestId = beginTrackedRequest(toolsRequestIdRef);
    try {
      const result = await mcpPluginApi.getPluginTools(pluginId);
      if (!isTrackedRequestActive(toolsRequestIdRef, requestId)) {
        return;
      }
      setViewingTools({ pluginId, tools: result.tools });
    } catch (error) {
      if (!isTrackedRequestActive(toolsRequestIdRef, requestId)) {
        return;
      }
      console.error('Get tools failed:', error);
      message.error('获取工具列表失败');
    }
  };

  const handleCheckFunctionCalling = async () => {
    // 从设置中获取当前配置
    const requestId = beginTrackedRequest(functionCallingRequestIdRef);
    setCheckingFunctionCalling(true);
    try {
      const { settings, storedApiKey } = await settingsApi.getSettingsWithStoredApiKey();
      if (!isTrackedRequestActive(functionCallingRequestIdRef, requestId)) {
        return;
      }
      
      if (!storedApiKey || !settings.llm_model) {
        message.warning('请先在设置页面配置 API Key 和模型');
        return;
      }

      const result = await settingsApi.checkFunctionCalling({
        api_key: storedApiKey,
        api_base_url: settings.api_base_url || '',
        provider: settings.provider_type || settings.api_provider || 'openai',
        llm_model: settings.llm_model,
        api_backup_urls: normalizeBackupUrls(settings.api_backup_urls),
        fallback_strategy: settings.fallback_strategy || 'auto',
      });
      if (!isTrackedRequestActive(functionCallingRequestIdRef, requestId)) {
        return;
      }

      const nextStatus = resolveFunctionCallingStatus(result);

      // 仅在检测结果明确时写入缓存，避免 502/网络异常覆盖已有的有效验证结果
      if (nextStatus !== 'unknown') {
        const configToCache = {
          ...buildVerifiedConfig(settings),
          status: nextStatus,
          testedAt: new Date().toISOString(),
        };
        localStorage.setItem('mcp_verified_config', JSON.stringify(configToCache));
      }

      if (nextStatus === 'supported') {
        setModelSupportStatus('supported');

        const functionCallingDetails = (result.details ?? {}) as {
          finish_reason?: string;
          tool_call_count?: number;
          test_tool?: string;
          response_type?: string;
        };

        modal.success({
          title: '✅ Function Calling 支持检测',
          centered: true,
          width: isMobile ? '95%' : 700,
          content: (
            <div style={{ padding: '8px 0' }}>
              <div style={{ marginBottom: 16, padding: 12, background: statusStyles.success.bg, border: `1px solid ${statusStyles.success.border}`, borderRadius: 8 }}>
                <Typography.Text strong style={{ color: statusStyles.success.text, fontSize: 14 }}>
                  ✓ {result.message}
                </Typography.Text>
              </div>

              <div style={{ display: 'grid', gridTemplateColumns: isMobile ? '1fr' : '1fr 1fr', gap: 12, marginBottom: 16 }}>
                <div style={{ padding: 12, background: token.colorBgLayout, borderRadius: 8 }}>
                  <Text type="secondary" style={{ fontSize: 12 }}>API 提供商</Text>
                  <div><Text strong style={{ fontSize: 16 }}>{result.provider}</Text></div>
                </div>
                <div style={{ padding: 12, background: token.colorBgLayout, borderRadius: 8 }}>
                  <Text type="secondary" style={{ fontSize: 12 }}>响应时间</Text>
                  <div><Text strong style={{ fontSize: 16 }}>{result.response_time_ms?.toFixed(0) || 0}ms</Text></div>
                </div>
              </div>

              <div style={{ marginBottom: 12, padding: 12, background: statusStyles.info.bg, borderRadius: 8, border: `1px solid ${statusStyles.info.border}` }}>
                <Text type="secondary" style={{ fontSize: 12, display: 'block', marginBottom: 4 }}>🔧 模型信息</Text>
                <Text code strong>{result.model}</Text>
                {functionCallingDetails.finish_reason && (
                  <Tag color="green" style={{ marginLeft: 8 }}>finish_reason: {functionCallingDetails.finish_reason}</Tag>
                )}
              </div>

              {result.details && (
                <div style={{ marginBottom: 12 }}>
                  <Text type="secondary" style={{ fontSize: 12, display: 'block', marginBottom: 4 }}>📊 检测详情</Text>
                  <div style={{ padding: 8, background: token.colorBgLayout, borderRadius: 4, fontSize: 12 }}>
                    <div>✓ 工具调用数量: {functionCallingDetails.tool_call_count || 0}</div>
                    <div>✓ 测试工具: {functionCallingDetails.test_tool || 'N/A'}</div>
                    <div>✓ 响应类型: {functionCallingDetails.response_type || 'N/A'}</div>
                  </div>
                </div>
              )}

              {renderEndpointDiagnostics(result.details, token.colorBgLayout)}
                {renderTransportDiagnostics(result.details, token.colorBgLayout)}

              {result.tool_calls && result.tool_calls.length > 0 && (
                <div style={{ marginBottom: 12 }}>
                  <Text type="secondary" style={{ fontSize: 12, display: 'block', marginBottom: 4 }}>🔨 工具调用示例</Text>
                  <pre style={{ margin: 0, padding: 8, background: token.colorBgLayout, borderRadius: 4, fontSize: 11, overflow: 'auto', maxHeight: 150 }}>
                    {JSON.stringify(result.tool_calls[0], null, 2)}
                  </pre>
                </div>
              )}

              {result.suggestions && result.suggestions.length > 0 && (
                <div style={{ padding: 12, background: statusStyles.success.bg, border: `1px solid ${statusStyles.success.border}`, borderRadius: 8 }}>
                  <Text strong style={{ fontSize: 13, display: 'block', marginBottom: 8 }}>💡 建议</Text>
                  <ul style={{ margin: 0, paddingLeft: 20, fontSize: 12 }}>
                    {result.suggestions.map((s: string, i: number) => (
                      <li key={i} style={{ marginBottom: 4 }}>{s}</li>
                    ))}
                  </ul>
                </div>
              )}
            </div>
          ),
        });
      } else if (nextStatus === 'unsupported') {
        setModelSupportStatus('unsupported');
        modal.warning({
          title: '❌ Function Calling 支持检测',
          centered: true,
          width: isMobile ? '95%' : 700,
          content: (
            <div style={{ padding: '8px 0' }}>
              <div style={{ marginBottom: 16 }}>
                <Alert
                  message={result.message || '模型不支持 Function Calling'}
                  type="warning"
                  showIcon
                />
              </div>

              {result.error && (
                <div style={{
                  padding: 16,
                  background: statusStyles.warning.bg,
                  border: `1px solid ${statusStyles.warning.border}`,
                  borderRadius: 8,
                  marginBottom: 16
                }}>
                  <Text strong style={{ fontSize: 14, display: 'block', marginBottom: 8 }}>错误信息:</Text>
                  <Text style={{ fontSize: 13, fontFamily: 'monospace' }}>
                    {result.error}
                  </Text>
                </div>
              )}

              {result.response_preview && (
                <div style={{ marginBottom: 12 }}>
                  <Text type="secondary" style={{ fontSize: 12, display: 'block', marginBottom: 4 }}>📝 模型返回内容（前200字符）</Text>
                  <pre style={{ margin: 0, padding: 8, background: token.colorBgLayout, borderRadius: 4, fontSize: 11, overflow: 'auto', maxHeight: 100, whiteSpace: 'pre-wrap' }}>
                    {result.response_preview}
                  </pre>
                </div>
              )}

              {renderEndpointDiagnostics(result.details, token.colorBgLayout)}
                {renderTransportDiagnostics(result.details, token.colorBgLayout)}

              {result.suggestions && result.suggestions.length > 0 && (
                <div style={{
                  padding: 16,
                  background: statusStyles.info.bg,
                  border: `1px solid ${statusStyles.info.border}`,
                  borderRadius: 8
                }}>
                  <Text strong style={{ fontSize: 14, display: 'block', marginBottom: 8 }}>💡 建议:</Text>
                  <ul style={{ margin: 0, paddingLeft: 20, fontSize: 13 }}>
                    {result.suggestions.map((s: string, i: number) => (
                      <li key={i} style={{ marginBottom: 4 }}>{s}</li>
                    ))}
                  </ul>
                </div>
              )}
            </div>
          ),
        });
      } else {
        setModelSupportStatus('unknown');
        modal.warning({
          title: '⚠️ Function Calling 检测未完成',
          centered: true,
          width: isMobile ? '95%' : 700,
          content: (
            <div style={{ padding: '8px 0' }}>
              <div style={{ marginBottom: 16 }}>
                <Alert
                  message={result.message || '本次检测未能确认当前模型是否支持 Function Calling'}
                  type="warning"
                  showIcon
                />
              </div>

              {result.error && (
                <div style={{
                  padding: 16,
                  background: statusStyles.warning.bg,
                  border: `1px solid ${statusStyles.warning.border}`,
                  borderRadius: 8,
                  marginBottom: 16
                }}>
                  <Text strong style={{ fontSize: 14, display: 'block', marginBottom: 8 }}>错误信息:</Text>
                  <Text style={{ fontSize: 13, fontFamily: 'monospace' }}>
                    {result.error}
                  </Text>
                </div>
              )}

              {renderEndpointDiagnostics(result.details, token.colorBgLayout)}
                {renderTransportDiagnostics(result.details, token.colorBgLayout)}

              {result.suggestions && result.suggestions.length > 0 && (
                <div style={{
                  padding: 16,
                  background: statusStyles.info.bg,
                  border: `1px solid ${statusStyles.info.border}`,
                  borderRadius: 8
                }}>
                  <Text strong style={{ fontSize: 14, display: 'block', marginBottom: 8 }}>💡 建议:</Text>
                  <ul style={{ margin: 0, paddingLeft: 20, fontSize: 13 }}>
                    {result.suggestions.map((s: string, i: number) => (
                      <li key={i} style={{ marginBottom: 4 }}>{s}</li>
                    ))}
                  </ul>
                </div>
              )}
            </div>
          ),
        });
      }
    } catch (error) {
      if (!isTrackedRequestActive(functionCallingRequestIdRef, requestId)) {
        return;
      }
      console.error('Check function calling failed:', error);
      message.error('检测失败，请稍后重试');
    } finally {
      if (isTrackedRequestActive(functionCallingRequestIdRef, requestId)) {
        setCheckingFunctionCalling(false);
      }
    }
  };

  const handleSubmit = async (values: { config_json: string; enabled: boolean; category?: string }) => {
    const requestId = beginTrackedRequest(submitRequestIdRef);
    setLoading(true);
    try {
      // 验证JSON格式
      try {
        JSON.parse(values.config_json);
      } catch {
        if (!isTrackedRequestActive(submitRequestIdRef, requestId)) {
          return;
        }
        message.error('配置JSON格式错误，请检查');
        return;
      }

      const data = {
        config_json: values.config_json,
        enabled: values.enabled,
        category: values.category || 'general',
      };

      // 统一使用简化API，后端会自动判断是创建还是更新
      await mcpPluginApi.createPluginSimple(data);
      if (!isTrackedRequestActive(submitRequestIdRef, requestId)) {
        return;
      }
      message.success(editingPlugin ? '插件已更新' : '插件已创建');

      setModalVisible(false);
      form.resetFields();
      await loadPlugins();
    } catch (error: unknown) {
      if (!isTrackedRequestActive(submitRequestIdRef, requestId)) {
        return;
      }
      const err = error as { response?: { data?: { detail?: string } } };
      const errorMsg = err?.response?.data?.detail || '操作失败';
      message.error(errorMsg);
    } finally {
      if (isTrackedRequestActive(submitRequestIdRef, requestId)) {
        setLoading(false);
      }
    }
  };

  const getStatusTag = (plugin: MCPPlugin) => {
    if (!plugin.enabled) {
      return <Tag color="default">已禁用</Tag>;
    }
    switch (plugin.status) {
      case 'active':
        return <Tag color="success" icon={<CheckCircleOutlined />}>运行中</Tag>;
      case 'error':
        return (
          <Tag color="error" icon={<CloseCircleOutlined />} title={plugin.last_error}>错误</Tag>
        );
      default:
        return <Tag color="default">未激活</Tag>;
    }
  };

  const activePluginCount = plugins.filter((plugin) => plugin.enabled).length;
  const runningPluginCount = plugins.filter((plugin) => plugin.status === 'active').length;
  const issuePluginCount = plugins.filter((plugin) => plugin.status === 'error').length;
  const overviewStats = [
    { label: '插件总数', value: `${plugins.length} 个`, accent: token.colorPrimary },
    { label: '已启用', value: `${activePluginCount} 个`, accent: token.colorInfo },
    { label: '运行中', value: `${runningPluginCount} 个`, accent: token.colorSuccess },
    { label: '异常状态', value: `${issuePluginCount} 个`, accent: token.colorError },
  ];
  const renderPluginWorkspaceFallback = () => (
    <InlineDeferredPanel
      eyebrow="Plugin Workspace"
      title="恢复插件目录与运行诊断"
      message="当前正在刷新插件清单、连接状态与模型能力检测结果。先等待目录、诊断入口、工具查看和生命周期管理面板回流，原有插件启停与测试逻辑保持不变。"
      minHeight={isMobile ? 320 : 360}
      tags={[
        {
          label: modelSupportStatus === 'supported'
            ? 'Function Calling 已支持'
            : modelSupportStatus === 'unsupported'
              ? 'Function Calling 待调整'
              : '等待能力检测',
          color: modelSupportStatus === 'supported'
            ? 'success'
            : modelSupportStatus === 'unsupported'
              ? 'error'
              : 'processing',
        },
        { label: '插件目录恢复中', color: 'processing' },
        { label: '工具暴露面校验', color: 'blue' },
      ]}
    />
  );

  return (
    <>
      {contextHolder}
      <div style={{
        minHeight: '90vh',
        background: pageBackground,
        padding: isMobile ? '20px 16px 70px' : '28px 24px 80px',
        display: 'flex',
        flexDirection: 'column',
      }}>
        <div style={{
          maxWidth: 1400,
          margin: '0 auto',
          width: '100%',
          flex: 1,
          display: 'flex',
          flexDirection: 'column',
        }}>
          <Card
            style={{
              background: heroBackground,
              borderRadius: isMobile ? 22 : 28,
              boxShadow: `0 32px 68px -42px ${alphaColor(token.colorTextBase, 0.55)}`,
              marginBottom: isMobile ? 20 : 24,
              border: 'none',
              position: 'relative',
              overflow: 'hidden'
            }}
          >
            <div style={{ position: 'absolute', top: -60, right: -60, width: 200, height: 200, borderRadius: '50%', background: alphaColor(token.colorWhite, 0.08), pointerEvents: 'none' }} />
            <div style={{ position: 'absolute', bottom: -40, left: '30%', width: 120, height: 120, borderRadius: '50%', background: alphaColor(token.colorWhite, 0.05), pointerEvents: 'none' }} />
            <div style={{ position: 'absolute', top: '50%', right: '15%', width: 80, height: 80, borderRadius: '50%', background: alphaColor(token.colorWhite, 0.06), pointerEvents: 'none' }} />

            <Row align="middle" justify="space-between" gutter={[16, 16]} style={{ position: 'relative', zIndex: 1 }}>
              <Col xs={24} sm={12}>
                <Space direction="vertical" size={8}>
                  <Tag
                    bordered={false}
                    style={{
                      alignSelf: 'flex-start',
                      borderRadius: 999,
                      paddingInline: 12,
                      lineHeight: '28px',
                      background: alphaColor(token.colorWhite, 0.12),
                      color: editorialInk,
                    }}
                  >
                    Connector Studio
                  </Tag>
                  <Title
                    level={isMobile ? 3 : 2}
                    style={{ margin: 0, color: editorialInk, fontFamily: designDisplayFont, letterSpacing: '-0.03em' }}
                  >
                    <ToolOutlined style={{ color: alphaColor(token.colorWhite, 0.9), marginRight: 10 }} />
                    MCP 插件管理
                  </Title>
                  <Paragraph style={{ fontSize: isMobile ? 13 : 15, color: alphaColor(token.colorWhite, 0.82), margin: 0, maxWidth: 640 }}>
                    把外部搜索、分析与系统能力接入创作链路，同时用统一的验证面板检查模型是否支持 Function Calling。
                  </Paragraph>
                </Space>
              </Col>
              <Col xs={24} sm={12}>
                <Space size={12} style={{ display: 'flex', justifyContent: isMobile ? 'flex-start' : 'flex-end', width: '100%' }}>
                  <Button
                    type="primary"
                    icon={<PlusOutlined />}
                    onClick={handleCreate}
                    style={{
                      borderRadius: 16,
                      background: alphaColor(token.colorWarning, 0.95),
                      border: `1px solid ${alphaColor(token.colorWhite, 0.3)}`,
                      boxShadow: `0 4px 16px ${alphaColor(token.colorWarning, 0.4)}`,
                      color: '#211a16',
                      fontWeight: 600
                    }}
                  >
                    添加插件
                  </Button>
                </Space>
              </Col>
            </Row>

            <Row gutter={[14, 14]} style={{ marginTop: isMobile ? 16 : 24 }}>
              {overviewStats.map((stat) => (
                <Col xs={24} sm={12} lg={6} key={stat.label}>
                  <Card
                    bordered={false}
                    style={{
                      height: '100%',
                      borderRadius: 20,
                      background: alphaColor(token.colorWhite, 0.08),
                      boxShadow: `inset 0 1px 0 ${alphaColor(token.colorWhite, 0.12)}`,
                    }}
                    styles={{ body: { padding: isMobile ? 16 : 18 } }}
                  >
                    <Text style={{ color: alphaColor(token.colorWhite, 0.68), fontSize: 12 }}>{stat.label}</Text>
                    <div style={{ marginTop: 10, display: 'flex', alignItems: 'center', gap: 10 }}>
                      <span
                        style={{
                          width: 10,
                          height: 10,
                          borderRadius: 999,
                          background: stat.accent,
                          boxShadow: `0 0 0 6px ${alphaColor(stat.accent, 0.18)}`,
                          flexShrink: 0,
                        }}
                      />
                      <Text style={{ color: token.colorWhite, fontSize: isMobile ? 18 : 20, fontWeight: 600 }}>
                        {stat.value}
                      </Text>
                    </div>
                  </Card>
                </Col>
              ))}
            </Row>

            <div style={{ marginTop: isMobile ? 16 : 24, display: 'flex', gap: isMobile ? 12 : 16, flexDirection: isMobile ? 'column' : 'row' }}>
              <Card
                variant="borderless"
                style={{
                  flex: 1,
                  borderRadius: 12,
                  background: alphaColor(token.colorBgContainer, 0.9),
                  border: `1px solid ${alphaColor(token.colorBorder, 0.6)}`,
                  backdropFilter: 'blur(10px)',
                  boxShadow: `0 4px 12px ${alphaColor(token.colorText, 0.06)}`
                }}
                styles={{ body: { padding: isMobile ? 14 : 20 } }}
              >
                <div style={{
                  display: 'flex',
                  flexDirection: isMobile ? 'column' : 'row',
                  justifyContent: 'space-between',
                  alignItems: isMobile ? 'stretch' : 'center',
                  gap: isMobile ? 12 : 0
                }}>
                  <Space align="start" style={{ flex: 1 }}>
                    <div style={{
                      width: isMobile ? 36 : 40,
                      height: isMobile ? 36 : 40,
                      borderRadius: '50%',
                      background: modelSupportStatus === 'supported' ? statusStyles.success.bg : modelSupportStatus === 'unsupported' ? statusStyles.error.bg : statusStyles.info.bg,
                      display: 'flex', alignItems: 'center', justifyContent: 'center',
                      border: `1px solid ${modelSupportStatus === 'supported' ? statusStyles.success.border : modelSupportStatus === 'unsupported' ? statusStyles.error.border : statusStyles.info.border}`,
                      flexShrink: 0
                    }}>
                      {modelSupportStatus === 'supported' ? (
                        <CheckCircleOutlined style={{ fontSize: isMobile ? 18 : 20, color: statusStyles.success.text }} />
                      ) : modelSupportStatus === 'unsupported' ? (
                        <CloseCircleOutlined style={{ fontSize: isMobile ? 18 : 20, color: statusStyles.error.text }} />
                      ) : (
                        <QuestionCircleOutlined style={{ fontSize: isMobile ? 18 : 20, color: statusStyles.info.text }} />
                      )}
                    </div>
                    <div style={{ flex: 1, minWidth: 0 }}>
                      <Text strong style={{ fontSize: isMobile ? 14 : 16, display: 'block', color: token.colorText }}>模型能力检查</Text>
                      <Text type="secondary" style={{ fontSize: isMobile ? 12 : 13, display: 'block', lineHeight: 1.5 }}>
                        {modelSupportStatus === 'supported'
                          ? '当前模型支持 Function Calling，可正常使用 MCP 插件'
                          : modelSupportStatus === 'unsupported'
                            ? '当前模型不支持 Function Calling，无法使用 MCP 插件'
                            : '请先检测模型是否支持 Function Calling 能力'}
                      </Text>
                    </div>
                  </Space>
                  <Button
                    type={modelSupportStatus === 'supported' ? 'default' : 'primary'}
                    icon={<ApiOutlined />}
                    onClick={handleCheckFunctionCalling}
                    loading={checkingFunctionCalling}
                    style={{ borderRadius: 8, width: isMobile ? '100%' : 'auto' }}
                    size={isMobile ? 'middle' : 'middle'}
                  >
                    {modelSupportStatus === 'unknown' ? '开始检测' : '重新检测'}
                  </Button>
                </div>
              </Card>

              <Card
                variant="borderless"
                style={{
                  flex: 1,
                  borderRadius: 12,
                  background: alphaColor(token.colorInfoBg, 0.7),
                  border: `1px solid ${alphaColor(token.colorInfoBorder, 0.8)}`,
                  backdropFilter: 'blur(10px)',
                  boxShadow: `0 4px 12px ${alphaColor(token.colorText, 0.06)}`
                }}
                styles={{ body: { padding: isMobile ? 14 : 20 } }}
              >
                <Space align="start">
                  <InfoCircleOutlined style={{ fontSize: isMobile ? 18 : 20, color: token.colorPrimary, marginTop: 2, flexShrink: 0 }} />
                  <div style={{ flex: 1, minWidth: 0 }}>
                    <Text strong style={{ fontSize: isMobile ? 14 : 16, display: 'block', color: token.colorText, marginBottom: 4 }}>什么是 MCP 插件？</Text>
                    <Text style={{ fontSize: isMobile ? 12 : 13, display: 'block', color: token.colorTextSecondary, lineHeight: 1.6 }}>
                      MCP (Model Context Protocol) 协议允许 AI 调用外部工具获取数据。通过添加插件，AI 可以访问搜索引擎、数据库、API 等服务，大幅增强创作能力。
                    </Text>
                  </div>
                </Space>
              </Card>
            </div>
          </Card>

          {/* 主内容区 */}
          <div style={{ flex: 1 }}>
            <Card
              bordered={false}
              style={{
                borderRadius: 24,
                border: `1px solid ${panelBorder}`,
                background: quietPanelBackground,
                boxShadow: `0 24px 48px -42px ${alphaColor(token.colorTextBase, 0.45)}`,
              }}
              styles={{ body: { padding: isMobile ? 16 : 22 } }}
            >
              <div
                style={{
                  display: 'flex',
                  justifyContent: 'space-between',
                  alignItems: isMobile ? 'flex-start' : 'center',
                  flexDirection: isMobile ? 'column' : 'row',
                  gap: 12,
                  marginBottom: 18,
                }}
              >
                <Space direction="vertical" size={4}>
                  <Text style={{ fontSize: 12, letterSpacing: '0.12em', textTransform: 'uppercase', color: token.colorTextTertiary }}>
                    Plugin Workspace
                  </Text>
                  <Title level={4} style={{ margin: 0, fontFamily: designDisplayFont, color: token.colorTextBase }}>
                    插件编排与运行状态
                  </Title>
                </Space>
                <Text type="secondary" style={{ maxWidth: 620 }}>
                  先校验模型能力，再在同一工作区里完成插件的创建、诊断、工具查看与生命周期管理。
                </Text>
              </div>

              {modelSupportStatus !== 'supported' && plugins.length > 0 && (
                <Alert
                  message={
                    modelSupportStatus === 'unsupported'
                      ? '当前模型不支持 Function Calling，所有插件操作已禁用'
                      : '请先完成模型能力检查，才能操作插件'
                  }
                  type={modelSupportStatus === 'unsupported' ? 'error' : 'warning'}
                  showIcon
                  icon={modelSupportStatus === 'unsupported' ? <CloseCircleOutlined /> : <WarningOutlined />}
                  style={{ marginBottom: 16, borderRadius: 14 }}
                  action={
                    <Button size="small" type="primary" onClick={handleCheckFunctionCalling} loading={checkingFunctionCalling}>
                      {modelSupportStatus === 'unknown' ? '开始检测' : '重新检测'}
                    </Button>
                  }
                />
              )}

              {loading ? (
                renderPluginWorkspaceFallback()
              ) : plugins.length === 0 ? (
                <Empty
                  description="还没有添加任何插件"
                  image={Empty.PRESENTED_IMAGE_SIMPLE}
                  style={{ padding: isMobile ? '40px 0' : '60px 0' }}
                >
                  <Button type="primary" icon={<PlusOutlined />} onClick={handleCreate}>
                    添加第一个插件
                  </Button>
                </Empty>
              ) : (
                <Space direction="vertical" size={isMobile ? 'small' : 'middle'} style={{ width: '100%' }}>
                  {plugins.map((plugin) => (
                    <Card
                      key={plugin.id}
                      size="small"
                      style={{
                        borderRadius: 16,
                        border: `1px solid ${alphaColor(token.colorPrimary, 0.1)}`,
                        background: token.colorBgContainer,
                        boxShadow: `0 18px 34px -32px ${alphaColor(token.colorTextBase, 0.34)}`,
                      }}
                      styles={{ body: { padding: isMobile ? 14 : 18 } }}
                    >
                      <div
                        style={{
                          display: 'flex',
                          flexDirection: 'column',
                          gap: isMobile ? 12 : 16,
                        }}
                      >
                        <div style={{ flex: 1, minWidth: 0 }}>
                          <Space direction="vertical" size="small" style={{ width: '100%' }}>
                            <div style={{
                              display: 'flex',
                              alignItems: 'center',
                              gap: '6px',
                              flexWrap: 'wrap',
                              justifyContent: 'space-between'
                            }}>
                              <div style={{ display: 'flex', alignItems: 'center', gap: '6px', flexWrap: 'wrap', flex: 1 }}>
                                <Text strong style={{ fontSize: isMobile ? '14px' : '16px' }}>
                                  {plugin.display_name || plugin.plugin_name}
                                </Text>
                                {getStatusTag(plugin)}
                              </div>
                              {isMobile && (
                                <Switch
                                  title={modelSupportStatus !== 'supported' ? '请先完成模型能力检查' : (plugin.enabled ? '禁用插件' : '启用插件')}
                                  checked={plugin.enabled}
                                  onChange={(checked) => handleToggle(plugin, checked)}
                                  disabled={modelSupportStatus !== 'supported'}
                                  size="small"
                                  checkedChildren="开"
                                  unCheckedChildren="关"
                                  style={{
                                    flexShrink: 0,
                                    height: 16,
                                    minHeight: 16,
                                    lineHeight: '16px'
                                  }}
                                />
                              )}
                            </div>

                            <div style={{ display: 'flex', gap: '4px', flexWrap: 'wrap' }}>
                              <Tag color={plugin.plugin_type === 'http' || plugin.plugin_type === 'streamable_http' || plugin.plugin_type === 'sse' ? 'blue' : 'cyan'} style={{ fontSize: isMobile ? 11 : 12 }}>
                                {plugin.plugin_type?.toUpperCase() || 'UNKNOWN'}
                              </Tag>
                              {plugin.category && plugin.category !== 'general' && (
                                <Tag style={{ fontSize: isMobile ? 11 : 12, borderRadius: 999, paddingInline: 10 }}>
                                  {plugin.category}
                                </Tag>
                              )}
                            </div>

                            {plugin.description && (
                              <Paragraph
                                type="secondary"
                                style={{
                                  margin: 0,
                                  fontSize: isMobile ? '12px' : '13px',
                                }}
                                ellipsis={{ rows: 2 }}
                              >
                                {plugin.description}
                              </Paragraph>
                            )}

                            {(plugin.plugin_type === 'http' || plugin.plugin_type === 'streamable_http' || plugin.plugin_type === 'sse') && plugin.server_url && (
                              <div style={{
                                fontSize: isMobile ? '11px' : '12px',
                                overflow: 'hidden',
                                textOverflow: 'ellipsis',
                                whiteSpace: 'nowrap'
                              }}>
                                <Text type="secondary" code style={{ fontSize: 'inherit' }}>
                                  {(() => {
                                    const url = plugin.server_url;
                                    try {
                                      const urlObj = new URL(url);
                                      const params = new URLSearchParams(urlObj.search);
                                      let maskedUrl = `${urlObj.protocol}//${urlObj.host}${urlObj.pathname}`;
                                      const sensitiveKeys = ['apiKey', 'api_key', 'key', 'token', 'secret', 'password', 'auth'];
                                      let hasParams = false;

                                      params.forEach((value, key) => {
                                        const isSensitive = sensitiveKeys.some(k => key.toLowerCase().includes(k.toLowerCase()));
                                        const maskedValue = isSensitive ? '***' : value;
                                        maskedUrl += (hasParams ? '&' : '?') + `${key}=${maskedValue}`;
                                        hasParams = true;
                                      });

                                      return maskedUrl;
                                    } catch {
                                      return url.replace(/([?&])(apiKey|api_key|key|token|secret|password|auth)=([^&]+)/gi, '$1$2=***');
                                    }
                                  })()}
                                </Text>
                              </div>
                            )}

                            {plugin.plugin_type === 'stdio' && plugin.command && (
                              <div style={{
                                fontSize: isMobile ? '11px' : '12px',
                                overflow: 'hidden',
                                textOverflow: 'ellipsis',
                                whiteSpace: 'nowrap'
                              }}>
                                <Text type="secondary" code style={{ fontSize: 'inherit' }}>
                                  {plugin.command} {plugin.args?.join(' ')}
                                </Text>
                              </div>
                            )}

                            {plugin.last_error && (
                              <Text type="danger" style={{ fontSize: isMobile ? '11px' : '12px' }}>
                                错误: {plugin.last_error}
                              </Text>
                            )}

                            <div
                              style={{
                                borderRadius: 14,
                                padding: '12px 14px',
                                border: `1px solid ${alphaColor(token.colorBorderSecondary, 0.84)}`,
                                background: `linear-gradient(180deg, ${alphaColor(token.colorBgContainer, 0.98)} 0%, ${alphaColor(token.colorFillQuaternary, 0.34)} 100%)`,
                              }}
                            >
                              <Text style={{ display: 'block', marginBottom: 6, fontSize: 11, letterSpacing: '0.08em', textTransform: 'uppercase', color: token.colorTextTertiary }}>
                                Runtime Brief
                              </Text>
                              <Text type="secondary" style={{ display: 'block', fontSize: 12, lineHeight: 1.7 }}>
                                {plugin.enabled
                                  ? plugin.status === 'active'
                                    ? '当前插件已启用且连接通过，可继续查看工具暴露面与输入参数。'
                                    : '插件已启用，但仍建议先做一次测试确认当前运行状态。'
                                  : '当前插件尚未启用，适合先检查配置和分类，再决定是否纳入创作链路。'}
                              </Text>
                            </div>
                          </Space>
                        </div>

                        <div style={{
                          display: 'flex',
                          justifyContent: isMobile ? 'flex-end' : 'flex-start',
                          alignItems: 'center',
                          gap: isMobile ? 8 : 8,
                          flexWrap: 'wrap',
                          borderTop: isMobile ? `1px solid ${token.colorBorderSecondary}` : 'none',
                          paddingTop: isMobile ? 12 : 0
                        }}>
                          {!isMobile && (
                            <Switch
                              title={modelSupportStatus !== 'supported' ? '请先完成模型能力检查' : (plugin.enabled ? '禁用插件' : '启用插件')}
                              checked={plugin.enabled}
                              onChange={(checked) => handleToggle(plugin, checked)}
                              disabled={modelSupportStatus !== 'supported'}
                              checkedChildren="开"
                              unCheckedChildren="关"
                            />
                          )}
                          <Button
                            title={modelSupportStatus !== 'supported' ? '请先完成模型能力检查' : '测试连接'}
                            icon={<ThunderboltOutlined />}
                            onClick={() => handleTest(plugin.id)}
                            loading={testingPluginId === plugin.id}
                            disabled={modelSupportStatus !== 'supported'}
                            size={isMobile ? 'small' : 'middle'}
                          >
                            {!isMobile && '测试'}
                          </Button>
                          <Button
                            title={modelSupportStatus !== 'supported' ? '请先完成模型能力检查' : '查看工具'}
                            icon={<ToolOutlined />}
                            onClick={() => handleViewTools(plugin.id)}
                            disabled={modelSupportStatus !== 'supported' || !plugin.enabled || plugin.status !== 'active'}
                            size={isMobile ? 'small' : 'middle'}
                          >
                            {!isMobile && '工具'}
                          </Button>
                          <Button
                            title={modelSupportStatus !== 'supported' ? '请先完成模型能力检查' : '编辑'}
                            icon={<EditOutlined />}
                            onClick={() => handleEdit(plugin)}
                            disabled={modelSupportStatus !== 'supported'}
                            size={isMobile ? 'small' : 'middle'}
                          >
                            {!isMobile && '编辑'}
                          </Button>
                          <Button
                            title={modelSupportStatus !== 'supported' ? '请先完成模型能力检查' : '删除'}
                            danger
                            icon={<DeleteOutlined />}
                            onClick={() => handleDelete(plugin)}
                            disabled={modelSupportStatus !== 'supported'}
                            size={isMobile ? 'small' : 'middle'}
                          >
                            {!isMobile && '删除'}
                          </Button>
                        </div>
                      </div>
                    </Card>
                  ))}
                </Space>
              )}
            </Card>
          </div>
        </div>

        {/* 创建/编辑插件模态框 */}
        <Modal
          title={(
            <div>
              <Text style={{ display: 'block', marginBottom: 4, fontSize: 11, letterSpacing: '0.08em', textTransform: 'uppercase', color: token.colorTextTertiary }}>
                Plugin Editor
              </Text>
              <Text strong style={{ display: 'block', fontSize: 18 }}>
                {editingPlugin ? '编辑插件' : '添加插件'}
              </Text>
              <Text type="secondary" style={{ display: 'block', marginTop: 4, lineHeight: 1.7 }}>
                粘贴标准 MCP 配置后，页面会自动提取插件名称；你只需要补齐分类并确认这个插件适合什么创作场景。
              </Text>
            </div>
          )}
          open={modalVisible}
          centered
          onCancel={() => {
            setModalVisible(false);
            form.resetFields();
          }}
          onOk={() => form.submit()}
          width={isMobile ? '100%' : 600}
          confirmLoading={loading}
          okText="保存"
          cancelText="取消"
          styles={{
            content: {
              borderRadius: 24,
              border: `1px solid ${alphaColor(token.colorBorderSecondary, 0.84)}`,
              background: `linear-gradient(180deg, ${alphaColor(token.colorBgContainer, 0.98)} 0%, ${alphaColor(token.colorFillQuaternary, 0.5)} 100%)`,
              boxShadow: `0 28px 56px ${alphaColor(token.colorText, 0.12)}`,
            },
            header: {
              background: 'transparent',
              borderBottom: 'none',
              paddingBottom: 0,
            },
            body: {
              paddingTop: 16,
            },
          }}
        >
          <Space direction="vertical" size="middle" style={{ width: '100%' }}>
            <Alert
              type="info"
              showIcon
              style={{ borderRadius: 14 }}
              message="配置建议"
              description="先确保 JSON 里只保留当前插件真正需要的服务器定义，再通过分类让后续的模型调用更容易匹配到正确工具。"
            />
            <Form form={form} layout="vertical" onFinish={handleSubmit}>
            <Form.Item
              label="MCP配置JSON"
              name="config_json"
              rules={[{ required: true, message: '请输入配置JSON' }]}
              extra="粘贴标准MCP配置，系统自动提取插件名称。支持HTTP和Stdio类型"
            >
              <TextArea
                rows={isMobile ? 12 : 16}
                placeholder={`示例：
{
  "mcpServers": {
    "exa": {
      "type": "streamable_http",
      "url": "https://mcp.exa.ai/mcp?exaApiKey=YOUR_API_KEY",
      "headers": {}
    }
  }
}`}
                style={{ fontFamily: 'monospace', fontSize: '13px' }}
              />
            </Form.Item>

            <Form.Item
              label="插件分类"
              name="category"
              rules={[{ required: true, message: '请选择插件分类' }]}
              extra="选择插件的功能类别，用于AI智能匹配使用场景"
            >
              <Select placeholder="请选择分类">
                <Select.Option value="search">搜索类 (Search) - 网络搜索、信息查询</Select.Option>
                <Select.Option value="analysis">分析类 (Analysis) - 数据分析、文本处理</Select.Option>
                <Select.Option value="filesystem">文件系统 (FileSystem) - 文件读写操作</Select.Option>
                <Select.Option value="database">数据库 (Database) - 数据库查询</Select.Option>
                <Select.Option value="api">API调用 (API) - 第三方服务接口</Select.Option>
                <Select.Option value="generation">生成类 (Generation) - 内容生成工具</Select.Option>
                <Select.Option value="general">通用 (General) - 其他功能</Select.Option>
              </Select>
            </Form.Item>
            </Form>
          </Space>
        </Modal>

        {/* 查看工具列表模态框 */}
        <Modal
          title={
            <Space direction="vertical" size={2}>
              <Text style={{ fontSize: 11, letterSpacing: '0.08em', textTransform: 'uppercase', color: token.colorTextTertiary }}>
                Tool Index
              </Text>
              <Space>
                <ToolOutlined style={{ color: token.colorPrimary }} />
                <span>可用工具列表</span>
              </Space>
              {viewingTools && viewingTools.tools.length > 0 && (
                <Tag color="blue">{viewingTools.tools.length} 个工具</Tag>
              )}
            </Space>
          }
          open={!!viewingTools}
          onCancel={() => setViewingTools(null)}
          footer={[
            <Button key="close" type="primary" onClick={() => setViewingTools(null)}>
              关闭
            </Button>,
          ]}
          width={isMobile ? '95%' : 800}
          centered
          styles={{
            content: {
              borderRadius: 24,
              border: `1px solid ${alphaColor(token.colorBorderSecondary, 0.84)}`,
              background: `linear-gradient(180deg, ${alphaColor(token.colorBgContainer, 0.98)} 0%, ${alphaColor(token.colorFillQuaternary, 0.5)} 100%)`,
              boxShadow: `0 28px 56px ${alphaColor(token.colorText, 0.12)}`,
            },
            header: {
              background: 'transparent',
              borderBottom: 'none',
              paddingBottom: 0,
            },
            body: {
              maxHeight: isMobile ? '60vh' : '70vh',
              overflowY: 'auto',
              padding: isMobile ? '16px' : '24px'
            }
          }}
        >
          {viewingTools && (
            <Space direction="vertical" size="middle" style={{ width: '100%' }}>
              <Alert
                type="info"
                showIcon
                style={{ borderRadius: 14 }}
                message="阅读提示"
                description="先看工具用途，再看输入参数结构。这里展示的是模型最终可调用的工具表，是判断插件是否值得启用的最后一道核对。"
              />
              {viewingTools.tools.length === 0 ? (
                <Empty
                  description="该插件没有提供任何工具"
                  image={Empty.PRESENTED_IMAGE_SIMPLE}
                  style={{ padding: '40px 0' }}
                />
              ) : (
                viewingTools.tools.map((tool, index) => (
                  <Card
                    key={index}
                    size="small"
                    style={{
                      borderRadius: 16,
                      border: `1px solid ${token.colorBorderSecondary}`,
                      boxShadow: `0 12px 24px ${alphaColor(token.colorText, 0.06)}`
                    }}
                    title={
                      <Space>
                        <Text code strong style={{ fontSize: isMobile ? '13px' : '14px', color: token.colorPrimary }}>
                          {tool.name}
                        </Text>
                        <Tag color="processing" style={{ fontSize: '11px' }}>
                          #{index + 1}
                        </Tag>
                      </Space>
                    }
                  >
                    <Space direction="vertical" size="small" style={{ width: '100%' }}>
                      {tool.description && (
                        <div>
                          <Text type="secondary" style={{ fontSize: isMobile ? '12px' : '13px', display: 'block', marginBottom: 4 }}>
                            描述：
                          </Text>
                          <Paragraph
                            style={{
                              margin: 0,
                              fontSize: isMobile ? '12px' : '13px',
                              padding: '8px 12px',
                              background: token.colorBgLayout,
                              borderRadius: 10,
                              borderLeft: `3px solid ${token.colorInfo}`
                            }}
                          >
                            {tool.description}
                          </Paragraph>
                        </div>
                      )}
                      {tool.inputSchema && (
                        <div>
                          <Text type="secondary" style={{ fontSize: isMobile ? '12px' : '13px', display: 'block', marginBottom: 4 }}>
                            输入参数：
                          </Text>
                          <pre
                            style={{
                              margin: 0,
                              padding: isMobile ? '8px' : '12px',
                              background: token.colorBgLayout,
                              borderRadius: 10,
                              fontSize: isMobile ? '11px' : '12px',
                              overflow: 'auto',
                              maxHeight: '200px',
                              border: `1px solid ${token.colorBorderSecondary}`,
                              lineHeight: 1.6
                            }}
                          >
                            {JSON.stringify(tool.inputSchema, null, 2)}
                          </pre>
                        </div>
                      )}
                    </Space>
                  </Card>
                ))
              )}
            </Space>
          )}
        </Modal>
      </div>
    </>
  );
}
