import { Suspense, lazy, useState, useEffect, useMemo, useRef, useCallback } from 'react';
import { useNavigate } from 'react-router-dom';
import {
  Button,
  Modal,
  Form,
  Input,
  Switch,
  Space,
  Tag,
  Popconfirm,
  message,
  Card,
  Typography,
  Badge,
  InputNumber,
  Row,
  Col,
  Pagination,
  Dropdown,
  theme,
} from 'antd';
import {
  PlusOutlined,
  EditOutlined,
  DeleteOutlined,
  KeyOutlined,
  StopOutlined,
  CheckCircleOutlined,
  ArrowLeftOutlined,
  TeamOutlined,
  UserOutlined,
  SearchOutlined,
  MoreOutlined,
} from '@ant-design/icons';
import { adminApi } from '../services/modularApi';
import type { User } from '../types';
import UserMenu from '../components/UserMenu';
import InlineDeferredPanel from '../components/InlineDeferredPanel';
import { useDeferredMount } from '../hooks/useDeferredMount';
import { designDisplayFont } from '../theme/themeConfig';

const { Title, Text } = Typography;

const LazyDeferredAntdTable = lazy(() => import('../components/DeferredAntdTable'));

interface UserWithStatus extends User {
  is_active?: boolean;
}

type SortField =
  | 'username'
  | 'display_name'
  | 'is_active'
  | 'is_admin'
  | 'trust_level'
  | 'created_at'
  | 'last_login';

type SortOrder = 'ascend' | 'descend' | null;

