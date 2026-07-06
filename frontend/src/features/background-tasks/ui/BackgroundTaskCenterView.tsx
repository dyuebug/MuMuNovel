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
  const backgroundTaskGuideSteps = [
    '先看当前项目与全局任务的状态分布，确定这一轮是要跟进进行中、处理失败项，还是清理已结束任务。',
    '再切换合适的过滤视图，把注意力锁定在最需要处理的任务集合上，避免不同项目的后台队列互相打断。',
    '最后再进入任务列表执行恢复、取消或清理操作，把动作建立在已经看清上下文之后。',
  ];
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
            paddingBottom: 12,
            borderBottom: `1px solid ${alphaColor(token.colorBorderSecondary, 0.9)}`,
          },
          body: {
            padding: 20,
            background: `linear-gradient(180deg, ${alphaColor(token.colorBgLayout, 0.92)} 0%, ${alphaColor(token.colorBgContainer, 0.98)} 100%)`,
          },
        }}
      >
        <div
          style={{
            padding: 18,
            borderRadius: 22,
            background: `linear-gradient(135deg, ${alphaColor(token.colorPrimaryBg, 0.9)} 0%, ${alphaColor(token.colorBgElevated, 0.98)} 100%)`,
            border: `1px solid ${alphaColor(token.colorPrimary, 0.12)}`,
            boxShadow: `0 18px 40px ${alphaColor(token.colorText, 0.08)}`,
            marginBottom: 16,
          }}
        >
          <div
            style={{
              display: 'grid',
              gridTemplateColumns: 'repeat(auto-fit, minmax(220px, 1fr))',
              gap: 16,
            }}
          >
            <div>
              <Text style={{ display: 'block', fontSize: 11, letterSpacing: '0.08em', textTransform: 'uppercase', color: token.colorTextTertiary, marginBottom: 6 }}>
                Task Center Guide
              </Text>
              <Text strong style={{ display: 'block', fontSize: 17, marginBottom: 8 }}>
                后台任务工作台
              </Text>
              <Text type="secondary" style={{ display: 'block', lineHeight: 1.7, marginBottom: 12 }}>
                这里不改变原有恢复、取消、轮询或清理逻辑，只把任务阅读顺序和操作重点提前说明，帮助你先看清队列状态，再执行具体动作。
              </Text>
              <div style={{ display: 'flex', flexWrap: 'wrap', gap: 8 }}>
                {backgroundTaskGuideSteps.map((item, index) => (
                  <span
                    key={item}
                    style={{
                      display: 'inline-flex',
                      alignItems: 'center',
                      gap: 8,
                      padding: '6px 12px',
                      borderRadius: 999,
                      background: token.colorBgContainer,
                      border: `1px solid ${alphaColor(token.colorPrimary, 0.12)}`,
                      color: token.colorText,
                      fontSize: 12,
                    }}
                  >
                    <span style={{ color: token.colorPrimary, fontWeight: 700 }}>{index + 1}</span>
                    {item}
                  </span>
                ))}
              </div>
            </div>
            <div
              style={{
                borderRadius: 18,
                padding: '16px 18px 14px',
                background: `linear-gradient(180deg, ${alphaColor(token.colorBgElevated, 0.98)} 0%, ${alphaColor(token.colorFillQuaternary, 0.5)} 100%)`,
                border: `1px solid ${alphaColor(token.colorPrimary, 0.12)}`,
              }}
            >
              <Text style={{ display: 'block', fontSize: 11, letterSpacing: '0.08em', textTransform: 'uppercase', color: token.colorTextTertiary, marginBottom: 6 }}>
                当前工作焦点
              </Text>
              <Text strong style={{ display: 'block', fontSize: 16, marginBottom: 8 }}>
                {backgroundTaskWorkspaceFocus.title}
              </Text>
              <Text type="secondary" style={{ display: 'block', lineHeight: 1.7, marginBottom: 12 }}>
                {backgroundTaskWorkspaceFocus.note}
              </Text>
              <Space wrap size={[8, 8]}>
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
            padding: 18,
            borderRadius: 22,
            background: `linear-gradient(135deg, ${alphaColor(token.colorPrimaryBg, 0.9)} 0%, ${alphaColor(token.colorBgElevated, 0.98)} 100%)`,
            border: `1px solid ${alphaColor(token.colorPrimary, 0.12)}`,
            boxShadow: `0 18px 40px ${alphaColor(token.colorText, 0.08)}`,
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
            marginTop: 16,
            padding: 18,
            borderRadius: 24,
            background: alphaColor(token.colorBgElevated, 0.98),
            border: `1px solid ${alphaColor(token.colorBorderSecondary, 0.9)}`,
            boxShadow: `0 18px 36px ${alphaColor(token.colorText, 0.06)}`,
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
