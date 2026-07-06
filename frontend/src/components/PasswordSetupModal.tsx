import { Card, Input, Modal, Space, Tag, Typography, theme } from 'antd';
import { designDisplayFont } from '../theme/themeConfig';

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
  const alphaColor = (color: string, alpha: number) => `color-mix(in srgb, ${color} ${(alpha * 100).toFixed(0)}%, transparent)`;
  const editorialInk = '#f7f1e8';
  const heroBackground = `linear-gradient(135deg, #171411 0%, color-mix(in srgb, #171411 58%, ${token.colorPrimary} 42%) 100%)`;
  const quietPanelBackground = `linear-gradient(180deg, color-mix(in srgb, ${token.colorBgContainer} 95%, ${token.colorFillAlter} 5%) 0%, color-mix(in srgb, ${token.colorBgContainer} 87%, ${token.colorFillAlter} 13%) 100%)`;
  const { Title, Paragraph, Text } = Typography;
  const passwordGuideSteps = [
    '先确认当前账号和初始密码说明，明确这一步是在补齐本地登录方式，而不是修改业务资料。',
    '再填写新密码和确认密码，把这次设置当作首次登录后的安全补全动作一次完成。',
    '最后提交前再看一眼当前焦点卡，确认是否仍在保存中，避免重复点击或误以为没有生效。',
  ];
  const passwordWorkspaceFocus = settingPassword
    ? {
        title: '当前正在提交本地密码设置，先等待保存完成',
        note: '这时最重要的是保持表单状态稳定，不需要重复点击确认按钮；保存完成后会回到正常登录路径。',
      }
    : !passwordStatus?.default_password
      ? {
          title: '当前没有展示初始密码，先直接完成新密码设置',
          note: '这次弹窗更适合作为一次快速补全步骤，重点是把本地密码设置好，而不是停留在账号说明里。',
        }
      : !newPassword && !confirmPassword
        ? {
            title: `先为账号 ${passwordStatus.username} 完成首次本地密码补齐`,
            note: '当前适合先读完账号摘要，再一次性填写新密码和确认密码，完成后就可以用本地密码继续登录。',
          }
        : newPassword !== confirmPassword
          ? {
              title: '当前两次密码输入还没有对齐，先回到表单校准内容',
              note: '这一步最值得优先关注确认密码是否一致，避免把欢迎式设置流程拖成多次重复提交。',
            }
          : {
              title: '当前密码表单已经基本就绪，可以准备提交设置',
              note: '现在更适合最后确认密码强度和一致性，然后完成这次本地登录方式补齐。',
            };

  return (
    <Modal
      title={
        <Space direction="vertical" size={6} style={{ width: '100%' }}>
          <Tag
            bordered={false}
            style={{
              alignSelf: 'center',
              borderRadius: 999,
              paddingInline: 12,
              lineHeight: '28px',
              background: alphaColor(token.colorPrimary, 0.12),
              color: token.colorPrimary,
            }}
          >
            First Login Setup
          </Tag>
          <Title level={4} style={{ margin: 0, textAlign: 'center', fontFamily: designDisplayFont, letterSpacing: '-0.03em' }}>
            设置登录密码
          </Title>
        </Space>
      }
      open={open}
      centered
      onOk={onOk}
      onCancel={onCancel}
      confirmLoading={settingPassword}
      okText="确认设置"
      cancelText="取消"
      width={500}
      okButtonProps={{ style: { borderRadius: 12 } }}
      cancelButtonProps={{ style: { borderRadius: 12 } }}
      styles={{
        body: {
          background: quietPanelBackground,
          padding: 20,
        },
      }}
    >
      <Card
        bordered={false}
        style={{
          marginBottom: 16,
          borderRadius: 20,
          overflow: 'hidden',
          background: heroBackground,
        }}
        styles={{ body: { padding: 20 } }}
      >
        <Text style={{ color: alphaColor(token.colorWhite, 0.68), letterSpacing: '0.14em', textTransform: 'uppercase' }}>
          Local Password Bridge
        </Text>
        <Title level={5} style={{ margin: '8px 0 10px', color: editorialInk, fontFamily: designDisplayFont, letterSpacing: '-0.03em' }}>
          首次登录需要补齐本地密码
        </Title>
        <Paragraph style={{ margin: 0, color: alphaColor(token.colorWhite, 0.82), lineHeight: 1.7 }}>
          检测到你正在使用 Linux DO 登录。为了后续也能通过用户名密码方式进入系统，请先为当前账号设置一个本地密码。
        </Paragraph>
      </Card>

      <Card
        bordered={false}
        style={{
          marginBottom: 16,
          borderRadius: 18,
          background: quietPanelBackground,
          border: `1px solid ${alphaColor(token.colorPrimary, 0.08)}`,
        }}
        styles={{ body: { padding: 18 } }}
      >
        <div
          style={{
            display: 'grid',
            gridTemplateColumns: 'repeat(auto-fit, minmax(220px, 1fr))',
            gap: 16,
          }}
        >
          <div>
            <Text style={{ fontSize: 12, letterSpacing: '0.12em', textTransform: 'uppercase', color: token.colorTextTertiary }}>
              Password Guide
            </Text>
            <Title level={5} style={{ margin: '6px 0 8px', fontFamily: designDisplayFont }}>
              首次密码设置顺序
            </Title>
            <Paragraph style={{ margin: 0, color: token.colorTextSecondary, lineHeight: 1.75 }}>
              这里现在只增强阅读顺序和当前焦点提示，不改变密码设置、提交 loading、取消关闭或输入回填逻辑。
            </Paragraph>
            <div style={{ display: 'flex', flexWrap: 'wrap', gap: 8, marginTop: 12 }}>
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
                    border: `1px solid ${token.colorBorderSecondary}`,
                    color: token.colorTextSecondary,
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
              background: token.colorBgContainer,
              border: `1px solid ${token.colorBorderSecondary}`,
            }}
          >
            <Text style={{ display: 'block', fontSize: 12, letterSpacing: '0.12em', textTransform: 'uppercase', color: token.colorTextTertiary }}>
              当前工作焦点
            </Text>
            <Title level={5} style={{ margin: '8px 0 6px', fontFamily: designDisplayFont }}>
              {passwordWorkspaceFocus.title}
            </Title>
            <Paragraph style={{ margin: 0, color: token.colorTextSecondary, lineHeight: 1.75 }}>
              {passwordWorkspaceFocus.note}
            </Paragraph>
            <Space wrap size={[8, 8]} style={{ marginTop: 12 }}>
              <Tag color={settingPassword ? 'processing' : 'blue'} style={{ margin: 0, borderRadius: 999, paddingInline: 10 }}>
                {settingPassword ? '保存中' : '待设置'}
              </Tag>
              <Tag color={passwordStatus?.default_password ? 'gold' : 'default'} style={{ margin: 0, borderRadius: 999, paddingInline: 10 }}>
                {passwordStatus?.default_password ? '含初始密码说明' : '无初始密码说明'}
              </Tag>
              <Tag color={newPassword && confirmPassword && newPassword === confirmPassword ? 'green' : 'default'} style={{ margin: 0, borderRadius: 999, paddingInline: 10 }}>
                {newPassword && confirmPassword && newPassword === confirmPassword ? '表单已对齐' : '等待表单完成'}
              </Tag>
            </Space>
          </div>
        </div>
      </Card>

      {passwordStatus?.default_password ? (
        <Card
          bordered={false}
          style={{
            marginBottom: 16,
            borderRadius: 18,
            background: token.colorBgContainer,
            border: `1px solid ${alphaColor(token.colorPrimary, 0.08)}`,
          }}
          styles={{ body: { padding: 16 } }}
        >
          <Text style={{ fontSize: 12, letterSpacing: '0.12em', textTransform: 'uppercase', color: token.colorTextTertiary }}>
            Account Summary
          </Text>
          <div style={{ marginTop: 10, display: 'grid', gap: 10 }}>
            <div>
              <Text type="secondary">当前账号</Text>
              <div><Text strong>{passwordStatus.username}</Text></div>
            </div>
            <div>
              <Text type="secondary">初始密码</Text>
              <div>
                <code
                  style={{
                    display: 'inline-block',
                    marginTop: 4,
                    background: quietPanelBackground,
                    padding: '4px 10px',
                    borderRadius: 8,
                    color: token.colorPrimary,
                    fontSize: 14,
                  }}
                >
                  {passwordStatus.default_password}
                </code>
              </div>
            </div>
          </div>
        </Card>
      ) : null}

      <Card
        bordered={false}
        style={{
          borderRadius: 18,
          background: token.colorBgContainer,
          border: `1px solid ${alphaColor(token.colorPrimary, 0.08)}`,
        }}
        styles={{ body: { padding: 16 } }}
      >
        <div style={{ marginBottom: 14 }}>
          <Text style={{ fontSize: 12, letterSpacing: '0.12em', textTransform: 'uppercase', color: token.colorTextTertiary }}>
            Password Form
          </Text>
        </div>

        <div style={{ marginBottom: 12 }}>
          <label>新密码（至少 6 位）</label>
          <Input.Password
            value={newPassword}
            onChange={(event) => onNewPasswordChange(event.target.value)}
            placeholder="请输入新密码"
            style={{ marginTop: 6, borderRadius: 12 }}
          />
        </div>
        <div>
          <label>确认密码</label>
          <Input.Password
            value={confirmPassword}
            onChange={(event) => onConfirmPasswordChange(event.target.value)}
            placeholder="请再次输入密码"
            style={{ marginTop: 6, borderRadius: 12 }}
          />
        </div>
      </Card>
    </Modal>
  );
}
