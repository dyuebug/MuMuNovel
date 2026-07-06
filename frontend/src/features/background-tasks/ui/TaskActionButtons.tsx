import { Button, Typography, theme } from 'antd';
import {
  CheckCircleOutlined,
  CloseCircleOutlined,
  LoadingOutlined,
  RedoOutlined,
  StopOutlined,
} from '@ant-design/icons';
import type { TrackedBackgroundTask } from '../../../store/backgroundTasks';

const { Text } = Typography;

export const TaskActionButtons = (props: {
  task: TrackedBackgroundTask;
  targetRoute: string | null;
  onNavigate: (to: string) => void;
  canCancelTask: (task: TrackedBackgroundTask) => boolean;
  canResumeTask: (task: TrackedBackgroundTask) => boolean;
  cancelling: boolean;
  resuming: boolean;
  onCancel: () => void;
  onResume: () => void;
  onRemove: () => void;
}) => {
  const {
    task,
    targetRoute,
    onNavigate,
    canCancelTask,
    canResumeTask,
    cancelling,
    resuming,
    onCancel,
    onResume,
    onRemove,
  } = props;

  const active = task.status === 'running' || task.status === 'pending';
  const { token } = theme.useToken();
  const alphaColor = (color: string, alpha: number) => `color-mix(in srgb, ${color} ${(alpha * 100).toFixed(0)}%, transparent)`;
  const resumable = canResumeTask(task);

  const actionFocus = active
    ? {
        title: targetRoute ? '先回到对应工作区，再决定是否取消' : '当前任务仍在执行，可直接处理',
        note: targetRoute
          ? '适合先进入对应页面确认上下文，再决定继续观察还是中断当前队列。'
          : canCancelTask(task)
            ? '当前任务还在运行或排队，必要时可以直接取消，不会改变现有任务中心逻辑。'
            : '当前任务不提供取消能力，若不再需要可直接移除这条记录。',
        tone: alphaColor(token.colorPrimaryBg, 0.88),
        border: alphaColor(token.colorPrimary, 0.16),
      }
    : resumable
      ? {
          title: '失败或中断任务可优先恢复',
          note: targetRoute
            ? '建议先看对应工作区，再执行继续操作，这样更容易确认恢复前后的上下文。'
            : '当前记录支持继续处理，适合先尝试恢复，再决定是否移除历史记录。',
          tone: alphaColor(token.colorWarningBg, 0.88),
          border: alphaColor(token.colorWarning, 0.18),
        }
      : {
          title: '任务已结束，可选择保留或清理记录',
          note: targetRoute
            ? '如果还需要复盘过程，可以先前往对应页面；确认无须保留后再移除记录。'
            : '当前任务已经完成或终止，主要动作就是清理这条历史记录。',
          tone: alphaColor(token.colorFillQuaternary, 0.8),
          border: alphaColor(token.colorBorderSecondary, 0.86),
        };

  return (
    <div style={{ display: 'grid', gap: 10 }}>
      <div
        style={{
          padding: '12px 14px',
          borderRadius: 16,
          background: actionFocus.tone,
          border: `1px solid ${actionFocus.border}`,
        }}
      >
        <Text style={{ display: 'block', fontSize: 11, letterSpacing: '0.08em', textTransform: 'uppercase', color: token.colorTextTertiary, marginBottom: 6 }}>
          Action Focus
        </Text>
        <Text strong style={{ display: 'block', fontSize: 14, marginBottom: 6 }}>
          {actionFocus.title}
        </Text>
        <Text type="secondary" style={{ display: 'block', fontSize: 12, lineHeight: 1.7 }}>
          {actionFocus.note}
        </Text>
      </div>

      <div style={{ display: 'flex', flexWrap: 'wrap', gap: 8 }}>
        {targetRoute ? (
          <Button
            size="small"
            type={active ? 'primary' : 'default'}
            onClick={() => onNavigate(targetRoute)}
            style={!active ? {
              borderColor: alphaColor(token.colorPrimary, 0.18),
              background: alphaColor(token.colorBgElevated, 0.96),
            } : undefined}
          >
            前往
          </Button>
        ) : null}

        {active ? (
          canCancelTask(task) ? (
            <Button
              size="small"
              danger
              icon={cancelling ? <LoadingOutlined /> : <StopOutlined />}
              loading={cancelling}
              onClick={onCancel}
            >
              取消
            </Button>
          ) : (
            <Button size="small" icon={<CloseCircleOutlined />} onClick={onRemove}>
              移除
            </Button>
          )
        ) : (
          <>
            {resumable ? (
              <Button
                size="small"
                type="primary"
                icon={resuming ? <LoadingOutlined /> : <RedoOutlined />}
                loading={resuming}
                onClick={onResume}
              >
                继续
              </Button>
            ) : null}
            <Button
              size="small"
              icon={task.status === 'completed' ? <CheckCircleOutlined /> : <CloseCircleOutlined />}
              onClick={onRemove}
            >
              移除
            </Button>
          </>
        )}
      </div>
    </div>
  );
};
