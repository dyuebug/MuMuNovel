import React from 'react';
import { Modal, Spin, Button, Card, Space, Tag, Typography, theme } from 'antd';
import { DownOutlined, LoadingOutlined, StopOutlined, UnorderedListOutlined, UpOutlined } from '@ant-design/icons';
import { useFloatingTaskCard } from './useFloatingTaskCard';
import { OPEN_BACKGROUND_TASK_CENTER_EVENT } from '../constants/backgroundTaskEvents';
import { useBackgroundTaskStore } from '../store/backgroundTasks';
import { selectActiveBackgroundTaskCount } from '../store/backgroundTaskSelectors';
import { designDisplayFont } from '../theme/themeConfig';
import { ModelOutputPanel, type ModelOutputPanelProps } from './ModelOutputPanel';

interface SSEProgressModalProps {
  visible: boolean;
  progress: number;
  message: string;
  title?: string;
  showPercentage?: boolean;
  showIcon?: boolean;
  onCancel?: () => void;
  cancelButtonText?: string;
  blocking?: boolean;
  modelOutput?: Omit<ModelOutputPanelProps, 'compact'>;
}

export const SSEProgressModal: React.FC<SSEProgressModalProps> = ({
  visible,
  progress,
  message,
  title = '正在生成中...',
  showPercentage = true,
  showIcon = true,
  onCancel,
  cancelButtonText = '取消任务',
  blocking = true,
  modelOutput,
}) => {
  const { collapse, collapsed, floatingBottom, toggleCollapsed } = useFloatingTaskCard({
    active: visible,
    blocking,
  });
  const { token } = theme.useToken();
  const alphaColor = (color: string, alpha: number) => `color-mix(in srgb, ${color} ${(alpha * 100).toFixed(0)}%, transparent)`;
  const { Paragraph, Text, Title } = Typography;
  const editorialInk = '#f7f1e8';
  const heroBackground = `linear-gradient(135deg, #171411 0%, color-mix(in srgb, #171411 58%, ${token.colorPrimary} 42%) 100%)`;
  const quietPanelBackground = `linear-gradient(180deg, color-mix(in srgb, ${token.colorBgContainer} 95%, ${token.colorFillAlter} 5%) 0%, color-mix(in srgb, ${token.colorBgContainer} 87%, ${token.colorFillAlter} 13%) 100%)`;
  const shouldTrackActiveTaskCount = visible && !blocking;
  const activeTaskCount = useBackgroundTaskStore((state) => (
    shouldTrackActiveTaskCount ? selectActiveBackgroundTaskCount(state.tasks) : 0
  ));
  const queueSummary = !blocking && activeTaskCount > 1
    ? `当前共 ${activeTaskCount} 个后台任务正在运行`
    : null;
  const progressGuideSteps = blocking
    ? [
        '先把当前界面当作一次生成会话进度板，核心目标是观察进度和当前提示，不要把它当作配置入口。',
        '再看右侧焦点卡确认当前是否仍在推进、是否接近完成，必要时再决定是否中止本次生成。',
        '最后等待进度完成或主动取消，页面会继续沿原有生成流程收口，不改变任何任务状态逻辑。',
      ]
    : [
        '先确认这是后台任务浮层，当前重点是持续感知进度，而不是打断正在进行的工作区操作。',
        '再根据右侧焦点卡判断是继续并行处理、查看全部任务，还是在必要时取消当前任务。',
        '最后等任务进入完成态或回到任务中心统一查看，原有后台任务恢复与轮询逻辑保持不变。',
      ];
  const progressWorkspaceFocus = progress >= 100
    ? {
        title: blocking ? '当前进度已抵达完成阈值，等待这次生成会话完成最后收口' : '当前后台任务已经接近收尾，可准备回到任务中心确认结果',
        note: blocking
          ? '这时最重要的是等待现有流程完成最后一步，不需要额外切换页面或重复触发操作。'
          : '如果你正在并行处理其他页面，可以稍后统一回到任务中心确认结果，不需要改变当前后台任务流。',
      }
    : !blocking && activeTaskCount > 1
      ? {
          title: `当前有 ${activeTaskCount} 个后台任务并行执行，先把它当作任务总览入口`,
          note: '这时更适合继续当前操作，再按需打开任务中心查看全部任务；现有轮询与恢复行为保持不变。',
        }
      : onCancel
        ? {
            title: blocking ? '当前生成仍在进行中，可继续观察进度或在必要时中止本次任务' : '当前后台任务仍在推进，可继续并行工作或按需取消',
            note: '这里的取消动作仍然沿用原有行为；现在只增强阅读顺序和焦点提示，不改变底层任务控制逻辑。',
          }
        : {
            title: blocking ? `当前生成会话正在推进，进度已到 ${progress}%` : `当前后台任务正在推进，进度已到 ${progress}%`,
            note: blocking
              ? '这一步适合专注查看进度条和当前生成提示，等待流程自然完成。'
              : '当前更适合作为轻量悬浮监控层，帮助你一边继续别的操作，一边感知任务推进状态。',
          };

  const openTaskCenter = () => {
    collapse();
    window.dispatchEvent(new Event(OPEN_BACKGROUND_TASK_CENTER_EVENT));
  };

  if (!visible) return null;

  const content = (
    <div style={{ display: 'grid', gap: 16 }}>
      <Card
        bordered={false}
        style={{
          borderRadius: 20,
          overflow: 'hidden',
          background: heroBackground,
        }}
        styles={{ body: { padding: blocking ? 20 : 18 } }}
      >
        <Text style={{ color: alphaColor(token.colorWhite, 0.68), letterSpacing: '0.14em', textTransform: 'uppercase' }}>
          {blocking ? 'Generation Session' : 'Background Run'}
        </Text>
        <Title
          level={blocking ? 4 : 5}
          style={{
            margin: '8px 0 10px',
            color: editorialInk,
            fontFamily: designDisplayFont,
            letterSpacing: '-0.03em',
          }}
        >
          {blocking ? '当前生成流程正在推进' : '后台任务保持运行中'}
        </Title>
        <Paragraph style={{ margin: 0, color: alphaColor(token.colorWhite, 0.82), lineHeight: 1.7 }}>
          {blocking
            ? '这里现在只增强进度阅读顺序与工作焦点说明，不改变生成会话推进、取消操作或完成收口逻辑。'
            : '这里现在只增强悬浮进度卡的信息层级，不改变后台任务轮询、恢复、取消或任务中心跳转逻辑。'}
        </Paragraph>
      </Card>

      <Card
        bordered={false}
        style={{
          borderRadius: 18,
          background: quietPanelBackground,
          border: `1px solid ${alphaColor(token.colorPrimary, 0.08)}`,
        }}
        styles={{ body: { padding: blocking ? 18 : 16 } }}
      >
        <div
          style={{
            display: 'grid',
            gridTemplateColumns: blocking ? 'repeat(auto-fit, minmax(220px, 1fr))' : '1fr',
            gap: 16,
          }}
        >
          <div>
            <Text style={{ fontSize: 12, letterSpacing: '0.12em', textTransform: 'uppercase', color: token.colorTextTertiary }}>
              {blocking ? 'Progress Guide' : 'Run Guide'}
            </Text>
            <Title level={5} style={{ margin: '6px 0 8px', fontFamily: designDisplayFont }}>
              {blocking ? '生成进度阅读顺序' : '后台任务查看顺序'}
            </Title>
            <Paragraph style={{ margin: 0, color: token.colorTextSecondary, lineHeight: 1.75 }}>
              {blocking
                ? '先看会话状态，再看焦点卡，最后决定继续等待还是取消。这样能在不打断原有生成流的前提下，把当前阶段读清楚。'
                : '当前更适合作为并行工作时的轻量监视层，先看任务状态，再看焦点卡，最后按需进入任务中心或取消。'}
            </Paragraph>
            <div style={{ display: 'flex', flexWrap: 'wrap', gap: 8, marginTop: 12 }}>
              {progressGuideSteps.map((item, index) => (
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
              {progressWorkspaceFocus.title}
            </Title>
            <Paragraph style={{ margin: 0, color: token.colorTextSecondary, lineHeight: 1.75 }}>
              {progressWorkspaceFocus.note}
            </Paragraph>
            <Space wrap size={[8, 8]} style={{ marginTop: 12 }}>
              <Tag color={blocking ? 'processing' : 'blue'} style={{ margin: 0, borderRadius: 999, paddingInline: 10 }}>
                {blocking ? '阻塞式生成会话' : '后台任务浮层'}
              </Tag>
              <Tag color={progress >= 100 ? 'green' : 'gold'} style={{ margin: 0, borderRadius: 999, paddingInline: 10 }}>
                进度 {progress}%
              </Tag>
              <Tag color={onCancel ? 'volcano' : 'default'} style={{ margin: 0, borderRadius: 999, paddingInline: 10 }}>
                {onCancel ? '允许主动取消' : '无取消入口'}
              </Tag>
              {!blocking && queueSummary ? (
                <Tag color="cyan" style={{ margin: 0, borderRadius: 999, paddingInline: 10 }}>
                  {activeTaskCount} 个后台任务
                </Tag>
              ) : null}
            </Space>
          </div>
        </div>
      </Card>

      <Card
        bordered={false}
        style={{
          borderRadius: 18,
          background: token.colorBgContainer,
          border: `1px solid ${alphaColor(token.colorPrimary, 0.08)}`,
        }}
        styles={{ body: { padding: blocking ? 20 : 18 } }}
      >
        <div style={{ marginBottom: 14 }}>
          <Text style={{ fontSize: 12, letterSpacing: '0.12em', textTransform: 'uppercase', color: token.colorTextTertiary }}>
            {blocking ? 'Generation Workspace' : 'Background Workspace'}
          </Text>
          <Title level={5} style={{ margin: '6px 0 0', fontFamily: designDisplayFont }}>
            {blocking ? '查看本次生成进度与当前提示' : '查看后台任务进度与任务入口'}
          </Title>
        </div>

        {showIcon && (
          <div
            style={{
              textAlign: 'center',
              marginBottom: 24,
            }}
          >
            <Spin
              indicator={
                <LoadingOutlined
                  style={{ fontSize: blocking ? 48 : 32, color: 'var(--color-primary)' }}
                  spin
                />
              }
            />
            <div
              style={{
                fontSize: blocking ? 20 : 16,
                fontWeight: 'bold',
                marginTop: 16,
                color: token.colorTextHeading,
              }}
            >
              {title}
            </div>
          </div>
        )}

        <div style={{ marginBottom: showPercentage ? 16 : 24 }}>
          <div
            style={{
              height: 12,
              background: alphaColor(token.colorFillQuaternary, 0.96),
              borderRadius: 6,
              overflow: 'hidden',
              marginBottom: showPercentage ? 12 : 0,
            }}
          >
            <div
              style={{
                height: '100%',
                background:
                  progress === 100
                    ? `linear-gradient(90deg, ${token.colorSuccess} 0%, ${token.colorSuccessActive} 100%)`
                    : `linear-gradient(90deg, ${token.colorPrimary} 0%, ${token.colorPrimaryActive} 100%)`,
                width: `${progress}%`,
                transition: 'all 0.3s ease',
                borderRadius: 6,
                boxShadow: progress > 0 ? `0 10px 22px ${alphaColor(progress === 100 ? token.colorSuccess : token.colorPrimary, 0.22)}` : 'none',
              }}
            />
          </div>

          {showPercentage && (
            <div
              style={{
                textAlign: 'center',
                fontSize: blocking ? 32 : 24,
                fontWeight: 'bold',
                color: progress === 100 ? token.colorSuccess : token.colorPrimary,
                marginBottom: 8,
              }}
            >
              {progress}%
            </div>
          )}
        </div>

        <div
          style={{
            textAlign: 'center',
            fontSize: blocking ? 16 : 14,
            color: token.colorTextSecondary,
            minHeight: 24,
            padding: '0 20px',
            marginBottom: 16,
            lineHeight: 1.8,
          }}
        >
          {message || '准备生成...'}
        </div>

        <div
          style={{
            textAlign: 'center',
            fontSize: 13,
            color: token.colorTextTertiary,
            marginBottom: onCancel ? 16 : 0,
            lineHeight: 1.7,
          }}
        >
          {blocking ? '请勿关闭页面，生成过程需要一定时间' : '后台处理中，可继续其他操作'}
        </div>

        {queueSummary ? (
          <div
            style={{
              textAlign: 'center',
              fontSize: 12,
              color: token.colorTextTertiary,
              marginBottom: onCancel ? 12 : 0,
            }}
          >
            {queueSummary}
          </div>
        ) : null}

        {!blocking ? (
          <div style={{ textAlign: 'center', marginBottom: onCancel ? 12 : 0 }}>
            <Button type="link" size="small" icon={<UnorderedListOutlined />} onClick={openTaskCenter}>
              查看全部任务
            </Button>
          </div>
        ) : null}

        {onCancel && (
          <div
            style={{
              textAlign: 'center',
              marginTop: 16,
            }}
          >
            <Button
              danger
              size="large"
              icon={<StopOutlined />}
              onClick={onCancel}
            >
              {cancelButtonText}
            </Button>
          </div>
        )}
      </Card>
      {modelOutput ? <ModelOutputPanel {...modelOutput} /> : null}
    </div>
  );

  const compactBackgroundContent = (
    <div style={{ display: 'grid', gap: 9 }}>
      <div style={{ display: 'grid', gridTemplateColumns: '1fr auto', alignItems: 'start', gap: 12 }}>
        <Space size={8} style={{ minWidth: 0, alignItems: 'flex-start' }}>
          {showIcon ? <Spin indicator={<LoadingOutlined style={{ fontSize: 20, color: 'var(--color-primary)' }} spin />} /> : null}
          <div style={{ minWidth: 0 }}>
            <Text style={{ display: 'block', fontSize: 11, letterSpacing: '0.1em', textTransform: 'uppercase', color: token.colorTextTertiary }}>
              Background Task
            </Text>
            <Text strong style={{ display: 'block', fontSize: 15, color: token.colorTextHeading, whiteSpace: 'nowrap', overflow: 'hidden', textOverflow: 'ellipsis' }}>
              {title}
            </Text>
          </div>
        </Space>
        {showPercentage ? (
          <Text strong style={{ fontSize: 16, lineHeight: 1.4, color: progress === 100 ? token.colorSuccess : token.colorPrimary }}>
            {progress}%
          </Text>
        ) : null}
      </div>

      <div
        style={{
          height: 7,
          background: alphaColor(token.colorFillQuaternary, 0.96),
          borderRadius: 999,
          overflow: 'hidden',
        }}
      >
        <div
          style={{
            height: '100%',
            width: `${progress}%`,
            background: progress === 100
              ? `linear-gradient(90deg, ${token.colorSuccess} 0%, ${token.colorSuccessActive} 100%)`
              : `linear-gradient(90deg, ${token.colorPrimary} 0%, ${token.colorPrimaryActive} 100%)`,
            borderRadius: 999,
            transition: 'width 0.3s ease',
          }}
        />
      </div>

      <Text type="secondary" style={{ fontSize: 12, lineHeight: 1.5 }}>
        {message || '后台处理中，可继续其他操作'}
      </Text>

      {modelOutput ? <ModelOutputPanel {...modelOutput} compact /> : null}

      <div style={{ display: 'grid', gridTemplateColumns: '1fr auto', alignItems: 'center', gap: 10 }}>
        <Text type="secondary" style={{ fontSize: 11, minWidth: 0 }}>
          {queueSummary || '可继续当前页面操作'}
        </Text>
        <Space size={6}>
          <Button size="small" icon={<UnorderedListOutlined />} onClick={openTaskCenter}>
            任务中心
          </Button>
          {onCancel ? (
            <Button danger size="small" icon={<StopOutlined />} onClick={onCancel}>
              取消
            </Button>
          ) : null}
        </Space>
      </div>
    </div>
  );

  if (!blocking) {
    return (
      <div
        style={{
          position: 'fixed',
          right: 'max(16px, env(safe-area-inset-right))',
          bottom: floatingBottom,
          zIndex: 900,
          pointerEvents: 'none',
        }}
      >
        <div
          style={{
            width: collapsed ? 'min(320px, calc(100vw - 32px))' : 'min(360px, calc(100vw - 32px))',
            background: `linear-gradient(135deg, ${alphaColor(token.colorBgElevated, 0.98)} 0%, ${alphaColor(token.colorPrimaryBg, 0.84)} 100%)`,
            borderRadius: 18,
            padding: collapsed ? '10px 12px' : '13px 15px',
            boxShadow: `0 16px 34px ${alphaColor(token.colorText, 0.12)}`,
            boxSizing: 'border-box',
            pointerEvents: 'auto',
            transition: 'width 0.2s ease, padding 0.2s ease',
            border: `1px solid ${alphaColor(token.colorPrimary, 0.14)}`,
            backdropFilter: 'blur(18px)',
          }}
        >
          {collapsed ? (
            <div style={{ display: 'grid', gap: 8 }}>
              <div style={{ display: 'grid', gridTemplateColumns: '1fr auto', alignItems: 'start', gap: 10 }}>
                <div style={{ display: 'flex', alignItems: 'center', gap: 8, minWidth: 0 }}>
                  <Spin indicator={<LoadingOutlined style={{ fontSize: 16, color: 'var(--color-primary)' }} spin />} />
                  <div style={{ minWidth: 0 }}>
                    <div
                      style={{
                        fontSize: 10,
                        letterSpacing: '0.08em',
                        textTransform: 'uppercase',
                        color: token.colorTextTertiary,
                        lineHeight: 1.2,
                        marginBottom: 2,
                      }}
                    >
                      Background Run
                    </div>
                    <div
                      style={{
                        fontSize: 14,
                        fontWeight: 600,
                        color: token.colorTextHeading,
                        lineHeight: 1.35,
                        whiteSpace: 'nowrap',
                        overflow: 'hidden',
                        textOverflow: 'ellipsis',
                      }}
                    >
                      {title}
                    </div>
                  </div>
                </div>
                {showPercentage ? (
                  <span
                    style={{
                      padding: '2px 8px',
                      borderRadius: 999,
                      background: alphaColor(token.colorPrimary, 0.1),
                      fontSize: 12,
                      fontWeight: 700,
                      color: token.colorPrimary,
                      lineHeight: 1.5,
                    }}
                  >
                    {progress}%
                  </span>
                ) : null}
              </div>
              <div
                style={{
                  fontSize: 12,
                  color: token.colorTextSecondary,
                  lineHeight: 1.4,
                  whiteSpace: 'nowrap',
                  overflow: 'hidden',
                  textOverflow: 'ellipsis',
                }}
              >
                {queueSummary
                  ? `${message || '后台处理中，可继续其他操作'} · 共 ${activeTaskCount} 项`
                  : message || '后台处理中，可继续其他操作'}
              </div>
              <div style={{ display: 'grid', gridTemplateColumns: '1fr auto', alignItems: 'center', gap: 8 }}>
                <div
                  style={{
                    height: 5,
                    background: alphaColor(token.colorFillQuaternary, 0.96),
                    borderRadius: 999,
                    overflow: 'hidden',
                  }}
                >
                  <div
                    style={{
                      height: '100%',
                      width: `${progress}%`,
                      background: `linear-gradient(90deg, ${token.colorPrimary} 0%, ${token.colorPrimaryActive} 100%)`,
                      transition: 'width 0.3s ease',
                    }}
                  />
                </div>
                <Space size={2}>
                  {onCancel ? (
                    <Button
                      type="text"
                      danger
                      size="small"
                      icon={<StopOutlined />}
                      onClick={onCancel}
                    />
                  ) : null}
                  <Button
                    type="text"
                    size="small"
                    icon={<UnorderedListOutlined />}
                    onClick={openTaskCenter}
                    style={{ color: token.colorTextSecondary }}
                  />
                  <Button
                    type="text"
                    size="small"
                    icon={<UpOutlined />}
                    onClick={toggleCollapsed}
                    style={{ color: token.colorTextSecondary }}
                  />
                </Space>
              </div>
            </div>
          ) : (
            <>
              <div style={{ display: 'flex', justifyContent: 'flex-end', marginBottom: 8 }}>
                <Button
                  type="text"
                  size="small"
                  icon={<DownOutlined />}
                  onClick={toggleCollapsed}
                  style={{ color: token.colorTextSecondary }}
                />
              </div>
              {compactBackgroundContent}
            </>
          )}
        </div>
      </div>
    );
  }

  return (
    <Modal
      title={null}
      open={visible}
      footer={null}
      closable={false}
      centered
      width={500}
      mask
      maskClosable={false}
      keyboard={false}
      styles={{
        body: {
          padding: '24px',
          background: `linear-gradient(135deg, ${alphaColor(token.colorBgElevated, 0.98)} 0%, ${alphaColor(token.colorPrimaryBg, 0.88)} 100%)`,
          borderRadius: 24,
        },
        mask: {
          background: `radial-gradient(circle at top, ${alphaColor(token.colorPrimaryBg, 0.28)} 0%, rgba(0, 0, 0, 0.5) 60%)`,
        },
        content: {
          borderRadius: 28,
          overflow: 'hidden',
          border: `1px solid ${alphaColor(token.colorPrimary, 0.14)}`,
          boxShadow: `0 28px 56px ${alphaColor(token.colorText, 0.18)}`,
        },
      }}
    >
      {content}
    </Modal>
  );
};

export default SSEProgressModal;
