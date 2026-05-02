import { Divider, Empty, List, Progress, Space, Tag, Typography } from 'antd';
import type { TrackedBackgroundTask } from '../../../store/backgroundTasks';
import type { BackgroundTaskSection } from '../model/selectors';
import { groupBackgroundTasksByCategory } from '../model/selectors';
import { TaskActionButtons } from './TaskActionButtons';
import {
  extractFailureReasonTags,
  formatRelativeTime,
  getTaskCheckpointSummary,
  getTaskCheckpointTags,
  getTaskDestination,
  getTaskDisplayMessage,
  getTaskStatusMeta,
} from '../../../components/backgroundTaskPresentation';
import { getBatchManualReviewInfo } from '../../../services/modularApi';
import { getTaskTypeLabel } from '../../../store/backgroundTasks';
import { formatActiveStoryRepairLabel } from '../../../utils/activeStoryRepair';

const { Text } = Typography;

export const TaskSectionList = (props: {
  sections: BackgroundTaskSection[];
  onNavigate: (to: string) => void;
  canCancelTask: (task: TrackedBackgroundTask) => boolean;
  canResumeTask: (task: TrackedBackgroundTask) => boolean;
  cancellingTaskIds: Record<string, boolean>;
  resumingTaskIds: Record<string, boolean>;
  onCancel: (task: TrackedBackgroundTask) => void;
  onResume: (task: TrackedBackgroundTask) => void;
  onRemove: (taskId: string) => void;
}) => {
  const {
    sections,
    onNavigate,
    canCancelTask,
    canResumeTask,
    cancellingTaskIds,
    resumingTaskIds,
    onCancel,
    onResume,
    onRemove,
  } = props;

  const renderTaskItem = (task: TrackedBackgroundTask, accent: BackgroundTaskSection['accent']) => {
    const status = getTaskStatusMeta(task);
    const manualReviewInfo = task.status === 'failed' && (task.taskType === 'chapters_batch_generate' || task.taskType === 'chapter_single_generate')
      ? getBatchManualReviewInfo(
        task.failedChapters,
        task.error,
        task.terminalReason,
        task.terminalLabel,
        task.reviewRequired,
      )
      : null;
    const hasError = task.status === 'failed' && Boolean(task.error || manualReviewInfo?.message);
    const failureReasonTags = task.status === 'failed' ? extractFailureReasonTags(task) : [];
    const checkpointSummary = getTaskCheckpointSummary(task);
    const checkpointTags = getTaskCheckpointTags(task);
    const targetRoute = getTaskDestination(task);

    return (
      <List.Item
        key={task.taskId}
        style={{
          marginBottom: 12,
          border: accent === 'current'
            ? '1px solid rgba(22, 119, 255, 0.25)'
            : accent === 'global'
              ? '1px solid rgba(114, 46, 209, 0.18)'
              : '1px solid var(--color-border-secondary)',
          background: accent === 'current'
            ? 'rgba(22, 119, 255, 0.03)'
            : accent === 'global'
              ? 'rgba(114, 46, 209, 0.03)'
              : '#fff',
          borderRadius: 8,
          padding: 12,
          display: 'block',
        }}
      >
        <Space direction="vertical" size={8} style={{ width: '100%' }}>
          <Space style={{ width: '100%', justifyContent: 'space-between', alignItems: 'flex-start' }}>
            <Space direction="vertical" size={2} style={{ maxWidth: '60%' }}>
              <Text strong>{getTaskTypeLabel(task.taskType)}</Text>
              <Text type="secondary" style={{ fontSize: 12 }}>
                {task.projectId ? `项目任务 · ${formatRelativeTime(task.updatedAt)}` : `全局任务 · ${formatRelativeTime(task.updatedAt)}`}
              </Text>
            </Space>
            <Space size={6} wrap>
              {task.executionMode === 'auto' ? <Tag color="geekblue">全自动</Tag> : <Tag>交互</Tag>}
              {task.stageCode ? <Tag color="purple">{task.stageCode}</Tag> : null}
              <Tag color={status.color}>{status.label}</Tag>
            </Space>
          </Space>

          <Progress
            percent={task.progress}
            size="small"
            status={
              task.status === 'failed'
                ? (manualReviewInfo ? 'normal' : 'exception')
                : task.status === 'completed'
                  ? 'success'
                  : 'active'
            }
          />

          <Text type="secondary" style={{ fontSize: 12 }}>
            {getTaskDisplayMessage(task)}
          </Text>

          {task.workflowScope ? (
            <Text type="secondary" style={{ fontSize: 12 }}>
              范围：{task.workflowScope}
            </Text>
          ) : null}

          {checkpointSummary ? (
            <Text type="secondary" style={{ fontSize: 12 }}>
              {checkpointSummary}
            </Text>
          ) : null}

          {checkpointTags.length > 0 ? (
            <Space size={[6, 6]} wrap>
              {checkpointTags.map((tag) => (
                <Tag key={`${task.taskId}-${tag.label}`} color={tag.color}>
                  {tag.label}
                </Tag>
              ))}
            </Space>
          ) : null}

          {formatActiveStoryRepairLabel(task.activeStoryRepairPayload) ? (
            <Text type="secondary" style={{ fontSize: 12 }}>
              {formatActiveStoryRepairLabel(task.activeStoryRepairPayload)}
            </Text>
          ) : null}

          {hasError ? (
            <Space direction="vertical" size={6} style={{ width: '100%' }}>
              {failureReasonTags.length > 0 ? (
                <Space size={[6, 6]} wrap>
                  {failureReasonTags.map((tag) => (
                    <Tag key={`${task.taskId}-${tag.label}`} color={tag.color}>
                      {tag.label}
                    </Tag>
                  ))}
                </Space>
              ) : null}
              <Text type={manualReviewInfo ? 'warning' : 'danger'} style={{ fontSize: 12 }}>
                {manualReviewInfo?.message ?? task.error}
              </Text>
            </Space>
          ) : null}

          <TaskActionButtons
            task={task}
            targetRoute={targetRoute}
            onNavigate={onNavigate}
            canCancelTask={canCancelTask}
            canResumeTask={canResumeTask}
            cancelling={Boolean(cancellingTaskIds[task.taskId])}
            resuming={Boolean(resumingTaskIds[task.taskId])}
            onCancel={() => onCancel(task)}
            onResume={() => onResume(task)}
            onRemove={() => onRemove(task.taskId)}
          />
        </Space>
      </List.Item>
    );
  };

  if (sections.length === 0) {
    return <Empty description="暂无后台任务" />;
  }

  return (
    <>
      {sections.map((section, index) => (
        <div key={section.key} style={{ marginBottom: 8 }}>
          {index > 0 ? <Divider style={{ margin: '12px 0' }} /> : null}
          <Space direction="vertical" size={4} style={{ width: '100%', marginBottom: 8 }}>
            <Space style={{ width: '100%', justifyContent: 'space-between' }}>
              <Text strong>{section.title}</Text>
              <Tag>{section.tasks.length}</Tag>
            </Space>
            <Text type="secondary" style={{ fontSize: 12 }}>
              {section.description}
            </Text>
          </Space>

          {section.tasks.length === 0 ? (
            <Empty image={Empty.PRESENTED_IMAGE_SIMPLE} description="暂无任务" />
          ) : (
            (() => {
              const groups = groupBackgroundTasksByCategory(section.tasks);
              return groups.map((group) => (
                <div key={`${section.key}-${group.key}`} style={{ marginBottom: 12 }}>
                  {groups.length > 1 ? (
                    <div style={{ marginBottom: 8 }}>
                      <Text type="secondary" style={{ fontSize: 12, fontWeight: 600 }}>
                        {group.title}
                      </Text>
                    </div>
                  ) : null}
                  <List
                    dataSource={group.tasks}
                    rowKey={(task) => task.taskId}
                    split={false}
                    renderItem={(task) => renderTaskItem(task, section.accent)}
                  />
                </div>
              ));
            })()
          )}
        </div>
      ))}
    </>
  );
};
