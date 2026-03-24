import { Alert, Button, Card, Empty, Progress, Space, Typography } from 'antd';
import { ReloadOutlined, StopOutlined } from '@ant-design/icons';
import type { BookImportTask } from '../types';

type BookImportTaskStatusStepProps = {
  taskId: string | null;
  taskStatus: BookImportTask | null;
  onRefreshStatus: () => void;
  onCancelTask: () => void;
};

const { Text } = Typography;

export default function BookImportTaskStatusStep({
  taskId,
  taskStatus,
  onRefreshStatus,
  onCancelTask,
}: BookImportTaskStatusStepProps) {
  return (
    <Card title="解析任务状态" style={{ marginBottom: 16 }}>
      {!taskId ? (
        <Empty description="尚未创建任务" />
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

          <Space style={{ marginTop: 24 }}>
            <Button icon={<ReloadOutlined />} onClick={onRefreshStatus}>刷新状态</Button>
            {taskStatus && ['pending', 'running'].includes(taskStatus.status) ? (
              <Button danger icon={<StopOutlined />} onClick={onCancelTask}>取消任务</Button>
            ) : null}
          </Space>
        </div>
      )}
    </Card>
  );
}
