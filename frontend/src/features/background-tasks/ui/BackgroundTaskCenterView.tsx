import { Badge, Button, Drawer, FloatButton, Grid, Space, Tag, Typography, theme } from 'antd';
import { UnorderedListOutlined } from '@ant-design/icons';
import { TaskSummaryBar } from './TaskSummaryBar';
import { TaskSectionList } from './TaskSectionList';
import type { BackgroundTaskCenterController } from '../hooks/useBackgroundTaskCenterController';

const { useBreakpoint } = Grid;
const { Text, Title } = Typography;

export const BackgroundTaskCenterView = (props: {
  controller: BackgroundTaskCenterController;
}) => {
  const { controller } = props;
  const screens = useBreakpoint();
  const isMobile = !screens.md;
  const { token } = theme.useToken();
  const alphaColor = (color: string, alpha: number) => `color-mix(in srgb, ${color} ${(alpha * 100).toFixed(0)}%, transparent)`;
  const backgroundTaskWorkspaceFocus = controller.summary.failedTaskCount > 0
    ? {
        title: '优先处理失败与可恢复任务',
        note: '当前后台任务中心已经检测到失败项，更适合先切到失败视图或直接恢复可恢复任务，再继续看其他队列。',
      }
    : controller.summary.currentProjectActiveCount > 0
      ? {
          title: '跟进当前项目正在运行的任务',
          note: '当前项目还有活动任务，适合先观察本项目的执行节奏和恢复状态，避免创作流程被跨项目任务打散。',
        }
      : controller.taskFilter === 'active'
        ? {
            title: '聚焦进行中的后台队列',
            note: '当前视图只保留进行中的任务，适合用来确认哪些工作仍在排队或执行，不必在已完成记录里来回翻找。',
          }
        : controller.taskFilter === 'failed'
          ? {
              title: '集中复核失败任务的阻塞点',
              note: '当前过滤器已经锁定失败任务，适合先看阻塞原因与可恢复性，再决定恢复、取消还是移除记录。',
            }
          : controller.summary.terminalTaskCount > 0
            ? {
                title: '判断是否需要清理已结束任务',
                note: '当前已结束任务数量不为零，适合先确认哪些记录仍值得保留，再用现有清理动作整理工作台。',
              }
            : {
                title: '维持当前后台工作台的阅读秩序',
                note: '当前没有明显的失败阻塞，适合先按项目优先级浏览任务，再进入具体操作，保持工作区节奏稳定。',
              };

  return (
    <>
      {!controller.open ? (
        <Badge count={controller.activeTaskCount} size="small" offset={[-2, 8]}>
          <FloatButton
            icon={<UnorderedListOutlined />}
            type={controller.summary.currentProjectActiveCount > 0 ? 'primary' : controller.activeTaskCount > 0 ? 'default' : 'default'}
            tooltip={
              controller.summary.currentProjectActiveCount > 0
                ? `当前项目后台任务 (${controller.summary.currentProjectActiveCount})`
                : controller.activeTaskCount > 0
                  ? `后台任务进行中 (${controller.activeTaskCount})`
                  : '后台任务'
            }
            onClick={() => controller.setOpen(true)}
            style={{
              right: 24,
              bottom: 24,
              zIndex: 10001,
              background: controller.summary.currentProjectActiveCount > 0
                ? `linear-gradient(135deg, ${token.colorPrimary} 0%, ${token.colorPrimaryHover} 100%)`
                : `linear-gradient(135deg, ${token.colorBgElevated} 0%, ${alphaColor(token.colorPrimaryBg, 0.88)} 100%)`,
              boxShadow: controller.summary.currentProjectActiveCount > 0
                ? `0 18px 36px ${alphaColor(token.colorPrimary, 0.24)}`
                : `0 18px 36px ${alphaColor(token.colorText, 0.12)}`,
              border: `1px solid ${alphaColor(token.colorPrimary, 0.14)}`,
            }}
          />
        </Badge>
      ) : null}

      <Drawer
        title={(
          <Space direction="vertical" size={2}>
            <Text style={{ fontSize: 12, letterSpacing: '0.08em', textTransform: 'uppercase', color: token.colorTextTertiary }}>
              Workflow Pulse
            </Text>
            <Title level={4} style={{ margin: 0 }}>
              {controller.focusProjectId ? `后台任务 · 当前项目优先 (${controller.tasks.length})` : `后台任务 (${controller.tasks.length})`}
            </Title>
            <Text type="secondary" style={{ fontSize: 13, lineHeight: 1.7 }}>
              集中查看生成、恢复与失败任务，保持当前创作工作区的执行脉络清晰可读。
            </Text>
          </Space>
        )}
        placement="right"
        open={controller.open}
        onClose={() => controller.setOpen(false)}
        width={isMobile ? '100vw' : 440}
        extra={
          <Space size={8}>
            <Button
              size="small"
              type="primary"
              onClick={() => void controller.resumeAllRecoverableTasks()}
              disabled={controller.summary.recoverableTaskCount === 0}
            >
              重试可恢复任务
            </Button>
            <Button
              size="small"
              onClick={controller.clearTerminalTasks}
              disabled={controller.activeTasks.length === controller.tasks.length}
            >
              清理已结束
            </Button>
          </Space>
        }
        styles={{
          header: {
            padding: '14px 20px 12px',
            borderBottom: `1px solid ${alphaColor(token.colorBorderSecondary, 0.9)}`,
          },
          body: {
            padding: 16,
            background: `linear-gradient(180deg, ${alphaColor(token.colorBgLayout, 0.92)} 0%, ${alphaColor(token.colorBgContainer, 0.98)} 100%)`,
          },
        }}
      >
        <div
          style={{
            padding: '12px 14px',
            borderRadius: 18,
            background: `linear-gradient(135deg, ${alphaColor(token.colorPrimaryBg, 0.9)} 0%, ${alphaColor(token.colorBgElevated, 0.98)} 100%)`,
            border: `1px solid ${alphaColor(token.colorPrimary, 0.12)}`,
            boxShadow: `0 10px 24px ${alphaColor(token.colorText, 0.06)}`,
            marginBottom: 12,
          }}
        >
          <div
            style={{
              display: 'flex',
              alignItems: 'flex-start',
              justifyContent: 'space-between',
              gap: 12,
              flexWrap: 'wrap',
            }}
          >
            <div style={{ flex: '1 1 220px', minWidth: 0 }}>
              <Text style={{ display: 'block', fontSize: 11, letterSpacing: '0.08em', textTransform: 'uppercase', color: token.colorTextTertiary, marginBottom: 6 }}>
                Task Focus
              </Text>
              <Text strong style={{ display: 'block', fontSize: 15, marginBottom: 4 }}>
                {backgroundTaskWorkspaceFocus.title}
              </Text>
              <Text type="secondary" style={{ display: 'block', fontSize: 12, lineHeight: 1.6 }}>
                {backgroundTaskWorkspaceFocus.note}
              </Text>
            </div>
            <div style={{ flex: '0 1 220px' }}>
              <Space wrap size={[6, 6]} style={{ justifyContent: 'flex-end', width: '100%' }}>
                <Tag color="processing">进行中 {controller.activeTaskCount}</Tag>
                <Tag color="blue">当前项目 {controller.summary.currentProjectActiveCount}</Tag>
                <Tag color={controller.summary.failedTaskCount > 0 ? 'error' : 'default'}>
                  失败 {controller.summary.failedTaskCount}
                </Tag>
                <Tag color={controller.summary.recoverableTaskCount > 0 ? 'gold' : 'default'}>
                  可恢复 {controller.summary.recoverableTaskCount}
                </Tag>
                <Tag color="default">已结束 {controller.summary.terminalTaskCount}</Tag>
              </Space>
            </div>
          </div>
        </div>

        <div
          style={{
            padding: 12,
            borderRadius: 18,
            background: `linear-gradient(135deg, ${alphaColor(token.colorPrimaryBg, 0.9)} 0%, ${alphaColor(token.colorBgElevated, 0.98)} 100%)`,
            border: `1px solid ${alphaColor(token.colorPrimary, 0.12)}`,
            boxShadow: `0 10px 24px ${alphaColor(token.colorText, 0.05)}`,
          }}
        >
          <TaskSummaryBar
            focusProjectId={controller.focusProjectId}
            taskFilter={controller.taskFilter}
            filterOptions={controller.filterOptions}
            onChangeFilter={controller.setTaskFilter}
            summary={controller.summary}
            activeCount={controller.activeTaskCount}
          />
        </div>

        <div
          style={{
            marginTop: 12,
            padding: 12,
            borderRadius: 18,
            background: alphaColor(token.colorBgElevated, 0.98),
            border: `1px solid ${alphaColor(token.colorBorderSecondary, 0.9)}`,
            boxShadow: `0 10px 24px ${alphaColor(token.colorText, 0.04)}`,
          }}
        >
          <TaskSectionList
            sections={controller.taskSections}
            onNavigate={(to) => controller.onNavigate(to)}
            canCancelTask={controller.canCancelTask}
            canResumeTask={controller.canResumeTask}
            cancellingTaskIds={controller.cancellingTaskIds}
            resumingTaskIds={controller.resumingTaskIds}
            onCancel={(task) => void controller.cancelTask(task)}
            onResume={(task) => void controller.resumeTask(task)}
            onRemove={(taskId) => controller.removeTask(taskId)}
          />
        </div>
      </Drawer>
    </>
  );
};
