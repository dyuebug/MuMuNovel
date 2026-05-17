import { Badge, Button, Drawer, FloatButton, Grid, Space } from 'antd';
import { UnorderedListOutlined } from '@ant-design/icons';
import { TaskSummaryBar } from './TaskSummaryBar';
import { TaskSectionList } from './TaskSectionList';
import type { BackgroundTaskCenterController } from '../hooks/useBackgroundTaskCenterController';

const { useBreakpoint } = Grid;

export const BackgroundTaskCenterView = (props: {
  controller: BackgroundTaskCenterController;
}) => {
  const { controller } = props;
  const screens = useBreakpoint();
  const isMobile = !screens.md;

  return (
    <>
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
          }}
        />
      </Badge>

      <Drawer
        title={controller.focusProjectId ? `后台任务 · 当前项目优先 (${controller.tasks.length})` : `后台任务 (${controller.tasks.length})`}
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
      >
        <TaskSummaryBar
          focusProjectId={controller.focusProjectId}
          taskFilter={controller.taskFilter}
          filterOptions={controller.filterOptions}
          onChangeFilter={controller.setTaskFilter}
          summary={controller.summary}
          activeCount={controller.activeTaskCount}
        />

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
      </Drawer>
    </>
  );
};
