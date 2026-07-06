import { useState, useEffect, useCallback, useRef } from 'react';
import {
  Card,
  Row,
  Col,
  Input,
  Select,
  Button,
  Tag,
  Space,
  Empty,
  Modal,
  Form,
  message,
  Tooltip,
  Badge,
  Tabs,
  Typography,
  Pagination,
  Alert,
  Statistic,
  theme,
} from 'antd';
import {
  SearchOutlined,
  DownloadOutlined,
  HeartOutlined,
  HeartFilled,
  CloudUploadOutlined,
  EyeOutlined,
  UserOutlined,
  ClockCircleOutlined,
  CheckCircleOutlined,
  CloseCircleOutlined,
  SyncOutlined,
  DeleteOutlined,
  DisconnectOutlined,
  SettingOutlined,
  PlusOutlined,
} from '@ant-design/icons';
import { promptWorkshopApi } from '../services/modularApi';
import { authApi } from '../services/modularApi';
import InlineDeferredPanel from '../components/InlineDeferredPanel';
import { designDisplayFont } from '../theme/themeConfig';
import type {
  PromptWorkshopItem,
  PromptSubmission,
  PromptSubmissionCreate,
  User,
} from '../types';
import { PROMPT_CATEGORIES } from '../types';

const { TextArea } = Input;
const { Text, Paragraph, Title } = Typography;

