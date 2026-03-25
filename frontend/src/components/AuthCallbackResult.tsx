import { Button, Result, theme } from 'antd';

type AuthCallbackResultProps = {
  status: 'success' | 'error';
  errorMessage?: string;
  showAnnouncement?: boolean;
  showPasswordModal?: boolean;
  onBackToLogin?: () => void;
};

export default function AuthCallbackResult({
  status,
  errorMessage,
  showAnnouncement = false,
  showPasswordModal = false,
  onBackToLogin,
}: AuthCallbackResultProps) {
  const { token } = theme.useToken();
  const alphaColor = (color: string, alpha: number) => `color-mix(in srgb, ${color} ${(alpha * 100).toFixed(0)}%, transparent)`;

  return (
    <div
      style={{
        display: 'flex',
        justifyContent: 'center',
        alignItems: 'center',
        minHeight: '100vh',
        background: `linear-gradient(135deg, ${token.colorPrimary} 0%, ${token.colorPrimaryHover} 100%)`,
      }}
    >
      <Result
        status={status}
        title={status === 'error' ? '登录失败' : '登录成功'}
        subTitle={status === 'error'
          ? errorMessage
          : showPasswordModal
            ? '登录成功，正在引导设置密码...'
            : showAnnouncement
              ? '登录成功，正在加载公告...'
              : '登录成功，正在跳转...'}
        extra={status === 'error' && onBackToLogin ? (
          <Button type="primary" onClick={onBackToLogin}>
            返回登录
          </Button>
        ) : undefined}
        style={{
          background: status === 'error'
            ? token.colorBgContainer
            : alphaColor(token.colorBgContainer, 0.96),
          padding: 40,
          borderRadius: 8,
        }}
      />
    </div>
  );
}
