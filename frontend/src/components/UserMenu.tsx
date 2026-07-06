import { useState, useEffect, useRef, useCallback } from 'react';
import { Dropdown, Avatar, Space, Typography, message, Modal, Form, Input, Button, theme } from 'antd';
import { UserOutlined, LogoutOutlined, TeamOutlined, CrownOutlined, LockOutlined } from '@ant-design/icons';
import { authApi } from '../services/modularApi';
import { clearAuthStatusCache } from '../utils/authStatus';
import type { User } from '../types';
import type { MenuProps } from 'antd';
import { useNavigate } from 'react-router-dom';
import { designDisplayFont } from '../theme/themeConfig';

const { Text } = Typography;
const passwordGuideSteps = [
  '先确认这次只是更新登录凭证，不会影响当前项目、角色或生成任务数据。',
  '再填写新密码与确认密码，把这次动作当作账号安全补全，而不是工作区配置修改。',
  '最后提交前看一眼当前焦点说明，确认是否仍在保存中，避免重复点击或误判提交状态。',
];

interface UserMenuProps {
  /** 是否总是显示完整信息（用于移动端侧边栏） */
  showFullInfo?: boolean;
  /** 紧凑模式（用于折叠侧边栏，仅展示头像） */
  compact?: boolean;
}

export default function UserMenu({ showFullInfo = false, compact = false }: UserMenuProps) {
  const navigate = useNavigate();
  const [currentUser, setCurrentUser] = useState<User | null>(null);
  const [showChangePassword, setShowChangePassword] = useState(false);
  const [changePasswordForm] = Form.useForm();
  const [changingPassword, setChangingPassword] = useState(false);
  const { token } = theme.useToken();
  const alphaColor = (color: string, alpha: number) => `color-mix(in srgb, ${color} ${(alpha * 100).toFixed(0)}%, transparent)`;
  const isCompactInfoHidden = compact || (window.innerWidth <= 768 && !showFullInfo);
  const userWorkspaceFocus = currentUser?.is_admin
    ? {
        title: '当前账号已经具备管理员权限，适合从这里进入工作区管理入口',
        note: '除了普通账号操作，还可以直接查看用户管理等控制能力，但当前入口仍然只负责展示和导航。',
      }
    : {
        title: '当前账号更适合作为个人创作入口使用，先从资料与密码管理切入',
        note: '这里主要承担账号信息确认、密码修改与退出登录等动作，不会改动项目内容本身。',
      };
  const passwordWorkspaceFocus = changingPassword
    ? {
        title: '当前正在提交密码修改，先等待保存完成',
        note: '现在最重要的是保持表单稳定，不需要重复点击确认修改；提交完成后会停留在当前工作区。',
      }
    : {
        title: '当前先把登录凭证更新完整，再继续返回创作工作流',
        note: '改密弹窗更适合做成一次性安全动作，完成后就可以回到原来的项目和写作页面继续工作。',
      };
  const mountedRef = useRef(true);
  const userRequestIdRef = useRef(0);
  const passwordRequestIdRef = useRef(0);
  const logoutRequestIdRef = useRef(0);

  useEffect(() => {
    mountedRef.current = true;
    return () => {
      mountedRef.current = false;
      userRequestIdRef.current += 1;
      passwordRequestIdRef.current += 1;
      logoutRequestIdRef.current += 1;
    };
  }, []);

  const beginRequest = useCallback((ref: React.MutableRefObject<number>) => {
    ref.current += 1;
    return ref.current;
  }, []);

  const isRequestActive = useCallback((ref: React.MutableRefObject<number>, requestId: number) => {
    return mountedRef.current && ref.current === requestId;
  }, []);

  useEffect(() => {
    void loadCurrentUser();
  }, []);

  const loadCurrentUser = async () => {
    const requestId = beginRequest(userRequestIdRef);
    try {
      const user = await authApi.getCurrentUser();
      if (!isRequestActive(userRequestIdRef, requestId)) {
        return;
      }
      setCurrentUser(user);
    } catch (error) {
      if (!isRequestActive(userRequestIdRef, requestId)) {
        return;
      }
      console.error('获取用户信息失败:', error);
    }
  };

  const handleLogout = async () => {
    const requestId = beginRequest(logoutRequestIdRef);
    try {
      await authApi.logout();
      if (!isRequestActive(logoutRequestIdRef, requestId)) {
        return;
      }
      clearAuthStatusCache();
      message.success('已退出登录');
      window.location.href = '/login';
    } catch (error) {
      if (!isRequestActive(logoutRequestIdRef, requestId)) {
        return;
      }
      console.error('退出登录失败:', error);
      message.error('退出登录失败');
    }
  };

  const handleShowUserManagement = () => {
    if (!currentUser?.is_admin) {
      message.warning('只有管理员可以访问用户管理');
      return;
    }
    navigate('/user-management');
  };

  const handleChangePassword = async (values: { oldPassword: string; newPassword: string }) => {
    const requestId = beginRequest(passwordRequestIdRef);
    try {
      setChangingPassword(true);
      await authApi.setPassword(values.newPassword);
      if (!isRequestActive(passwordRequestIdRef, requestId)) {
        return;
      }
      message.success('密码修改成功');
      setShowChangePassword(false);
      changePasswordForm.resetFields();
    } catch (error: unknown) {
      if (!isRequestActive(passwordRequestIdRef, requestId)) {
        return;
      }
      console.error('修改密码失败:', error);
      const err = error as { response?: { data?: { detail?: string } } };
      message.error(err.response?.data?.detail || '修改密码失败');
    } finally {
      if (isRequestActive(passwordRequestIdRef, requestId)) {
        setChangingPassword(false);
      }
    }
  };

  const menuItems: MenuProps['items'] = [
    {
      key: 'user-info',
      label: (
        <div style={{ padding: '10px 0 6px' }}>
          <Text style={{ display: 'block', fontSize: 11, letterSpacing: '0.08em', textTransform: 'uppercase', color: token.colorTextTertiary }}>
            Workspace Account
          </Text>
          <Text strong style={{ display: 'block', fontSize: 15, marginTop: 4 }}>
            {currentUser?.display_name || currentUser?.username}
          </Text>
          <Text type="secondary" style={{ display: 'block', fontSize: 12, marginTop: 2, lineHeight: 1.6 }}>
            Trust Level {currentUser?.trust_level}
            {currentUser?.is_admin && ' · 管理员权限已启用'}
          </Text>
        </div>
      ),
      disabled: true,
    },
    {
      type: 'divider',
    },
    ...(currentUser?.is_admin ? [
      {
        key: 'user-management',
        icon: <TeamOutlined />,
        label: '用户管理',
        onClick: handleShowUserManagement,
      },
      {
        type: 'divider' as const,
      }
    ] : []),
    {
      key: 'change-password',
      icon: <LockOutlined />,
      label: '修改密码',
      onClick: () => setShowChangePassword(true),
    },
    {
      type: 'divider',
    },
    {
      key: 'logout',
      icon: <LogoutOutlined />,
      label: '退出登录',
      onClick: handleLogout,
    },
  ];

  if (!currentUser) {
    return null;
  }

  return (
    <>
      <Dropdown
        menu={{ items: menuItems }}
        placement="bottomRight"
        trigger={['click']}
      >
        <div
          style={{
            cursor: 'pointer',
            display: 'flex',
            alignItems: 'center',
            gap: compact ? 0 : 12,
            padding: compact ? '6px' : '10px 16px',
            background: `linear-gradient(135deg, ${alphaColor(token.colorBgContainer, 0.96)} 0%, ${alphaColor(token.colorPrimaryBg, 0.88)} 100%)`,
            backdropFilter: 'blur(18px)',
            WebkitBackdropFilter: 'blur(18px)',
            borderRadius: compact ? 18 : 28,
            border: `1px solid ${alphaColor(token.colorPrimary, 0.14)}`,
            transition: 'all 0.3s ease',
            boxShadow: `0 16px 36px ${alphaColor(token.colorText, 0.1)}`,
            minWidth: compact ? 'auto' : 212,
            position: 'relative',
            overflow: 'hidden',
          }}
          title={userWorkspaceFocus.note}
          onMouseEnter={(e) => {
            e.currentTarget.style.background = `linear-gradient(135deg, ${token.colorBgContainer} 0%, ${alphaColor(token.colorPrimaryBgHover, 0.96)} 100%)`;
            e.currentTarget.style.transform = 'translateY(-2px)';
            e.currentTarget.style.boxShadow = `0 22px 46px ${alphaColor(token.colorText, 0.14)}`;
          }}
          onMouseLeave={(e) => {
            e.currentTarget.style.background = `linear-gradient(135deg, ${alphaColor(token.colorBgContainer, 0.96)} 0%, ${alphaColor(token.colorPrimaryBg, 0.88)} 100%)`;
            e.currentTarget.style.transform = 'translateY(0)';
            e.currentTarget.style.boxShadow = `0 16px 36px ${alphaColor(token.colorText, 0.1)}`;
          }}
        >
          {!compact && (
            <div
              style={{
                position: 'absolute',
                inset: 0,
                background: `radial-gradient(circle at top right, ${alphaColor(token.colorPrimary, 0.14)} 0%, transparent 42%)`,
                pointerEvents: 'none',
              }}
            />
          )}
          <div style={{ position: 'relative' }}>
            <Avatar
              src={currentUser.avatar_url}
              icon={<UserOutlined />}
              size={compact ? 32 : 40}
              style={{
                backgroundColor: token.colorPrimary,
                border: `3px solid ${token.colorWhite}`,
                boxShadow: `0 10px 24px ${alphaColor(token.colorText, 0.14)}`,
              }}
            />
            {currentUser.is_admin && (
              <div style={{
                position: 'absolute',
                bottom: -2,
                right: -2,
                width: 18,
                height: 18,
                background: `linear-gradient(135deg, ${token.colorWarning} 0%, ${token.colorWarningHover} 100%)`,
                borderRadius: '50%',
                display: 'flex',
                alignItems: 'center',
                justifyContent: 'center',
                border: `2px solid ${token.colorWhite}`,
                boxShadow: `0 2px 4px ${alphaColor(token.colorText, 0.2)}`,
              }}>
                <CrownOutlined style={{ fontSize: 9, color: token.colorWhite }} />
              </div>
            )}
          </div>
          <Space direction="vertical" size={1} style={{ display: isCompactInfoHidden ? 'none' : 'flex', position: 'relative' }}>
            <Text style={{
              color: token.colorPrimary,
              fontSize: 10,
              lineHeight: '14px',
              letterSpacing: '0.14em',
              textTransform: 'uppercase',
              fontFamily: designDisplayFont,
            }}>
              Account Hub
            </Text>
            <Text style={{
              color: token.colorTextTertiary,
              fontSize: 11,
              lineHeight: '16px',
              letterSpacing: '0.08em',
              textTransform: 'uppercase',
            }}>
              Workspace
            </Text>
            <Text strong style={{
              color: token.colorText,
              fontSize: 14,
              lineHeight: '20px',
            }}>
              {currentUser.display_name || currentUser.username}
            </Text>
            <Text style={{
              color: token.colorTextSecondary,
              fontSize: 12,
              lineHeight: '18px',
            }}>
              {currentUser.is_admin ? 'Admin access enabled' : `Trust Level ${currentUser.trust_level}`}
            </Text>
            <div style={{ display: 'flex', flexWrap: 'wrap', gap: 6, marginTop: 2 }}>
              <span
                style={{
                  display: 'inline-flex',
                  alignItems: 'center',
                  padding: '2px 8px',
                  borderRadius: 999,
                  background: alphaColor(token.colorBgContainer, 0.9),
                  border: `1px solid ${alphaColor(token.colorPrimary, 0.12)}`,
                  fontSize: 11,
                  color: token.colorTextSecondary,
                }}
              >
                {currentUser.is_admin ? '管理视角' : '个人视角'}
              </span>
              <span
                style={{
                  display: 'inline-flex',
                  alignItems: 'center',
                  padding: '2px 8px',
                  borderRadius: 999,
                  background: alphaColor(token.colorBgContainer, 0.9),
                  border: `1px solid ${alphaColor(token.colorPrimary, 0.12)}`,
                  fontSize: 11,
                  color: token.colorTextSecondary,
                }}
              >
                {currentUser.display_name ? '已同步昵称' : '使用账号名'}
              </span>
            </div>
          </Space>
        </div>
      </Dropdown>

      <Modal
        title={(
          <Space direction="vertical" size={2}>
            <Text style={{ fontSize: 12, letterSpacing: '0.08em', textTransform: 'uppercase', color: token.colorTextTertiary }}>
              First Login Setup
            </Text>
            <Text strong style={{ fontSize: 18 }}>
              修改登录密码
            </Text>
            <Text type="secondary" style={{ fontSize: 13 }}>
              更新当前账号的登录凭证，不影响其它业务配置与项目数据。
            </Text>
          </Space>
        )}
        open={showChangePassword}
        onCancel={() => {
          setShowChangePassword(false);
          changePasswordForm.resetFields();
        }}
        footer={null}
        width={480}
        centered
        styles={{
          header: {
            paddingBottom: 8,
            borderBottom: `1px solid ${alphaColor(token.colorBorderSecondary, 0.9)}`,
          },
          body: {
            paddingTop: 20,
          },
        }}
      >
        <Form
          form={changePasswordForm}
          layout="vertical"
          onFinish={handleChangePassword}
          autoComplete="off"
        >
          <div
            style={{
              marginBottom: 18,
              padding: '16px 18px',
              borderRadius: 18,
              background: `linear-gradient(135deg, ${alphaColor(token.colorPrimaryBg, 0.92)} 0%, ${alphaColor(token.colorBgElevated, 0.96)} 100%)`,
              border: `1px solid ${alphaColor(token.colorPrimary, 0.12)}`,
            }}
          >
            <div
              style={{
                display: 'grid',
                gridTemplateColumns: 'repeat(auto-fit, minmax(200px, 1fr))',
                gap: 16,
              }}
            >
              <div>
                <Text style={{ display: 'block', fontSize: 11, letterSpacing: '0.08em', textTransform: 'uppercase', color: token.colorTextTertiary, marginBottom: 6 }}>
                  Password Guide
                </Text>
                <Text strong style={{ display: 'block', fontSize: 16, marginBottom: 8, fontFamily: designDisplayFont }}>
                  改密操作顺序
                </Text>
                <Text type="secondary" style={{ display: 'block', lineHeight: 1.7, marginBottom: 12 }}>
                  这里现在只增强阅读顺序和当前焦点提示，不改变密码校验、提交、取消关闭或错误提示逻辑。
                </Text>
                <div style={{ display: 'flex', flexWrap: 'wrap', gap: 8 }}>
                  {passwordGuideSteps.map((item, index) => (
                    <span
                      key={item}
                      style={{
                        display: 'inline-flex',
                        alignItems: 'center',
                        gap: 8,
                        padding: '6px 12px',
                        borderRadius: 999,
                        background: token.colorBgContainer,
                        border: `1px solid ${alphaColor(token.colorPrimary, 0.12)}`,
                        color: token.colorText,
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
                  borderRadius: 16,
                  padding: '16px 18px',
                  background: alphaColor(token.colorBgContainer, 0.96),
                  border: `1px solid ${alphaColor(token.colorPrimary, 0.12)}`,
                }}
              >
                <Text style={{ display: 'block', fontSize: 11, letterSpacing: '0.08em', textTransform: 'uppercase', color: token.colorTextTertiary, marginBottom: 6 }}>
                  当前工作焦点
                </Text>
                <Text strong style={{ display: 'block', fontSize: 15, marginBottom: 8 }}>
                  {passwordWorkspaceFocus.title}
                </Text>
                <Text type="secondary" style={{ display: 'block', lineHeight: 1.7, marginBottom: 12 }}>
                  {passwordWorkspaceFocus.note}
                </Text>
                <Space wrap>
                  <Button size="small" type={changingPassword ? 'primary' : 'default'}>
                    {changingPassword ? '保存中' : '待提交'}
                  </Button>
                  <Button size="small" type="text">
                    当前账号: {currentUser?.display_name || currentUser?.username}
                  </Button>
                </Space>
              </div>
            </div>
          </div>

          <div
            style={{
              marginBottom: 20,
              padding: '14px 16px',
              borderRadius: 16,
              background: `linear-gradient(135deg, ${alphaColor(token.colorPrimaryBg, 0.92)} 0%, ${alphaColor(token.colorBgElevated, 0.96)} 100%)`,
              border: `1px solid ${alphaColor(token.colorPrimary, 0.12)}`,
            }}
          >
            <Text style={{ color: token.colorTextSecondary, fontSize: 13, lineHeight: 1.7 }}>
              为了保证工作区安全，密码至少需要 6 个字符。建议使用便于记忆但不易猜测的组合。
            </Text>
          </div>

          <Form.Item
            label="新密码"
            name="newPassword"
            rules={[
              { required: true, message: '请输入新密码' },
              { min: 6, message: '密码至少6个字符' },
            ]}
          >
            <Input.Password
              prefix={<LockOutlined />}
              placeholder="请输入新密码（至少6个字符）"
              autoComplete="new-password"
            />
          </Form.Item>

          <Form.Item
            label="确认密码"
            name="confirmPassword"
            dependencies={['newPassword']}
            rules={[
              { required: true, message: '请确认新密码' },
              ({ getFieldValue }) => ({
                validator(_, value) {
                  if (!value || getFieldValue('newPassword') === value) {
                    return Promise.resolve();
                  }
                  return Promise.reject(new Error('两次输入的密码不一致'));
                },
              }),
            ]}
          >
            <Input.Password
              prefix={<LockOutlined />}
              placeholder="请再次输入新密码"
              autoComplete="new-password"
            />
          </Form.Item>

          <Form.Item style={{ marginBottom: 0 }}>
            <Space style={{ width: '100%', justifyContent: 'flex-end' }}>
              <Button onClick={() => {
                setShowChangePassword(false);
                changePasswordForm.resetFields();
              }}>
                取消
              </Button>
              <Button type="primary" htmlType="submit" loading={changingPassword}>
                确认修改
              </Button>
            </Space>
          </Form.Item>
        </Form>
      </Modal>
    </>
  );
}
