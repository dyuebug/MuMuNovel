import { useState, useEffect } from 'react';
import {
  Card,
  Tabs,
  Button,
  Switch,
  Modal,
  Input,
  Tag,
  message,
  Space,
  Typography,
  Row,
  Col,
  Alert,
  Upload,
  Empty,
  theme,
} from 'antd';
import { useCallback } from 'react';
import { useRef } from 'react';
import {
  EditOutlined,
  ReloadOutlined,
  DownloadOutlined,
  UploadOutlined,
  CheckCircleOutlined,
  FileSearchOutlined,
  InfoCircleOutlined
} from '@ant-design/icons';
import axios from 'axios';
import { designDisplayFont } from '../theme/themeConfig';
import { cardStyles, cardHoverHandlers, gridConfig } from '../components/CardStyles';
import { useThemeMode } from '../theme/useThemeMode';
import InlineDeferredPanel from '../components/InlineDeferredPanel';

const { TextArea } = Input;
const { Title, Text, Paragraph } = Typography;

interface PromptTemplate {
  id: string;
  user_id: string;
  template_key: string;
  template_name: string;
  template_content: string;
  description: string;
  category: string;
  parameters: string;
  is_active: boolean;
  is_system_default: boolean;
  created_at: string;
  updated_at: string;
}

interface CategoryGroup {
  category: string;
  count: number;
  templates: PromptTemplate[];
}

interface PromptTemplateSyncStatusItem {
  template_key: string;
  template_name: string;
  category?: string;
  has_custom_template: boolean;
  is_active: boolean;
  sync_status: 'system_default' | 'up_to_date' | 'legacy_default' | 'customized' | 'system_template_missing';
  is_diff_from_system: boolean;
  is_legacy_default: boolean;
  can_auto_sync: boolean;
  can_sync_to_default: boolean;
  user_content_hash?: string;
  system_content_hash?: string;
  updated_at?: string;
}

interface PromptTemplateSyncStatusResponse {
  total: number;
  managed_only: boolean;
  items: PromptTemplateSyncStatusItem[];
}

