import { Modal, Button, Card, Space, Tag, Typography, theme } from 'antd';
import { useEffect, useState } from 'react';
import { designDisplayFont } from '../theme/themeConfig';

interface AnnouncementModalProps {
  visible: boolean;
  onClose: () => void;
  onDoNotShowToday: () => void;
  onNeverShow: () => void;
}

export default function AnnouncementModal({ visible, onClose, onDoNotShowToday, onNeverShow }: AnnouncementModalProps) {
  const [qqImageError, setQqImageError] = useState(false);
  const [wxImageError, setWxImageError] = useState(false);
  const { token } = theme.useToken();
  const alphaColor = (color: string, alpha: number) => `color-mix(in srgb, ${color} ${(alpha * 100).toFixed(0)}%, transparent)`;
  const editorialInk = '#f7f1e8';
  const heroBackground = `linear-gradient(135deg, #171411 0%, color-mix(in srgb, #171411 58%, ${token.colorPrimary} 42%) 100%)`;
  const quietPanelBackground = `linear-gradient(180deg, color-mix(in srgb, ${token.colorBgContainer} 95%, ${token.colorFillAlter} 5%) 0%, color-mix(in srgb, ${token.colorBgContainer} 87%, ${token.colorFillAlter} 13%) 100%)`;
  const { Title, Paragraph, Text } = Typography;
  const announcementGuideSteps = [
    '先确认这次弹窗的目标是入群还是快速跳过，避免把欢迎提示当成必须停留的流程。',
    '再看右侧焦点卡里的渠道状态，优先选择当前可用的二维码入口完成加入。',
    '最后按需要决定只隐藏今天，还是永久关闭这条欢迎提示，不影响后续创作流程。',
  ];
  const announcementWorkspaceFocus = qqImageError && wxImageError
    ? {
        title: '当前两个社群二维码都未成功加载，先决定是否直接关闭欢迎提示',
        note: '这次更适合把弹窗当作一次性公告处理；如果稍后还想加入群组，可以在后续版本或社群入口恢复后再查看。',
      }
    : qqImageError || wxImageError
      ? {
          title: '当前只有一个群组入口可直接使用，优先从可用二维码进入',
          note: '欢迎弹窗已经给出当前可用渠道，建议先完成加入，再决定是否继续保留这个欢迎提示。',
        }
      : {
          title: '当前两个社群入口都可用，先按习惯选择 QQ 或微信加入',
          note: '这一步更适合作为进入产品前的轻量欢迎信息，适合快速看完后继续返回创作主流程。',
        };

  useEffect(() => {
    if (visible) {
      setQqImageError(false);
      setWxImageError(false);
    }
  }, [visible]);

  const handleDoNotShowToday = () => {
    onDoNotShowToday();
    onClose();
  };

  const handleNeverShow = () => {
    onNeverShow();
    onClose();
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
            Community Briefing
          </Tag>
          <Title level={3} style={{ margin: 0, textAlign: 'center', fontFamily: designDisplayFont, letterSpacing: '-0.03em' }}>
            欢迎加入 AI 小说创作助手
          </Title>
        </Space>
      }
      open={visible}
      onCancel={onClose}
      footer={
        <Space style={{ width: '100%', justifyContent: 'center' }}>
          <Button
            onClick={handleDoNotShowToday}
            size="large"
            style={{
              borderRadius: '8px',
              height: '42px',
              fontSize: '14px',
            }}
          >
            今日内不再展示
          </Button>
          <Button
            type="primary"
            onClick={handleNeverShow}
            size="large"
            style={{
              borderRadius: '8px',
              height: '42px',
              fontSize: '14px',
              background: token.colorPrimary,
              borderColor: token.colorPrimary,
              boxShadow: `0 8px 20px ${alphaColor(token.colorPrimary, 0.32)}`,
            }}
          >
            永不再展示
          </Button>
        </Space>
      }
      width={700}
      centered
      styles={{
        body: {
          padding: '20px',
          background: quietPanelBackground,
        },
        header: {
          background: `linear-gradient(135deg, ${alphaColor(token.colorPrimary, 0.08)} 0%, ${alphaColor(token.colorBgContainer, 0.98)} 100%)`,
          borderBottom: `1px solid ${token.colorBorderSecondary}`,
          padding: '16px 24px',
        },
        footer: {
          background: token.colorBgContainer,
          borderTop: `1px solid ${token.colorBorderSecondary}`,
          padding: '16px 24px',
        },
      }}
    >
      <div style={{ textAlign: 'center' }}>
        <Card
          bordered={false}
          style={{
            marginBottom: 16,
            borderRadius: 22,
            background: heroBackground,
            overflow: 'hidden',
          }}
          styles={{ body: { padding: 22, textAlign: 'left' } }}
        >
          <Text style={{ color: alphaColor(token.colorWhite, 0.68), letterSpacing: '0.14em', textTransform: 'uppercase' }}>
            Community Welcome
          </Text>
          <Title level={4} style={{ margin: '8px 0 10px', color: editorialInk, fontFamily: designDisplayFont, letterSpacing: '-0.03em' }}>
            欢迎加入我们的交流群
          </Title>
          <Paragraph style={{ margin: 0, color: alphaColor(token.colorWhite, 0.82), lineHeight: 1.7 }}>
            在这里你可以交流创作心得、第一时间获取更新、反馈问题，也能分享自己的灵感与使用技巧。
          </Paragraph>
        </Card>

        <Card
          bordered={false}
          style={{
            marginBottom: 16,
            borderRadius: 20,
            background: quietPanelBackground,
            border: `1px solid ${alphaColor(token.colorPrimary, 0.08)}`,
            textAlign: 'left',
          }}
          styles={{ body: { padding: 20 } }}
        >
          <div
            style={{
              display: 'grid',
              gridTemplateColumns: 'repeat(auto-fit, minmax(240px, 1fr))',
              gap: 16,
            }}
          >
            <div>
              <Text style={{ fontSize: 12, letterSpacing: '0.12em', textTransform: 'uppercase', color: token.colorTextTertiary }}>
                Welcome Guide
              </Text>
              <Title level={5} style={{ margin: '6px 0 8px', fontFamily: designDisplayFont }}>
                欢迎弹窗阅读顺序
              </Title>
              <Paragraph style={{ margin: 0, color: token.colorTextSecondary, lineHeight: 1.75 }}>
                这里现在只增强阅读顺序与当前焦点说明，不改变任何弹窗开关、当日隐藏或永久关闭逻辑。
              </Paragraph>
              <div style={{ display: 'flex', flexWrap: 'wrap', gap: 8, marginTop: 12 }}>
                {announcementGuideSteps.map((item, index) => (
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
                borderRadius: 18,
                padding: '16px 18px',
                background: token.colorBgContainer,
                border: `1px solid ${token.colorBorderSecondary}`,
              }}
            >
              <Text style={{ display: 'block', fontSize: 12, letterSpacing: '0.12em', textTransform: 'uppercase', color: token.colorTextTertiary }}>
                当前工作焦点
              </Text>
              <Title level={5} style={{ margin: '8px 0 6px', fontFamily: designDisplayFont }}>
                {announcementWorkspaceFocus.title}
              </Title>
              <Paragraph style={{ margin: 0, color: token.colorTextSecondary, lineHeight: 1.75 }}>
                {announcementWorkspaceFocus.note}
              </Paragraph>
              <Space wrap size={[8, 8]} style={{ marginTop: 12 }}>
                <Tag color={qqImageError ? 'default' : 'processing'} style={{ margin: 0, borderRadius: 999, paddingInline: 10 }}>
                  QQ {qqImageError ? '加载失败' : '可加入'}
                </Tag>
                <Tag color={wxImageError ? 'default' : 'green'} style={{ margin: 0, borderRadius: 999, paddingInline: 10 }}>
                  微信 {wxImageError ? '加载失败' : '可加入'}
                </Tag>
                <Tag color="gold" style={{ margin: 0, borderRadius: 999, paddingInline: 10 }}>
                  可选择隐藏策略
                </Tag>
              </Space>
            </div>
          </div>
        </Card>

        <Card
          bordered={false}
          style={{
            borderRadius: 20,
            background: token.colorBgContainer,
            border: `1px solid ${alphaColor(token.colorPrimary, 0.08)}`,
          }}
          styles={{ body: { padding: 20 } }}
        >
          <div style={{ marginBottom: 16 }}>
            <Text style={{ fontSize: 12, letterSpacing: '0.12em', textTransform: 'uppercase', color: token.colorTextTertiary }}>
              Community Workspace
            </Text>
            <Title level={5} style={{ margin: '6px 0 0', fontFamily: designDisplayFont }}>
              扫描二维码加入交流群
            </Title>
            <Paragraph style={{ margin: '6px 0 0', color: token.colorTextSecondary, lineHeight: 1.7 }}>
              选择你更常用的社群入口即可，不需要在这里完成额外配置；加入后就可以继续返回创作页面。
            </Paragraph>
          </div>

          <Space
            size={16}
            wrap
            style={{
              width: '100%',
              justifyContent: 'center',
              alignItems: 'flex-start',
            }}
          >
            {[
              { title: 'QQ交流群', src: '/qq.jpg', failed: qqImageError, onError: () => setQqImageError(true) },
              { title: '微信交流群', src: '/WX.png', failed: wxImageError, onError: () => setWxImageError(true) },
            ].map((group) => (
              <Card
                key={group.title}
                bordered={false}
                style={{
                  width: 220,
                  borderRadius: 18,
                  background: quietPanelBackground,
                  border: `1px solid ${alphaColor(token.colorPrimary, 0.08)}`,
                  textAlign: 'center',
                }}
                styles={{ body: { padding: 16 } }}
              >
                <Text strong style={{ display: 'block', marginBottom: 10 }}>{group.title}</Text>
                {!group.failed ? (
                  <div
                    style={{
                      display: 'flex',
                      justifyContent: 'center',
                      alignItems: 'center',
                      background: token.colorBgContainer,
                      borderRadius: 12,
                      padding: 8,
                      boxShadow: `0 18px 34px -30px ${alphaColor(token.colorText, 0.28)}`,
                    }}
                  >
                    <img
                      src={group.src}
                      alt={`${group.title}二维码`}
                      style={{
                        maxWidth: '180px',
                        maxHeight: '180px',
                        width: 'auto',
                        height: 'auto',
                        display: 'block',
                        objectFit: 'contain',
                      }}
                      onError={group.onError}
                    />
                  </div>
                ) : (
                  <div
                    style={{
                      width: 180,
                      height: 180,
                      margin: '0 auto',
                      display: 'flex',
                      justifyContent: 'center',
                      alignItems: 'center',
                      background: token.colorBgContainer,
                      borderRadius: 12,
                      color: token.colorTextTertiary,
                    }}
                  >
                    二维码加载失败
                  </div>
                )}
              </Card>
            ))}
          </Space>
        </Card>

        <div
          style={{
            marginTop: 16,
            padding: '12px 14px',
            background: token.colorWarningBg,
            borderRadius: 12,
            border: `1px solid ${token.colorWarningBorder}`,
            fontSize: 13,
            color: token.colorWarning,
            textAlign: 'left',
          }}
        >
          提示：选择“今日内不再展示”会在当天隐藏公告，选择“永不再展示”会永久关闭这个欢迎提示。
        </div>
      </div>
    </Modal>
  );
}
