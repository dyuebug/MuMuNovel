import { useMemo, useState } from 'react';
import { DownOutlined, ReloadOutlined } from '@ant-design/icons';
import {
  Alert,
  Button,
  Dropdown,
  Empty,
  Input,
  Modal,
  Space,
  Spin,
  Tag,
  Tooltip,
  Typography,
  message,
} from 'antd';
import type { MenuProps } from 'antd';

import { requestBackgroundTaskCenterOpen } from '../../../constants/backgroundTaskEvents';
import { backgroundTaskApi, projectApi } from '../../../services/modularApi';
import type {
  AutopilotInvocationAuditHistoryItem,
  AutopilotInvocationAuditStatus,
} from '../../../services/modules/projects';
import type { NovelWorkflowPhase } from '../../../types';
import {
  getNovelWorkflowPhasePresentation,
  NOVEL_WORKFLOW_PHASE_PRESENTATIONS,
  isNovelWorkflowRollbackTransition,
  requiresNovelWorkflowTransitionConfirmation,
} from './presentation';
import { useProjectWorkflowState } from './useProjectWorkflowState';

const { Text } = Typography;
const { TextArea } = Input;

const AUTOPILOT_INVOCATION_STATUS_PRESENTATIONS: Record<
  AutopilotInvocationAuditStatus,
  { label: string; color: string }
> = {
  queued: { label: '已排队', color: 'default' },
  running: { label: '执行中', color: 'processing' },
  succeeded: { label: '已完成', color: 'success' },
  failed: { label: '执行失败', color: 'error' },
  cancelled: { label: '已取消', color: 'warning' },
};

const formatAuditPhase = (phase: string) => {
  const presentation = NOVEL_WORKFLOW_PHASE_PRESENTATIONS.find((item) => item.phase === phase);
  return presentation?.label ?? phase;
};

const formatAuditExecutionMode = (executionMode: string) => (
  executionMode === 'direct_business_tool' ? '直接业务工具' : executionMode
);

const formatAuditTimestamp = (value: string | null, emptyLabel: string) => {
  if (!value) {
    return emptyLabel;
  }

  const normalizedValue = /(?:Z|[+-]\d{2}:\d{2})$/.test(value) ? value : `${value}Z`;
  const date = new Date(normalizedValue);
  if (Number.isNaN(date.getTime())) {
    return '时间不可用';
  }

  return new Intl.DateTimeFormat('zh-CN', {
    dateStyle: 'medium',
    timeStyle: 'medium',
  }).format(date);
};

const getAuditHistoryErrorMessage = (error: unknown) => {
  const detail = (error as { response?: { data?: { detail?: unknown } } })
    .response?.data?.detail;
  return typeof detail === 'string' && detail.trim()
    ? detail
    : '加载受控调用记录失败，请稍后重试';
};

export interface ProjectWorkflowStatePanelProps {
  projectId: string;
  compact?: boolean;
}

