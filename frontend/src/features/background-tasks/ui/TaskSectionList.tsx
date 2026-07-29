import { Empty, List, Progress, Space, Tag, Typography, theme } from 'antd';
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
  const { token } = theme.useToken();
  const alphaColor = (color: string, alpha: number) => `color-mix(in srgb, ${color} ${(alpha * 100).toFixed(0)}%, transparent)`;

  const getSectionAccentStyles = (accent: BackgroundTaskSection['accent']) => {
    if (accent === 'current') {
      return {
        eyebrow: 'Current Project Queue',
        border: alphaColor(token.colorPrimary, 0.2),
        shell: `linear-gradient(135deg, ${alphaColor(token.colorPrimaryBg, 0.84)} 0%, ${alphaColor(token.colorBgContainer, 0.98)} 100%)`,
        badge: alphaColor(token.colorPrimary, 0.12),
      };
    }

    if (accent === 'global') {
      return {
        eyebrow: 'Global Queue',
        border: alphaColor(token.colorInfo, 0.18),
        shell: `linear-gradient(135deg, ${alphaColor(token.colorInfoBg, 0.84)} 0%, ${alphaColor(token.colorBgContainer, 0.98)} 100%)`,
        badge: alphaColor(token.colorInfo, 0.1),
      };
    }

    return {
      eyebrow: 'Recent Archive',
      border: alphaColor(token.colorBorderSecondary, 0.92),
      shell: `linear-gradient(180deg, ${alphaColor(token.colorBgElevated, 0.98)} 0%, ${alphaColor(token.colorBgContainer, 0.98)} 100%)`,
      badge: alphaColor(token.colorFillSecondary, 0.85),
    };
  };

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
    const repairLabel = formatActiveStoryRepairLabel(task.activeStoryRepairPayload);
    const accentStyles = getSectionAccentStyles(accent);
    const progressStatus =
      task.status === 'failed'
        ? (manualReviewInfo ? 'normal' : 'exception')
        : task.status === 'completed'
          ? 'success'
          : 'active';

    return (
      <List.Item
        key={task.taskId}
        style={{
          marginBottom: 10,
          border: `1px solid ${accentStyles.border}`,
          background: accentStyles.shell,
          borderRadius: 18,
          padding: 14,
          display: 'block',
          boxShadow: `0 10px 24px ${alphaColor(token.colorText, 0.05)}`,
        }}
      >
        <div style={{ display: 'grid', gap: 10 }}>
          <div
            style={{
              display: 'flex',
              flexWrap: 'wrap',
              gap: 10,
              alignItems: 'center',
            }}
          >
            <div style={{ minWidth: 0, flex: '1 1 180px' }}>
              <Text style={{ display: 'block', fontSize: 11, letterSpacing: '0.08em', textTransform: 'uppercase', color: token.colorTextTertiary, marginBottom: 6 }}>
                Task Dossier
              </Text>
              <Text strong style={{ display: 'block', fontSize: 16, marginBottom: 4 }}>
                {getTaskTypeLabel(task.taskType)}
              </Text>
              <Text type="secondary" style={{ display: 'block', fontSize: 12, lineHeight: 1.5 }}>
                {task.projectId ? `项目任务 · ${formatRelativeTime(task.updatedAt)}` : `全局任务 · ${formatRelativeTime(task.updatedAt)}`}
              </Text>
            </div>
            <div style={{ display: 'flex', flex: '0 1 auto', flexWrap: 'wrap', justifyContent: 'flex-start', gap: 6 }}>
              {task.executionMode === 'auto' ? <Tag color="geekblue">全自动</Tag> : <Tag>交互</Tag>}
              {task.stageCode ? <Tag color="purple">{task.stageCode}</Tag> : null}
              <Tag color={status.color}>{status.label}</Tag>
            </div>
          </div>

          <div
            style={{
              padding: 12,
              borderRadius: 16,
              background: alphaColor(token.colorBgElevated, 0.94),
              border: `1px solid ${alphaColor(token.colorBorderSecondary, 0.86)}`,
            }}
          >
            <div
              style={{
                display: 'grid',
                gridTemplateColumns: 'minmax(0, 1fr)',
                gap: 10,
                alignItems: 'start',
              }}
            >
              <div style={{ minWidth: 0 }}>
                <Text style={{ display: 'block', fontSize: 11, letterSpacing: '0.08em', textTransform: 'uppercase', color: token.colorTextTertiary, marginBottom: 6 }}>
                  Current Focus
                </Text>
                <Text strong style={{ display: 'block', fontSize: 15, marginBottom: 4 }}>
                  {status.label} · {task.progress}%
                </Text>
                <Text type="secondary" style={{ display: 'block', fontSize: 12, lineHeight: 1.55, marginBottom: 6 }}>
                  {getTaskDisplayMessage(task)}
                </Text>
                {task.workflowScope ? (
                  <Text type="secondary" style={{ display: 'block', fontSize: 12, lineHeight: 1.7, marginBottom: 4 }}>
                    范围：{task.workflowScope}
                  </Text>
                ) : null}
                {checkpointSummary ? (
                  <Text type="secondary" style={{ display: 'block', fontSize: 12, lineHeight: 1.7, marginBottom: repairLabel ? 4 : 0 }}>
                    {checkpointSummary}
                  </Text>
                ) : null}
                {repairLabel ? (
                  <Text type="secondary" style={{ display: 'block', fontSize: 12, lineHeight: 1.7 }}>
                    {repairLabel}
                  </Text>
                ) : null}
              </div>

              <div
                style={{
                  padding: '10px 12px',
                  borderRadius: 14,
                  background: accentStyles.badge,
                  border: `1px solid ${alphaColor(token.colorPrimary, 0.1)}`,
                }}
              >
                <Text style={{ display: 'block', fontSize: 11, letterSpacing: '0.08em', textTransform: 'uppercase', color: token.colorTextTertiary, marginBottom: 10 }}>
                  Execution Pulse
                </Text>
                <Progress percent={task.progress} size="small" status={progressStatus} strokeWidth={6} />
              </div>
            </div>
          </div>

          {checkpointTags.length > 0 ? (
            <div
              style={{
                padding: '10px 12px',
                borderRadius: 14,
                background: alphaColor(token.colorFillQuaternary, 0.74),
              }}
            >
              <Text style={{ display: 'block', fontSize: 11, letterSpacing: '0.08em', textTransform: 'uppercase', color: token.colorTextTertiary, marginBottom: 8 }}>
                Checkpoints
              </Text>
              <Space size={[6, 6]} wrap>
                {checkpointTags.map((tag) => (
                  <Tag key={`${task.taskId}-${tag.label}`} color={tag.color}>
                    {tag.label}
                  </Tag>
                ))}
              </Space>
            </div>
          ) : null}

          {hasError ? (
            <div
              style={{
                padding: '14px 16px',
                borderRadius: 18,
                background: manualReviewInfo
                  ? alphaColor(token.colorWarningBg, 0.92)
                  : alphaColor(token.colorErrorBg, 0.92),
                border: `1px solid ${manualReviewInfo ? alphaColor(token.colorWarning, 0.24) : alphaColor(token.colorError, 0.22)}`,
              }}
            >
              {failureReasonTags.length > 0 ? (
                <Space size={[6, 6]} wrap style={{ marginBottom: 8 }}>
                  {failureReasonTags.map((tag) => (
                    <Tag key={`${task.taskId}-${tag.label}`} color={tag.color}>
                      {tag.label}
                    </Tag>
                  ))}
                </Space>
              ) : null}
              <Text type={manualReviewInfo ? 'warning' : 'danger'} style={{ fontSize: 12, lineHeight: 1.75 }}>
                {manualReviewInfo?.message ?? task.error}
              </Text>
            </div>
          ) : null}

          <div
            style={{
              padding: '10px 12px',
              borderRadius: 14,
              background: alphaColor(token.colorBgContainer, 0.96),
              border: `1px solid ${alphaColor(token.colorBorderSecondary, 0.86)}`,
            }}
          >
            <Text style={{ display: 'block', fontSize: 11, letterSpacing: '0.08em', textTransform: 'uppercase', color: token.colorTextTertiary, marginBottom: 10 }}>
              Available Actions
            </Text>
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
          </div>
        </div>
      </List.Item>
    );
  };

  if (sections.length === 0) {
    return <Empty description="暂无后台任务" />;
  }

  return (
    <>
      {sections.map((section) => {
        const accentStyles = getSectionAccentStyles(section.accent);

        return (
          <div
            key={section.key}
            style={{
              marginBottom: 12,
              padding: 12,
              borderRadius: 18,
              background: accentStyles.shell,
              border: `1px solid ${accentStyles.border}`,
              boxShadow: `0 10px 24px ${alphaColor(token.colorText, 0.04)}`,
            }}
          >
            <div
              style={{
                display: 'grid',
                gridTemplateColumns: 'minmax(0, 1fr) auto',
                gap: 10,
                alignItems: 'start',
                marginBottom: 12,
              }}
            >
              <div style={{ minWidth: 0 }}>
                <Text style={{ display: 'block', fontSize: 11, letterSpacing: '0.08em', textTransform: 'uppercase', color: token.colorTextTertiary, marginBottom: 6 }}>
                  {accentStyles.eyebrow}
                </Text>
                <Text strong style={{ display: 'block', fontSize: 16, marginBottom: 4 }}>
                  {section.title}
                </Text>
                <Text type="secondary" style={{ display: 'block', fontSize: 12, lineHeight: 1.55 }}>
                  {section.description}
                </Text>
              </div>
              <div
                style={{
                  minWidth: 72,
                  padding: '8px 10px',
                  borderRadius: 14,
                  background: alphaColor(token.colorBgElevated, 0.94),
                  border: `1px solid ${accentStyles.border}`,
                  textAlign: 'center',
                }}
              >
                <Text style={{ display: 'block', fontSize: 11, letterSpacing: '0.08em', textTransform: 'uppercase', color: token.colorTextTertiary, marginBottom: 4 }}>
                  Visible
                </Text>
                <Text strong style={{ display: 'block', fontSize: 18 }}>
                  {section.tasks.length}
                </Text>
              </div>
            </div>

            {section.tasks.length === 0 ? (
              <div
                style={{
                  padding: '22px 18px',
                  borderRadius: 20,
                  background: alphaColor(token.colorBgElevated, 0.94),
                  border: `1px dashed ${accentStyles.border}`,
                }}
              >
                <Empty image={Empty.PRESENTED_IMAGE_SIMPLE} description="暂无任务" />
              </div>
            ) : (
              (() => {
                const groups = groupBackgroundTasksByCategory(section.tasks);

                return groups.map((group) => (
                  <div
                    key={`${section.key}-${group.key}`}
                    style={{
                      marginBottom: 10,
                      padding: 10,
                      borderRadius: 16,
                      background: alphaColor(token.colorBgElevated, 0.92),
                      border: `1px solid ${alphaColor(token.colorBorderSecondary, 0.84)}`,
                    }}
                  >
                    {groups.length > 1 ? (
                      <div
                        style={{
                          marginBottom: 8,
                          padding: '8px 10px',
                          borderRadius: 12,
                          background: alphaColor(token.colorFillQuaternary, 0.74),
                        }}
                      >
                        <Text style={{ display: 'block', fontSize: 11, letterSpacing: '0.08em', textTransform: 'uppercase', color: token.colorTextTertiary, marginBottom: 4 }}>
                          Task Cluster
                        </Text>
                        <Text strong style={{ fontSize: 14 }}>
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
        );
      })}
    </>
  );
};