export default function UserManagement() {
  const navigate = useNavigate();
  const [users, setUsers] = useState<UserWithStatus[]>([]);
  const [loading, setLoading] = useState(false);
  const [modalVisible, setModalVisible] = useState(false);
  const [editModalVisible, setEditModalVisible] = useState(false);
  const [resetPasswordModalVisible, setResetPasswordModalVisible] = useState(false);
  const [currentUser, setCurrentUser] = useState<UserWithStatus | null>(null);
  const [newPassword, setNewPassword] = useState('');
  const [pageSize, setPageSize] = useState(20);
  const [currentPage, setCurrentPage] = useState(1);
  const [searchText, setSearchText] = useState('');
  const [sortField, setSortField] = useState<SortField | null>('created_at');
  const [sortOrder, setSortOrder] = useState<SortOrder>('descend');

  const [form] = Form.useForm();
  const [editForm] = Form.useForm();
  const [modal, contextHolder] = Modal.useModal();
  const { token } = theme.useToken();
  const userTableReady = useDeferredMount();
  const alphaColor = (color: string, alpha: number) => `color-mix(in srgb, ${color} ${(alpha * 100).toFixed(0)}%, transparent)`;
  const editorialInk = '#f7f1e8';
  const pageBackground = `linear-gradient(180deg, ${alphaColor(token.colorPrimary, 0.06)} 0%, ${token.colorBgLayout} 30%, ${token.colorBgLayout} 100%)`;
  const heroBackground = `linear-gradient(135deg, #171411 0%, color-mix(in srgb, #171411 60%, ${token.colorPrimary} 40%) 100%)`;
  const quietPanelBackground = `linear-gradient(180deg, color-mix(in srgb, ${token.colorBgContainer} 94%, ${token.colorFillAlter} 6%) 0%, color-mix(in srgb, ${token.colorBgContainer} 86%, ${token.colorFillAlter} 14%) 100%)`;
  const panelBorder = alphaColor(token.colorPrimary, 0.12);
  const mountedRef = useRef(true);
  const userListRequestIdRef = useRef(0);
  const userActionRequestIdRef = useRef(0);

  useEffect(() => {
    mountedRef.current = true;
    return () => {
      mountedRef.current = false;
      userListRequestIdRef.current += 1;
      userActionRequestIdRef.current += 1;
    };
  }, []);

  const beginUserListRequest = useCallback(() => {
    userListRequestIdRef.current += 1;
    return userListRequestIdRef.current;
  }, []);

  const isUserListRequestActive = useCallback((requestId: number) => {
    return mountedRef.current && userListRequestIdRef.current === requestId;
  }, []);

  const beginUserActionRequest = useCallback(() => {
    userActionRequestIdRef.current += 1;
    return userActionRequestIdRef.current;
  }, []);

  const isUserActionRequestActive = useCallback((requestId: number) => {
    return mountedRef.current && userActionRequestIdRef.current === requestId;
  }, []);

  // 过滤用户列表
  const filteredUsers = users.filter(user => {
    if (!searchText) return true;
    const searchLower = searchText.toLowerCase();
    return (
      user.username?.toLowerCase().includes(searchLower) ||
      user.display_name?.toLowerCase().includes(searchLower) ||
      user.user_id?.toLowerCase().includes(searchLower)
    );
  });

  // 排序后的用户列表
  const sortedUsers = useMemo(() => {
    if (!sortField || !sortOrder) {
      return filteredUsers;
    }

    const compareValues = (
      a: string | number | boolean | null | undefined,
      b: string | number | boolean | null | undefined
    ) => {
      // 空值始终置底
      if (a == null && b == null) return 0;
      if (a == null) return 1;
      if (b == null) return -1;

      if (typeof a === 'string' && typeof b === 'string') {
        return a.localeCompare(b, 'zh-CN');
      }

      if (typeof a === 'boolean' && typeof b === 'boolean') {
        return Number(a) - Number(b);
      }

      return Number(a) - Number(b);
    };

    const getSortValue = (user: UserWithStatus) => {
      switch (sortField) {
        case 'username':
          return user.username ?? null;
        case 'display_name':
          return user.display_name ?? null;
        case 'is_active':
          return user.is_active !== false;
        case 'is_admin':
          return user.is_admin;
        case 'trust_level':
          return user.trust_level ?? null;
        case 'created_at':
          return user.created_at ? new Date(user.created_at).getTime() : null;
        case 'last_login':
          return user.last_login ? new Date(user.last_login).getTime() : null;
        default:
          return null;
      }
    };

    const sorted = [...filteredUsers].sort((a, b) => {
      const result = compareValues(getSortValue(a), getSortValue(b));
      return sortOrder === 'ascend' ? result : -result;
    });

    return sorted;
  }, [filteredUsers, sortField, sortOrder]);

  // 加载用户列表
  const loadUsers = useCallback(async () => {
    const requestId = beginUserListRequest();
    setLoading(true);
    try {
      const res = await adminApi.getUsers();
      if (!isUserListRequestActive(requestId)) {
        return;
      }
      setUsers(res.users);
    } catch (error) {
      if (!isUserListRequestActive(requestId)) {
        return;
      }
      console.error('加载用户列表失败:', error);
      message.error('加载用户列表失败');
    } finally {
      if (isUserListRequestActive(requestId)) {
        setLoading(false);
      }
    }
  }, [beginUserListRequest, isUserListRequestActive]);

  useEffect(() => {
    void loadUsers();
  }, [loadUsers]);

  // 添加用户
  interface CreateUserValues {
    username: string;
    display_name: string;
    password?: string;
    avatar_url?: string;
    trust_level?: number;
    is_admin?: boolean;
  }

  const handleCreate = async (values: CreateUserValues) => {
    const requestId = beginUserActionRequest();
    try {
      const res = await adminApi.createUser(values);
      if (!isUserActionRequestActive(requestId)) {
        return;
      }
      message.success('用户创建成功');

      // 如果有默认密码，显示给管理员
      if (res.default_password) {
        modal.info({
          title: '用户创建成功',
          content: (
            <div>
              <p>用户名：<Text strong>{values.username}</Text></p>
              <p>初始密码：<Text strong copyable>{res.default_password}</Text></p>
              <p style={{ color: token.colorError, marginTop: 16 }}>
                ⚠️ 请复制密码并告知用户，此密码仅显示一次！
              </p>
            </div>
          ),
          width: 500,
          centered: true,
        });
      }

      setModalVisible(false);
      form.resetFields();
      await loadUsers();
    } catch (error) {
      if (!isUserActionRequestActive(requestId)) {
        return;
      }
      console.error('创建用户失败:', error);
      message.error('创建用户失败');
    }
  };

  // 编辑用户
  const handleEdit = (user: UserWithStatus) => {
    setCurrentUser(user);
    editForm.setFieldsValue({
      display_name: user.display_name,
      avatar_url: user.avatar_url,
      trust_level: user.trust_level,
      is_admin: user.is_admin,
    });
    setEditModalVisible(true);
  };

  interface UpdateUserValues {
    display_name: string;
    avatar_url?: string;
    trust_level?: number;
    is_admin?: boolean;
  }

  const handleUpdate = async (values: UpdateUserValues) => {
    if (!currentUser) return;

    const requestId = beginUserActionRequest();
    try {
      await adminApi.updateUser(currentUser.user_id, values);
      if (!isUserActionRequestActive(requestId)) {
        return;
      }
      message.success('用户信息更新成功');
      setEditModalVisible(false);
      editForm.resetFields();
      await loadUsers();
    } catch (error) {
      if (!isUserActionRequestActive(requestId)) {
        return;
      }
      console.error('更新用户失败:', error);
      message.error('更新用户失败');
    }
  };

  // 切换用户状态
  const handleToggleStatus = async (user: UserWithStatus) => {
    const isActive = user.is_active !== false;
    const action = isActive ? '禁用' : '启用';

    const requestId = beginUserActionRequest();
    try {
      await adminApi.toggleUserStatus(user.user_id, !isActive);
      if (!isUserActionRequestActive(requestId)) {
        return;
      }
      message.success(`用户已${action}`);
      await loadUsers();
    } catch (error) {
      if (!isUserActionRequestActive(requestId)) {
        return;
      }
      console.error(`${action}用户失败:`, error);
      message.error(`${action}用户失败`);
    }
  };

  // 重置密码
  const handleResetPassword = (user: UserWithStatus) => {
    setCurrentUser(user);
    setNewPassword('');
    setResetPasswordModalVisible(true);
  };

  const handleResetPasswordConfirm = async () => {
    if (!currentUser) return;

    const requestId = beginUserActionRequest();
    try {
      const res = await adminApi.resetPassword(
        currentUser.user_id,
        newPassword || undefined
      );
      if (!isUserActionRequestActive(requestId)) {
        return;
      }

      modal.info({
        title: '密码重置成功',
        content: (
          <div>
            <p>用户：<Text strong>{currentUser.username}</Text></p>
            <p>新密码：<Text strong copyable>{res.new_password}</Text></p>
            <p style={{ color: token.colorError, marginTop: 16 }}>
              ⚠️ 请复制密码并告知用户！
            </p>
          </div>
        ),
        width: 500,
        centered: true,
      });

      setResetPasswordModalVisible(false);
      setNewPassword('');
    } catch (error) {
      if (!isUserActionRequestActive(requestId)) {
        return;
      }
      console.error('重置密码失败:', error);
      message.error('重置密码失败');
    }
  };

  // 删除用户
  const handleDelete = async (user: UserWithStatus) => {
    const requestId = beginUserActionRequest();
    try {
      await adminApi.deleteUser(user.user_id);
      if (!isUserActionRequestActive(requestId)) {
        return;
      }
      message.success('用户已删除');
      await loadUsers();
    } catch (error) {
      if (!isUserActionRequestActive(requestId)) {
        return;
      }
      console.error('删除用户失败:', error);
      message.error('删除用户失败');
    }
  };

  const isMobile = window.innerWidth <= 768;
  const adminCount = users.filter((user) => user.is_admin).length;
  const activeCount = users.filter((user) => user.is_active !== false).length;
  const disabledCount = users.filter((user) => user.is_active === false).length;
  const overviewStats = [
    { label: '用户总数', value: `${users.length} 位`, accent: token.colorPrimary },
    { label: '管理员', value: `${adminCount} 位`, accent: token.colorWarning },
    { label: '活跃用户', value: `${activeCount} 位`, accent: token.colorSuccess },
    { label: '当前筛选', value: `${filteredUsers.length} 位`, accent: token.colorInfo },
  ];
  // 表格列定义
  const columns = [
    {
      title: '用户名',
      dataIndex: 'username',
      key: 'username',
      width: 150,
      sorter: true,
      sortOrder: sortField === 'username' ? sortOrder : null,
      render: (text: string) => (
        <Space>
          <UserOutlined style={{ color: token.colorPrimary }} />
          <Text strong>{text}</Text>
        </Space>
      ),
    },
    {
      title: '显示名称',
      dataIndex: 'display_name',
      key: 'display_name',
      width: 150,
      sorter: true,
      sortOrder: sortField === 'display_name' ? sortOrder : null,
    },
    {
      title: '状态',
      dataIndex: 'is_active',
      key: 'is_active',
      width: 100,
      sorter: true,
      sortOrder: sortField === 'is_active' ? sortOrder : null,
      render: (isActive: boolean) => (
        <Badge
          status={isActive !== false ? 'success' : 'error'}
          text={isActive !== false ? '正常' : '已禁用'}
        />
      ),
    },
    {
      title: '角色',
      dataIndex: 'is_admin',
      key: 'is_admin',
      width: 100,
      sorter: true,
      sortOrder: sortField === 'is_admin' ? sortOrder : null,
      render: (isAdmin: boolean) => (
        <Tag color={isAdmin ? 'gold' : 'blue'}>
          {isAdmin ? '👑 管理员' : '普通用户'}
        </Tag>
      ),
    },
    {
      title: '信任等级',
      dataIndex: 'trust_level',
      key: 'trust_level',
      width: 100,
      sorter: true,
      sortOrder: sortField === 'trust_level' ? sortOrder : null,
      render: (level: number) => (
        <Tag color={level === -1 ? 'default' : level >= 5 ? 'green' : 'blue'}>
          {level === -1 ? '已禁用' : `Level ${level}`}
        </Tag>
      ),
    },
    {
      title: '创建时间',
      dataIndex: 'created_at',
      key: 'created_at',
      width: 180,
      sorter: true,
      sortOrder: sortField === 'created_at' ? sortOrder : null,
      render: (date: string) => date ? new Date(date).toLocaleString('zh-CN') : '-',
    },
    {
      title: '最后登录',
      dataIndex: 'last_login',
      key: 'last_login',
      width: 180,
      sorter: true,
      sortOrder: sortField === 'last_login' ? sortOrder : null,
      render: (date: string) => date ? new Date(date).toLocaleString('zh-CN') : '从未登录',
    },
    {
      title: '操作',
      key: 'action',
      width: isMobile ? 80 : 300,
      fixed: 'right' as const,
      render: (_: unknown, record: UserWithStatus) => {
        const isActive = record.is_active !== false;

        // 移动端：使用下拉菜单
        if (isMobile) {
          const menuItems = [
            {
              key: 'edit',
              label: '编辑用户',
              icon: <EditOutlined />,
              onClick: () => handleEdit(record),
            },
            {
              key: 'reset',
              label: '重置密码',
              icon: <KeyOutlined />,
              onClick: () => handleResetPassword(record),
            },
            {
              key: 'toggle',
              label: isActive ? '禁用用户' : '启用用户',
              icon: isActive ? <StopOutlined /> : <CheckCircleOutlined />,
              danger: isActive,
              onClick: () => {
                modal.confirm({
                  title: `确定${isActive ? '禁用' : '启用'}该用户吗？`,
                  onOk: () => handleToggleStatus(record),
                  okText: '确定',
                  cancelText: '取消',
                });
              },
            },
            ...(!record.is_admin ? [{
              key: 'delete',
              label: '删除用户',
              icon: <DeleteOutlined />,
              danger: true,
              onClick: () => {
                modal.confirm({
                  title: '确定删除该用户吗？此操作不可恢复！',
                  onOk: () => handleDelete(record),
                  okText: '确定',
                  cancelText: '取消',
                  okButtonProps: { danger: true },
                });
              },
            }] : []),
          ];

          return (
            <Dropdown menu={{ items: menuItems }} trigger={['click']}>
              <Button type="text" icon={<MoreOutlined />} />
            </Dropdown>
          );
        }

        // 桌面端：保持原有按钮样式
        return (
          <Space size="small">
            <Button
              type="link"
              size="small"
              icon={<EditOutlined />}
              onClick={() => handleEdit(record)}
            >
              编辑
            </Button>

            <Button
              type="link"
              size="small"
              icon={<KeyOutlined />}
              onClick={() => handleResetPassword(record)}
            >
              重置密码
            </Button>

            <Popconfirm
              title={`确定${isActive ? '禁用' : '启用'}该用户吗？`}
              onConfirm={() => handleToggleStatus(record)}
              okText="确定"
              cancelText="取消"
            >
              <Button
                type="link"
                size="small"
                danger={isActive}
                icon={isActive ? <StopOutlined /> : <CheckCircleOutlined />}
              >
                {isActive ? '禁用' : '启用'}
              </Button>
            </Popconfirm>

            {!record.is_admin && (
              <Popconfirm
                title="确定删除该用户吗？此操作不可恢复！"
                onConfirm={() => handleDelete(record)}
                okText="确定"
                cancelText="取消"
                okButtonProps={{ danger: true }}
              >
                <Button
                  type="link"
                  size="small"
                  danger
                  icon={<DeleteOutlined />}
                >
                  删除
                </Button>
              </Popconfirm>
            )}
          </Space>
        );
      },
    },
  ];

  return (
    <div style={{
      minHeight: '100vh',
      background: pageBackground,
      padding: isMobile ? '20px 16px' : '28px 24px 80px',
      display: 'flex',
      flexDirection: 'column',
      overflow: 'auto',
    }}>
      {contextHolder}
      <div style={{
        maxWidth: 1400,
        margin: '0 auto',
        width: '100%',
        minHeight: '100%',
        display: 'flex',
        flexDirection: 'column',
        gap: 20,
      }}>
        <Card
          style={{
            background: heroBackground,
            borderRadius: isMobile ? 22 : 28,
            boxShadow: `0 32px 68px -42px ${alphaColor(token.colorTextBase, 0.55)}`,
            border: 'none',
            position: 'relative',
            overflow: 'hidden',
          }}
          styles={{ body: { padding: isMobile ? 20 : 28 } }}
        >
          <div style={{ position: 'absolute', top: -60, right: -60, width: 200, height: 200, borderRadius: '50%', background: alphaColor(token.colorWhite, 0.08), pointerEvents: 'none' }} />
          <div style={{ position: 'absolute', bottom: -40, left: '30%', width: 120, height: 120, borderRadius: '50%', background: alphaColor(token.colorWhite, 0.05), pointerEvents: 'none' }} />
          <div style={{ position: 'absolute', top: '50%', right: '15%', width: 80, height: 80, borderRadius: '50%', background: alphaColor(token.colorWhite, 0.06), pointerEvents: 'none' }} />

          <div style={{ position: 'relative', zIndex: 1, display: 'flex', flexDirection: 'column', gap: 24 }}>
            <Row align="middle" justify="space-between" gutter={[16, 16]}>
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
                    Admin Ledger
                  </Tag>
                  <Title
                    level={isMobile ? 3 : 2}
                    style={{ margin: 0, color: editorialInk, fontFamily: designDisplayFont, letterSpacing: '-0.03em' }}
                  >
                    <TeamOutlined style={{ color: alphaColor(token.colorWhite, 0.9), marginRight: 12 }} />
                    用户管理
                  </Title>
                  <Text style={{ fontSize: isMobile ? 13 : 15, color: alphaColor(token.colorWhite, 0.82), maxWidth: 720 }}>
                    统一查看系统成员、角色权限与账号状态，在同一个工作台里完成检索、维护和安全操作。
                  </Text>
                </Space>
              </Col>
              <Col xs={24} sm={12}>
                <Space size={12} style={{ display: 'flex', justifyContent: isMobile ? 'flex-start' : 'flex-end', width: '100%', flexWrap: 'wrap' }}>
                  <Button
                    icon={<ArrowLeftOutlined />}
                    onClick={() => navigate('/')}
                    style={{
                      borderRadius: 16,
                      background: alphaColor(token.colorWhite, 0.15),
                      border: `1px solid ${alphaColor(token.colorWhite, 0.3)}`,
                      boxShadow: `0 2px 8px ${alphaColor(token.colorText, 0.15)}`,
                      color: token.colorWhite,
                      backdropFilter: 'blur(10px)',
                    }}
                  >
                    返回主页
                  </Button>
                  <Button
                    type="primary"
                    icon={<PlusOutlined />}
                    onClick={() => setModalVisible(true)}
                    style={{
                      borderRadius: 16,
                      background: alphaColor(token.colorWarning, 0.95),
                      border: `1px solid ${alphaColor(token.colorWhite, 0.3)}`,
                      boxShadow: `0 4px 16px ${alphaColor(token.colorWarning, 0.4)}`,
                      color: '#211a16',
                      fontWeight: 600,
                    }}
                  >
                    添加用户
                  </Button>
                  <UserMenu />
                </Space>
              </Col>
            </Row>

            <Row gutter={[14, 14]}>
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
          </div>
        </Card>

        <Card
          bordered={false}
          style={{
            borderRadius: 24,
            border: `1px solid ${panelBorder}`,
            background: quietPanelBackground,
            boxShadow: `0 24px 48px -42px ${alphaColor(token.colorTextBase, 0.45)}`,
            flex: 1,
          }}
          styles={{
            body: {
              padding: isMobile ? 16 : 22,
              display: 'flex',
              flexDirection: 'column',
              gap: 18,
              minHeight: isMobile ? 'auto' : 'calc(100vh - 280px)',
            },
          }}
        >
          <div
            style={{
              display: 'flex',
              justifyContent: 'space-between',
              alignItems: isMobile ? 'flex-start' : 'center',
              flexDirection: isMobile ? 'column' : 'row',
              gap: 12,
            }}
          >
            <Space direction="vertical" size={4}>
              <Text style={{ fontSize: 12, letterSpacing: '0.12em', textTransform: 'uppercase', color: token.colorTextTertiary }}>
                User Workspace
              </Text>
              <Title level={4} style={{ margin: 0, fontFamily: designDisplayFont, color: token.colorTextBase }}>
                成员检索与权限维护
              </Title>
            </Space>
            <Text type="secondary" style={{ maxWidth: 620 }}>
              支持用户名、显示名和用户 ID 搜索，保留原有排序、分页和账号安全操作流程。
            </Text>
          </div>

          <Card
            bordered={false}
            style={{
              borderRadius: 20,
              background: token.colorBgContainer,
              border: `1px solid ${alphaColor(token.colorPrimary, 0.08)}`,
              flex: 1,
              display: 'flex',
              flexDirection: 'column',
              overflow: 'hidden',
            }}
            styles={{
              body: {
                padding: 0,
                height: '100%',
                display: 'flex',
                flexDirection: 'column',
                overflow: 'hidden',
              },
            }}
          >
            <div style={{
              padding: isMobile ? '16px 16px 0 16px' : '16px 24px 0 24px',
              borderBottom: `1px solid ${alphaColor(token.colorText, 0.06)}`,
            }}>
              <Input
                placeholder="搜索用户名、显示名称或用户ID"
                prefix={<SearchOutlined style={{ color: token.colorTextTertiary }} />}
                value={searchText}
                onChange={(e) => {
                  setSearchText(e.target.value);
                  setCurrentPage(1);
                }}
                allowClear
                style={{
                  borderRadius: 12,
                }}
              />
              {disabledCount > 0 ? (
                <Text style={{ display: 'block', marginTop: 10, fontSize: 12, color: token.colorTextTertiary }}>
                  当前共有 {disabledCount} 位已禁用用户，可在操作列中重新启用。
                </Text>
              ) : null}
            </div>

            <div style={{
              flex: 1,
              overflow: 'auto',
              padding: isMobile ? '16px 16px 0 16px' : '16px 24px 0 24px',
            }}>
              {userTableReady ? (
                <Suspense
                  fallback={(
                    <InlineDeferredPanel
                      eyebrow="User Table"
                      title="正在整理用户管理列表"
                      message="用户表格正在接入排序、分页与批量操作列。这里只补充轻量表格过渡，不改变用户状态和管理逻辑。"
                      tags={[
                        { label: '用户列表', color: 'blue' },
                        { label: '排序分页恢复中', color: 'processing' },
                      ]}
                    />
                  )}
                >
                  <LazyDeferredAntdTable
                    columns={columns}
                    dataSource={sortedUsers.slice((currentPage - 1) * pageSize, currentPage * pageSize)}
                    rowKey="user_id"
                    loading={loading}
                    scroll={{
                      x: 1400,
                      y: 'calc(100vh - 430px)'
                    }}
                    pagination={false}
                    onChange={(_pagination: unknown, _filters: unknown, sorter: { field?: string | number | symbol; order?: SortOrder | null } | Array<{ field?: string | number | symbol; order?: SortOrder | null }>) => {
                      const currentSorter = Array.isArray(sorter) ? sorter[0] : sorter;
                      setCurrentPage(1);

                      if (currentSorter && currentSorter.field && currentSorter.order) {
                        setSortField(currentSorter.field as SortField);
                        setSortOrder(currentSorter.order as SortOrder);
                      } else {
                        setSortField(null);
                        setSortOrder(null);
                      }
                    }}
                  />
                </Suspense>
              ) : (
                <InlineDeferredPanel
                  eyebrow="User Workspace"
                  title="正在接管用户管理列表"
                  message="系统正在准备用户表格、筛选结果与分页区域，原有账号状态、权限调整与管理操作逻辑保持不变。"
                  minHeight={240}
                  tags={[
                    { label: '用户列表接管中', color: 'processing' },
                    { label: `当前 ${filteredUsers.length} 位`, color: 'blue' },
                    { label: '管理逻辑保持原样', color: 'green' },
                  ]}
                />
              )}
            </div>

            <div style={{
              padding: isMobile ? '16px' : '16px 24px 24px 24px',
              borderTop: `1px solid ${alphaColor(token.colorText, 0.06)}`,
              background: 'transparent',
              display: 'flex',
              justifyContent: 'center',
            }}>
              <Pagination
                current={currentPage}
                pageSize={pageSize}
                total={filteredUsers.length}
                showSizeChanger
                showTotal={(total) => `共 ${total} 个用户${searchText ? ' (已过滤)' : ''}`}
                pageSizeOptions={[20, 50, 100]}
                onChange={(page, size) => {
                  setCurrentPage(page);
                  setPageSize(size);
                }}
                onShowSizeChange={(_current, size) => {
                  setCurrentPage(1);
                  setPageSize(size);
                }}
              />
            </div>
          </Card>
        </Card>
      </div>

      {/* 添加用户对话框 */}
      <Modal
        title={<span><PlusOutlined style={{ marginRight: 8 }} />添加用户</span>}
        open={modalVisible}
        onCancel={() => {
          setModalVisible(false);
          form.resetFields();
        }}
        onOk={() => form.submit()}
        width={isMobile ? '90%' : 600}
        centered
        okText="创建"
        cancelText="取消"
      >
        <Form
          form={form}
          layout="vertical"
          onFinish={handleCreate}
        >
          <Form.Item
            label="用户名"
            name="username"
            rules={[
              { required: true, message: '请输入用户名' },
              { min: 3, max: 20, message: '用户名长度3-20位' },
              { pattern: /^[a-zA-Z0-9_]+$/, message: '只能包含字母、数字和下划线' },
            ]}
          >
            <Input placeholder="请输入用户名" />
          </Form.Item>

          <Form.Item
            label="显示名称"
            name="display_name"
            rules={[
              { required: true, message: '请输入显示名称' },
              { min: 2, max: 50, message: '显示名称长度2-50位' },
            ]}
          >
            <Input placeholder="请输入显示名称" />
          </Form.Item>

          <Form.Item
            label="初始密码"
            name="password"
            extra="留空则自动生成 username@666"
            rules={[
              { min: 6, message: '密码长度至少6位' },
            ]}
          >
            <Input.Password placeholder="留空则自动生成" />
          </Form.Item>

          <Form.Item
            label="头像URL"
            name="avatar_url"
          >
            <Input placeholder="请输入头像URL（可选）" />
          </Form.Item>

          <Form.Item
            label="信任等级"
            name="trust_level"
            initialValue={0}
          >
            <InputNumber min={0} max={9} style={{ width: '100%' }} />
          </Form.Item>

          <Form.Item
            label="设为管理员"
            name="is_admin"
            valuePropName="checked"
            initialValue={false}
          >
            <Switch
              size={isMobile ? 'small' : 'default'}
              style={{
                flexShrink: 0,
                height: isMobile ? 16 : 22,
                minHeight: isMobile ? 16 : 22,
                lineHeight: isMobile ? '16px' : '22px'
              }}
            />
          </Form.Item>
        </Form>
      </Modal>

      {/* 编辑用户对话框 */}
      <Modal
        title={<span><EditOutlined style={{ marginRight: 8 }} />编辑用户</span>}
        open={editModalVisible}
        onCancel={() => {
          setEditModalVisible(false);
          editForm.resetFields();
        }}
        onOk={() => editForm.submit()}
        width={isMobile ? '90%' : 600}
        centered
        okText="保存"
        cancelText="取消"
      >
        <Form
          form={editForm}
          layout="vertical"
          onFinish={handleUpdate}
        >
          <Form.Item
            label="显示名称"
            name="display_name"
            rules={[
              { required: true, message: '请输入显示名称' },
              { min: 2, max: 50, message: '显示名称长度2-50位' },
            ]}
          >
            <Input placeholder="请输入显示名称" />
          </Form.Item>

          <Form.Item
            label="头像URL"
            name="avatar_url"
          >
            <Input placeholder="请输入头像URL（可选）" />
          </Form.Item>

          <Form.Item
            label="信任等级"
            name="trust_level"
          >
            <InputNumber min={0} max={9} style={{ width: '100%' }} />
          </Form.Item>

          <Form.Item
            label="设为管理员"
            name="is_admin"
            valuePropName="checked"
          >
            <Switch
              size={isMobile ? 'small' : 'default'}
              style={{
                flexShrink: 0,
                height: isMobile ? 16 : 22,
                minHeight: isMobile ? 16 : 22,
                lineHeight: isMobile ? '16px' : '22px'
              }}
            />
          </Form.Item>
        </Form>
      </Modal>

      {/* 重置密码对话框 */}
      <Modal
        title={<span><KeyOutlined style={{ marginRight: 8 }} />重置密码</span>}
        open={resetPasswordModalVisible}
        onCancel={() => {
          setResetPasswordModalVisible(false);
          setNewPassword('');
        }}
        onOk={handleResetPasswordConfirm}
        width={isMobile ? '90%' : 500}
        centered
        okText="确认重置"
        cancelText="取消"
      >
        <div style={{ marginBottom: 16 }}>
          <Text>用户：<Text strong>{currentUser?.username}</Text></Text>
        </div>
        <Form layout="vertical">
          <Form.Item
            label="新密码"
            extra="留空则重置为默认密码 username@666"
          >
            <Input.Password
              value={newPassword}
              onChange={(e) => setNewPassword(e.target.value)}
              placeholder="留空则使用默认密码"
            />
          </Form.Item>
        </Form>
      </Modal>
    </div>
  );
}