export default function PromptWorkshop() {
  const mountedRef = useRef(true);
  const initRequestIdRef = useRef(0);
  const itemRequestIdRef = useRef(0);
  const submissionRequestIdRef = useRef(0);
  const adminSubmissionRequestIdRef = useRef(0);
  const publishedRequestIdRef = useRef(0);
  const detailRequestIdRef = useRef(0);
  const [items, setItems] = useState<PromptWorkshopItem[]>([]);
  const [loading, setLoading] = useState(false);
  const [total, setTotal] = useState(0);
  const [currentPage, setCurrentPage] = useState(1);
  const [pageSize] = useState(12);
  
  // 筛选条件
  const [category, setCategory] = useState<string>('');
  const [searchKeyword, setSearchKeyword] = useState('');
  const [sortBy, setSortBy] = useState<'newest' | 'popular' | 'downloads'>('newest');
  
  // 服务状态
  const [serviceStatus, setServiceStatus] = useState<{
    mode: string;
    instance_id: string;
    cloud_connected?: boolean;
  } | null>(null);
  
  // 提交相关
  const [isSubmitModalOpen, setIsSubmitModalOpen] = useState(false);
  const [submitLoading, setSubmitLoading] = useState(false);
  const [submitForm] = Form.useForm();
  
  // 我的提交
  const [mySubmissions, setMySubmissions] = useState<PromptSubmission[]>([]);
  const [submissionsLoading, setSubmissionsLoading] = useState(false);
  
  // 详情弹窗
  const [detailItem, setDetailItem] = useState<PromptWorkshopItem | null>(null);
  const [isDetailModalOpen, setIsDetailModalOpen] = useState(false);
  
  // 导入状态
  const [importingId, setImportingId] = useState<string | null>(null);
  
  // 当前用户
  const [currentUser, setCurrentUser] = useState<User | null>(null);
  
  // 管理员审核相关
  const [adminSubmissions, setAdminSubmissions] = useState<PromptSubmission[]>([]);
  const [adminSubmissionsLoading, setAdminSubmissionsLoading] = useState(false);
  const [adminPendingCount, setAdminPendingCount] = useState(0);
  const [adminStats, setAdminStats] = useState<{
    total_items: number;
    total_official: number;
    total_pending: number;
    total_downloads: number;
    total_likes: number;
  } | null>(null);
  const [reviewModalOpen, setReviewModalOpen] = useState(false);
  const [reviewingSubmission, setReviewingSubmission] = useState<PromptSubmission | null>(null);
  const [reviewForm] = Form.useForm();
  const [reviewLoading, setReviewLoading] = useState(false);
  const [addOfficialModalOpen, setAddOfficialModalOpen] = useState(false);
  const [addOfficialForm] = Form.useForm();
  const [addOfficialLoading, setAddOfficialLoading] = useState(false);
  
  // 已发布提示词管理
  const [publishedItems, setPublishedItems] = useState<PromptWorkshopItem[]>([]);
  const [publishedLoading, setPublishedLoading] = useState(false);
  const [editingItem, setEditingItem] = useState<PromptWorkshopItem | null>(null);
  const [editModalOpen, setEditModalOpen] = useState(false);
  const [editForm] = Form.useForm();
  const [editLoading, setEditLoading] = useState(false);
  
  // 当前活动的 Tab
  const [activeTab, setActiveTab] = useState<string>('browse');
  
  const isMobile = window.innerWidth <= 768;
  const { token } = theme.useToken();
  const alphaColor = (color: string, alpha: number) => `color-mix(in srgb, ${color} ${(alpha * 100).toFixed(0)}%, transparent)`;
  const editorialInk = '#f7f1e8';
  const pageBackground = `linear-gradient(180deg, ${alphaColor(token.colorPrimary, 0.06)} 0%, ${token.colorBgLayout} 30%, ${token.colorBgLayout} 100%)`;
  const heroBackground = `linear-gradient(135deg, #171411 0%, color-mix(in srgb, #171411 60%, ${token.colorPrimary} 40%) 100%)`;
  const quietPanelBackground = `linear-gradient(180deg, color-mix(in srgb, ${token.colorBgContainer} 94%, ${token.colorFillAlter} 6%) 0%, color-mix(in srgb, ${token.colorBgContainer} 86%, ${token.colorFillAlter} 14%) 100%)`;
  const panelBorder = alphaColor(token.colorPrimary, 0.12);
  
  // 判断是否为服务端管理员
  const isServerAdmin = serviceStatus?.mode === 'server' && currentUser?.is_admin;

  // 卡片网格配置 - 与 WritingStyles 保持一致
  const gridConfig = {
    gutter: isMobile ? 8 : 16,
    xs: 24,
    sm: 24,
    md: 12,
    lg: 8,
    xl: 6,
  };

  useEffect(() => {
    mountedRef.current = true;
    return () => {
      mountedRef.current = false;
    };
  }, []);

  // 加载服务状态和用户信息
  useEffect(() => {
    const init = async () => {
      const requestId = ++initRequestIdRef.current;
      try {
        const [status, user] = await Promise.all([
          promptWorkshopApi.getStatus(),
          authApi.getCurrentUser().catch(() => null),
        ]);
        if (!mountedRef.current || initRequestIdRef.current !== requestId) {
          return;
        }
        setServiceStatus(status);
        setCurrentUser(user);
      } catch (error) {
        console.error('Failed to initialize:', error);
      }
    };
    init();
  }, []);

  // 加载工坊列表
  const loadItems = useCallback(async () => {
    const requestId = ++itemRequestIdRef.current;
    setLoading(true);
    try {
      const response = await promptWorkshopApi.getItems({
        category: category || undefined,
        search: searchKeyword || undefined,
        sort: sortBy,
        page: currentPage,
        limit: pageSize,
      });
      if (!mountedRef.current || itemRequestIdRef.current !== requestId) {
        return;
      }
      setItems(response.data?.items || []);
      setTotal(response.data?.total || 0);
    } catch (error) {
      console.error('Failed to load workshop items:', error);
      message.error('加载提示词工坊失败');
    } finally {
      if (mountedRef.current && itemRequestIdRef.current === requestId) {
        setLoading(false);
      }
    }
  }, [category, searchKeyword, sortBy, currentPage, pageSize]);

  useEffect(() => {
    loadItems();
  }, [loadItems]);

  // 加载我的提交
  const loadMySubmissions = async () => {
    const requestId = ++submissionRequestIdRef.current;
    setSubmissionsLoading(true);
    try {
      const response = await promptWorkshopApi.getMySubmissions();
      if (!mountedRef.current || submissionRequestIdRef.current !== requestId) {
        return;
      }
      setMySubmissions(response.data?.items || []);
    } catch (error) {
      console.error('Failed to load submissions:', error);
    } finally {
      if (mountedRef.current && submissionRequestIdRef.current === requestId) {
        setSubmissionsLoading(false);
      }
    }
  };

  // 导入到本地
  const handleImport = async (item: PromptWorkshopItem) => {
    setImportingId(item.id);
    try {
      await promptWorkshopApi.importItem(item.id);
      if (!mountedRef.current) {
        return;
      }
      message.success(`已导入「${item.name}」到本地写作风格`);
      // 刷新列表更新下载计数
      loadItems();
    } catch (error) {
      console.error('Failed to import item:', error);
      message.error('导入失败');
    } finally {
      if (mountedRef.current) {
        setImportingId(null);
      }
    }
  };

  // 点赞
  const handleLike = async (item: PromptWorkshopItem) => {
    try {
      const response = await promptWorkshopApi.toggleLike(item.id);
      // 更新本地状态
      setItems(prev => prev.map(i => 
        i.id === item.id 
          ? { ...i, is_liked: response.liked, like_count: response.like_count }
          : i
      ));
    } catch (error) {
      console.error('Failed to toggle like:', error);
      message.error('操作失败');
    }
  };

  // 提交新提示词
  const handleSubmit = async (values: PromptSubmissionCreate) => {
    setSubmitLoading(true);
    try {
      await promptWorkshopApi.submit({
        ...values,
        tags: values.tags ? (values.tags as unknown as string).split(',').map((t: string) => t.trim()).filter(Boolean) : [],
      });
      message.success('提交成功，等待管理员审核');
      setIsSubmitModalOpen(false);
      submitForm.resetFields();
      loadMySubmissions();
      // 如果是服务端管理员，刷新待审核列表
      if (isServerAdmin) {
        loadAdminSubmissions();
      }
    } catch (error) {
      console.error('Failed to submit:', error);
      message.error('提交失败');
    } finally {
      setSubmitLoading(false);
    }
  };

  // 撤回提交（pending状态）
  const handleWithdraw = async (submissionId: string) => {
    try {
      await promptWorkshopApi.withdrawSubmission(submissionId);
      message.success('已撤回');
      loadMySubmissions();
      // 如果是服务端管理员，刷新待审核列表
      if (isServerAdmin) {
        loadAdminSubmissions();
      }
    } catch (error) {
      console.error('Failed to withdraw:', error);
      message.error('撤回失败');
    }
  };

  // 删除提交记录（已审核状态）
  const handleDeleteSubmission = async (submission: PromptSubmission) => {
    Modal.confirm({
      title: '删除提交记录',
      content: `确定要删除「${submission.name}」的提交记录吗？此操作不可恢复。`,
      okText: '删除',
      okType: 'danger',
      cancelText: '取消',
      centered: true,
      onOk: async () => {
        try {
          await promptWorkshopApi.deleteSubmission(submission.id);
          message.success('删除成功');
          loadMySubmissions();
          // 如果是服务端管理员，刷新相关列表
          if (isServerAdmin) {
            loadAdminSubmissions();
          }
        } catch (error) {
          console.error('Failed to delete submission:', error);
          message.error('删除失败');
        }
      },
    });
  };

  // 查看详情
  const handleViewDetail = async (item: PromptWorkshopItem) => {
    const requestId = ++detailRequestIdRef.current;
    try {
      const response = await promptWorkshopApi.getItem(item.id);
      if (!mountedRef.current || detailRequestIdRef.current !== requestId) {
        return;
      }
      setDetailItem(response.data);
      setIsDetailModalOpen(true);
    } catch (error) {
      console.error('Failed to load detail:', error);
      message.error('加载详情失败');
    }
  };

  // 获取分类标签颜色
  const getCategoryColor = (cat: string) => {
    const colors: Record<string, string> = {
      general: 'blue',
      fantasy: 'purple',
      martial: 'orange',
      romance: 'pink',
      scifi: 'cyan',
      horror: 'red',
      history: 'gold',
      urban: 'green',
      game: 'magenta',
      other: 'default',
    };
    return colors[cat] || 'default';
  };

  // 获取分类名称
  const getCategoryName = (cat: string) => {
    return PROMPT_CATEGORIES[cat] || cat;
  };
  
  // 获取分类选项列表
  const categoryOptions = Object.entries(PROMPT_CATEGORIES).map(([value, label]) => ({
    value,
    label,
  }));

  // 获取提交状态标签
  const getStatusTag = (status: string) => {
    const config: Record<string, { color: string; icon: React.ReactNode; text: string }> = {
      pending: { color: 'processing', icon: <ClockCircleOutlined />, text: '待审核' },
      approved: { color: 'success', icon: <CheckCircleOutlined />, text: '已通过' },
      rejected: { color: 'error', icon: <CloseCircleOutlined />, text: '已拒绝' },
    };
    const cfg = config[status] || config.pending;
    return <Tag color={cfg.color} icon={cfg.icon}>{cfg.text}</Tag>;
  };

  const renderWorkshopFallback = (
    options: {
      eyebrow: string;
      title: string;
      message: string;
      minHeight?: number;
      tags: Array<{ label: string; color?: string }>;
    },
  ) => (
    <InlineDeferredPanel
      eyebrow={options.eyebrow}
      title={options.title}
      message={options.message}
      minHeight={options.minHeight ?? 260}
      tags={options.tags}
    />
  );

  // 渲染筛选区域（固定在顶部）
  const renderFilterBar = () => (
    <div style={{ marginBottom: 16 }}>
      {/* 服务状态 */}
      {serviceStatus && !serviceStatus.cloud_connected && serviceStatus.mode === 'client' && (
        <Alert
          type="warning"
          message="云端服务未连接"
          description="无法访问提示词工坊，请检查网络连接或稍后重试"
          icon={<DisconnectOutlined />}
          showIcon
          style={{ marginBottom: 16 }}
        />
      )}
      
      {/* 筛选区域 */}
      <div style={{
        display: 'flex',
        flexWrap: 'wrap',
        gap: 12,
        alignItems: 'center',
        padding: isMobile ? 14 : 16,
        borderRadius: 18,
        border: `1px solid ${panelBorder}`,
        background: quietPanelBackground,
        boxShadow: `0 18px 36px -30px ${alphaColor(token.colorTextBase, 0.28)}`,
      }}>
        <Input
          placeholder="搜索提示词..."
          prefix={<SearchOutlined />}
          value={searchKeyword}
          onChange={e => setSearchKeyword(e.target.value)}
          onPressEnter={() => { setCurrentPage(1); loadItems(); }}
          style={{ width: isMobile ? '100%' : 200 }}
          allowClear
        />
        <Select
          placeholder="选择分类"
          value={category}
          onChange={v => { setCategory(v); setCurrentPage(1); }}
          style={{ width: isMobile ? '100%' : 150 }}
          allowClear
        >
          {categoryOptions.map(cat => (
            <Select.Option key={cat.value} value={cat.value}>{cat.label}</Select.Option>
          ))}
        </Select>
        <Select
          value={sortBy}
          onChange={v => { setSortBy(v); setCurrentPage(1); }}
          style={{ width: isMobile ? '100%' : 120 }}
        >
          <Select.Option value="newest">最新发布</Select.Option>
          <Select.Option value="popular">最受欢迎</Select.Option>
          <Select.Option value="downloads">下载最多</Select.Option>
        </Select>
        <Button
          icon={<SyncOutlined />}
          onClick={() => { setCurrentPage(1); loadItems(); }}
        >
          刷新
        </Button>
      </div>
    </div>
  );

  // 渲染工坊列表（只有卡片部分，用于滚动区域）
  const renderWorkshopList = () => (
    loading ? (
      renderWorkshopFallback({
        eyebrow: 'Prompt Library',
        title: '正在整理提示词工坊目录',
        message: '系统正在恢复卡片目录、筛选结果与导入入口，原有点赞、详情和导入逻辑保持不变。',
        minHeight: 320,
        tags: [
          { label: '提示词目录', color: 'processing' },
          { label: '卡片工作区恢复中', color: 'purple' },
          { label: '导入逻辑保持原样', color: 'green' },
        ],
      })
    ) : items.length === 0 ? (
      <Empty description="暂无提示词" />
    ) : (
      <>
        <Row
          gutter={[0, gridConfig.gutter]}
          style={{ marginLeft: 0, marginRight: 0 }}
        >
          {items.map(item => (
            <Col
              key={item.id}
              xs={gridConfig.xs}
              sm={gridConfig.sm}
              md={gridConfig.md}
              lg={gridConfig.lg}
              xl={gridConfig.xl}
              style={{
                paddingLeft: 0,
                paddingRight: gridConfig.gutter / 2,
                marginBottom: gridConfig.gutter
              }}
            >
              <Card
                hoverable
                style={{ 
                  height: '100%', 
                  borderRadius: 16,
                  display: 'flex',
                  flexDirection: 'column',
                  border: `1px solid ${token.colorBorderSecondary}`,
                  boxShadow: `0 16px 32px ${alphaColor(token.colorText, 0.05)}`,
                }}
                bodyStyle={{ 
                  padding: 16, 
                  display: 'flex', 
                  flexDirection: 'column', 
                  flex: 1,
                }}
                actions={[
                  <Tooltip title="查看详情" key="view">
                    <EyeOutlined onClick={() => handleViewDetail(item)} />
                  </Tooltip>,
                  <Tooltip title={item.is_liked ? '取消点赞' : '点赞'} key="like">
                    <span onClick={() => handleLike(item)}>
                      {item.is_liked ? (
                        <HeartFilled style={{ color: token.colorError }} />
                      ) : (
                        <HeartOutlined />
                      )}
                      <span style={{ marginLeft: 4 }}>{item.like_count || 0}</span>
                    </span>
                  </Tooltip>,
                  <Tooltip title="导入到本地" key="import">
                    <Button
                      type="link"
                      size="small"
                      icon={<DownloadOutlined />}
                      loading={importingId === item.id}
                      onClick={() => handleImport(item)}
                    >
                      {item.download_count || 0}
                    </Button>
                  </Tooltip>,
                ]}
              >
                <div style={{ flex: 1, display: 'flex', flexDirection: 'column' }}>
                  <Space style={{ marginBottom: 12 }} wrap>
                    <Text strong style={{ fontSize: 16 }}>{item.name}</Text>
                    <Tag color={getCategoryColor(item.category)}>
                      {getCategoryName(item.category)}
                    </Tag>
                    {item.is_official && <Tag color="gold">官方</Tag>}
                  </Space>
                  
                  {item.description && (
                    <Paragraph
                      type="secondary"
                      style={{ fontSize: 13, marginBottom: 12 }}
                      ellipsis={{ rows: 2, tooltip: item.description }}
                    >
                      {item.description}
                    </Paragraph>
                  )}
                  
                  <div
                    style={{
                      backgroundColor: token.colorFillQuaternary,
                      padding: 10,
                      borderRadius: 10,
                      flex: 1,
                      minHeight: 90,
                    }}
                  >
                    <Text style={{ display: 'block', marginBottom: 6, fontSize: 11, letterSpacing: '0.08em', textTransform: 'uppercase', color: token.colorTextTertiary }}>
                      Prompt Preview
                    </Text>
                    <Paragraph
                      type="secondary"
                      style={{
                        fontSize: 12,
                        marginBottom: 0,
                      }}
                      ellipsis={{ rows: 3 }}
                    >
                      {item.prompt_content}
                    </Paragraph>
                  </div>
                  
                  {item.tags && item.tags.length > 0 && (
                    <Space size={4} wrap style={{ marginTop: 8 }}>
                      {item.tags.slice(0, 3).map(tag => (
                        <Tag key={tag} style={{ fontSize: 11 }}>{tag}</Tag>
                      ))}
                      {item.tags.length > 3 && (
                        <Tag style={{ fontSize: 11 }}>+{item.tags.length - 3}</Tag>
                      )}
                    </Space>
                  )}
                </div>
                
                <div style={{ marginTop: 8, color: token.colorTextTertiary, fontSize: 12 }}>
                  <Space wrap size={[10, 6]}>
                    <span><UserOutlined /> {item.author_name || '匿名'}</span>
                    <span><HeartOutlined /> {item.like_count || 0}</span>
                    <span><DownloadOutlined /> {item.download_count || 0}</span>
                  </Space>
                </div>
              </Card>
            </Col>
          ))}
        </Row>
        
        {total > pageSize && (
          <div style={{ marginTop: 24, textAlign: 'center', paddingBottom: 16 }}>
            <Pagination
              current={currentPage}
              total={total}
              pageSize={pageSize}
              onChange={page => setCurrentPage(page)}
              showSizeChanger={false}
              showTotal={t => `共 ${t} 个提示词`}
            />
          </div>
        )}
      </>
    )
  );

  // 渲染我的提交
  const renderMySubmissions = () => (
    <div>
      <div style={{ marginBottom: 16, display: 'flex', justifyContent: 'space-between', alignItems: isMobile ? 'flex-start' : 'center', flexDirection: isMobile ? 'column' : 'row', gap: 10 }}>
        <Space direction="vertical" size={4}>
          <Text strong>查看您提交的提示词及审核状态</Text>
          <Text type="secondary">这里更像你的投递台账，适合判断哪些条目需要补充、撤回或归档。</Text>
        </Space>
        <Button icon={<SyncOutlined />} onClick={loadMySubmissions}>
          刷新
        </Button>
      </div>
      
      {submissionsLoading ? (
        renderWorkshopFallback({
          eyebrow: 'Submission Desk',
          title: '正在整理我的提交台账',
          message: '系统正在恢复投稿记录、审核状态与撤回入口，原有提交、删除和撤回逻辑保持不变。',
          minHeight: 280,
          tags: [
            { label: '我的提交', color: 'processing' },
            { label: '审核状态恢复中', color: 'gold' },
            { label: '撤回逻辑保持原样', color: 'green' },
          ],
        })
      ) : mySubmissions.length === 0 ? (
        <Empty description="暂无提交记录" />
      ) : (
        <Row gutter={[0, gridConfig.gutter]} style={{ marginLeft: 0, marginRight: 0 }}>
          {mySubmissions.map(sub => (
          <Col 
            key={sub.id} 
            xs={gridConfig.xs} 
            sm={gridConfig.sm} 
            md={gridConfig.md} 
            lg={gridConfig.lg}
            xl={gridConfig.xl}
            style={{
              paddingLeft: 0,
              paddingRight: gridConfig.gutter / 2,
              marginBottom: gridConfig.gutter
            }}
          >
            <Card
              style={{ borderRadius: 16, height: '100%', border: `1px solid ${token.colorBorderSecondary}`, boxShadow: `0 16px 32px ${alphaColor(token.colorText, 0.05)}` }}
              bodyStyle={{ padding: 16 }}
            >
              <Space direction="vertical" style={{ width: '100%' }}>
                <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
                  <Text strong>{sub.name}</Text>
                  {getStatusTag(sub.status)}
                </div>
                
                <Tag color={getCategoryColor(sub.category)}>
                  {getCategoryName(sub.category)}
                </Tag>
                
                <Paragraph
                  type="secondary"
                  style={{ fontSize: 12, marginBottom: 0, background: token.colorFillQuaternary, borderRadius: 10, padding: '10px 12px' }}
                  ellipsis={{ rows: 2 }}
                >
                  {sub.prompt_content}
                </Paragraph>
                
                {sub.status === 'rejected' && sub.review_note && (
                  <Alert
                    type="error"
                    message="拒绝原因"
                    description={sub.review_note}
                    style={{ fontSize: 12 }}
                  />
                )}
                
                <div style={{ fontSize: 12, color: token.colorTextTertiary }}>
                  提交时间: {sub.created_at ? new Date(sub.created_at).toLocaleDateString() : '-'}
                </div>
                
                <Space>
                  {sub.status === 'pending' && (
                    <Button
                      type="link"
                      danger
                      size="small"
                      icon={<DeleteOutlined />}
                      onClick={() => handleWithdraw(sub.id)}
                    >
                      撤回
                    </Button>
                  )}
                  {sub.status !== 'pending' && (
                    <Button
                      type="link"
                      danger
                      size="small"
                      icon={<DeleteOutlined />}
                      onClick={() => handleDeleteSubmission(sub)}
                    >
                      删除记录
                    </Button>
                  )}
                </Space>
              </Space>
            </Card>
          </Col>
        ))}
        </Row>
      )}
    </div>
  );

  // 加载管理员待审核列表
  const loadAdminSubmissions = async () => {
    if (!isServerAdmin) return;

    const requestId = ++adminSubmissionRequestIdRef.current;
    setAdminSubmissionsLoading(true);
    try {
      const [subsResponse, statsResponse] = await Promise.all([
        promptWorkshopApi.adminGetSubmissions({ status: 'pending', limit: 50 }),
        promptWorkshopApi.adminGetStats(),
      ]);
      if (!mountedRef.current || adminSubmissionRequestIdRef.current !== requestId) {
        return;
      }
      setAdminSubmissions(subsResponse.data?.items || []);
      setAdminPendingCount(subsResponse.data?.pending_count || 0);
      setAdminStats(statsResponse.data || null);
    } catch (error) {
      console.error('Failed to load admin submissions:', error);
    } finally {
      if (mountedRef.current && adminSubmissionRequestIdRef.current === requestId) {
        setAdminSubmissionsLoading(false);
      }
    }
  };

  // 加载已发布的提示词列表（管理员用）
  const loadPublishedItems = async () => {
    if (!isServerAdmin) return;

    const requestId = ++publishedRequestIdRef.current;
    setPublishedLoading(true);
    try {
      const response = await promptWorkshopApi.getItems({ limit: 100 });
      if (!mountedRef.current || publishedRequestIdRef.current !== requestId) {
        return;
      }
      setPublishedItems(response.data?.items || []);
    } catch (error) {
      console.error('Failed to load published items:', error);
    } finally {
      if (mountedRef.current && publishedRequestIdRef.current === requestId) {
        setPublishedLoading(false);
      }
    }
  };

  // 删除已发布的提示词
  const handleDeleteItem = async (item: PromptWorkshopItem) => {
    Modal.confirm({
      title: '确认删除',
      content: `确定要删除「${item.name}」吗？此操作不可恢复。`,
      okText: '删除',
      okType: 'danger',
      cancelText: '取消',
      centered: true,
      onOk: async () => {
        try {
          await promptWorkshopApi.adminDeleteItem(item.id);
          message.success('删除成功');
          loadPublishedItems();
          loadAdminSubmissions();
          loadItems();
        } catch (error) {
          console.error('Failed to delete item:', error);
          message.error('删除失败');
        }
      },
    });
  };

  // 编辑已发布的提示词
  const handleEditItem = async (values: { name: string; category: string; description?: string; prompt_content: string; tags?: string }) => {
    if (!editingItem) return;
    
    setEditLoading(true);
    try {
      await promptWorkshopApi.adminUpdateItem(editingItem.id, {
        ...values,
        tags: values.tags ? values.tags.split(',').map(t => t.trim()).filter(Boolean) : undefined,
      });
      message.success('修改成功');
      setEditModalOpen(false);
      setEditingItem(null);
      editForm.resetFields();
      loadPublishedItems();
      loadItems();
    } catch (error) {
      console.error('Failed to update item:', error);
      message.error('修改失败');
    } finally {
      setEditLoading(false);
    }
  };

  // 打开编辑弹窗
  const openEditModal = (item: PromptWorkshopItem) => {
    setEditingItem(item);
    editForm.setFieldsValue({
      name: item.name,
      category: item.category,
      description: item.description,
      prompt_content: item.prompt_content,
      tags: item.tags?.join(', '),
    });
    setEditModalOpen(true);
  };

  // 审核提交
  const handleReview = async (action: 'approve' | 'reject') => {
    if (!reviewingSubmission) return;
    
    setReviewLoading(true);
    try {
      const values = reviewForm.getFieldsValue();
      await promptWorkshopApi.adminReviewSubmission(reviewingSubmission.id, {
        action,
        review_note: values.review_note,
        category: values.category,
        tags: values.tags ? values.tags.split(',').map((t: string) => t.trim()).filter(Boolean) : undefined,
      });
      message.success(action === 'approve' ? '已通过审核' : '已拒绝');
      setReviewModalOpen(false);
      setReviewingSubmission(null);
      reviewForm.resetFields();
      // 刷新所有相关数据
      loadAdminSubmissions();
      loadItems();
      loadPublishedItems();  // 通过时会新增到已发布列表
    } catch (error) {
      console.error('Failed to review:', error);
      message.error('审核失败');
    } finally {
      setReviewLoading(false);
    }
  };

  // 添加官方提示词
  const handleAddOfficial = async (values: { name: string; category: string; description?: string; prompt_content: string; tags?: string }) => {
    setAddOfficialLoading(true);
    try {
      await promptWorkshopApi.adminCreateItem({
        ...values,
        tags: values.tags ? values.tags.split(',').map(t => t.trim()).filter(Boolean) : undefined,
      });
      message.success('添加成功');
      setAddOfficialModalOpen(false);
      addOfficialForm.resetFields();
      loadItems();
      loadAdminSubmissions();
      loadPublishedItems();
    } catch (error) {
      console.error('Failed to add official item:', error);
      message.error('添加失败');
    } finally {
      setAddOfficialLoading(false);
    }
  };

  // 渲染管理员面板
  const renderAdminPanel = () => (
    <div>
      <Alert
        type="info"
        showIcon
        style={{ marginBottom: 18, borderRadius: 14 }}
        message="审核工作台说明"
        description="上半区处理待审核提交，下半区维护已经发布的公共资产。优先判断正文质量，再修正分类、标签和发布形态。"
      />
      {/* 统计数据 */}
      {adminStats && (
        <Row gutter={16} style={{ marginBottom: 24 }}>
          <Col span={4}>
            <Card size="small" style={{ borderRadius: 16 }}>
              <Statistic title="总提示词" value={adminStats.total_items} />
            </Card>
          </Col>
          <Col span={4}>
            <Card size="small" style={{ borderRadius: 16 }}>
              <Statistic title="官方提示词" value={adminStats.total_official} />
            </Card>
          </Col>
          <Col span={4}>
            <Card size="small" style={{ borderRadius: 16 }}>
              <Statistic title="待审核" value={adminStats.total_pending} valueStyle={{ color: token.colorWarning }} />
            </Card>
          </Col>
          <Col span={4}>
            <Card size="small" style={{ borderRadius: 16 }}>
              <Statistic title="总下载" value={adminStats.total_downloads} />
            </Card>
          </Col>
          <Col span={4}>
            <Card size="small" style={{ borderRadius: 16 }}>
              <Statistic title="总点赞" value={adminStats.total_likes} />
            </Card>
          </Col>
          <Col span={4}>
            <Card size="small" style={{ display: 'flex', alignItems: 'center', justifyContent: 'center', height: '100%', borderRadius: 16 }}>
              <Button type="primary" icon={<PlusOutlined />} onClick={() => setAddOfficialModalOpen(true)}>
                添加官方
              </Button>
            </Card>
          </Col>
        </Row>
      )}
      
      {/* 待审核列表 */}
      <div style={{ marginBottom: 16, display: 'flex', justifyContent: 'space-between', alignItems: isMobile ? 'flex-start' : 'center', flexDirection: isMobile ? 'column' : 'row', gap: 10 }}>
        <Space direction="vertical" size={4}>
          <Text strong>待审核提交 ({adminPendingCount})</Text>
          <Text type="secondary">这里优先处理新进入工坊的投稿，确认是否值得进入公共资产层。</Text>
        </Space>
        <Button icon={<SyncOutlined />} onClick={loadAdminSubmissions}>
          刷新
        </Button>
      </div>
      
      {adminSubmissionsLoading ? (
        renderWorkshopFallback({
          eyebrow: 'Review Queue',
          title: '正在整理待审核提示词队列',
          message: '系统正在恢复投稿正文、审核入口与来源信息，原有审核、分类和标签处理逻辑保持不变。',
          minHeight: 300,
          tags: [
            { label: '待审核队列', color: 'processing' },
            { label: '审核工作台恢复中', color: 'volcano' },
            { label: '审核逻辑保持原样', color: 'green' },
          ],
        })
      ) : adminSubmissions.length === 0 ? (
        <Empty description="暂无待审核提交" />
      ) : (
        <Row gutter={[0, gridConfig.gutter]} style={{ marginLeft: 0, marginRight: 0 }}>
          {adminSubmissions.map(sub => (
            <Col 
              key={sub.id} 
              xs={gridConfig.xs} 
              sm={gridConfig.sm} 
              md={gridConfig.md} 
              lg={gridConfig.lg}
              xl={gridConfig.xl}
              style={{
                paddingLeft: 0,
                paddingRight: gridConfig.gutter / 2,
                marginBottom: gridConfig.gutter
              }}
            >
              <Card
                style={{ borderRadius: 16, border: `1px solid ${token.colorBorderSecondary}`, boxShadow: `0 16px 32px ${alphaColor(token.colorText, 0.05)}` }}
                bodyStyle={{ padding: 16 }}
                actions={[
                  <Button
                    key="approve"
                    type="link"
                    style={{ color: token.colorSuccess }}
                    onClick={() => {
                      setReviewingSubmission(sub);
                      reviewForm.setFieldsValue({
                        category: sub.category,
                        tags: sub.tags?.join(', '),
                      });
                      setReviewModalOpen(true);
                    }}
                  >
                    审核
                  </Button>,
                ]}
              >
                <Space direction="vertical" style={{ width: '100%' }}>
                  <Text strong>{sub.name}</Text>
                  <Tag color={getCategoryColor(sub.category)}>
                    {getCategoryName(sub.category)}
                  </Tag>
                  
                  <Paragraph
                    type="secondary"
                    style={{ fontSize: 12, marginBottom: 0, background: token.colorFillQuaternary, borderRadius: 10, padding: '10px 12px' }}
                    ellipsis={{ rows: 3 }}
                  >
                    {sub.prompt_content}
                  </Paragraph>
                  
                  <div style={{ fontSize: 11, color: token.colorTextTertiary }}>
                    <div>提交者: {sub.submitter_name || '未知'}</div>
                    <div>来源: {sub.source_instance}</div>
                    <div>时间: {sub.created_at ? new Date(sub.created_at).toLocaleDateString() : '-'}</div>
                  </div>
                </Space>
              </Card>
            </Col>
          ))}
        </Row>
      )}
      
      {/* 已发布提示词管理 */}
      <div style={{ marginTop: 32, marginBottom: 16, display: 'flex', justifyContent: 'space-between', alignItems: isMobile ? 'flex-start' : 'center', flexDirection: isMobile ? 'column' : 'row', gap: 10 }}>
        <Space direction="vertical" size={4}>
          <Text strong>已发布提示词管理 ({publishedItems.length})</Text>
          <Text type="secondary">这一层负责维护已经对外可见的提示词库，适合做文案修正、标签整理和运营清理。</Text>
        </Space>
        <Button icon={<SyncOutlined />} onClick={loadPublishedItems}>
          刷新
        </Button>
      </div>
      
      {publishedLoading ? (
        renderWorkshopFallback({
          eyebrow: 'Published Assets',
          title: '正在整理已发布提示词库',
          message: '系统正在恢复公共提示词档案、编辑入口与运营清理动作，原有编辑和删除逻辑保持不变。',
          minHeight: 300,
          tags: [
            { label: '已发布资产', color: 'blue' },
            { label: '公共提示词库恢复中', color: 'processing' },
            { label: '编辑逻辑保持原样', color: 'green' },
          ],
        })
      ) : publishedItems.length === 0 ? (
        <Empty description="暂无已发布提示词" />
      ) : (
        <Row gutter={[0, gridConfig.gutter]} style={{ marginLeft: 0, marginRight: 0 }}>
          {publishedItems.map(item => (
            <Col 
              key={item.id} 
              xs={gridConfig.xs} 
              sm={gridConfig.sm} 
              md={gridConfig.md} 
              lg={gridConfig.lg}
              xl={gridConfig.xl}
              style={{
                paddingLeft: 0,
                paddingRight: gridConfig.gutter / 2,
                marginBottom: gridConfig.gutter
              }}
            >
              <Card
                style={{ borderRadius: 16, border: `1px solid ${token.colorBorderSecondary}`, boxShadow: `0 16px 32px ${alphaColor(token.colorText, 0.05)}` }}
                bodyStyle={{ padding: 16 }}
                actions={[
                  <Tooltip title="编辑" key="edit">
                    <Button
                      type="link"
                      icon={<SettingOutlined />}
                      onClick={() => openEditModal(item)}
                    />
                  </Tooltip>,
                  <Tooltip title="删除" key="delete">
                    <Button
                      type="link"
                      danger
                      icon={<DeleteOutlined />}
                      onClick={() => handleDeleteItem(item)}
                    />
                  </Tooltip>,
                ]}
              >
                <Space direction="vertical" style={{ width: '100%' }}>
                  <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
                    <Text strong ellipsis style={{ maxWidth: 120 }}>{item.name}</Text>
                      {item.is_official && <Tag color="gold">官方</Tag>}
                    </div>
                    <Tag color={getCategoryColor(item.category)}>
                      {getCategoryName(item.category)}
                    </Tag>
                    
                    <Paragraph
                      type="secondary"
                      style={{ fontSize: 12, marginBottom: 0, background: token.colorFillQuaternary, borderRadius: 10, padding: '10px 12px' }}
                      ellipsis={{ rows: 2 }}
                    >
                      {item.prompt_content}
                    </Paragraph>
                    
                    <div style={{ fontSize: 11, color: token.colorTextTertiary }}>
                      <Space>
                        <span><HeartOutlined /> {item.like_count || 0}</span>
                        <span><DownloadOutlined /> {item.download_count || 0}</span>
                      </Space>
                    </div>
                  </Space>
                </Card>
              </Col>
            ))}
          </Row>
        )}
    </div>
  );

  const pendingSubmissionsCount = mySubmissions.filter((submission) => submission.status === 'pending').length;
  const selectedCategoryLabel = category ? getCategoryName(category) : '全部题材';
  const currentTabLabel = activeTab === 'browse'
    ? '浏览工坊'
    : activeTab === 'submissions'
      ? '我的提交'
      : '管理审核';
  const workspaceTitle = activeTab === 'browse'
    ? 'Workshop Floor'
    : activeTab === 'submissions'
      ? 'Submission Desk'
      : 'Review Ledger';
  const workspaceDescription = activeTab === 'browse'
    ? '筛选、预览并导入适合当前项目的提示词资产。'
    : activeTab === 'submissions'
      ? '跟踪自己的提交记录与审核状态，及时回收无效投递。'
      : '集中处理待审核条目，并维护已发布提示词库。';
  const heroStats = [
    {
      label: '当前视图',
      value: currentTabLabel,
      accent: token.colorInfo,
    },
    {
      label: '工坊总量',
      value: `${total} 条`,
      accent: token.colorPrimary,
    },
    {
      label: '筛选题材',
      value: selectedCategoryLabel,
      accent: token.colorWarning,
    },
    {
      label: isServerAdmin ? '待审核' : '待处理提交',
      value: `${isServerAdmin ? adminPendingCount : pendingSubmissionsCount} 项`,
      accent: token.colorSuccess,
    },
  ];
  const workspaceGuideItems = activeTab === 'browse'
    ? [
      { label: '浏览顺序', value: '先看用途说明，再看预览片段，最后决定导入或收藏。' },
      { label: '当前任务', value: '把公共提示词当作可比较的创作资产，而不只是可复制的文本。' },
    ]
    : activeTab === 'submissions'
      ? [
        { label: '提交策略', value: '优先维护还在等待审核的投递，并及时清理已经失效的旧记录。' },
        { label: '当前任务', value: '把个人提示词沉淀成可审核、可回溯、可再利用的资产。' },
      ]
      : [
        { label: '审核顺序', value: '先看正文质量，再校正分类与标签，最后决定是否发布到工坊。' },
        { label: '当前任务', value: '维持公共提示词库的可读性、一致性与后续运营便利性。' },
      ];
  const modalSurfaceStyles = {
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
  } as const;

  return (
    <div style={{ minHeight: '100%', background: pageBackground }}>
      <div
        style={{
          maxWidth: 1440,
          margin: '0 auto',
          padding: isMobile ? '20px 16px 72px' : '28px 24px 88px',
          display: 'flex',
          flexDirection: 'column',
          gap: 20,
        }}
      >
        <Card
          bordered={false}
          style={{
            background: heroBackground,
            borderRadius: 28,
            overflow: 'hidden',
            boxShadow: `0 32px 68px -42px ${alphaColor(token.colorTextBase, 0.55)}`,
          }}
          styles={{ body: { padding: isMobile ? 20 : 28 } }}
        >
          <div style={{ position: 'relative' }}>
            <div
              style={{
                position: 'absolute',
                inset: 0,
                background: 'radial-gradient(circle at top right, rgba(255,255,255,0.16), transparent 32%)',
                pointerEvents: 'none',
              }}
            />
            <div style={{ position: 'relative', display: 'flex', flexDirection: 'column', gap: 24 }}>
              <div
                style={{
                  display: 'flex',
                  justifyContent: 'space-between',
                  alignItems: isMobile ? 'flex-start' : 'center',
                  flexDirection: isMobile ? 'column' : 'row',
                  gap: 16,
                }}
              >
                <Space direction="vertical" size={8} style={{ maxWidth: 760 }}>
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
                    Prompt Atelier
                  </Tag>
                  <Title
                    level={isMobile ? 3 : 2}
                    style={{ margin: 0, color: editorialInk, fontFamily: designDisplayFont, letterSpacing: '-0.03em' }}
                  >
                    提示词工坊
                  </Title>
                  <Paragraph
                    style={{
                      margin: 0,
                      color: alphaColor(token.colorWhite, 0.82),
                      fontSize: isMobile ? 13 : 15,
                      maxWidth: 720,
                    }}
                  >
                    在同一个工作台里浏览公共提示词、追踪自己的投递，并在需要时切到审核视角维护工坊资产。
                  </Paragraph>
                  <Space size={10} wrap>
                    <Tag color={serviceStatus?.mode === 'server' ? 'success' : 'processing'} style={{ borderRadius: 999, paddingInline: 10 }}>
                      {serviceStatus?.mode === 'server' ? '服务端模式' : '客户端模式'}
                    </Tag>
                    {serviceStatus?.instance_id ? (
                      <Tag style={{ borderRadius: 999, paddingInline: 10 }}>
                        实例 {serviceStatus.instance_id.slice(0, 8)}
                      </Tag>
                    ) : null}
                    {serviceStatus?.mode === 'client' && serviceStatus.cloud_connected === false ? (
                      <Tag color="warning" style={{ borderRadius: 999, paddingInline: 10 }}>
                        云端未连接
                      </Tag>
                    ) : null}
                  </Space>
                </Space>

                <Button
                  type="primary"
                  icon={<CloudUploadOutlined />}
                  onClick={() => setIsSubmitModalOpen(true)}
                  size="large"
                  style={{
                    borderRadius: 16,
                    minWidth: isMobile ? '100%' : 168,
                    background: alphaColor(token.colorWarning, 0.92),
                    borderColor: alphaColor(token.colorWhite, 0.16),
                    color: '#211a16',
                  }}
                >
                  分享我的提示词
                </Button>
              </div>

              <Row gutter={[14, 14]}>
                {heroStats.map((stat) => (
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
            </div>
          </div>
        </Card>

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
          <Tabs
            activeKey={activeTab}
            onChange={key => {
              setActiveTab(key);
              if (key === 'submissions') loadMySubmissions();
              if (key === 'admin') {
                loadAdminSubmissions();
                loadPublishedItems();
              }
            }}
            items={[
              { key: 'browse', label: '浏览工坊' },
              {
                key: 'submissions',
                label: (
                  <Badge count={pendingSubmissionsCount} size="small">
                    我的提交
                  </Badge>
                ),
              },
              ...(isServerAdmin ? [{
                key: 'admin',
                label: (
                  <Badge count={adminPendingCount} size="small">
                    <span><SettingOutlined /> 管理审核</span>
                  </Badge>
                ),
              }] : []),
            ]}
            tabBarStyle={{ marginBottom: activeTab === 'browse' ? 18 : 6 }}
          />

          {activeTab === 'browse' && renderFilterBar()}

          <div
            style={{
              display: 'grid',
              gridTemplateColumns: isMobile ? '1fr' : 'minmax(0, 1.15fr) minmax(320px, 0.95fr)',
              gap: 14,
              marginBottom: 18,
            }}
          >
            <Card
              bordered={false}
              style={{
                borderRadius: 18,
                border: `1px solid ${alphaColor(token.colorPrimary, 0.08)}`,
                background: token.colorBgContainer,
              }}
              styles={{ body: { padding: isMobile ? 14 : 18 } }}
            >
              <Text style={{ fontSize: 12, letterSpacing: '0.12em', textTransform: 'uppercase', color: token.colorTextTertiary }}>
                Workspace Guide
              </Text>
              <Title level={5} style={{ margin: '8px 0 10px', fontFamily: designDisplayFont }}>
                当前面板阅读顺序
              </Title>
              <Text type="secondary" style={{ display: 'block', lineHeight: 1.7 }}>
                不同标签页承担的是不同工作流。先理解当前面板在做什么，再进入具体卡片、提交记录或审核条目，效率会更高。
              </Text>
            </Card>
            <div style={{ display: 'grid', gap: 10 }}>
              {workspaceGuideItems.map((item) => (
                <Card
                  key={item.label}
                  bordered={false}
                  style={{
                    borderRadius: 18,
                    border: `1px solid ${alphaColor(token.colorPrimary, 0.08)}`,
                    background: token.colorBgContainer,
                  }}
                  styles={{ body: { padding: '12px 14px' } }}
                >
                  <Text style={{ display: 'block', marginBottom: 4, fontSize: 12, color: token.colorTextTertiary }}>
                    {item.label}
                  </Text>
                  <Text strong style={{ lineHeight: 1.7 }}>
                    {item.value}
                  </Text>
                </Card>
              ))}
            </div>
          </div>

          <Card
            bordered={false}
            style={{
              borderRadius: 20,
              background: token.colorBgContainer,
              border: `1px solid ${alphaColor(token.colorPrimary, 0.08)}`,
            }}
            styles={{ body: { padding: isMobile ? 16 : 20 } }}
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
                  {workspaceTitle}
                </Text>
                <Title level={4} style={{ margin: 0, fontFamily: designDisplayFont, color: token.colorTextBase }}>
                  {currentTabLabel}
                </Title>
              </Space>
              <Text type="secondary" style={{ maxWidth: 560 }}>
                {workspaceDescription}
              </Text>
            </div>

            {activeTab === 'browse' && renderWorkshopList()}
            {activeTab === 'submissions' && renderMySubmissions()}
            {activeTab === 'admin' && renderAdminPanel()}
          </Card>
        </Card>
      </div>

      {/* 提交弹窗 */}
      <Modal
        title={(
          <div>
            <Text style={{ display: 'block', marginBottom: 4, fontSize: 11, letterSpacing: '0.08em', textTransform: 'uppercase', color: token.colorTextTertiary }}>
              Submission Draft
            </Text>
            <Text strong style={{ display: 'block', fontSize: 18 }}>
              分享提示词到工坊
            </Text>
            <Text type="secondary" style={{ display: 'block', marginTop: 4, lineHeight: 1.7 }}>
              先写清楚用途、正文和作者署名，再把它提交到审核链路里，作为可运营的公共提示词资产处理。
            </Text>
          </div>
        )}
        open={isSubmitModalOpen}
        onCancel={() => {
          setIsSubmitModalOpen(false);
          submitForm.resetFields();
        }}
        footer={null}
        width={isMobile ? '100%' : 600}
        centered
        styles={modalSurfaceStyles}
      >
        <Alert
          type="info"
          message="提交须知"
          description="您的提示词将提交给管理员审核，审核通过后会在工坊中展示。请确保内容原创且不含敏感信息。"
          style={{ marginBottom: 16 }}
          showIcon
        />
        
        <Form
          form={submitForm}
          layout="vertical"
          onFinish={handleSubmit}
        >
          <Form.Item
            name="name"
            label="名称"
            rules={[{ required: true, message: '请输入名称' }]}
          >
            <Input placeholder="给您的提示词起个名字" maxLength={50} />
          </Form.Item>
          
          <Form.Item
            name="category"
            label="分类"
            rules={[{ required: true, message: '请选择分类' }]}
          >
            <Select placeholder="选择分类">
              {categoryOptions.map(cat => (
                <Select.Option key={cat.value} value={cat.value}>{cat.label}</Select.Option>
              ))}
            </Select>
          </Form.Item>
          
          <Form.Item name="description" label="描述">
            <TextArea rows={2} placeholder="简要描述这个提示词的用途和效果" maxLength={200} />
          </Form.Item>
          
          <Form.Item
            name="prompt_content"
            label="提示词内容"
            rules={[{ required: true, message: '请输入提示词内容' }]}
          >
            <TextArea rows={6} placeholder="输入完整的提示词内容..." />
          </Form.Item>
          
          <Form.Item
            name="author_display_name"
            label="作者署名"
            rules={[{ required: true, message: '请输入作者署名' }]}
            tooltip="发布后显示的作者名称"
          >
            <Input placeholder="请输入作者署名（必填）" maxLength={50} />
          </Form.Item>
          
          <Form.Item name="tags" label="标签">
            <Input placeholder="输入标签，多个用逗号分隔，如: 武侠,对话,细腻" />
          </Form.Item>
          
          <Form.Item>
            <Space style={{ width: '100%', justifyContent: 'flex-end' }}>
              <Button onClick={() => {
                setIsSubmitModalOpen(false);
                submitForm.resetFields();
              }}>
                取消
              </Button>
              <Button type="primary" htmlType="submit" loading={submitLoading}>
                提交审核
              </Button>
            </Space>
          </Form.Item>
        </Form>
      </Modal>

      {/* 详情弹窗 */}
      <Modal
        title={(
          <div>
            <Text style={{ display: 'block', marginBottom: 4, fontSize: 11, letterSpacing: '0.08em', textTransform: 'uppercase', color: token.colorTextTertiary }}>
              Prompt Detail
            </Text>
            <Text strong style={{ display: 'block', fontSize: 18 }}>
              {detailItem?.name}
            </Text>
          </div>
        )}
        open={isDetailModalOpen}
        onCancel={() => {
          setIsDetailModalOpen(false);
          setDetailItem(null);
        }}
        footer={[
          <Button key="close" onClick={() => setIsDetailModalOpen(false)}>
            关闭
          </Button>,
          <Button
            key="import"
            type="primary"
            icon={<DownloadOutlined />}
            loading={importingId === detailItem?.id}
            onClick={() => detailItem && handleImport(detailItem)}
          >
            导入到本地
          </Button>,
        ]}
        width={isMobile ? '100%' : 700}
        centered
        styles={modalSurfaceStyles}
      >
        {detailItem && (
          <div>
            <Alert
              type="info"
              showIcon
              style={{ marginBottom: 16, borderRadius: 14 }}
              message="阅读提示"
              description="先确认分类和标签，再阅读正文；如果和当前项目匹配，再决定是否导入到本地资产库。"
            />
            <Space style={{ marginBottom: 16 }} wrap>
              <Tag color={getCategoryColor(detailItem.category)}>
                {getCategoryName(detailItem.category)}
              </Tag>
              {detailItem.tags?.map(tag => (
                <Tag key={tag}>{tag}</Tag>
              ))}
            </Space>
            
            {detailItem.description && (
              <Paragraph style={{ marginBottom: 16 }}>
                {detailItem.description}
              </Paragraph>
            )}
            
            <div style={{
              backgroundColor: token.colorFillSecondary,
              padding: 16,
              borderRadius: 12,
              marginBottom: 16,
              maxHeight: 400,
              overflow: 'auto',
            }}>
              <Text strong style={{ display: 'block', marginBottom: 8 }}>提示词内容</Text>
              <pre style={{
                whiteSpace: 'pre-wrap',
                wordBreak: 'break-word',
                margin: 0,
                fontSize: 13,
              }}>
                {detailItem.prompt_content}
              </pre>
            </div>
            
            <Row gutter={16}>
              <Col span={8}>
                <Text type="secondary">作者</Text>
                <div><UserOutlined /> {detailItem.author_name || '匿名'}</div>
              </Col>
              <Col span={8}>
                <Text type="secondary">点赞</Text>
                <div><HeartOutlined /> {detailItem.like_count || 0}</div>
              </Col>
              <Col span={8}>
                <Text type="secondary">下载</Text>
                <div><DownloadOutlined /> {detailItem.download_count || 0}</div>
              </Col>
            </Row>
          </div>
        )}
      </Modal>
      {/* 审核弹窗 */}
      <Modal
        title={(
          <div>
            <Text style={{ display: 'block', marginBottom: 4, fontSize: 11, letterSpacing: '0.08em', textTransform: 'uppercase', color: token.colorTextTertiary }}>
              Review Desk
            </Text>
            <Text strong style={{ display: 'block', fontSize: 18 }}>
              {`审核：${reviewingSubmission?.name || ''}`}
            </Text>
          </div>
        )}
        open={reviewModalOpen}
        onCancel={() => {
          setReviewModalOpen(false);
          setReviewingSubmission(null);
          reviewForm.resetFields();
        }}
        footer={null}
        width={700}
        centered
        styles={modalSurfaceStyles}
      >
        {reviewingSubmission && (
          <div>
            <Alert
              type="info"
              showIcon
              style={{ marginBottom: 16, borderRadius: 14 }}
              message="审核建议"
              description="通过前优先核对正文质量、分类与标签；如果拒绝，尽量给出可执行的修改方向。"
            />
            <div style={{
              backgroundColor: token.colorFillSecondary,
              padding: 16,
              borderRadius: 12,
              marginBottom: 16,
              maxHeight: 300,
              overflow: 'auto',
            }}>
              <Text strong style={{ display: 'block', marginBottom: 8 }}>提示词内容预览</Text>
              <pre style={{
                whiteSpace: 'pre-wrap',
                wordBreak: 'break-word',
                margin: 0,
                fontSize: 13,
              }}>
                {reviewingSubmission.prompt_content}
              </pre>
            </div>
            
            <Form form={reviewForm} layout="vertical">
              <Form.Item name="category" label="分类（可修改）">
                <Select>
                  {categoryOptions.map(cat => (
                    <Select.Option key={cat.value} value={cat.value}>{cat.label}</Select.Option>
                  ))}
                </Select>
              </Form.Item>
              
              <Form.Item name="tags" label="标签（可修改，逗号分隔）">
                <Input placeholder="武侠, 对话, 细腻" />
              </Form.Item>
              
              <Form.Item name="review_note" label="审核备注">
                <TextArea rows={2} placeholder="拒绝时请填写原因..." />
              </Form.Item>
              
              <Form.Item>
                <Space style={{ width: '100%', justifyContent: 'flex-end' }}>
                  <Button onClick={() => setReviewModalOpen(false)}>
                    取消
                  </Button>
                  <Button danger loading={reviewLoading} onClick={() => handleReview('reject')}>
                    拒绝
                  </Button>
                  <Button type="primary" loading={reviewLoading} onClick={() => handleReview('approve')}>
                    通过
                  </Button>
                </Space>
              </Form.Item>
            </Form>
          </div>
        )}
      </Modal>

      {/* 添加官方提示词弹窗 */}
      <Modal
        title={(
          <div>
            <Text style={{ display: 'block', marginBottom: 4, fontSize: 11, letterSpacing: '0.08em', textTransform: 'uppercase', color: token.colorTextTertiary }}>
              Official Entry
            </Text>
            <Text strong style={{ display: 'block', fontSize: 18 }}>
              添加官方提示词
            </Text>
          </div>
        )}
        open={addOfficialModalOpen}
        onCancel={() => {
          setAddOfficialModalOpen(false);
          addOfficialForm.resetFields();
        }}
        footer={null}
        width={600}
        centered
        styles={modalSurfaceStyles}
      >
        <Alert
          type="info"
          showIcon
          style={{ marginBottom: 16, borderRadius: 14 }}
          message="发布建议"
          description="官方提示词更适合作为模板范本或公共能力入口，所以命名、描述和分类都尽量保持稳定、清晰。"
        />
        <Form
          form={addOfficialForm}
          layout="vertical"
          onFinish={handleAddOfficial}
        >
          <Form.Item
            name="name"
            label="名称"
            rules={[{ required: true, message: '请输入名称' }]}
          >
            <Input placeholder="提示词名称" maxLength={50} />
          </Form.Item>
          
          <Form.Item
            name="category"
            label="分类"
            rules={[{ required: true, message: '请选择分类' }]}
          >
            <Select placeholder="选择分类">
              {categoryOptions.map(cat => (
                <Select.Option key={cat.value} value={cat.value}>{cat.label}</Select.Option>
              ))}
            </Select>
          </Form.Item>
          
          <Form.Item name="description" label="描述">
            <TextArea rows={2} placeholder="简要描述" maxLength={200} />
          </Form.Item>
          
          <Form.Item
            name="prompt_content"
            label="提示词内容"
            rules={[{ required: true, message: '请输入提示词内容' }]}
          >
            <TextArea rows={8} placeholder="输入完整的提示词内容..." />
          </Form.Item>
          
          <Form.Item name="tags" label="标签">
            <Input placeholder="逗号分隔，如: 武侠,对话,细腻" />
          </Form.Item>
          
          <Form.Item>
            <Space style={{ width: '100%', justifyContent: 'flex-end' }}>
              <Button onClick={() => {
                setAddOfficialModalOpen(false);
                addOfficialForm.resetFields();
              }}>
                取消
              </Button>
              <Button type="primary" htmlType="submit" loading={addOfficialLoading}>
                添加
              </Button>
            </Space>
          </Form.Item>
        </Form>
      </Modal>

      {/* 编辑提示词弹窗 */}
      <Modal
        title={(
          <div>
            <Text style={{ display: 'block', marginBottom: 4, fontSize: 11, letterSpacing: '0.08em', textTransform: 'uppercase', color: token.colorTextTertiary }}>
              Library Editor
            </Text>
            <Text strong style={{ display: 'block', fontSize: 18 }}>
              {`编辑：${editingItem?.name || ''}`}
            </Text>
          </div>
        )}
        open={editModalOpen}
        onCancel={() => {
          setEditModalOpen(false);
          setEditingItem(null);
          editForm.resetFields();
        }}
        footer={null}
        width={600}
        centered
        styles={modalSurfaceStyles}
      >
        <Alert
          type="info"
          showIcon
          style={{ marginBottom: 16, borderRadius: 14 }}
          message="维护建议"
          description="编辑已发布提示词时，优先修正可读性、标签和定位说明，避免让公共资产失去统一风格。"
        />
        <Form
          form={editForm}
          layout="vertical"
          onFinish={handleEditItem}
        >
          <Form.Item
            name="name"
            label="名称"
            rules={[{ required: true, message: '请输入名称' }]}
          >
            <Input placeholder="提示词名称" maxLength={50} />
          </Form.Item>
          
          <Form.Item
            name="category"
            label="分类"
            rules={[{ required: true, message: '请选择分类' }]}
          >
            <Select placeholder="选择分类">
              {categoryOptions.map(cat => (
                <Select.Option key={cat.value} value={cat.value}>{cat.label}</Select.Option>
              ))}
            </Select>
          </Form.Item>
          
          <Form.Item name="description" label="描述">
            <TextArea rows={2} placeholder="简要描述" maxLength={200} />
          </Form.Item>
          
          <Form.Item
            name="prompt_content"
            label="提示词内容"
            rules={[{ required: true, message: '请输入提示词内容' }]}
          >
            <TextArea rows={8} placeholder="输入完整的提示词内容..." />
          </Form.Item>
          
          <Form.Item name="tags" label="标签">
            <Input placeholder="逗号分隔，如: 武侠,对话,细腻" />
          </Form.Item>
          
          <Form.Item>
            <Space style={{ width: '100%', justifyContent: 'flex-end' }}>
              <Button onClick={() => {
                setEditModalOpen(false);
                setEditingItem(null);
                editForm.resetFields();
              }}>
                取消
              </Button>
              <Button type="primary" htmlType="submit" loading={editLoading}>
                保存修改
              </Button>
            </Space>
          </Form.Item>
        </Form>
      </Modal>
    </div>
  );
}