const ProjectWorkflowStatePanel = ({
  projectId,
  compact = false,
}: ProjectWorkflowStatePanelProps) => {
  const [messageApi, messageContextHolder] = message.useMessage();
  const {
    state,
    loading,
    transitioning,
    error,
    refresh,
    transition,
  } = useProjectWorkflowState(projectId);
  const [confirmTarget, setConfirmTarget] = useState<NovelWorkflowPhase | null>(null);
  const [reason, setReason] = useState('');
  const [autopilotTarget, setAutopilotTarget] = useState<NovelWorkflowPhase | null>(null);
  const [autopilotReason, setAutopilotReason] = useState('');
  const [autopilotLaunching, setAutopilotLaunching] = useState(false);
  const [auditHistoryOpen, setAuditHistoryOpen] = useState(false);
  const [auditHistoryLoading, setAuditHistoryLoading] = useState(false);
  const [auditHistoryError, setAuditHistoryError] = useState<string | null>(null);
  const [auditHistoryItems, setAuditHistoryItems] = useState<
    AutopilotInvocationAuditHistoryItem[]
  >([]);

  const currentPresentation = state
    ? getNovelWorkflowPhasePresentation(state.phase)
    : null;
  const confirmPresentation = confirmTarget
    ? getNovelWorkflowPhasePresentation(confirmTarget)
    : null;
  const isRollback = Boolean(
    state
      && confirmTarget
      && isNovelWorkflowRollbackTransition(state.phase, confirmTarget),
  );
  const autopilotPresentation = autopilotTarget
    ? getNovelWorkflowPhasePresentation(autopilotTarget)
    : null;

  const closeConfirm = () => {
    if (transitioning) {
      return;
    }
    setConfirmTarget(null);
    setReason('');
  };

  const executeTransition = async (targetPhase: NovelWorkflowPhase, transitionReason?: string) => {
    const outcome = await transition(targetPhase, { reason: transitionReason });
    if (outcome.status === 'success') {
      const targetPresentation = getNovelWorkflowPhasePresentation(outcome.receipt.state.phase);
      messageApi.success(`创作阶段已切换为“${targetPresentation.label}”`);
      return true;
    }
    if (outcome.status === 'conflict') {
      messageApi.warning(outcome.message);
      return true;
    }

    messageApi.error(outcome.message);
    return false;
  };

  const handleTransitionClick = async (targetPhase: NovelWorkflowPhase) => {
    if (!state) {
      return;
    }
    if (requiresNovelWorkflowTransitionConfirmation(state.phase, targetPhase)) {
      setReason('');
      setConfirmTarget(targetPhase);
      return;
    }

    await executeTransition(targetPhase);
  };

  const closeAutopilotConfirm = () => {
    if (autopilotLaunching) {
      return;
    }
    setAutopilotTarget(null);
    setAutopilotReason('');
  };

  const launchAutopilotTransition = async () => {
    if (!state || !autopilotTarget) {
      return;
    }

    setAutopilotLaunching(true);
    try {
      await backgroundTaskApi.createConfirmedAutopilotWorkflowTransition(projectId, {
        tool_name: 'transition_project_workflow',
        arguments: {
          expected_phase: state.phase,
          target_phase: autopilotTarget,
          ...(autopilotReason.trim() ? { reason: autopilotReason.trim() } : {}),
        },
        confirmed_by_user: true,
      });
      messageApi.success('后台受控切换任务已创建，可在后台任务中心查看进度');
      setAutopilotTarget(null);
      setAutopilotReason('');
      requestBackgroundTaskCenterOpen();
    } catch (launchError: unknown) {
      const apiError = launchError as {
        response?: { data?: { detail?: string } };
        message?: string;
      };
      messageApi.error(apiError.response?.data?.detail || apiError.message || '创建后台受控切换任务失败');
    } finally {
      setAutopilotLaunching(false);
    }
  };

  const handleAutopilotTransitionClick = (targetPhase: NovelWorkflowPhase) => {
    if (!state) {
      return;
    }
    setAutopilotReason('');
    setAutopilotTarget(targetPhase);
  };

  const loadAutopilotInvocationHistory = async () => {
    setAuditHistoryLoading(true);
    setAuditHistoryError(null);
    try {
      const response = await projectApi.getAutopilotInvocationHistory(projectId, {
        suppressErrorToast: true,
        suppressErrorLog: true,
      });
      setAuditHistoryItems(response.items);
    } catch (historyError: unknown) {
      setAuditHistoryItems([]);
      setAuditHistoryError(getAuditHistoryErrorMessage(historyError));
    } finally {
      setAuditHistoryLoading(false);
    }
  };

  const openAutopilotInvocationHistory = () => {
    setAuditHistoryOpen(true);
    void loadAutopilotInvocationHistory();
  };

  const transitionItems = useMemo<MenuProps['items']>(() => (
    state?.allowed_transitions.map((phase) => {
      const presentation = getNovelWorkflowPhasePresentation(phase);
      return {
        key: phase,
        label: (
          <Space size={8}>
            <Tag color={presentation.color} style={{ marginInlineEnd: 0 }}>
              {presentation.label}
            </Tag>
            <Text type="secondary" style={{ fontSize: 12 }}>
              {presentation.description}
            </Text>
          </Space>
        ),
      };
    }) ?? []
  ), [state?.allowed_transitions]);

  const menu: MenuProps = {
    items: transitionItems,
    onClick: ({ key }) => {
      void handleTransitionClick(key as NovelWorkflowPhase);
    },
  };

  const autopilotMenu: MenuProps = {
    items: transitionItems,
    onClick: ({ key }) => {
      handleAutopilotTransitionClick(key as NovelWorkflowPhase);
    },
  };

  if (loading && !state) {
    return (
      <Space size={8} aria-label="正在加载创作阶段">
        {messageContextHolder}
        <Spin size="small" />
        <Text style={{ color: 'rgba(247, 241, 232, 0.72)', fontSize: 12 }}>
          加载创作阶段
        </Text>
      </Space>
    );
  }

  if (!state || !currentPresentation) {
    return (
      <Space size={8} wrap>
        {messageContextHolder}
        <Text style={{ color: '#ffccc7', fontSize: 12 }}>
          {error || '暂无创作阶段信息'}
        </Text>
        <Button
          type="link"
          size="small"
          icon={<ReloadOutlined />}
          onClick={() => void refresh()}
          loading={loading}
          style={{ paddingInline: 0 }}
        >
          重试
        </Button>
      </Space>
    );
  }

  const suggestedPresentation = state.suggested_next_phase
    ? getNovelWorkflowPhasePresentation(state.suggested_next_phase)
    : null;

  return (
    <>
      {messageContextHolder}
      <Space
        size={compact ? 6 : 10}
        wrap
        style={{ justifyContent: compact ? 'flex-start' : 'flex-end' }}
      >
        <Tooltip title={currentPresentation.description}>
          <Tag
            color={currentPresentation.color}
            style={{ marginInlineEnd: 0, borderRadius: 999, paddingInline: 10 }}
          >
            当前：{currentPresentation.label}
          </Tag>
        </Tooltip>
        {suggestedPresentation ? (
          <Text style={{ color: 'rgba(247, 241, 232, 0.72)', fontSize: 12 }}>
            建议下一步：{suggestedPresentation.label}
          </Text>
        ) : null}
        <Dropdown
          menu={menu}
          trigger={['click']}
          disabled={transitioning || state.allowed_transitions.length === 0}
        >
          <Button
            size="small"
            loading={transitioning}
            disabled={state.allowed_transitions.length === 0}
            icon={<DownOutlined />}
          >
            {state.allowed_transitions.length > 0 ? '切换阶段' : '暂无可切换阶段'}
          </Button>
        </Dropdown>
        <Dropdown
          menu={autopilotMenu}
          trigger={['click']}
          disabled={autopilotLaunching || state.allowed_transitions.length === 0}
        >
          <Button
            size="small"
            loading={autopilotLaunching}
            disabled={state.allowed_transitions.length === 0}
            icon={<DownOutlined />}
          >
            {state.allowed_transitions.length > 0 ? '后台受控切换' : '暂无可后台切换阶段'}
          </Button>
        </Dropdown>
        <Button size="small" onClick={openAutopilotInvocationHistory}>
          受控调用记录
        </Button>
      </Space>

      <Modal
        title="受控调用记录"
        open={auditHistoryOpen}
        onCancel={() => setAuditHistoryOpen(false)}
        footer={<Button onClick={() => setAuditHistoryOpen(false)}>关闭</Button>}
        destroyOnHidden
        width={720}
      >
        <Space direction="vertical" size={12} style={{ width: '100%' }}>
          <Text type="secondary">
            此处仅展示脱敏的调用审计摘要，不提供恢复、重试或重新执行能力。
          </Text>
          {auditHistoryLoading ? (
            <div style={{ paddingBlock: 32, textAlign: 'center' }}>
              <Spin tip="正在加载受控调用记录" />
            </div>
          ) : null}
          {!auditHistoryLoading && auditHistoryError ? (
            <Alert
              type="error"
              showIcon
              message={auditHistoryError}
              action={<Button size="small" onClick={() => void loadAutopilotInvocationHistory()}>重新加载</Button>}
            />
          ) : null}
          {!auditHistoryLoading && !auditHistoryError && auditHistoryItems.length === 0 ? (
            <Empty description="暂无受控调用记录" />
          ) : null}
          {!auditHistoryLoading && !auditHistoryError && auditHistoryItems.length > 0 ? (
            <Space direction="vertical" size={10} style={{ width: '100%' }}>
              {auditHistoryItems.map((item) => {
                const statusPresentation = AUTOPILOT_INVOCATION_STATUS_PRESENTATIONS[item.status];
                const resultSummary = item.result_summary;
                return (
                  <div
                    key={item.audit_id}
                    style={{
                      border: '1px solid rgba(247, 241, 232, 0.16)',
                      borderRadius: 8,
                      padding: 12,
                    }}
                  >
                    <Space direction="vertical" size={6} style={{ width: '100%' }}>
                      <Space size={[6, 6]} wrap>
                        <Text strong>{item.tool_name}</Text>
                        <Tag color={statusPresentation.color}>{statusPresentation.label}</Tag>
                        <Tag color={item.confirmed_by_user ? 'success' : 'default'}>
                          {item.confirmed_by_user ? '已人工确认' : '未人工确认'}
                        </Tag>
                        <Tag>{formatAuditExecutionMode(item.execution_mode)}</Tag>
                      </Space>
                      <Text type="secondary">
                        契约：{item.tool_schema_version}
                      </Text>
                      <Text>
                        请求阶段：{formatAuditPhase(item.input_summary.expected_phase)} →
                        {formatAuditPhase(item.input_summary.target_phase)}
                      </Text>
                      {resultSummary ? (
                        <Text>
                          执行结果：{resultSummary.changed ? '已变更' : '未变更'}，
                          {formatAuditPhase(resultSummary.previous_phase)} →
                          {formatAuditPhase(resultSummary.current_phase)}
                        </Text>
                      ) : null}
                      {item.error_code ? <Text type="danger">错误码：{item.error_code}</Text> : null}
                      <Text type="secondary" style={{ fontSize: 12 }}>
                        创建：{formatAuditTimestamp(item.created_at, '时间不可用')} · 开始：
                        {formatAuditTimestamp(item.started_at, '未开始')} · 完成：
                        {formatAuditTimestamp(item.completed_at, '未完成')}
                      </Text>
                    </Space>
                  </div>
                );
              })}
            </Space>
          ) : null}
        </Space>
      </Modal>

      <Modal
        title="确认后台受控切换"
        open={Boolean(autopilotTarget)}
        onCancel={closeAutopilotConfirm}
        confirmLoading={autopilotLaunching}
        okText="确认创建任务"
        cancelText="取消"
        destroyOnHidden
        onOk={() => void launchAutopilotTransition()}
      >
        <Space direction="vertical" size={12} style={{ width: '100%' }}>
          <Text>
            将创建后台任务，从“{currentPresentation.label}”受控切换到“
            {autopilotPresentation?.label ?? ''}”。任务由服务端执行，当前阶段不会在此处提前更新。
          </Text>
          <TextArea
            value={autopilotReason}
            onChange={(event) => setAutopilotReason(event.target.value)}
            placeholder="可选：补充本次后台受控切换的原因"
            maxLength={500}
            showCount
            autoSize={{ minRows: 3, maxRows: 6 }}
          />
        </Space>
      </Modal>

      <Modal
        title={isRollback ? '确认回退创作阶段' : '确认标记作品完结'}
        open={Boolean(confirmTarget)}
        onCancel={closeConfirm}
        confirmLoading={transitioning}
        okButtonProps={{ disabled: isRollback && !reason.trim() }}
        okText="确认切换"
        cancelText="取消"
        destroyOnHidden
        onOk={async () => {
          if (!confirmTarget || (isRollback && !reason.trim())) {
            return;
          }
          const shouldClose = await executeTransition(confirmTarget, reason);
          if (shouldClose) {
            setConfirmTarget(null);
            setReason('');
          }
        }}
      >
        <Space direction="vertical" size={12} style={{ width: '100%' }}>
          <Text>
            将从“{currentPresentation.label}”切换到“{confirmPresentation?.label ?? ''}”。
            {isRollback
              ? '回退会改变后续创作依据，请填写可追溯原因。'
              : '完结属于高影响操作，请确认当前版本已经完成审校。'}
          </Text>
          <TextArea
            value={reason}
            onChange={(event) => setReason(event.target.value)}
            placeholder={isRollback ? '请输入回退原因（必填）' : '可选：补充完结说明'}
            maxLength={500}
            showCount
            autoSize={{ minRows: 3, maxRows: 6 }}
          />
        </Space>
      </Modal>
    </>
  );
};

export default ProjectWorkflowStatePanel;
