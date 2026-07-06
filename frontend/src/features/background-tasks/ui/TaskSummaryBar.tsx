import { Segmented, Space, Tag, Typography, theme } from 'antd';
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
  const { token } = theme.useToken();
  const alphaColor = (color: string, alpha: number) => `color-mix(in srgb, ${color} ${(alpha * 100).toFixed(0)}%, transparent)`;
  const summaryFocus = summary.failedTaskCount > 0
    ? {
        title: '先处理失败与可恢复任务',
        note: '失败项已经出现时，更适合先确认阻塞原因和恢复入口，再继续浏览其他后台队列。',
        tone: alphaColor(token.colorErrorBg, 0.92),
        border: alphaColor(token.colorError, 0.2),
      }
    : summary.currentProjectActiveCount > 0
      ? {
          title: '当前项目仍在推进中',
          note: '你现在最可能需要跟进本项目中的进行中任务，保持创作流程的上下文连续。',
          tone: alphaColor(token.colorPrimaryBg, 0.92),
          border: alphaColor(token.colorPrimary, 0.18),
        }
      : summary.otherActiveCount > 0
        ? {
            title: '全局队列里还有其他项目任务',
            note: '如果当前页面不是重点项目，先看全局概况能帮助你判断是否需要切换注意力。',
            tone: alphaColor(token.colorInfoBg, 0.92),
            border: alphaColor(token.colorInfo, 0.18),
          }
        : {
            title: '当前工作台以复盘和清理为主',
            note: '没有明显的活动阻塞时，适合用这里快速浏览已结束记录并整理工作区。',
            tone: alphaColor(token.colorFillQuaternary, 0.84),
            border: alphaColor(token.colorBorderSecondary, 0.88),
          };

  const filterGuide = focusProjectId
    ? taskFilter === 'current'
      ? '当前只展示本项目任务，适合在创作过程中保持注意力集中。'
      : taskFilter === 'active'
        ? '当前只展示仍在排队或执行中的任务，方便跟进进度。'
        : taskFilter === 'failed'
          ? '当前只展示失败任务，便于集中排查与恢复。'
          : '当前项目任务会优先显示，避免多项目并行时被其他队列淹没。'
    : taskFilter === 'active'
      ? '当前视图仅保留进行中的后台任务。'
      : taskFilter === 'failed'
        ? '当前视图仅保留失败任务。'
        : '这里汇总所有后台任务；进入项目页后会自动优先展示当前项目任务。';

  return (
    <Space direction="vertical" size={14} style={{ width: '100%', marginBottom: 4 }}>
      <div
        style={{
          padding: '14px 16px',
          borderRadius: 18,
          background: `linear-gradient(135deg, ${alphaColor(token.colorBgElevated, 0.96)} 0%, ${alphaColor(token.colorPrimaryBg, 0.82)} 100%)`,
          border: `1px solid ${alphaColor(token.colorPrimary, 0.12)}`,
        }}
      >
        <Text style={{ display: 'block', fontSize: 11, letterSpacing: '0.08em', textTransform: 'uppercase', color: token.colorTextTertiary, marginBottom: 6 }}>
          Queue Snapshot
        </Text>
        <Text strong style={{ display: 'block', fontSize: 16, marginBottom: 8 }}>
          后台队列摘要
        </Text>
        <Text type="secondary" style={{ display: 'block', fontSize: 12, lineHeight: 1.7, marginBottom: 12 }}>
          先看整体分布，再决定是继续跟进当前项目、切换到失败视图，还是清理已结束任务。
        </Text>
        <div style={{ display: 'grid', gridTemplateColumns: 'repeat(auto-fit, minmax(104px, 1fr))', gap: 10 }}>
          <div style={{ padding: '12px 14px', borderRadius: 16, background: alphaColor(token.colorBgElevated, 0.96), border: `1px solid ${alphaColor(token.colorPrimary, 0.1)}` }}>
            <Text style={{ display: 'block', fontSize: 11, letterSpacing: '0.08em', textTransform: 'uppercase', color: token.colorTextTertiary, marginBottom: 4 }}>
              Active
            </Text>
            <Text strong style={{ display: 'block', fontSize: 18 }}>
              {activeCount}
            </Text>
          </div>
          <div style={{ padding: '12px 14px', borderRadius: 16, background: alphaColor(token.colorBgElevated, 0.96), border: `1px solid ${alphaColor(token.colorInfo, 0.1)}` }}>
            <Text style={{ display: 'block', fontSize: 11, letterSpacing: '0.08em', textTransform: 'uppercase', color: token.colorTextTertiary, marginBottom: 4 }}>
              Current
            </Text>
            <Text strong style={{ display: 'block', fontSize: 18 }}>
              {summary.currentProjectActiveCount}
            </Text>
          </div>
          <div style={{ padding: '12px 14px', borderRadius: 16, background: alphaColor(token.colorBgElevated, 0.96), border: `1px solid ${alphaColor(token.colorError, 0.1)}` }}>
            <Text style={{ display: 'block', fontSize: 11, letterSpacing: '0.08em', textTransform: 'uppercase', color: token.colorTextTertiary, marginBottom: 4 }}>
              Failed
            </Text>
            <Text strong style={{ display: 'block', fontSize: 18 }}>
              {summary.failedTaskCount}
            </Text>
          </div>
          <div style={{ padding: '12px 14px', borderRadius: 16, background: alphaColor(token.colorBgElevated, 0.96), border: `1px solid ${alphaColor(token.colorBorderSecondary, 0.86)}` }}>
            <Text style={{ display: 'block', fontSize: 11, letterSpacing: '0.08em', textTransform: 'uppercase', color: token.colorTextTertiary, marginBottom: 4 }}>
              Archive
            </Text>
            <Text strong style={{ display: 'block', fontSize: 18 }}>
              {summary.terminalTaskCount}
            </Text>
          </div>
        </div>
        <Space wrap size={[8, 8]} style={{ marginTop: 12 }}>
          <Tag color="processing">进行中 {activeCount}</Tag>
          <Tag color="blue">当前项目 {summary.currentProjectActiveCount}</Tag>
          {summary.failedTaskCount > 0 ? <Tag color="error">失败 {summary.failedTaskCount}</Tag> : null}
          {summary.otherActiveCount > 0 ? <Tag>其他项目 {summary.otherActiveCount}</Tag> : null}
          {summary.terminalTaskCount > 0 ? <Tag color="default">已结束 {summary.terminalTaskCount}</Tag> : null}
        </Space>
      </div>

      <div
        style={{
          padding: '14px 16px',
          borderRadius: 18,
          background: alphaColor(token.colorBgElevated, 0.98),
          border: `1px solid ${alphaColor(token.colorBorderSecondary, 0.88)}`,
        }}
      >
        <Text style={{ display: 'block', fontSize: 11, letterSpacing: '0.08em', textTransform: 'uppercase', color: token.colorTextTertiary, marginBottom: 6 }}>
          Filter Focus
        </Text>
        <Text strong style={{ display: 'block', fontSize: 15, marginBottom: 8 }}>
          任务筛选工作台
        </Text>
        <Text type="secondary" style={{ display: 'block', fontSize: 12, lineHeight: 1.7, marginBottom: 12 }}>
          过滤器只改变任务阅读范围，不改变后台任务本身的状态、恢复逻辑和操作能力。
        </Text>
        <Segmented
          block
          size="small"
          value={taskFilter}
          onChange={(value) => onChangeFilter(value as TaskFilter)}
          options={filterOptions}
        />
      </div>

      <div
        style={{
          padding: '14px 16px',
          borderRadius: 18,
          background: summaryFocus.tone,
          border: `1px solid ${summaryFocus.border}`,
        }}
      >
        <Text style={{ display: 'block', fontSize: 11, letterSpacing: '0.08em', textTransform: 'uppercase', color: token.colorTextTertiary, marginBottom: 6 }}>
          Reading Note
        </Text>
        <Text strong style={{ display: 'block', fontSize: 14, marginBottom: 6 }}>
          {summaryFocus.title}
        </Text>
        <Text type="secondary" style={{ display: 'block', fontSize: 12, lineHeight: 1.7, marginBottom: 8 }}>
          {summaryFocus.note}
        </Text>
        <Text type="secondary" style={{ display: 'block', fontSize: 12, lineHeight: 1.7 }}>
          {filterGuide}
        </Text>
      </div>
    </Space>
  );
};
