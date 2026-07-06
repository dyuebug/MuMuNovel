import { Alert, Button, Card, Empty, Progress, Space, Tag, Typography, theme } from 'antd';
import { ReloadOutlined, StopOutlined } from '@ant-design/icons';
import type { BookImportTask } from '../types';
import { designDisplayFont } from '../theme/themeConfig';

type BookImportTaskStatusStepProps = {
  taskId: string | null;
  taskStatus: BookImportTask | null;
  onRefreshStatus: () => void;
  onCancelTask: () => void;
};

const { Text, Paragraph, Title } = Typography;

export default function BookImportTaskStatusStep({
  taskId,
  taskStatus,
  onRefreshStatus,
  onCancelTask,
}: BookImportTaskStatusStepProps) {
  const { token } = theme.useToken();
  const alphaColor = (color: string, alpha: number) => `color-mix(in srgb, ${color} ${(alpha * 100).toFixed(0)}%, transparent)`;
  const heroBackground = `linear-gradient(135deg,
    color-mix(in srgb, ${token.colorPrimary} 80%, #704734 20%) 0%,
    color-mix(in srgb, ${token.colorInfo} 30%, #1f262e 70%) 100%)`;
  const quietPanelBackground = `linear-gradient(180deg,
    color-mix(in srgb, ${token.colorBgContainer} 96%, white 4%) 0%,
    color-mix(in srgb, ${token.colorFillAlter} 34%, ${token.colorBgContainer} 66%) 100%)`;
  const panelBorder = `1px solid ${alphaColor(token.colorBorderSecondary, 0.9)}`;
  const statusGuideSteps = [
    '先判断任务是否已经创建，再看当前处于等待、运行、完成还是失败阶段。',
    '再读进度和任务消息，决定是继续等待、刷新，还是手动取消本次解析。',
    '最后再进入预览或回退；原有刷新与取消任务逻辑保持不变。',
  ];
  const taskStatusLabel = taskStatus?.status === 'pending'
    ? '等待调度'
    : taskStatus?.status === 'running'
      ? '解析进行中'
      : taskStatus?.status === 'completed'
        ? '解析完成'
        : taskStatus?.status === 'failed'
          ? '解析失败'
          : taskStatus?.status === 'cancelled'
            ? '任务已取消'
            : '未开始';

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
          Parse Watch
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
          解析任务观察与切换前确认台
        </Title>
        <Paragraph style={{ margin: 0, color: 'rgba(255,255,255,0.84)', lineHeight: 1.75 }}>
          这一步负责把 TXT 解析阶段讲清楚。原有任务刷新、取消与状态轮询逻辑保持不变，这里只把“现在到了哪一步”表达得更清楚。
        </Paragraph>
        <Space wrap size={[8, 8]} style={{ marginTop: 16 }}>
          <Tag color={taskId ? 'blue' : 'default'} style={{ margin: 0, borderRadius: 999, paddingInline: 10 }}>
            {taskId ? '任务已创建' : '尚未创建任务'}
          </Tag>
          <Tag color={taskStatus?.status === 'failed' ? 'error' : (taskStatus?.status === 'completed' ? 'green' : 'processing')} style={{ margin: 0, borderRadius: 999, paddingInline: 10 }}>
            {taskStatusLabel}
          </Tag>
          <Tag color="default" style={{ margin: 0, borderRadius: 999, paddingInline: 10 }}>
            {`当前进度 ${taskStatus?.progress || 0}%`}
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
          Status Guide
        </Text>
        <Paragraph style={{ margin: '8px 0 0', color: token.colorText, lineHeight: 1.75 }}>
          先判断任务状态，再决定继续等待、刷新还是取消。这里只强化状态阅读顺序，不改变原有解析任务控制逻辑。
        </Paragraph>
        <div style={{ display: 'flex', flexWrap: 'wrap', gap: 8, marginTop: 12 }}>
          {statusGuideSteps.map((item, index) => (
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
            Status Workspace
          </Text>
          <Title level={5} style={{ margin: '6px 0 8px', fontFamily: designDisplayFont }}>
            当前解析任务工作区
          </Title>
          <Paragraph style={{ margin: 0, color: token.colorTextSecondary, lineHeight: 1.75 }}>
            这里保留原有进度、状态消息、报错和控制按钮，只把任务观察与下一步决策放进更稳定的工作流壳层。
          </Paragraph>
        </div>

        {!taskId ? (
          <Card
            variant="borderless"
            style={{
              borderRadius: 20,
              minHeight: 220,
              background: `linear-gradient(180deg, ${alphaColor(token.colorBgElevated, 0.98)} 0%, ${alphaColor(token.colorFillAlter, 0.8)} 100%)`,
              border: `1px dashed ${alphaColor(token.colorBorder, 0.88)}`,
              display: 'flex',
              alignItems: 'center',
              justifyContent: 'center',
            }}
            styles={{ body: { width: '100%' } }}
          >
            <Empty description="尚未创建任务" />
          </Card>
        ) : (
          <div style={{ textAlign: 'center', padding: '24px 0' }}>
            <Progress
              type="circle"
              percent={taskStatus?.progress || 0}
              status={
                taskStatus?.status === 'failed'
                  ? 'exception'
                  : taskStatus?.status === 'completed'
                    ? 'success'
                    : 'active'
              }
            />
            <div style={{ marginTop: 24 }}>
              <Text strong style={{ fontSize: 16 }}>
                {taskStatus?.status === 'pending' && '等待调度...'}
                {taskStatus?.status === 'running' && '正在解析TXT文件...'}
                {taskStatus?.status === 'completed' && '解析完成！正在生成预览...'}
                {taskStatus?.status === 'failed' && '解析失败'}
                {taskStatus?.status === 'cancelled' && '已取消'}
              </Text>
              {taskStatus?.message ? (
                <div style={{ marginTop: 8 }}>
                  <Text type="secondary">{taskStatus.message}</Text>
                </div>
              ) : null}
            </div>

            {taskStatus?.error ? (
              <Alert type="error" message={taskStatus.error} showIcon style={{ marginTop: 16, textAlign: 'left' }} />
            ) : null}

            <Space style={{ marginTop: 24 }} wrap>
              <Button icon={<ReloadOutlined />} onClick={onRefreshStatus} style={{ borderRadius: 12 }}>刷新状态</Button>
              {taskStatus && ['pending', 'running'].includes(taskStatus.status) ? (
                <Button danger icon={<StopOutlined />} onClick={onCancelTask} style={{ borderRadius: 12 }}>取消任务</Button>
              ) : null}
            </Space>
          </div>
        )}
      </Card>
    </div>
  );
}
