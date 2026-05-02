import { Segmented, Space, Tag, Typography } from 'antd';
import type { TaskFilter } from '../model/selectors';
import type { BackgroundTaskCenterSummary } from '../model/summary';

const { Text } = Typography;

export const TaskSummaryBar = (props: {
  focusProjectId: string | null;
  taskFilter: TaskFilter;
  filterOptions: Array<{ label: string; value: TaskFilter }>;
  onChangeFilter: (next: TaskFilter) => void;
  summary: BackgroundTaskCenterSummary;
  activeCount: number;
}) => {
  const { focusProjectId, taskFilter, filterOptions, onChangeFilter, summary, activeCount } = props;

  return (
    <Space direction="vertical" size={12} style={{ width: '100%', marginBottom: 16 }}>
      <Space wrap>
        <Tag color="processing">进行中 {activeCount}</Tag>
        <Tag color="blue">当前项目 {summary.currentProjectActiveCount}</Tag>
        {summary.failedTaskCount > 0 ? <Tag color="error">失败 {summary.failedTaskCount}</Tag> : null}
        {summary.otherActiveCount > 0 ? <Tag>其他项目 {summary.otherActiveCount}</Tag> : null}
        {summary.terminalTaskCount > 0 ? <Tag color="default">已结束 {summary.terminalTaskCount}</Tag> : null}
      </Space>

      <Segmented
        block
        size="small"
        value={taskFilter}
        onChange={(value) => onChangeFilter(value as TaskFilter)}
        options={filterOptions}
      />

      {focusProjectId ? (
        <Text type="secondary" style={{ fontSize: 12 }}>
          {taskFilter === 'current'
            ? '仅展示当前项目任务，方便专注处理本项目。'
            : taskFilter === 'active'
              ? '仅展示仍在排队或执行中的任务。'
              : taskFilter === 'failed'
                ? '仅展示失败任务，便于集中排查和恢复。'
              : '当前项目任务会优先显示，避免在多项目并行时被其他任务淹没。'}
        </Text>
      ) : (
        <Text type="secondary" style={{ fontSize: 12 }}>
          {taskFilter === 'active'
            ? '当前视图仅保留进行中的后台任务。'
            : taskFilter === 'failed'
              ? '当前视图仅保留失败任务。'
            : '这里汇总所有后台任务；进入项目页后会自动优先展示当前项目任务。'}
        </Text>
      )}
    </Space>
  );
};
