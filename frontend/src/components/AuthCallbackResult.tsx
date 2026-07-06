import { Button, Card, Col, Result, Row, Space, Tag, Typography, theme } from 'antd';
import { designDisplayFont } from '../theme/themeConfig';

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
  const editorialInk = '#f7f1e8';
  const pageBackground = `linear-gradient(180deg, ${alphaColor(token.colorPrimary, 0.08)} 0%, color-mix(in srgb, ${token.colorBgLayout} 92%, ${token.colorPrimary} 8%) 100%)`;
  const heroBackground = `linear-gradient(135deg, #171411 0%, color-mix(in srgb, #171411 58%, ${token.colorPrimary} 42%) 100%)`;
  const phaseText = status === 'error'
    ? '身份校验未完成，请返回登录页重新发起认证。'
    : showPasswordModal
      ? '首次登录正在进入密码初始化流程。'
      : showAnnouncement
        ? '身份已确认，正在准备进入公告与工作区。'
        : '身份已确认，正在为你跳转目标页面。';
  const phaseTag = status === 'error'
    ? 'Needs Retry'
    : showPasswordModal
      ? 'Password Setup'
      : showAnnouncement
        ? 'Announcement Gate'
        : 'Redirecting';
  const resultTitle = status === 'error' ? '登录失败' : '登录成功';
  const resultSubtitle = status === 'error'
    ? errorMessage
    : showPasswordModal
      ? '登录成功，正在引导设置密码...'
      : showAnnouncement
        ? '登录成功，正在加载公告...'
        : '登录成功，正在跳转...';
  const callbackSequence = [
    '身份校验通过后恢复原目标页',
    '如有公告，先进入公告提示',
    '首次登录时补密码初始化',
  ];
  const focusNote = status === 'error'
    ? '这次停在身份校验阶段，最自然的下一步是返回登录页重新发起认证。'
    : showPasswordModal
      ? '当前焦点是完成密码初始化，完成后会继续沿用原有公告与跳转分流。'
      : showAnnouncement
        ? '当前焦点是阅读公告或选择隐藏方式，完成后会继续进入目标工作区。'
        : '当前焦点是等待跳转完成，不需要额外操作。';
  const { Title, Paragraph, Text } = Typography;

  return (
    <div
      style={{
        display: 'flex',
        justifyContent: 'center',
        alignItems: 'center',
        minHeight: '100vh',
        background: pageBackground,
        padding: '24px 16px',
      }}
    >
      <Card
        bordered={false}
        style={{
          width: '100%',
          maxWidth: 780,
          borderRadius: 28,
          overflow: 'hidden',
          background: heroBackground,
          boxShadow: `0 32px 68px -42px ${alphaColor(token.colorTextBase, 0.55)}`,
        }}
        styles={{ body: { padding: 0 } }}
      >
        <div style={{ position: 'relative', padding: '28px 28px 0 28px' }}>
          <div
            style={{
              position: 'absolute',
              inset: 0,
              background: 'radial-gradient(circle at top right, rgba(255,255,255,0.14), transparent 32%)',
              pointerEvents: 'none',
            }}
          />
          <div style={{ position: 'relative' }}>
            <Space direction="vertical" size={10} style={{ width: '100%' }}>
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
                Callback Bridge
              </Tag>
              <Title level={2} style={{ margin: 0, color: editorialInk, fontFamily: designDisplayFont, letterSpacing: '-0.03em' }}>
                身份回调处理中
              </Title>
              <Paragraph style={{ margin: 0, color: alphaColor(token.colorWhite, 0.82), fontSize: 14, maxWidth: 620 }}>
                {phaseText}
              </Paragraph>
              <Tag style={{ alignSelf: 'flex-start', borderRadius: 999, paddingInline: 10 }}>
                {phaseTag}
              </Tag>
            </Space>
          </div>
        </div>

        <div style={{ padding: '24px 28px 28px 28px' }}>
          <Card
            bordered={false}
            style={{
              borderRadius: 22,
              background: alphaColor(token.colorBgContainer, 0.9),
              border: `1px solid ${alphaColor(token.colorWhite, 0.12)}`,
              marginBottom: 18,
            }}
            styles={{ body: { padding: 20 } }}
          >
            <Row gutter={[16, 16]}>
              <Col xs={24} lg={15}>
                <Space direction="vertical" size={8} style={{ width: '100%' }}>
                  <Text style={{ color: token.colorTextTertiary, letterSpacing: '0.12em', textTransform: 'uppercase', fontSize: 12 }}>
                    Flow Note
                  </Text>
                  <Title level={5} style={{ margin: 0, fontFamily: designDisplayFont }}>
                    回调链路阅读顺序
                  </Title>
                  <Paragraph style={{ margin: 0, color: token.colorTextSecondary, lineHeight: 1.75 }}>
                    这个结果页不改变任何业务分支，只把原有的回调、公告分流和首次密码初始化顺序解释得更清楚。
                  </Paragraph>
                  <div style={{ display: 'flex', flexWrap: 'wrap', gap: 8 }}>
                    {callbackSequence.map((item, index) => (
                      <span
                        key={item}
                        style={{
                          display: 'inline-flex',
                          alignItems: 'center',
                          gap: 8,
                          padding: '6px 12px',
                          borderRadius: 999,
                          background: alphaColor(token.colorPrimary, 0.06),
                          border: `1px solid ${alphaColor(token.colorPrimary, 0.12)}`,
                          color: token.colorTextSecondary,
                          fontSize: 12,
                        }}
                      >
                        <span style={{ color: token.colorPrimary, fontWeight: 700 }}>{index + 1}</span>
                        {item}
                      </span>
                    ))}
                  </div>
                </Space>
              </Col>
              <Col xs={24} lg={9}>
                <div
                  style={{
                    height: '100%',
                    borderRadius: 18,
                    padding: '16px 18px',
                    background: alphaColor(token.colorBgContainer, 0.92),
                    border: `1px solid ${alphaColor(token.colorWhite, 0.12)}`,
                  }}
                >
                  <Text style={{ display: 'block', color: token.colorTextTertiary, letterSpacing: '0.12em', textTransform: 'uppercase', fontSize: 12 }}>
                    当前焦点
                  </Text>
                  <Title level={5} style={{ margin: '8px 0 6px', fontFamily: designDisplayFont }}>
                    {phaseTag}
                  </Title>
                  <Paragraph style={{ margin: 0, color: token.colorTextSecondary, lineHeight: 1.75 }}>
                    {focusNote}
                  </Paragraph>
                </div>
              </Col>
            </Row>
          </Card>

          <Card
            bordered={false}
            style={{
              borderRadius: 22,
              background: alphaColor(token.colorBgContainer, 0.96),
              border: `1px solid ${alphaColor(token.colorWhite, 0.18)}`,
            }}
            styles={{ body: { padding: 24 } }}
          >
            <Result
              status={status}
              title={resultTitle}
              subTitle={resultSubtitle}
              extra={status === 'error' && onBackToLogin ? (
                <Button type="primary" onClick={onBackToLogin} style={{ borderRadius: 14 }}>
                  返回登录
                </Button>
              ) : undefined}
            />
            <Text type="secondary" style={{ display: 'block', textAlign: 'center', marginTop: -8 }}>
              登录回调、公告分流与首次密码初始化逻辑保持不变，仅升级入口展示层。
            </Text>
          </Card>
        </div>
      </Card>
    </div>
  );
}
