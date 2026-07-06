import type { CSSProperties } from 'react';
import { Card, Space, Tag, Typography, theme } from 'antd';
import { designDisplayFont } from '../theme/themeConfig';

interface LoadingScreenProps {
  message?: string;
  minHeight?: string;
}

const spinnerStyle: CSSProperties = {
  width: 34,
  height: 34,
  borderRadius: '50%',
  border: '3px solid color-mix(in srgb, var(--ant-color-primary, #4D8088) 20%, transparent)',
  borderTopColor: 'var(--ant-color-primary, #4D8088)',
  animation: 'app-loading-spin 0.8s linear infinite',
  boxShadow: '0 0 0 8px color-mix(in srgb, var(--ant-color-primary, #4D8088) 8%, transparent)',
};

const messageStyle: CSSProperties = {
  fontSize: 14,
  fontWeight: 500,
  letterSpacing: '0.02em',
  lineHeight: 1.7,
};

export default function LoadingScreen({
  message = '加载中...',
  minHeight = '40vh',
}: LoadingScreenProps) {
  const { token } = theme.useToken();
  const { Paragraph, Text, Title } = Typography;
  const alphaColor = (color: string, alpha: number) =>
    `color-mix(in srgb, ${color} ${(alpha * 100).toFixed(0)}%, transparent)`;
  const fullScreen = minHeight === '100vh';
  const heroBackground = `linear-gradient(135deg, #171411 0%, color-mix(in srgb, #171411 58%, ${token.colorPrimary} 42%) 100%)`;
  const quietPanelBackground = `linear-gradient(180deg, color-mix(in srgb, ${token.colorBgContainer} 95%, ${token.colorFillAlter} 5%) 0%, color-mix(in srgb, ${token.colorBgContainer} 87%, ${token.colorFillAlter} 13%) 100%)`;
  const editorialInk = '#f7f1e8';
  const loadingTitle = fullScreen
    ? '正在核对进入工作区前的必要条件'
    : '正在把当前页面整理到可继续阅读的状态';
  const loadingNote = fullScreen
    ? '这里现在只升级全局加载态的阅读顺序与情绪表达，不改变鉴权检查、路由恢复或上游状态解析逻辑。条件满足后，应用会继续原有入口链路。'
    : '这里现在只增强页面切换时的信息层级与视觉焦点，不改变路由懒加载、数据准备或调用时序。';
  const guideSteps = [
    '先把这次停顿当成工作区切换缓冲，而不是错误中断；当前消息只是在提示系统仍在继续准备内容。',
    '再根据下方焦点卡确认现在处于全屏入口守候还是局部页面过渡，避免在过渡期反复刷新或重复点击。',
    '最后等待当前链路完成，应用会按既有逻辑继续进入目标页面、恢复鉴权判断或结束当前加载态。',
  ];
  const workspaceFocusTitle = fullScreen ? '全局入口正在等待条件齐备' : '当前页面正在完成过渡加载';
  const workspaceFocusNote = fullScreen
    ? '适用于鉴权检测、工作区初始化等全屏入口场景。只要上游检查完成，这里就会自然让出入口，不需要额外切换模式。'
    : '适用于路由懒加载与局部数据准备场景。当前壳层只负责解释“现在发生了什么”，不接管任何业务状态。';

  return (
    <>
      <style>{'@keyframes app-loading-spin { from { transform: rotate(0deg); } to { transform: rotate(360deg); } }'}</style>
      <div
        style={{
          minHeight,
          width: '100%',
          padding: fullScreen ? '40px 24px' : '32px 18px',
          color: token.colorText,
          background: `radial-gradient(circle at top, ${alphaColor(token.colorPrimaryBg, 0.92)} 0%, ${alphaColor(token.colorBgLayout, 0.98)} 56%, ${alphaColor(token.colorBgContainer, 1)} 100%)`,
        }}
        role="status"
        aria-live="polite"
      >
        <div style={{ width: 'min(960px, 100%)', margin: '0 auto', display: 'grid', gap: 18 }}>
          <Card
            bordered={false}
            style={{
              borderRadius: 24,
              overflow: 'hidden',
              background: heroBackground,
            }}
            styles={{ body: { padding: fullScreen ? 24 : 22 } }}
          >
            <Text
              style={{
                color: alphaColor(token.colorWhite, 0.68),
                letterSpacing: '0.14em',
                textTransform: 'uppercase',
              }}
            >
              Workspace Loading
            </Text>
            <Title
              level={2}
              style={{
                margin: '10px 0 12px',
                color: editorialInk,
                fontFamily: designDisplayFont,
                letterSpacing: '-0.03em',
              }}
            >
              {loadingTitle}
            </Title>
            <Paragraph
              style={{
                margin: 0,
                color: alphaColor(token.colorWhite, 0.82),
                lineHeight: 1.8,
                maxWidth: 680,
              }}
            >
              {loadingNote}
            </Paragraph>
          </Card>

          <Card
            bordered={false}
            style={{
              borderRadius: 22,
              background: quietPanelBackground,
              border: `1px solid ${alphaColor(token.colorPrimary, 0.08)}`,
            }}
            styles={{ body: { padding: 20 } }}
          >
            <div
              style={{
                display: 'grid',
                gridTemplateColumns: 'repeat(auto-fit, minmax(260px, 1fr))',
                gap: 16,
              }}
            >
              <div>
                <Text
                  style={{
                    fontSize: 12,
                    letterSpacing: '0.12em',
                    textTransform: 'uppercase',
                    color: token.colorTextTertiary,
                  }}
                >
                  Loading Guide
                </Text>
                <Title level={5} style={{ margin: '6px 0 8px', fontFamily: designDisplayFont }}>
                  过渡态的阅读顺序
                </Title>
                <Paragraph style={{ margin: 0, color: token.colorTextSecondary, lineHeight: 1.75 }}>
                  这里延续当前 Claude + Notion + Mintlify 融合风格，只重新组织说明顺序与视觉焦点，不改既有加载链路。
                </Paragraph>
              </div>
              <div style={{ display: 'grid', gap: 10 }}>
                {guideSteps.map((item, index) => (
                  <div
                    key={item}
                    style={{
                      display: 'flex',
                      gap: 10,
                      alignItems: 'flex-start',
                      padding: '12px 14px',
                      borderRadius: 18,
                      background: token.colorBgContainer,
                      border: `1px solid ${token.colorBorderSecondary}`,
                    }}
                  >
                    <span
                      style={{
                        minWidth: 26,
                        height: 26,
                        borderRadius: 999,
                        display: 'inline-flex',
                        alignItems: 'center',
                        justifyContent: 'center',
                        background: alphaColor(token.colorPrimary, 0.14),
                        color: token.colorPrimary,
                        fontSize: 12,
                        fontWeight: 700,
                        marginTop: 1,
                      }}
                    >
                      {index + 1}
                    </span>
                    <span style={{ color: token.colorTextSecondary, fontSize: 13, lineHeight: 1.7 }}>{item}</span>
                  </div>
                ))}
              </div>
            </div>
          </Card>

          <Card
            bordered={false}
            style={{
              borderRadius: 22,
              background: token.colorBgContainer,
              border: `1px solid ${alphaColor(token.colorPrimary, 0.08)}`,
              boxShadow: `0 24px 52px ${alphaColor(token.colorText, 0.1)}`,
            }}
            styles={{ body: { padding: 22 } }}
          >
            <div
              style={{
                display: 'grid',
                gridTemplateColumns: 'minmax(0, 1.4fr) minmax(260px, 0.9fr)',
                gap: 18,
              }}
            >
              <div style={{ display: 'grid', gap: 14 }}>
                <div>
                  <Text
                    style={{
                      fontSize: 12,
                      letterSpacing: '0.12em',
                      textTransform: 'uppercase',
                      color: token.colorTextTertiary,
                    }}
                  >
                    Focus Workspace
                  </Text>
                  <Title level={4} style={{ margin: '6px 0 8px', fontFamily: designDisplayFont }}>
                    {workspaceFocusTitle}
                  </Title>
                  <Paragraph style={{ margin: 0, color: token.colorTextSecondary, lineHeight: 1.75 }}>
                    {workspaceFocusNote}
                  </Paragraph>
                </div>

                <div
                  style={{
                    padding: '16px 18px',
                    borderRadius: 18,
                    background: quietPanelBackground,
                    border: `1px solid ${token.colorBorderSecondary}`,
                    display: 'flex',
                    alignItems: 'center',
                    gap: 14,
                  }}
                >
                  <div style={spinnerStyle} aria-hidden="true" />
                  <div style={{ display: 'grid', gap: 6 }}>
                    <Text
                      style={{
                        fontSize: 12,
                        letterSpacing: '0.12em',
                        textTransform: 'uppercase',
                        color: token.colorTextTertiary,
                      }}
                    >
                      Current Signal
                    </Text>
                    <div style={messageStyle}>{message}</div>
                  </div>
                </div>
              </div>

              <div
                style={{
                  borderRadius: 18,
                  padding: '16px 18px',
                  background: quietPanelBackground,
                  border: `1px solid ${token.colorBorderSecondary}`,
                }}
              >
                <Text
                  style={{
                    display: 'block',
                    fontSize: 12,
                    letterSpacing: '0.12em',
                    textTransform: 'uppercase',
                    color: token.colorTextTertiary,
                  }}
                >
                  Session Tags
                </Text>
                <Title level={5} style={{ margin: '8px 0 6px', fontFamily: designDisplayFont }}>
                  这是一次正常的工作区守候
                </Title>
                <Paragraph style={{ margin: 0, color: token.colorTextSecondary, lineHeight: 1.75 }}>
                  你看到的是统一后的加载说明层。路由、鉴权和数据准备仍按原有时序推进，这里不劫持业务状态，只帮助快速判断现在所处的过渡阶段。
                </Paragraph>
                <Space wrap size={[8, 8]} style={{ marginTop: 12 }}>
                  <Tag color={fullScreen ? 'processing' : 'blue'} style={{ margin: 0, borderRadius: 999, paddingInline: 10 }}>
                    {fullScreen ? '全屏入口守候' : '页面过渡加载'}
                  </Tag>
                  <Tag color="gold" style={{ margin: 0, borderRadius: 999, paddingInline: 10 }}>
                    焦点已整理
                  </Tag>
                  <Tag color="green" style={{ margin: 0, borderRadius: 999, paddingInline: 10 }}>
                    逻辑保持原样
                  </Tag>
                </Space>
              </div>
            </div>
          </Card>
        </div>
      </div>
    </>
  );
}
