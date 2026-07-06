import { Alert, Button, Card, List, Progress, Space, Tag, Typography, theme } from 'antd';
import { RedoOutlined, WarningOutlined } from '@ant-design/icons';
import type { BookImportStepFailure } from '../types';
import { designDisplayFont } from '../theme/themeConfig';
import { renderCompactSettingHint } from './storyCreationCommonUi';

const { Text, Paragraph, Title } = Typography;

type BookImportProgressStepProps = {
  applyProgress: number;
  applyMessage: string;
  applyError: string | null;
  failedSteps: BookImportStepFailure[];
  isApplyComplete: boolean;
  retryProgress: number;
  retrying: boolean;
  retryMessage: string;
  onRetryFailedSteps: () => void;
  onSkipFailedSteps: () => void;
};

export default function BookImportProgressStep({
  applyProgress,
  applyMessage,
  applyError,
  failedSteps,
  isApplyComplete,
  retryProgress,
  retrying,
  retryMessage,
  onRetryFailedSteps,
  onSkipFailedSteps,
}: BookImportProgressStepProps) {
  const { token } = theme.useToken();
  const alphaColor = (color: string, alpha: number) => `color-mix(in srgb, ${color} ${(alpha * 100).toFixed(0)}%, transparent)`;
  const heroBackground = `linear-gradient(135deg,
    color-mix(in srgb, ${token.colorPrimary} 80%, #704734 20%) 0%,
    color-mix(in srgb, ${token.colorInfo} 30%, #1f262e 70%) 100%)`;
  const quietPanelBackground = `linear-gradient(180deg,
    color-mix(in srgb, ${token.colorBgContainer} 96%, white 4%) 0%,
    color-mix(in srgb, ${token.colorFillAlter} 34%, ${token.colorBgContainer} 66%) 100%)`;
  const panelBorder = `1px solid ${alphaColor(token.colorBorderSecondary, 0.9)}`;
  const progressGuideSteps = [
    '先看导入主进度和当前消息，判断现在是在写入、收尾还是失败补跑阶段。',
    '再检查失败步骤列表，确认哪些环节需要重试，哪些可以跳过继续推进。',
    '最后再执行补跑或跳过；原有失败步骤重试与继续导入逻辑保持不变。',
  ];
  const currentPercent = Math.min(100, Math.max(0, Math.round(retrying ? retryProgress : applyProgress)));
  const progressTone = applyError
    ? 'error'
    : (failedSteps.length > 0 && isApplyComplete && !retrying)
      ? 'warning'
      : isApplyComplete && failedSteps.length === 0
        ? 'success'
        : 'processing';
  const workspaceFocus = applyError
    ? {
        title: '导入主流程已被错误打断',
        note: '适合先读错误详情，再决定是否回退到上一步或修正当前输入。',
      }
    : retrying
      ? {
          title: '失败步骤正在补跑',
          note: '当前优先观察补跑进度与重试消息，避免重复触发多次重试。',
        }
      : failedSteps.length > 0 && isApplyComplete
        ? {
            title: `还有 ${failedSteps.length} 个失败步骤待决策`,
            note: '主导入流程已结束，但仍需决定哪些步骤补跑、哪些步骤跳过。',
          }
        : {
            title: isApplyComplete ? '导入已经完成' : '导入仍在持续推进',
            note: isApplyComplete
              ? '可以准备进入章节页做最终校对与润色。'
              : '当前适合继续观察写入进度，等待章节与结构全部落库。',
          };

  return (
    <div style={{ marginBottom: 16 }}>
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
        <Text style={{ color: 'rgba(255,255,255,0.68)', letterSpacing: '0.14em', textTransform: 'uppercase' }}>
          Import Run
        </Text>
        <Title
          level={5}
          style={{
            margin: '8px 0 10px',
            color: '#f7f1e8',
            fontFamily: designDisplayFont,
            letterSpacing: '-0.03em',
          }}
        >
          导入进度与失败步骤决策工作台
        </Title>
        <Paragraph style={{ margin: 0, color: 'rgba(255,255,255,0.84)', lineHeight: 1.75 }}>
          这里负责把写入进度、失败步骤和补跑状态说清楚。原有导入进度推进、失败步骤重试和跳过逻辑保持不变，这里只强化阅读顺序和决策焦点。
        </Paragraph>
        <Space wrap size={[8, 8]} style={{ marginTop: 16 }}>
          <Tag color={progressTone} style={{ margin: 0, borderRadius: 999, paddingInline: 10 }}>
            {`当前进度 ${currentPercent}%`}
          </Tag>
          <Tag color={failedSteps.length > 0 ? 'gold' : 'green'} style={{ margin: 0, borderRadius: 999, paddingInline: 10 }}>
            {failedSteps.length > 0 ? `${failedSteps.length} 个失败步骤` : '暂无失败步骤'}
          </Tag>
          <Tag color={retrying ? 'processing' : 'default'} style={{ margin: 0, borderRadius: 999, paddingInline: 10 }}>
            {retrying ? '正在补跑失败步骤' : (isApplyComplete ? '导入收尾完成' : '主流程进行中')}
          </Tag>
        </Space>
      </Card>

      <Card
        bordered={false}
        style={{
          marginBottom: 16,
          borderRadius: 18,
          background: `linear-gradient(135deg, color-mix(in srgb, ${token.colorPrimary} 8%, white 92%) 0%, color-mix(in srgb, ${token.colorWarning} 8%, white 92%) 100%)`,
          border: `1px solid color-mix(in srgb, ${token.colorPrimary} 14%, white 86%)`,
        }}
        styles={{ body: { padding: 18 } }}
      >
        <Text style={{ fontSize: 12, color: token.colorTextTertiary, letterSpacing: '0.12em', textTransform: 'uppercase' }}>
          Progress Guide
        </Text>
        <Paragraph style={{ margin: '8px 0 0', color: token.colorText, lineHeight: 1.75 }}>
          先看主进度，再判断是否进入失败步骤决策，最后再执行补跑或跳过。这里只增强工作流阅读顺序，不改变原有导入控制逻辑。
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
                lineHeight: 1.5,
              }}
            >
              <span style={{ color: token.colorPrimary, fontWeight: 700 }}>{index + 1}</span>
              {item}
            </span>
          ))}
        </div>
      </Card>

      <Card
        bordered={false}
        style={{
          borderRadius: 24,
          border: panelBorder,
          background: quietPanelBackground,
        }}
        styles={{ body: { padding: 20 } }}
      >
        <div style={{ marginBottom: 16 }}>
          <Text style={{ fontSize: 12, color: token.colorTextTertiary, letterSpacing: '0.12em', textTransform: 'uppercase' }}>
            Progress Workspace
          </Text>
          <Title level={5} style={{ margin: '6px 0 8px', fontFamily: designDisplayFont }}>
            {workspaceFocus.title}
          </Title>
          <Paragraph style={{ margin: 0, color: token.colorTextSecondary, lineHeight: 1.75 }}>
            {workspaceFocus.note}
          </Paragraph>
        </div>

        <Progress
          percent={currentPercent}
          status={
            applyError
              ? 'exception'
              : (failedSteps.length > 0 && isApplyComplete && !retrying)
                ? 'exception'
                : isApplyComplete && failedSteps.length === 0
                  ? 'success'
                  : 'active'
          }
          strokeColor={{
            '0%': token.colorPrimary,
            '100%': failedSteps.length > 0 ? '#faad14' : token.colorInfo,
          }}
          style={{ marginBottom: 24 }}
        />

        <Paragraph
          style={{
            fontSize: 16,
            marginBottom: 24,
            color: applyError
              ? token.colorError
              : (failedSteps.length > 0 && isApplyComplete && !retrying)
                ? '#faad14'
                : token.colorTextSecondary,
            textAlign: 'center',
          }}
        >
          {retrying ? retryMessage : (applyError || applyMessage)}
        </Paragraph>

        {applyError ? (
          <Alert
            type="error"
            message="导入失败"
            description={applyError}
            showIcon
            style={{ textAlign: 'left', marginBottom: 24 }}
          />
        ) : null}

        {failedSteps.length > 0 && isApplyComplete && !retrying ? (
          <div style={{ textAlign: 'left', marginBottom: 24 }}>
            <Alert
              type="warning"
              icon={<WarningOutlined />}
              showIcon
              message={`${failedSteps.length} 个步骤失败`}
              description={
                <div>
                  <Paragraph style={{ marginBottom: 12, color: token.colorTextSecondary }}>
                    以下步骤执行失败。你可以重试失败步骤，或跳过失败步骤继续完成导入。
                  </Paragraph>
                  <List
                    size="small"
                    bordered
                    dataSource={failedSteps}
                    renderItem={(item) => (
                      <List.Item style={{ padding: '8px 12px' }}>
                        <List.Item.Meta
                          title={
                            <Space>
                              <Tag color="error">{item.step_label}</Tag>
                              {(item.retry_count ?? 0) > 0 ? <Tag color="orange">已重试 {item.retry_count} 次</Tag> : null}
                            </Space>
                          }
                          description={
                            <Text type="secondary" style={{ fontSize: 12 }}>
                              {item.error.length > 120 ? `${item.error.slice(0, 120)}...` : item.error}
                            </Text>
                          }
                        />
                      </List.Item>
                    )}
                  />
                  <Space style={{ marginTop: 16, display: 'flex', justifyContent: 'center' }} wrap>
                    <Button type="primary" icon={<RedoOutlined />} onClick={onRetryFailedSteps} loading={retrying} style={{ borderRadius: 12 }}>
                      重试失败步骤
                    </Button>
                    <Button onClick={onSkipFailedSteps} style={{ borderRadius: 12 }}>跳过失败步骤</Button>
                  </Space>
                </div>
              }
              style={{ marginBottom: 16 }}
            />
          </div>
        ) : null}

        {retrying ? (
          <div style={{ marginBottom: 24 }}>
            {renderCompactSettingHint(
              '失败步骤正在补跑',
              `${retryMessage} 当前更适合继续观察补跑结果，避免重复触发多次重试；导入主流程与补跑逻辑保持不变。`,
              {
                style: {
                  marginBottom: 0,
                },
              },
            )}
          </div>
        ) : null}

        {!failedSteps.length && !retrying ? (
          <div
            style={{
              background: alphaColor(token.colorFillQuaternary, 0.84),
              padding: 16,
              borderRadius: 18,
              textAlign: 'left',
              marginTop: 24,
            }}
          >
            <Text type="secondary" style={{ fontSize: 13 }}>
              导入过程中系统会自动：<br />
              - 创建或更新项目基础信息<br />
              - 写入章节标题、摘要与正文<br />
              - 保留解析得到的结构与顺序<br />
              - 支持失败步骤单独重试<br />
              {isApplyComplete ? '导入已完成，可前往章节页继续校对与润色。' : '导入完成后，可前往章节页继续校对与润色。'}
            </Text>
          </div>
        ) : null}
      </Card>
    </div>
  );
}
