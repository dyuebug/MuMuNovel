import { Input, Modal, theme } from 'antd';

type PasswordStatus = {
  username: string;
  default_password: string;
};

type PasswordSetupModalProps = {
  open: boolean;
  settingPassword: boolean;
  passwordStatus: PasswordStatus | null;
  newPassword: string;
  confirmPassword: string;
  onNewPasswordChange: (value: string) => void;
  onConfirmPasswordChange: (value: string) => void;
  onOk: () => void;
  onCancel: () => void;
};

export default function PasswordSetupModal({
  open,
  settingPassword,
  passwordStatus,
  newPassword,
  confirmPassword,
  onNewPasswordChange,
  onConfirmPasswordChange,
  onOk,
  onCancel,
}: PasswordSetupModalProps) {
  const { token } = theme.useToken();

  return (
    <Modal
      title="设置登录密码"
      open={open}
      centered
      onOk={onOk}
      onCancel={onCancel}
      confirmLoading={settingPassword}
      okText="确认设置"
      cancelText="取消"
      width={500}
    >
      <div style={{ marginBottom: 20 }}>
        <p>检测到你正在使用 Linux DO 登录。</p>
        <p>为了后续通过用户名密码登录，请先为当前账号设置一个本地密码。</p>
        {passwordStatus?.default_password ? (
          <div
            style={{
              background: token.colorFillTertiary,
              padding: 12,
              borderRadius: 4,
              marginTop: 12,
            }}
          >
            <strong>当前账号：</strong>{passwordStatus.username}<br />
            <strong>初始密码：</strong>
            <code
              style={{
                background: token.colorBgContainer,
                padding: '2px 8px',
                borderRadius: 3,
                color: token.colorPrimary,
                fontSize: 14,
              }}
            >
              {passwordStatus.default_password}
            </code>
          </div>
        ) : null}
      </div>

      <div style={{ marginTop: 20 }}>
        <div style={{ marginBottom: 12 }}>
          <label>新密码（至少 6 位）</label>
          <Input.Password
            value={newPassword}
            onChange={(event) => onNewPasswordChange(event.target.value)}
            placeholder="请输入新密码"
            style={{ marginTop: 4 }}
          />
        </div>
        <div>
          <label>确认密码</label>
          <Input.Password
            value={confirmPassword}
            onChange={(event) => onConfirmPasswordChange(event.target.value)}
            placeholder="请再次输入密码"
            style={{ marginTop: 4 }}
          />
        </div>
      </div>
    </Modal>
  );
}
