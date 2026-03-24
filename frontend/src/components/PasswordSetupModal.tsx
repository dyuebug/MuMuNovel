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
      title="??????"
      open={open}
      centered
      onOk={onOk}
      onCancel={onCancel}
      confirmLoading={settingPassword}
      okText="????"
      cancelText="????"
      width={500}
    >
      <div style={{ marginBottom: 20 }}>
        <p>?????? Linux DO ?????</p>
        <p>????????????????????????????????????</p>
        {passwordStatus?.default_password ? (
          <div
            style={{
              background: token.colorFillTertiary,
              padding: 12,
              borderRadius: 4,
              marginTop: 12,
            }}
          >
            <strong>???</strong>{passwordStatus.username}<br />
            <strong>?????</strong>
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
          <label>??????6?????</label>
          <Input.Password
            value={newPassword}
            onChange={(event) => onNewPasswordChange(event.target.value)}
            placeholder="??????"
            style={{ marginTop: 4 }}
          />
        </div>
        <div>
          <label>?????</label>
          <Input.Password
            value={confirmPassword}
            onChange={(event) => onConfirmPasswordChange(event.target.value)}
            placeholder="???????"
            style={{ marginTop: 4 }}
          />
        </div>
      </div>
    </Modal>
  );
}
