import { Suspense, lazy, useEffect, useRef, useState } from 'react';
import { useNavigate } from 'react-router-dom';
import { Card, Col, Row, Typography, message, theme } from 'antd';
import { authApi } from '../services/modularApi';
import { clearAuthStatusCache } from '../utils/authStatus';
import { consumeLoginRedirect } from '../utils/loginRedirect';
import InlineDeferredPanel from '../components/InlineDeferredPanel';
import LoadingScreen from '../components/LoadingScreen';
import WorkflowEntryFallback from '../components/WorkflowEntryFallback';
import { designDisplayFont } from '../theme/themeConfig';
const LazyAnnouncementModal = lazy(() => import('../components/AnnouncementModal'));
const LazyPasswordSetupModal = lazy(() => import('../components/PasswordSetupModal'));
const LazyAuthCallbackResult = lazy(() => import('../components/AuthCallbackResult'));


export default function AuthCallback() {
  const navigate = useNavigate();
  const [status, setStatus] = useState<'loading' | 'success' | 'error'>('loading');
  const [errorMessage, setErrorMessage] = useState('');
  const [showAnnouncement, setShowAnnouncement] = useState(false);
  const [showPasswordModal, setShowPasswordModal] = useState(false);
  const { token } = theme.useToken();
  const alphaColor = (color: string, alpha: number) => `color-mix(in srgb, ${color} ${(alpha * 100).toFixed(0)}%, transparent)`;
  const editorialInk = '#f7f1e8';
  const pageBackground = `linear-gradient(180deg, ${alphaColor(token.colorPrimary, 0.08)} 0%, color-mix(in srgb, ${token.colorBgLayout} 92%, ${token.colorPrimary} 8%) 100%)`;
  const heroBackground = `linear-gradient(135deg, #171411 0%, color-mix(in srgb, #171411 58%, ${token.colorPrimary} 42%) 100%)`;
  const { Title, Paragraph, Text } = Typography;
  interface PasswordStatus {
    has_password: boolean;
    has_custom_password: boolean;
    username: string;
    default_password: string;
  }
  const [passwordStatus, setPasswordStatus] = useState<PasswordStatus | null>(null);
  const [newPassword, setNewPassword] = useState('');
  const [confirmPassword, setConfirmPassword] = useState('');
  const [settingPassword, setSettingPassword] = useState(false);
  const redirectRef = useRef('/');
  const redirectResolvedRef = useRef(false);
  const mountedRef = useRef(true);
  const requestIdRef = useRef(0);
  const callbackSteps = [
    '校验当前认证状态',
    '恢复登录前目标地址',
    '判断是否需要公告或首次密码初始化',
  ];

  useEffect(() => {
    return () => {
      mountedRef.current = false;
      requestIdRef.current += 1;
    };
  }, []);

  const beginRequest = () => {
    requestIdRef.current += 1;
    return requestIdRef.current;
  };

  const isRequestActive = (requestId: number) => mountedRef.current && requestIdRef.current === requestId;

  const resolveRedirect = (): string => {
    if (!redirectResolvedRef.current) {
      redirectRef.current = consumeLoginRedirect();
      redirectResolvedRef.current = true;
    }

    return redirectRef.current;
  };

  useEffect(() => {
    const handleCallback = async () => {
      const requestId = beginRequest();
      try {
        // 后端会通过 Cookie 自动设置认证信息
        // 这里只需要验证登录状态
        const currentUser = await authApi.getCurrentUser();
        if (!isRequestActive(requestId)) {
          return;
        }

        // 检查是否是首次登录（优先 Cookie，兼容查询参数兜底）
        const callbackSearchParams = new URLSearchParams(window.location.search);
        const isFirstLogin = document.cookie.includes('first_login=true')
          || callbackSearchParams.get('first_login') === 'true'
          || callbackSearchParams.get('first_login') === '1';
        
        setStatus('success');

        if (isFirstLogin) {
          // 首次登录：生成默认密码并显示提示
          const defaultPassword = `${currentUser.username}@666`;
          const pwdStatus = {
            has_password: false,
            has_custom_password: false,
            username: currentUser.username,
            default_password: defaultPassword
          };
          setPasswordStatus(pwdStatus);

          // 清除首次登录标记 Cookie
          document.cookie = 'first_login=; path=/; max-age=0';

          // 显示密码初始化弹窗
          setTimeout(() => {
            if (isRequestActive(requestId)) {
              setShowPasswordModal(true);
            }
          }, 1000);
          return;
        }

        // 非首次登录：正常流程
        // 从 sessionStorage 获取重定向地址
        const redirect = resolveRedirect();

        // 检查是否永久隐藏公告或今日已隐藏
        const hideForever = localStorage.getItem('announcement_hide_forever');
        const hideToday = localStorage.getItem('announcement_hide_today');
        const today = new Date().toDateString();

        if (hideForever === 'true' || hideToday === today) {
          // 延迟一下再跳转，让用户看到成功提示
          setTimeout(() => {
            if (isRequestActive(requestId)) {
              clearAuthStatusCache();
              navigate(redirect);
            }
          }, 1000);
        } else {
          // 延迟一下再显示公告，让用户看到成功提示
          setTimeout(() => {
            if (isRequestActive(requestId)) {
              setShowAnnouncement(true);
            }
          }, 1000);
        }
      } catch (error) {
        if (!isRequestActive(requestId)) {
          return;
        }
        console.error('登录失败:', error);
        setStatus('error');
        setErrorMessage('登录失败，请重试');
      }
    };

    handleCallback();
  }, [navigate]);

  if (status === 'loading') {
    return (
      <div style={{
        display: 'flex',
        justifyContent: 'center',
        alignItems: 'center',
        minHeight: '100vh',
        background: pageBackground,
        padding: '24px 16px',
      }}>
        <Card
          bordered={false}
          style={{
            width: '100%',
            maxWidth: 720,
            borderRadius: 28,
            overflow: 'hidden',
            background: heroBackground,
            boxShadow: `0 32px 68px -42px ${alphaColor(token.colorTextBase, 0.55)}`,
          }}
          styles={{ body: { padding: 0 } }}
        >
          <div style={{ position: 'relative', padding: '28px' }}>
            <div
              style={{
                position: 'absolute',
                inset: 0,
                background: 'radial-gradient(circle at top right, rgba(255,255,255,0.14), transparent 32%)',
                pointerEvents: 'none',
              }}
            />
            <div style={{ position: 'relative', display: 'flex', flexDirection: 'column', gap: 20 }}>
              <div>
                <Text style={{ color: alphaColor(token.colorWhite, 0.68), letterSpacing: '0.14em', textTransform: 'uppercase' }}>
                  Callback Bridge
                </Text>
                <Title level={2} style={{ margin: '8px 0 0', color: editorialInk, fontFamily: designDisplayFont, letterSpacing: '-0.03em' }}>
                  正在处理登录
                </Title>
                <Paragraph style={{ margin: '10px 0 0', color: alphaColor(token.colorWhite, 0.82), maxWidth: 560 }}>
                  正在校验认证状态、恢复跳转目标，并准备后续的公告或首次登录初始化流程。
                </Paragraph>
              </div>

              <Card
                bordered={false}
                style={{
                  borderRadius: 22,
                  background: alphaColor(token.colorBgContainer, 0.96),
                }}
                styles={{ body: { padding: 28 } }}
              >
                <Row gutter={[20, 20]} align="middle">
                  <Col xs={24} lg={10}>
                    <InlineDeferredPanel
                      eyebrow="Callback Status"
                      title="等待认证结果回流"
                      message="当前正在等待回调结果写回、账号状态确认以及跳转目标恢复。这一步不会更改现有登录逻辑，只负责把你安全送回原工作流。"
                      minHeight={240}
                      tags={[
                        { label: '认证状态核对中', color: 'processing' },
                        { label: '回跳地址恢复', color: 'blue' },
                        { label: '首次登录分流', color: 'default' },
                      ]}
                    />
                  </Col>
                  <Col xs={24} lg={14}>
                    <div
                      style={{
                        borderRadius: 18,
                        padding: '16px 18px',
                        background: alphaColor(token.colorPrimary, 0.04),
                        border: `1px solid ${alphaColor(token.colorPrimary, 0.12)}`,
                      }}
                    >
                      <Text style={{ display: 'block', color: token.colorTextTertiary, letterSpacing: '0.12em', textTransform: 'uppercase', fontSize: 12 }}>
                        Callback Guide
                      </Text>
                      <Paragraph style={{ margin: '10px 0 12px', lineHeight: 1.75 }}>
                        登录回调页更像一座桥接站：它只负责确认身份、恢复去向，并根据首次登录状态决定是否进入后续提示流程。
                      </Paragraph>
                      <div style={{ display: 'flex', flexDirection: 'column', gap: 8 }}>
                        {callbackSteps.map((item, index) => (
                          <div
                            key={item}
                            style={{
                              display: 'flex',
                              alignItems: 'center',
                              gap: 10,
                              padding: '8px 10px',
                              borderRadius: 14,
                              background: alphaColor(token.colorBgContainer, 0.72),
                              border: `1px solid ${alphaColor(token.colorPrimary, 0.08)}`,
                            }}
                          >
                            <span style={{ color: token.colorPrimary, fontWeight: 700 }}>{index + 1}</span>
                            <Text style={{ color: token.colorTextSecondary }}>{item}</Text>
                          </div>
                        ))}
                      </div>
                    </div>
                  </Col>
                </Row>
              </Card>
            </div>
          </div>
        </Card>
      </div>
    );
  }

  if (status === 'error') {
    return (
      <Suspense fallback={<LoadingScreen message="正在整理登录回调结果..." minHeight="100vh" />}>
        <LazyAuthCallbackResult
          status="error"
          errorMessage={errorMessage}
          onBackToLogin={() => navigate('/login')}
        />
      </Suspense>
    );
  }

  const handleAnnouncementClose = () => {
    setShowAnnouncement(false);
    const redirect = resolveRedirect();
    clearAuthStatusCache();
    navigate(redirect);
  };

  const handleDoNotShowToday = () => {
    // 设置今日不再显示
    const today = new Date().toDateString();
    localStorage.setItem('announcement_hide_today', today);
  };

  const handleNeverShow = () => {
    // 设置永久不再显示
    localStorage.setItem('announcement_hide_forever', 'true');
  };

  const handleSetPassword = async () => {
    // 如果没有输入新密码，使用默认密码
    const passwordToSet = newPassword || passwordStatus?.default_password;
    
    if (!passwordToSet) {
      message.error('请输入新密码');
      return;
    }
    if (passwordToSet.length < 6) {
      message.error('密码长度至少为6个字符');
      return;
    }
    if (newPassword && newPassword !== confirmPassword) {
      message.error('两次输入的密码不一致');
      return;
    }

    setSettingPassword(true);
    const requestId = beginRequest();
    try {
      // 首次登录使用初始化接口，后续使用修改接口
      const isFirstLogin = !passwordStatus?.has_password;
      if (isFirstLogin) {
        await authApi.initializePassword(passwordToSet);
        message.success('密码初始化成功');
      } else {
        await authApi.setPassword(passwordToSet);
        message.success('密码设置成功');
      }
      setShowPasswordModal(false);
      if (!isRequestActive(requestId)) {
        return;
      }

      // 继续后续流程
      const redirect = resolveRedirect();

      const hideForever = localStorage.getItem('announcement_hide_forever');
      const hideToday = localStorage.getItem('announcement_hide_today');
      const today = new Date().toDateString();

      if (hideForever === 'true' || hideToday === today) {
        setTimeout(() => {
          if (isRequestActive(requestId)) {
            clearAuthStatusCache();
            navigate(redirect);
          }
        }, 500);
      } else {
        setTimeout(() => {
          if (isRequestActive(requestId)) {
            setShowAnnouncement(true);
          }
        }, 500);
      }
    } catch {
      if (!isRequestActive(requestId)) {
        return;
      }
      message.error('密码设置失败，请重试');
    } finally {
      if (isRequestActive(requestId)) {
        setSettingPassword(false);
      }
    }
  };

  const handleSkipPasswordSetting = async () => {
    const requestId = beginRequest();
    // 首次登录时，如果跳过设置，使用默认密码初始化
    const isFirstLogin = !passwordStatus?.has_password;
    if (isFirstLogin && passwordStatus?.default_password) {
      try {
        await authApi.initializePassword(passwordStatus.default_password);
      } catch (error) {
        console.error('初始化默认密码失败:', error);
      }
    }

    setShowPasswordModal(false);
    if (!isRequestActive(requestId)) {
      return;
    }

    // 继续后续流程
    const redirect = resolveRedirect();

    const hideForever = localStorage.getItem('announcement_hide_forever');
    const hideToday = localStorage.getItem('announcement_hide_today');
    const today = new Date().toDateString();

    if (hideForever === 'true' || hideToday === today) {
      setTimeout(() => {
        if (isRequestActive(requestId)) {
          clearAuthStatusCache();
          navigate(redirect);
        }
      }, 500);
    } else {
      setTimeout(() => {
        if (isRequestActive(requestId)) {
          setShowAnnouncement(true);
        }
      }, 500);
    }
  };

  return (
    <>
      {showAnnouncement ? (
        <Suspense
          fallback={(
            <WorkflowEntryFallback
              eyebrow="Workspace Notice"
              title="正在展开登录后的公告说明"
              message="系统正在接入登录成功后的公告内容与确认入口，原有关闭与忽略逻辑保持不变。"
              tags={[
                { label: '登录后公告', color: 'blue' },
                { label: '说明层恢复中', color: 'processing' },
                { label: '行为逻辑保持原样', color: 'green' },
              ]}
            />
          )}
        >
          <LazyAnnouncementModal
            visible={showAnnouncement}
            onClose={handleAnnouncementClose}
            onDoNotShowToday={handleDoNotShowToday}
            onNeverShow={handleNeverShow}
          />
        </Suspense>
      ) : null}

      {showPasswordModal ? (
        <Suspense
          fallback={(
            <WorkflowEntryFallback
              eyebrow="Password Setup"
              title="正在展开密码设置面板"
              message="系统正在恢复首次登录后的密码设置入口，原有设置、跳过与校验逻辑保持不变。"
              tags={[
                { label: '密码设置', color: 'gold' },
                { label: '首次登录引导', color: 'processing' },
                { label: '校验逻辑保持原样', color: 'green' },
              ]}
            />
          )}
        >
          <LazyPasswordSetupModal
            open={showPasswordModal}
            settingPassword={settingPassword}
            passwordStatus={passwordStatus ? {
              username: passwordStatus.username,
              default_password: passwordStatus.default_password,
            } : null}
            newPassword={newPassword}
            confirmPassword={confirmPassword}
            onNewPasswordChange={setNewPassword}
            onConfirmPasswordChange={setConfirmPassword}
            onOk={handleSetPassword}
            onCancel={handleSkipPasswordSetting}
          />
        </Suspense>
      ) : null}

      <Suspense fallback={<LoadingScreen message="正在整理登录回调结果..." minHeight="100vh" />}>
        <LazyAuthCallbackResult
          status="success"
          showAnnouncement={showAnnouncement}
          showPasswordModal={showPasswordModal}
        />
      </Suspense>
    </>
  );
}
