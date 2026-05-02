import { Button, Space } from 'antd';
import {
  CheckCircleOutlined,
  CloseCircleOutlined,
  LoadingOutlined,
  RedoOutlined,
  StopOutlined,
} from '@ant-design/icons';
import type { TrackedBackgroundTask } from '../../../store/backgroundTasks';

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

  return (
    <Space size={8} wrap>
      {targetRoute ? (
        <Button size="small" onClick={() => onNavigate(targetRoute)}>
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
          {canResumeTask(task) ? (
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
    </Space>
  );
};