export default function PromptTemplates() {
  const [modal, contextHolder] = Modal.useModal();
  const { token } = theme.useToken();
  const { resolvedMode } = useThemeMode();
  const [categories, setCategories] = useState<CategoryGroup[]>([]);
  const [selectedCategory, setSelectedCategory] = useState<string>('0');
  const [editingTemplate, setEditingTemplate] = useState<PromptTemplate | null>(null);
  const [editorVisible, setEditorVisible] = useState(false);
  const [loading, setLoading] = useState(false);
  const [syncStatusMap, setSyncStatusMap] = useState<Record<string, PromptTemplateSyncStatusItem>>({});
  const [syncStatusEnabled, setSyncStatusEnabled] = useState(true);
  const mountedRef = useRef(true);
  const syncStatusRequestIdRef = useRef(0);
  const templatesRequestIdRef = useRef(0);
  const mutationRequestIdRef = useRef(0);

  const isMobile = window.innerWidth <= 768;
  const isDark = resolvedMode === 'dark';
  const alphaColor = (color: string, alpha: number) => `color-mix(in srgb, ${color} ${(alpha * 100).toFixed(0)}%, transparent)`;
  const editorialInk = '#f7f1e8';
  const pageBackground = `linear-gradient(180deg, ${alphaColor(token.colorPrimary, 0.05)} 0%, ${token.colorBgLayout} 32%, ${token.colorBgLayout} 100%)`;
  const heroBackground = `linear-gradient(135deg, #171411 0%, color-mix(in srgb, #171411 62%, ${token.colorPrimary} 38%) 100%)`;
  const panelBorder = alphaColor(token.colorPrimary, 0.1);
  const quietPanelBackground = `linear-gradient(180deg, color-mix(in srgb, ${token.colorBgContainer} 92%, ${token.colorFillAlter} 8%) 0%, color-mix(in srgb, ${token.colorBgContainer} 84%, ${token.colorFillAlter} 16%) 100%)`;

  useEffect(() => {
    mountedRef.current = true;
    return () => {
      mountedRef.current = false;
      syncStatusRequestIdRef.current += 1;
      templatesRequestIdRef.current += 1;
      mutationRequestIdRef.current += 1;
    };
  }, []);

  // 加载模板数据
  const loadSyncStatus = useCallback(async () => {
    syncStatusRequestIdRef.current += 1;
    const requestId = syncStatusRequestIdRef.current;
    try {
      const response = await axios.get<PromptTemplateSyncStatusResponse>('/api/prompt-templates/sync-status', {
        params: { managed_only: true }
      });
      if (!mountedRef.current || syncStatusRequestIdRef.current !== requestId) {
        return;
      }
      const nextMap: Record<string, PromptTemplateSyncStatusItem> = {};
      response.data.items.forEach((item) => {
        nextMap[item.template_key] = item;
      });
      setSyncStatusMap(nextMap);
      setSyncStatusEnabled(true);
    } catch (error: unknown) {
      if (!mountedRef.current || syncStatusRequestIdRef.current !== requestId) {
        return;
      }
      const err = error as { response?: { status?: number } };
      if (err.response?.status === 404) {
        setSyncStatusEnabled(false);
        setSyncStatusMap({});
      } else {
        setSyncStatusEnabled(false);
        setSyncStatusMap({});
        message.warning('同步状态获取失败，已使用基础模式显示');
      }
    }
  }, []);

  const loadTemplates = useCallback(async () => {
    templatesRequestIdRef.current += 1;
    const requestId = templatesRequestIdRef.current;
    try {
      setLoading(true);
      const response = await axios.get<CategoryGroup[]>('/api/prompt-templates/categories');
      if (!mountedRef.current || templatesRequestIdRef.current !== requestId) {
        return;
      }
      setCategories(response.data);
      await loadSyncStatus();
    } catch (error: unknown) {
      if (!mountedRef.current || templatesRequestIdRef.current !== requestId) {
        return;
      }
      const err = error as { response?: { data?: { detail?: string } } };
      message.error(err.response?.data?.detail || '加载失败');
    } finally {
      if (mountedRef.current && templatesRequestIdRef.current === requestId) {
        setLoading(false);
      }
    }
  }, [loadSyncStatus]);

  useEffect(() => {
    loadTemplates();
  }, [loadTemplates]);

  // 获取当前分类的模板
  const getCurrentTemplates = (): PromptTemplate[] => {
    const index = parseInt(selectedCategory);
    if (index === 0) {
      return categories.flatMap(cat => cat.templates);
    }
    return categories[index - 1]?.templates || [];
  };

  const getSyncStatus = (templateKey: string): PromptTemplateSyncStatusItem | undefined => {
    return syncStatusMap[templateKey];
  };

  const getSyncStatusTagConfig = (templateKey: string): { color: string; text: string } | null => {
    if (!syncStatusEnabled) {
      return null;
    }
    const status = getSyncStatus(templateKey);
    if (!status) {
      return null;
    }

    switch (status.sync_status) {
      case 'system_default':
        return { color: 'default', text: '系统默认' };
      case 'up_to_date':
        return { color: 'success', text: '已同步' };
      case 'legacy_default':
        return { color: 'warning', text: '旧默认，可升级' };
      case 'customized':
        return { color: 'processing', text: '已自定义（有差异）' };
      case 'system_template_missing':
        return { color: 'error', text: '系统模板缺失' };
      default:
        return null;
    }
  };

  const isSystemManagedTemplate = (template: PromptTemplate): boolean => {
    const status = getSyncStatus(template.template_key);
    return template.is_system_default || status?.sync_status === 'system_default';
  };

  const canSyncTemplateToDefault = (template: PromptTemplate): boolean => {
    const status = getSyncStatus(template.template_key);
    return status?.can_sync_to_default ?? !template.is_system_default;
  };

  // 编辑模板
  const handleEdit = (template: PromptTemplate) => {
    setEditingTemplate({ ...template });
    setEditorVisible(true);
  };

  // 保存模板
  const handleSave = async () => {
    if (!editingTemplate) return;

    mutationRequestIdRef.current += 1;
    const requestId = mutationRequestIdRef.current;
    try {
      setLoading(true);
      await axios.post('/api/prompt-templates', {
        template_key: editingTemplate.template_key,
        template_name: editingTemplate.template_name,
        template_content: editingTemplate.template_content,
        description: editingTemplate.description,
        category: editingTemplate.category,
        parameters: editingTemplate.parameters,
        is_active: editingTemplate.is_active
      });
      if (!mountedRef.current || mutationRequestIdRef.current !== requestId) {
        return;
      }
      message.success('保存成功');
      setEditorVisible(false);
      await loadTemplates();
    } catch (error: unknown) {
      if (!mountedRef.current || mutationRequestIdRef.current !== requestId) {
        return;
      }
      const err = error as { response?: { data?: { detail?: string } } };
      message.error(err.response?.data?.detail || '保存失败');
    } finally {
      if (mountedRef.current && mutationRequestIdRef.current === requestId) {
        setLoading(false);
      }
    }
  };

  // 重置为系统默认
  const handleReset = async (templateKey: string) => {
    const status = getSyncStatus(templateKey);
    const canSync = status ? status.can_sync_to_default : true;
    if (!canSync) {
      message.info('Already system default');
      return;
    }

    modal.confirm({
      title: '确认同步',
      content: '确定同步到系统默认模板吗？这会覆盖当前自定义内容。',
      okText: '同步',
      cancelText: '取消',
      centered: true,
      onOk: async () => {
        mutationRequestIdRef.current += 1;
        const requestId = mutationRequestIdRef.current;
        try {
          setLoading(true);
          try {
            const response = await axios.post(`/api/prompt-templates/${templateKey}/sync-to-default`);
            if (!mountedRef.current || mutationRequestIdRef.current !== requestId) {
              return;
            }
            const latestStatus = response?.data?.status as PromptTemplateSyncStatusItem | undefined;
            if (latestStatus) {
              setSyncStatusMap((prev) => ({
                ...prev,
                [templateKey]: latestStatus
              }));
            }
            message.success(response?.data?.message || 'Synced to system default');
          } catch (syncError: unknown) {
            const syncErr = syncError as { response?: { status?: number } };
            if (syncErr.response?.status === 404) {
              await axios.post(`/api/prompt-templates/${templateKey}/reset`);
              if (!mountedRef.current || mutationRequestIdRef.current !== requestId) {
                return;
              }
              message.success('已重置为系统默认模板');
            } else {
              throw syncError;
            }
          }
          await loadTemplates();
        } catch (error: unknown) {
          if (!mountedRef.current || mutationRequestIdRef.current !== requestId) {
            return;
          }
          const err = error as { response?: { data?: { detail?: string } } };
          message.error(err.response?.data?.detail || '同步失败');
        } finally {
          if (mountedRef.current && mutationRequestIdRef.current === requestId) {
            setLoading(false);
          }
        }
      }
    });
  };

  // 切换启用状态
  const handleToggleActive = async (template: PromptTemplate, checked: boolean) => {
    mutationRequestIdRef.current += 1;
    const requestId = mutationRequestIdRef.current;
    try {
      await axios.put(`/api/prompt-templates/${template.template_key}`, {
        is_active: checked
      });
      if (!mountedRef.current || mutationRequestIdRef.current !== requestId) {
        return;
      }
      await loadTemplates();
    } catch (error: unknown) {
      if (!mountedRef.current || mutationRequestIdRef.current !== requestId) {
        return;
      }
      const err = error as { response?: { data?: { detail?: string } } };
      message.error(err.response?.data?.detail || '操作失败');
    }
  };

  // 导出所有模板
  const handleExport = async () => {
    try {
      const response = await axios.post('/api/prompt-templates/export');
      const stats = response.data.statistics;
      
      const blob = new Blob([JSON.stringify(response.data, null, 2)], { type: 'application/json' });
      const url = URL.createObjectURL(blob);
      const a = document.createElement('a');
      a.href = url;
      a.download = `prompt-templates-${new Date().toISOString().split('T')[0]}.json`;
      a.click();
      URL.revokeObjectURL(url);
      
      if (stats) {
        message.success(
          `成功导出 ${stats.total} 个提示词配置（${stats.customized} 个自定义，${stats.system_default} 个系统默认）`,
          5
        );
      } else {
        message.success('导出成功');
      }
    } catch (error: unknown) {
      const err = error as { response?: { data?: { detail?: string } } };
      message.error(err.response?.data?.detail || '导出失败');
    }
  };

  // 导入模板
  const handleImport = async (file: File) => {
    mutationRequestIdRef.current += 1;
    const requestId = mutationRequestIdRef.current;
    try {
      const text = await file.text();
      const data = JSON.parse(text);
      const response = await axios.post('/api/prompt-templates/import', data);
      if (!mountedRef.current || mutationRequestIdRef.current !== requestId) {
        return false;
      }
      
      const result = response.data;
      const stats = result.statistics;
      
      // 构建详细的成功消息
      let successMsg = `导入成功！\n`;
      if (stats) {
        successMsg += `• 保持系统默认：${stats.kept_system_default} 个\n`;
        successMsg += `• 创建/更新自定义：${stats.created_or_updated} 个`;
        
        if (stats.converted_to_custom > 0) {
          successMsg += `\n• 检测到修改（已转为自定义）：${stats.converted_to_custom} 个`;
        }
      }
      
      // 如果有被转换的模板，显示详细信息
      if (result.converted_templates && result.converted_templates.length > 0) {
        modal.info({
          title: '导入完成',
          width: 600,
          centered: true,
          content: (
            <div>
              <p style={{ marginBottom: 16 }}>{successMsg}</p>
              {result.converted_templates.length > 0 && (
                <div>
                  <p style={{ fontWeight: 'bold', marginBottom: 8 }}>以下模板内容与系统默认不一致，已转为自定义：</p>
                  <ul style={{ marginLeft: 20 }}>
                    {result.converted_templates.map((t: { template_key: string; template_name: string }) => (
                      <li key={t.template_key}>
                        {t.template_name} ({t.template_key})
                      </li>
                    ))}
                  </ul>
                </div>
              )}
            </div>
          ),
          okText: '确定'
        });
      } else {
        message.success(successMsg, 5);
      }
      
      await loadTemplates();
    } catch (error: unknown) {
      if (!mountedRef.current || mutationRequestIdRef.current !== requestId) {
        return false;
      }
      const err = error as { response?: { data?: { detail?: string } } };
      message.error(err.response?.data?.detail || '导入失败');
    }
    return false; // 阻止默认上传行为
  };

  const currentTemplates = getCurrentTemplates();
  const totalTemplateCount = categories.reduce((sum, category) => sum + category.count, 0);
  const customizedTemplateCount = categories
    .flatMap((category) => category.templates)
    .filter((template) => !isSystemManagedTemplate(template))
    .length;
  const systemTemplateCount = Math.max(totalTemplateCount - customizedTemplateCount, 0);
  const syncHealthyCount = Object.values(syncStatusMap).filter((item) => (
    item.sync_status === 'system_default' || item.sync_status === 'up_to_date'
  )).length;
  const currentCategoryLabel = selectedCategory === '0'
    ? '全部模板'
    : categories[Number(selectedCategory) - 1]?.category || '全部模板';
  const workshopSummaryItems = [
    { label: '模板总数', value: `${totalTemplateCount}` },
    { label: '系统默认', value: `${systemTemplateCount}` },
    { label: '自定义副本', value: `${customizedTemplateCount}` },
    { label: '同步状态正常', value: syncStatusEnabled ? `${syncHealthyCount}` : '基础模式' },
  ];
  const promptGuideSteps = [
    '先确认当前分类视角，再判断这轮是在巡检系统默认模板，还是整理自己的自定义副本。',
    '再读模板用途、预览片段与同步标签，把“阅读判断”放在真正进入正文编辑之前。',
    '最后再决定是同步默认、重置差异，还是继续打磨当前模板内容，避免过早改动正文。',
  ];
  const promptWorkspaceFocus = loading
    ? {
        title: '等待模板工作台刷新',
        note: '当前正在拉取模板列表与同步状态，适合先等待结果回流，再决定本轮要巡检、同步还是编辑哪一类模板。',
      }
    : editorVisible && editingTemplate
      ? {
          title: `编辑模板：${editingTemplate.template_name}`,
          note: '编辑窗口已经打开，适合围绕这一个模板确认用途、变量占位符和正文内容，不必同时切换多个分类来回比较。',
        }
      : currentTemplates.length === 0
        ? {
            title: `补齐“${currentCategoryLabel}”的模板入口`,
            note: '当前分类下没有可操作模板，适合先切回全部模板或其它分类确认范围，再决定是否导入、恢复或新建对应的工作副本。',
          }
        : !syncStatusEnabled
          ? {
              title: '按基础模式阅读当前模板批次',
              note: '当前页面没有展示同步诊断信息，适合先围绕模板说明、预览与正文内容做人工判断，再决定哪些模板需要继续维护。',
            }
          : currentTemplates.some((template) => getSyncStatus(template.template_key)?.sync_status === 'legacy_default')
            ? {
                title: `优先处理“${currentCategoryLabel}”里的旧默认模板`,
                note: '当前视图里存在可升级的旧默认模板，适合先看同步标签与预览差异，把需要回收或升级的模板先处理掉。',
              }
            : currentTemplates.some((template) => !isSystemManagedTemplate(template))
              ? {
                  title: `整理“${currentCategoryLabel}”中的自定义副本`,
                  note: '当前分类里已经有人工改写过的模板，适合先统一检查语气、变量和启用状态，再决定哪些要继续沿用或回收。',
                }
              : {
                  title: `阅读“${currentCategoryLabel}”的系统默认模板`,
                  note: '当前视图以系统默认模板为主，适合先借由用途说明和预览理解模板职责，再决定是否真的需要创建自定义副本。',
                };
  const renderPromptWorkspaceFallback = () => (
    <InlineDeferredPanel
      eyebrow="Template Workspace"
      title={promptWorkspaceFocus.title}
      message={`${promptWorkspaceFocus.note} 当前会同步恢复模板目录、分类标签、编辑入口与默认同步诊断，原有编辑、导入和同步逻辑保持不变。`}
      minHeight={isMobile ? 320 : 360}
      tags={[
        { label: currentCategoryLabel, color: 'blue' },
        { label: syncStatusEnabled ? '同步诊断已开启' : '基础模式', color: syncStatusEnabled ? 'success' : 'default' },
        { label: '模板目录刷新中', color: 'processing' },
      ]}
    />
  );

  return (
    <>
      {contextHolder}
      <div style={{
      minHeight: '90vh',
      background: pageBackground,
      padding: isMobile ? '20px 16px 70px' : '24px 24px 70px',
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
        {/* 顶部导航卡片 */}
        <Card
          variant="borderless"
          style={{
            background: heroBackground,
            borderRadius: isMobile ? 20 : 28,
            boxShadow: `0 24px 48px ${alphaColor(token.colorText, 0.16)}`,
            marginBottom: isMobile ? 20 : 24,
            border: `1px solid ${alphaColor(editorialInk, 0.08)}`,
            position: 'relative',
            overflow: 'hidden'
          }}
        >
          {/* 装饰性背景元素 */}
          <div style={{ position: 'absolute', top: -60, right: -60, width: 200, height: 200, borderRadius: '50%', background: 'rgba(255, 255, 255, 0.08)', pointerEvents: 'none' }} />
          <div style={{ position: 'absolute', bottom: -40, left: '30%', width: 120, height: 120, borderRadius: '50%', background: 'rgba(255, 255, 255, 0.05)', pointerEvents: 'none' }} />
          <div style={{ position: 'absolute', top: '50%', right: '15%', width: 80, height: 80, borderRadius: '50%', background: 'rgba(255, 255, 255, 0.06)', pointerEvents: 'none' }} />

          <Row align="middle" justify="space-between" gutter={[16, 16]} style={{ position: 'relative', zIndex: 1 }}>
            <Col xs={24} sm={12} md={14}>
              <Space direction="vertical" size={8}>
                <Text style={{ color: alphaColor(editorialInk, 0.72), fontSize: 11, letterSpacing: '0.18em', textTransform: 'uppercase' }}>
                  Prompt Workshop
                </Text>
                <Title level={isMobile ? 3 : 2} style={{ margin: 0, color: editorialInk, fontFamily: designDisplayFont, letterSpacing: '-0.03em' }}>
                  <FileSearchOutlined style={{ color: alphaColor(editorialInk, 0.9), marginRight: 8 }} />
                  提示词模板管理
                </Title>
                <Text style={{ fontSize: isMobile ? 12 : 14, color: alphaColor(editorialInk, 0.82), lineHeight: 1.8, maxWidth: 560 }}>
                  这里管理系统默认模板与自定义副本。页面更偏文档型工作台，让你能边阅读说明、边校对模板用途、边调整实际生成提示词。
                </Text>
              </Space>
            </Col>
            <Col xs={24} sm={12} md={10}>
              <Space wrap style={{ justifyContent: isMobile ? 'flex-start' : 'flex-end', width: '100%' }}>
                <Button
                  icon={<DownloadOutlined />}
                  onClick={handleExport}
                  size={isMobile ? 'small' : 'middle'}
                  style={{
                    borderRadius: 999,
                    background: alphaColor('#ffffff', 0.08),
                    border: `1px solid ${alphaColor(editorialInk, 0.14)}`,
                    boxShadow: `0 10px 18px ${alphaColor(token.colorText, 0.12)}`,
                    color: editorialInk,
                    backdropFilter: 'blur(10px)',
                    transition: 'all 0.3s ease'
                  }}
                >
                  导出配置
                </Button>
                <Upload
                  accept=".json"
                  showUploadList={false}
                  beforeUpload={handleImport}
                >
                  <Button
                    icon={<UploadOutlined />}
                    size={isMobile ? 'small' : 'middle'}
                    style={{
                      borderRadius: 999,
                      background: alphaColor('#ffffff', 0.08),
                      border: `1px solid ${alphaColor(editorialInk, 0.14)}`,
                      boxShadow: `0 10px 18px ${alphaColor(token.colorText, 0.12)}`,
                      color: editorialInk,
                      backdropFilter: 'blur(10px)',
                    }}
                  >
                    导入配置
                  </Button>
                </Upload>
              </Space>
            </Col>
          </Row>

          {/* 使用提示 */}
        <Alert
            message={
              <Space align="center">
                <InfoCircleOutlined style={{ fontSize: 16, color: 'var(--color-primary)' }} />
                <Text strong style={{ fontSize: isMobile ? 13 : 14 }}>使用说明</Text>
              </Space>
            }
            description={
              <div>
                <Text style={{ fontSize: isMobile ? 12 : 13, display: 'block', marginBottom: 8 }}>
                  • <strong>系统默认模板</strong>（灰色头部）：始终启用，无需手动开关。点击"编辑"后将创建您的自定义副本。
                </Text>
                <Text style={{ fontSize: isMobile ? 12 : 13, display: 'block' }}>
                  • <strong>已自定义模板</strong>（紫色头部）：可通过开关控制启用/禁用，使用 <Text code>{'{variable_name}'}</Text> 格式表示变量占位符。点击"重置"可恢复为系统默认。
                </Text>
              </div>
            }
            type="info"
            showIcon={false}
            style={{
              marginTop: isMobile ? 16 : 24,
              borderRadius: 16,
              background: alphaColor(token.colorInfo, 0.08),
              border: `1px solid ${alphaColor(token.colorInfo, 0.2)}`
            }}
          />
        </Card>

        <Card
          variant="borderless"
          style={{
            marginBottom: isMobile ? 16 : 20,
            borderRadius: isMobile ? 18 : 24,
            background: `linear-gradient(135deg, ${alphaColor(token.colorPrimary, 0.1)} 0%, ${alphaColor(token.colorInfo, 0.08)} 100%)`,
            border: `1px solid ${alphaColor(token.colorPrimary, 0.16)}`,
            boxShadow: `0 18px 36px ${alphaColor(token.colorText, 0.08)}`,
          }}
          styles={{ body: { padding: isMobile ? 16 : 20 } }}
        >
          <div
            style={{
              display: 'grid',
              gridTemplateColumns: isMobile ? '1fr' : 'minmax(0, 1.7fr) minmax(280px, 0.92fr)',
              gap: 16,
            }}
          >
            <div style={{ display: 'flex', flexDirection: 'column', gap: 10 }}>
              <Text style={{ color: token.colorTextTertiary, fontSize: 12, letterSpacing: '0.12em', textTransform: 'uppercase' }}>
                Template Guide
              </Text>
              <Text style={{ color: token.colorText, lineHeight: 1.75 }}>
                这个页面更像提示词资产的阅读式工作台。现有的模板分类、同步状态、导入导出、编辑弹窗和保存行为都保持不变，这里只把阅读顺序与本轮维护焦点提前说明。
              </Text>
              <div style={{ display: 'flex', flexWrap: 'wrap', gap: 8 }}>
                {promptGuideSteps.map((item, index) => (
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
                      color: token.colorTextBase,
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
                borderRadius: 18,
                padding: isMobile ? '14px 14px 12px' : '16px 18px 14px',
                background: `linear-gradient(180deg, ${token.colorBgContainer} 0%, ${token.colorFillAlter} 100%)`,
                border: `1px solid ${token.colorBorderSecondary}`,
              }}
            >
              <Text style={{ color: token.colorTextTertiary, fontSize: 12, letterSpacing: '0.12em', textTransform: 'uppercase' }}>
                当前工作焦点
              </Text>
              <Title level={5} style={{ margin: '8px 0 6px', color: token.colorTextBase, fontFamily: designDisplayFont, letterSpacing: '-0.02em' }}>
                {promptWorkspaceFocus.title}
              </Title>
              <Text style={{ color: token.colorTextSecondary, lineHeight: 1.75 }}>
                {promptWorkspaceFocus.note}
              </Text>
            </div>
          </div>
        </Card>

        <div
          style={{
            display: 'grid',
            gridTemplateColumns: isMobile ? '1fr' : 'minmax(0, 1.25fr) minmax(320px, 0.9fr)',
            gap: isMobile ? 12 : 16,
            marginBottom: isMobile ? 16 : 20,
          }}
        >
          <Card
            variant="borderless"
            style={{
              background: quietPanelBackground,
              borderRadius: isMobile ? 16 : 22,
              border: `1px solid ${panelBorder}`,
              boxShadow: `0 18px 36px ${alphaColor(token.colorText, 0.06)}`,
            }}
            styles={{ body: { padding: isMobile ? 14 : 18 } }}
          >
            <Text style={{ display: 'block', marginBottom: 6, fontSize: 11, letterSpacing: '0.08em', textTransform: 'uppercase', color: token.colorTextTertiary }}>
              Workshop Snapshot
            </Text>
            <Text strong style={{ display: 'block', marginBottom: 8, fontSize: 16 }}>
              模板总览
            </Text>
            <Text type="secondary" style={{ display: 'block', marginBottom: 14, lineHeight: 1.7 }}>
              先确认你是在维护系统默认模板、同步旧默认，还是沉淀自己的写作工作流。这里汇总的是整个提示词工坊当前的配置状态。
            </Text>
            <div style={{ display: 'flex', flexWrap: 'wrap', gap: 10 }}>
              {workshopSummaryItems.map((item) => (
                <div
                  key={item.label}
                  style={{
                    flex: isMobile ? '1 1 calc(50% - 8px)' : '1 1 140px',
                    minWidth: isMobile ? 0 : 140,
                    borderRadius: 16,
                    padding: '12px 14px',
                    border: `1px solid ${alphaColor(token.colorBorderSecondary, 0.84)}`,
                    background: `linear-gradient(180deg, ${alphaColor(token.colorBgContainer, 0.98)} 0%, ${alphaColor(token.colorFillQuaternary, 0.38)} 100%)`,
                  }}
                >
                  <Text style={{ display: 'block', marginBottom: 4, fontSize: 12, color: token.colorTextTertiary }}>
                    {item.label}
                  </Text>
                  <Text strong style={{ fontSize: 18 }}>
                    {item.value}
                  </Text>
                </div>
              ))}
            </div>
          </Card>

          <Card
            variant="borderless"
            style={{
              background: quietPanelBackground,
              borderRadius: isMobile ? 16 : 22,
              border: `1px solid ${panelBorder}`,
              boxShadow: `0 18px 36px ${alphaColor(token.colorText, 0.06)}`,
            }}
            styles={{ body: { padding: isMobile ? 14 : 18 } }}
          >
            <Text style={{ display: 'block', marginBottom: 6, fontSize: 11, letterSpacing: '0.08em', textTransform: 'uppercase', color: token.colorTextTertiary }}>
              Reading Guide
            </Text>
            <Text strong style={{ display: 'block', marginBottom: 8, fontSize: 16 }}>
              当前筛选说明
            </Text>
            <Text type="secondary" style={{ display: 'block', marginBottom: 14, lineHeight: 1.7 }}>
              现在看到的是“{currentCategoryLabel}”。先读描述与预览，再决定是直接编辑、保留系统默认，还是同步回官方版本。
            </Text>
            <Space direction="vertical" size={10} style={{ width: '100%' }}>
              <div style={{ borderRadius: 16, padding: '12px 14px', border: `1px solid ${alphaColor(token.colorInfo, 0.14)}`, background: alphaColor(token.colorInfoBg, 0.86) }}>
                <Text strong style={{ display: 'block', marginBottom: 4 }}>
                  推荐阅读顺序
                </Text>
                <Text type="secondary" style={{ display: 'block', lineHeight: 1.7 }}>
                  先看模板用途，再看预览片段，最后再进入正文编辑；这样更容易判断是微调语气，还是需要建立一份新的工作副本。
                </Text>
              </div>
              <div style={{ borderRadius: 16, padding: '12px 14px', border: `1px solid ${alphaColor(token.colorSuccess, 0.14)}`, background: alphaColor(token.colorSuccessBg, 0.86) }}>
                <Text strong style={{ display: 'block', marginBottom: 4 }}>
                  同步状态提示
                </Text>
                <Text type="secondary" style={{ display: 'block', lineHeight: 1.7 }}>
                  {syncStatusEnabled
                    ? '带有同步标签的模板，说明它与系统默认存在明确关系，适合统一治理和批量校对。'
                    : '当前处于基础模式，只显示模板本身，不额外展示同步诊断信息。'}
                </Text>
              </div>
            </Space>
          </Card>
        </div>

        {/* 主内容区 */}
        <div style={{ flex: 1 }}>
          {loading ? (
            renderPromptWorkspaceFallback()
          ) : (
            <>
              {/* 分类标签 */}
              {categories.length > 0 && (
                <Card
                  variant="borderless"
                  style={{
                    background: quietPanelBackground,
                    borderRadius: isMobile ? 16 : 22,
                    border: `1px solid ${panelBorder}`,
                    boxShadow: `0 18px 36px ${alphaColor(token.colorText, 0.06)}`,
                    marginBottom: isMobile ? 16 : 24
                  }}
                  styles={{ body: { padding: isMobile ? '12px' : '16px' } }}
                >
                  <Tabs
                    activeKey={selectedCategory}
                    onChange={setSelectedCategory}
                    items={[
                      { key: '0', label: `全部 (${categories.reduce((sum, cat) => sum + cat.count, 0)})` },
                      ...categories.map((cat, index) => ({
                        key: (index + 1).toString(),
                        label: `${cat.category} (${cat.count})`
                      }))
                    ]}
                  />
                </Card>
              )}

              {/* 模板列表 */}
              {currentTemplates.length === 0 ? (
                <Card
                  variant="borderless"
                  style={{
                    background: quietPanelBackground,
                    borderRadius: isMobile ? 16 : 22,
                    border: `1px solid ${panelBorder}`,
                    boxShadow: `0 18px 36px ${alphaColor(token.colorText, 0.06)}`,
                  }}
                >
                  <Empty
                    description="暂无模板数据"
                    style={{ padding: '80px 0' }}
                  />
                </Card>
              ) : (
                <Row gutter={[16, 16]}>
                  {currentTemplates.map(template => {
                    const syncTag = getSyncStatusTagConfig(template.template_key);
                    const isSystemManaged = isSystemManagedTemplate(template);
                    const canSyncToDefault = canSyncTemplateToDefault(template);

                    return (
                    <Col {...gridConfig} key={template.id}>
                      <Card
                        hoverable
                        variant="borderless"
                        style={cardStyles.project}
                        styles={{ body: { padding: 0, overflow: 'hidden' } }}
                        {...cardHoverHandlers}
                      >
                        {/* 头部 */}
                        <div style={{
                          background: isSystemManaged
                            ? `linear-gradient(180deg, ${alphaColor(token.colorBgLayout, isDark ? 0.78 : 0.88)} 0%, ${alphaColor(token.colorFillAlter, isDark ? 0.48 : 0.72)} 100%)`
                            : `linear-gradient(135deg, #171411 0%, color-mix(in srgb, #171411 68%, ${token.colorPrimary} 32%) 100%)`,
                          padding: isMobile ? '16px' : '20px',
                          position: 'relative'
                        }}>
                          <Space direction="vertical" size={8} style={{ width: '100%' }}>
                            <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between' }}>
                              <Title
                                level={isMobile ? 5 : 4}
                                style={{
                                  margin: 0,
                                  color: isSystemManaged ? token.colorText : editorialInk,
                                  flex: 1,
                                  fontFamily: designDisplayFont,
                                  letterSpacing: '-0.02em',
                                }}
                                ellipsis
                              >
                                {template.template_name}
                              </Title>
                              {!isSystemManaged && (
                                <Switch
                                  checked={template.is_active}
                                  onChange={(checked) => handleToggleActive(template, checked)}
                                  size={isMobile ? 'small' : 'default'}
                                  style={{ marginLeft: 8 }}
                                />
                              )}
                            </div>
                            <Space wrap>
                              <Tag color={isSystemManaged ? 'default' : alphaColor('#ffffff', 0.12)} style={{ color: isSystemManaged ? token.colorTextSecondary : editorialInk, border: 'none' }}>
                                {template.category}
                              </Tag>
                              <Tag color={isSystemManaged ? 'default' : alphaColor('#ffffff', 0.12)} style={{ color: isSystemManaged ? token.colorTextSecondary : editorialInk, border: 'none' }}>
                                {isSystemManaged ? '系统默认' : '已自定义'}
                              </Tag>
                              {syncTag && (
                                <Tag color={syncTag.color} style={{ border: 'none' }}>
                                  {syncTag.text}
                                </Tag>
                              )}
                            </Space>
                          </Space>
                        </div>

                        {/* 内容 */}
                        <div style={{ padding: isMobile ? '16px' : '20px' }}>
                          <Paragraph
                            type="secondary"
                            ellipsis={{ rows: 3 }}
                            style={{ minHeight: 66, marginBottom: 16 }}
                          >
                            {template.description || '暂无描述'}
                          </Paragraph>

                          <div
                            style={{
                              marginBottom: 16,
                              borderRadius: 14,
                              padding: '12px 14px',
                              border: `1px solid ${alphaColor(token.colorBorderSecondary, 0.84)}`,
                              background: `linear-gradient(180deg, ${alphaColor(token.colorBgContainer, 0.96)} 0%, ${alphaColor(token.colorFillQuaternary, 0.34)} 100%)`,
                            }}
                          >
                            <Text style={{ display: 'block', marginBottom: 6, fontSize: 11, letterSpacing: '0.08em', textTransform: 'uppercase', color: token.colorTextTertiary }}>
                              Prompt Preview
                            </Text>
                            <Paragraph
                              ellipsis={{ rows: 4 }}
                              style={{
                                margin: 0,
                                fontSize: 12,
                                lineHeight: 1.8,
                                whiteSpace: 'pre-wrap',
                                fontFamily: token.fontFamilyCode,
                                color: token.colorTextSecondary,
                              }}
                            >
                              {template.template_content || '暂无模板内容'}
                            </Paragraph>
                          </div>

                          <Space wrap style={{ marginBottom: 16 }}>
                            <Tag
                              icon={<CheckCircleOutlined />}
                              color={isSystemManaged || template.is_active ? 'success' : 'default'}
                            >
                              {isSystemManaged ? '始终启用' : (template.is_active ? '已启用' : '已禁用')}
                            </Tag>
                          </Space>

                          <Text type="secondary" style={{ fontSize: 12, display: 'block', marginBottom: 16 }}>
                            模板键: {template.template_key}
                          </Text>

                          {/* 操作按钮 */}
                          <Space style={{ width: '100%', justifyContent: 'space-between' }}>
                            <Button
                              type="primary"
                              icon={<EditOutlined />}
                              onClick={() => handleEdit(template)}
                              size={isMobile ? 'small' : 'middle'}
                              style={{ borderRadius: 999, paddingInline: 16 }}
                            >
                              编辑
                            </Button>
                            <Button
                              icon={<ReloadOutlined />}
                              onClick={() => handleReset(template.template_key)}
                              disabled={!canSyncToDefault}
                              size={isMobile ? 'small' : 'middle'}
                              style={{ borderRadius: 999, paddingInline: 16 }}
                            >
                              {syncStatusEnabled ? '同步默认' : '重置'}
                            </Button>
                          </Space>
                        </div>
                      </Card>
                    </Col>
                    );
                  })}
                </Row>
              )}
            </>
          )}
        </div>
      </div>

      {/* 编辑对话框 */}
      <Modal
        title={(
          <div>
            <Text style={{ display: 'block', marginBottom: 4, fontSize: 11, letterSpacing: '0.08em', textTransform: 'uppercase', color: token.colorTextTertiary }}>
              Template Editor
            </Text>
            <Text strong style={{ display: 'block', fontSize: 18 }}>
              {editingTemplate ? `编辑模板：${editingTemplate.template_name}` : '编辑模板'}
            </Text>
            <Text type="secondary" style={{ display: 'block', marginTop: 4, lineHeight: 1.7 }}>
              先确认用途与预览，再调整正文内容，让模板修改保持可阅读、可回滚、可比较。
            </Text>
          </div>
        )}
        open={editorVisible}
        onCancel={() => setEditorVisible(false)}
        onOk={handleSave}
        width={isMobile ? '100%' : 900}
        centered={!isMobile}
        confirmLoading={loading}
        okText="保存"
        cancelText="取消"
        style={isMobile ? { top: 0, paddingBottom: 0, maxWidth: '100vw' } : undefined}
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
          body: isMobile
            ? {
              maxHeight: 'calc(100vh - 110px)',
              overflowY: 'auto',
              padding: '16px'
            }
            : {
              paddingTop: 16,
            },
        }}
      >
        <Space direction="vertical" style={{ width: '100%' }} size="middle">
          <Alert
            message="编辑说明"
            description="系统默认模板在这里会先转成你的自定义副本再保存。变量占位符继续使用 {variable_name} 语法。"
            type="info"
            showIcon
            style={{ borderRadius: 12 }}
          />
          <div>
            <label style={{ display: 'block', marginBottom: '8px', fontWeight: 500 }}>模板名称</label>
            <Input
              value={editingTemplate?.template_name || ''}
              onChange={(e) => setEditingTemplate(prev => prev ? { ...prev, template_name: e.target.value } : null)}
              placeholder="输入模板名称"
            />
          </div>

          <div>
            <label style={{ display: 'block', marginBottom: '8px', fontWeight: 500 }}>描述</label>
            <TextArea
              value={editingTemplate?.description || ''}
              onChange={(e) => setEditingTemplate(prev => prev ? { ...prev, description: e.target.value } : null)}
              rows={2}
              placeholder="简要描述模板用途"
            />
          </div>

          <div>
            <label style={{ display: 'block', marginBottom: '8px', fontWeight: 500 }}>模板内容</label>
            <TextArea
              value={editingTemplate?.template_content || ''}
              onChange={(e) => setEditingTemplate(prev => prev ? { ...prev, template_content: e.target.value } : null)}
              rows={isMobile ? 15 : 20}
              style={{ fontFamily: 'monospace', fontSize: '13px' }}
              placeholder="输入提示词模板内容..."
            />
          </div>
        </Space>
      </Modal>
    </div>
    </>
  );
}
