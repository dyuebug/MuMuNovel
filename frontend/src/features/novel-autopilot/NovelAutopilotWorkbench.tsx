import {
  Alert,
  Button,
  Card,
  Col,
  Collapse,
  Descriptions,
  Divider,
  Empty,
  Form,
  Input,
  InputNumber,
  Popconfirm,
  Progress,
  Row,
  Select,
  Space,
  Spin,
  Statistic,
  Switch,
  Table,
  Tag,
  Tooltip,
  Typography,
  message,
  theme,
} from 'antd';
import {
  CaretRightOutlined,
  CheckCircleOutlined,
  CloseCircleOutlined,
  DownloadOutlined,
  PauseCircleOutlined,
  PlayCircleOutlined,
  ReloadOutlined,
  RobotOutlined,
  StopOutlined,
} from '@ant-design/icons';
import { useMemo, useState } from 'react';

import { ModelOutputSections, type ModelOutputTaskStatus } from '../../components/ModelOutputPanel';
import { useBackgroundTaskOutputStream } from '../../hooks/useBackgroundTaskOutputStream';
import { useNovelAutopilotWorkbench } from './useNovelAutopilotWorkbench';
import type {
  CreateNovelAutopilotRunRequest,
  NovelAutopilotCreateHumanGateMode,
  NovelAutopilotCreateRunConfig,
  NovelAutopilotExecutionScope,
  NovelAutopilotHumanDecision,
  NovelAutopilotHumanGateMode,
  NovelAutopilotPhase,
  NovelAutopilotQualityDecision,
  NovelAutopilotRun,
  NovelAutopilotRunStatus,
  NovelAutopilotStepRun,
  NovelAutopilotStepStatus,
  NovelAutopilotStepType,
  ProjectExportArtifactDescriptorV1,
} from './types';
import { isNovelAutopilotRunTerminal } from './types';

const { Paragraph, Text, Title } = Typography;

const SHOW_RUNTIME_STATUS_KEY = 'mumu-novel-autopilot-show-runtime-status';
const SHOW_REASONING_KEY = 'mumu-novel-autopilot-show-provider-reasoning';
const SHOW_GENERATED_CONTENT_KEY = 'mumu-novel-autopilot-show-generated-content';

interface NovelAutopilotWorkbenchProps {
  projectId: string;
}

type CreateRunFormValues = Omit<
  NovelAutopilotCreateRunConfig,
  'regenerate_existing' | 'export_format'
> & {
  total_chapters: number;
};

const DEFAULT_FORM_VALUES: CreateRunFormValues = {
  execution_scope: 'complete_book',
  human_gate_mode: 'high_risk_only',
  gate_interval: 5,
  next_chapter_count: null,
  total_chapters: 100,
  max_chapters: 200,
  max_tokens: 4_000_000,
  max_estimated_cost: null,
  max_runtime_seconds: 604_800,
  max_step_attempts: 3,
  max_consecutive_provider_failures: 5,
  max_consecutive_quality_failures: 5,
  run_book_review: true,
  run_book_polish: true,
};

const RUN_STATUS_META: Record<NovelAutopilotRunStatus, { label: string; color: string }> = {
  queued: { label: '排队中', color: 'default' },
  running: { label: '运行中', color: 'processing' },
  waiting_human: { label: '等待人工决定', color: 'warning' },
  paused: { label: '已暂停', color: 'gold' },
  completed: { label: '已完成', color: 'success' },
  failed: { label: '失败', color: 'error' },
  cancelled: { label: '已取消', color: 'default' },
};

const STEP_STATUS_META: Record<NovelAutopilotStepStatus, { label: string; color: string }> = {
  queued: { label: '排队中', color: 'default' },
  running: { label: '运行中', color: 'processing' },
  completed: { label: '完成', color: 'success' },
  skipped: { label: '跳过', color: 'default' },
  failed: { label: '失败', color: 'error' },
  cancelled: { label: '取消', color: 'default' },
  stale: { label: '过期', color: 'warning' },
};

const QUALITY_DECISION_META: Record<NovelAutopilotQualityDecision, { label: string; color: string }> = {
  accept: { label: '通过', color: 'success' },
  auto_repair: { label: '自动修复', color: 'warning' },
  retry: { label: '重试', color: 'processing' },
  manual_review: { label: '人工复核', color: 'gold' },
  reject: { label: '拒绝', color: 'error' },
};

const PHASE_LABELS: Record<NovelAutopilotPhase, string> = {
  validate: '前置校验',
  foundation: '基础设定',
  world_building: '世界观',
  career_design: '职业设计',
  character_design: '角色设计',
  organization_design: '组织设计',
  outline: '大纲生成',
  chapter_loop: '逐章生成与质量闭环',
  book_review: '全书审查',
  book_polish: '全书润色',
  export: '真实导出',
  completed: '完成',
};

const STEP_LABELS: Record<NovelAutopilotStepType, string> = {
  validate: '前置校验',
  foundation: '基础设定',
  world_building: '世界观生成',
  career_design: '职业生成',
  character_design: '角色生成',
  organization_design: '组织生成',
  outline: '大纲生成',
  outline_expand: '一纲多章展开',
  chapter_generate: '章节生成',
  chapter_analyze: '章节分析',
  chapter_repair: '章节返修',
  book_review: '全书审查',
  book_polish: '章节润色',
  export: '项目导出',
};

const EXECUTION_SCOPE_LABELS: Record<NovelAutopilotExecutionScope, string> = {
  planning_only: '仅完成设定与大纲',
  next_n_chapters: '只生成接下来 N 章',
  continue_from_current: '从当前进度续写',
  complete_book: '完整生成整本小说',
};

const HUMAN_GATE_LABELS: Record<NovelAutopilotHumanGateMode, string> = {
  fully_automatic: '全自动',
  high_risk_only: '仅高风险时确认',
  every_n_chapters: '每 N 章确认',
  every_volume: '每卷确认',
  every_chapter: '每章确认',
};

const EXPORT_FORMAT_LABELS: Record<'txt' | 'markdown' | 'docx', string> = {
  txt: 'TXT',
  markdown: 'Markdown',
  docx: 'DOCX',
};

const CREATE_HUMAN_GATE_MODES: readonly NovelAutopilotCreateHumanGateMode[] = [
  'fully_automatic',
  'high_risk_only',
  'every_n_chapters',
  'every_chapter',
];

const RUN_ERROR_MESSAGES: Record<string, string> = {
  novel_autopilot_budget_chapters_exhausted: '章节预算已用尽，请提高最大章节预算或缩小执行范围。',
  novel_autopilot_budget_tokens_exhausted: '已观察模型输出的估算 Token 达到上限，请提高预算后继续。',
  novel_autopilot_budget_cost_exhausted: '预估成本达到上限，请调整成本预算后继续。',
  novel_autopilot_budget_runtime_exhausted: '运行时间达到上限，请提高最大运行时长后继续。',
  novel_autopilot_step_attempts_exhausted: '当前步骤尝试次数达到上限，请检查失败原因后重试或修复。',
  novel_autopilot_provider_failures_exhausted: '模型 Provider 连续失败达到上限，请检查模型配置或服务状态。',
  novel_autopilot_quality_failures_exhausted: '连续质量失败达到上限，请调整指导、质量阈值或人工修复。',
  novel_autopilot_execution_failed: '自动创作步骤执行失败，运行状态已安全收敛；请根据步骤时间线和服务日志处理后重试。',
  novel_autopilot_cost_estimation_unavailable: '当前 Provider 尚无统一计价来源，无法可靠执行成本预算；请清空成本上限或配置受支持的计价能力。',
  chapter_quality_manual_review: '候选已保存，等待人工复核。',
  chapter_generation_attempts_exhausted: '候选已保存，等待人工复核。',
  chapter_repair_manual_review: '候选已保存，等待人工复核。',
  chapter_analysis_provider_failed: '章节分析 Provider 调用失败，未生成可供人工接受的候选。',
  chapter_repair_provider_failed: '章节返修 Provider 调用失败，未生成可供人工接受的候选。',
  chapter_analysis_result_invalid: '章节分析结果无效，未生成可供人工接受的候选。',
  chapter_repair_result_invalid: '章节返修结果无效，未生成可供人工接受的候选。',
  chapter_analysis_context_invalid: '章节分析上下文无效，未生成可供人工接受的候选。',
  chapter_repair_context_invalid: '章节返修上下文无效，未生成可供人工接受的候选。',
  chapter_analysis_provider_timeout: '章节分析 Provider 超时，请稍后重试或检查上游服务。',
  chapter_repair_provider_timeout: '章节返修 Provider 超时，请稍后重试或检查上游服务。',
  chapter_analysis_provider_rate_limited: '章节分析 Provider 触发限流，请稍后重试。',
  chapter_repair_provider_rate_limited: '章节返修 Provider 触发限流，请稍后重试。',
  chapter_analysis_provider_upstream_unavailable: '章节分析上游不可用，请稍后重试或检查服务状态。',
  chapter_repair_provider_upstream_unavailable: '章节返修上游不可用，请稍后重试或检查服务状态。',
  chapter_analysis_provider_authentication_or_configuration: '章节分析 Provider 鉴权或配置异常，请检查模型与接口配置。',
  chapter_repair_provider_authentication_or_configuration: '章节返修 Provider 鉴权或配置异常，请检查模型与接口配置。',
};

const WAITING_HUMAN_PROVIDER_FAILURE_PREFIXES = [
  'chapter_analysis_provider_',
  'chapter_repair_provider_',
];

const WAITING_HUMAN_RESULT_INVALID_CODES = new Set([
  'chapter_analysis_result_invalid',
  'chapter_repair_result_invalid',
]);

const WAITING_HUMAN_CONTEXT_INVALID_CODES = new Set([
  'chapter_analysis_context_invalid',
  'chapter_repair_context_invalid',
]);

const isProviderFailureWaitingHumanCode = (errorCode: string) => (
  WAITING_HUMAN_PROVIDER_FAILURE_PREFIXES.some((prefix) => errorCode.startsWith(prefix))
);

const describeRunError = (errorCode: string) =>
  `${RUN_ERROR_MESSAGES[errorCode] ?? '已记录错误，请根据步骤时间线和服务日志定位原因。'}（错误代码：${errorCode}）`;

const readBooleanPreference = (key: string, fallback: boolean) => {
  if (typeof window === 'undefined') {
    return fallback;
  }
  const value = window.localStorage.getItem(key);
  return value === null ? fallback : value === 'true';
};

const writeBooleanPreference = (key: string, value: boolean) => {
  if (typeof window !== 'undefined') {
    window.localStorage.setItem(key, String(value));
  }
};

const formatNumber = (value: number) => new Intl.NumberFormat('zh-CN').format(value);

const formatTimestamp = (value: string | null) => {
  if (!value) {
    return '—';
  }
  const timestamp = new Date(value);
  return Number.isNaN(timestamp.getTime()) ? value : timestamp.toLocaleString('zh-CN');
};

const formatRuntime = (run: NovelAutopilotRun) => {
  if (!run.started_at) {
    return '尚未开始';
  }
  const startedAt = new Date(run.started_at).getTime();
  const finishedAt = run.completed_at ? new Date(run.completed_at).getTime() : Date.now();
  if (!Number.isFinite(startedAt) || !Number.isFinite(finishedAt)) {
    return '—';
  }
  const seconds = Math.max(0, Math.floor((finishedAt - startedAt) / 1000));
  const hours = Math.floor(seconds / 3600);
  const minutes = Math.floor((seconds % 3600) / 60);
  const remainingSeconds = seconds % 60;
  return hours > 0 ? `${hours}小时 ${minutes}分钟` : `${minutes}分钟 ${remainingSeconds}秒`;
};

const parseExportDescriptor = (value: string | null): ProjectExportArtifactDescriptorV1 | null => {
  if (!value) {
    return null;
  }
  try {
    const candidate = JSON.parse(value) as Partial<ProjectExportArtifactDescriptorV1>;
    if (
      candidate.schema_version === 'project-export-artifact/v1'
      && candidate.format === 'txt'
      && typeof candidate.filename === 'string'
      && typeof candidate.content_digest === 'string'
      && typeof candidate.chapter_count === 'number'
      && typeof candidate.total_word_count === 'number'
    ) {
      return candidate as ProjectExportArtifactDescriptorV1;
    }
  } catch {
    return null;
  }
  return null;
};

const toOutputTaskStatus = (status: NovelAutopilotRunStatus): ModelOutputTaskStatus => {
  if (status === 'completed') return 'completed';
  if (status === 'failed') return 'failed';
  if (status === 'cancelled') return 'cancelled';
  return 'running';
};

const buildCreateRequest = (values: CreateRunFormValues): CreateNovelAutopilotRunRequest => {
  const nextChapterCount = values.execution_scope === 'next_n_chapters'
    ? values.next_chapter_count
    : null;
  return {
    total_chapters: values.total_chapters,
    config: {
      execution_scope: values.execution_scope,
      human_gate_mode: values.human_gate_mode,
      gate_interval: values.gate_interval,
      next_chapter_count: nextChapterCount,
      max_chapters: values.max_chapters,
      max_tokens: values.max_tokens,
      max_estimated_cost: values.max_estimated_cost || null,
      max_runtime_seconds: values.max_runtime_seconds,
      max_step_attempts: values.max_step_attempts,
      max_consecutive_provider_failures: values.max_consecutive_provider_failures,
      max_consecutive_quality_failures: values.max_consecutive_quality_failures,
      regenerate_existing: false,
      run_book_review: values.run_book_review,
      run_book_polish: values.run_book_polish,
      export_format: 'txt',
    },
  };
};

const PreferenceSwitch = ({
  label,
  checked,
  onChange,
}: {
  label: string;
  checked: boolean;
  onChange: (checked: boolean) => void;
}) => (
  <Space size={6}>
    <Text>{label}</Text>
    <Switch aria-label={label} size="small" checked={checked} onChange={onChange} />
  </Space>
);

const RunMetrics = ({
  run,
  steps,
}: {
  run: NovelAutopilotRun;
  steps: NovelAutopilotStepRun[];
}) => {
  const chapterProgress = run.total_chapters > 0
    ? Math.min(100, Math.round((run.completed_chapters / run.total_chapters) * 100))
    : 0;
  const activeStep = steps.find((step) => step.status === 'running' || step.status === 'queued') ?? null;
  const tokenLimit = run.max_tokens === null ? '无限制' : formatNumber(run.max_tokens);
  const costLimit = run.max_estimated_cost === null ? '无限制' : `$${run.max_estimated_cost.toFixed(4)}`;
  const stepAttemptLimit = run.max_step_attempts ?? '—';
  const providerFailureLimit = run.max_consecutive_provider_failures ?? '—';
  const qualityFailureLimit = run.max_consecutive_quality_failures ?? '—';

  return (
    <Card size="small" title="运行指标" styles={{ body: { padding: 12 } }}>
      <Row gutter={[12, 12]}>
        <Col xs={12} md={6}>
          <Statistic title="章节进度" value={run.completed_chapters} suffix={`/ ${run.total_chapters}`} />
        </Col>
        <Col xs={12} md={6}>
          <Statistic title="累计字数" value={run.total_word_count} formatter={(value) => formatNumber(Number(value))} />
        </Col>
        <Col xs={12} md={6}>
          <Statistic
            title="已观察输出 Token（估算）"
            value={run.used_tokens}
            formatter={(value) => formatNumber(Number(value))}
            suffix={`/ ${tokenLimit}`}
          />
        </Col>
        <Col xs={12} md={6}>
          <Statistic
            title="成本预算（需 Provider 计价）"
            value={run.estimated_cost}
            precision={4}
            prefix="$"
            suffix={`/ ${costLimit}`}
          />
        </Col>
      </Row>
      <Progress percent={chapterProgress} status={run.status === 'failed' ? 'exception' : undefined} />
      <Row gutter={[12, 8]} style={{ marginTop: 8 }}>
        <Col xs={24} md={8}><Text type="secondary">运行时间：{formatRuntime(run)}</Text></Col>
        <Col xs={12} md={8}><Text type={run.failed_chapter_count ? 'danger' : 'secondary'}>失败章节：{run.failed_chapter_count}</Text></Col>
        <Col xs={12} md={8}><Text type={run.pending_rewrite_count ? 'warning' : 'secondary'}>待返修：{run.pending_rewrite_count}</Text></Col>
        <Col xs={12} md={8}><Text type={run.consecutive_provider_failures ? 'danger' : 'secondary'}>Provider 连续失败：{run.consecutive_provider_failures} / {providerFailureLimit}</Text></Col>
        <Col xs={12} md={8}><Text type={run.consecutive_quality_failures ? 'warning' : 'secondary'}>质量连续失败：{run.consecutive_quality_failures} / {qualityFailureLimit}</Text></Col>
        <Col xs={12} md={8}><Text type="secondary">当前 Step 尝试：{activeStep?.attempt ?? '—'} / {stepAttemptLimit}</Text></Col>
        <Col xs={12} md={8}><Text type="secondary">Run 版本：{run.version} / Epoch {run.epoch}</Text></Col>
      </Row>
    </Card>
  );
};

const RunSummary = ({ run }: { run: NovelAutopilotRun }) => {
  const statusMeta = RUN_STATUS_META[run.status];
  const errorDescription = run.last_error_code ? describeRunError(run.last_error_code) : null;
  return (
    <Card size="small" title="当前运行状态">
      <Descriptions size="small" column={{ xs: 1, sm: 2, lg: 3 }}>
        <Descriptions.Item label="状态"><Tag color={statusMeta.color}>{statusMeta.label}</Tag></Descriptions.Item>
        <Descriptions.Item label="阶段">{PHASE_LABELS[run.current_phase]}</Descriptions.Item>
        <Descriptions.Item label="当前步骤">{run.current_step ? STEP_LABELS[run.current_step] : '—'}</Descriptions.Item>
        <Descriptions.Item label="当前章节">{run.current_chapter_number ? `第 ${run.current_chapter_number} 章` : '—'}</Descriptions.Item>
        <Descriptions.Item label="执行范围">{EXECUTION_SCOPE_LABELS[run.execution_scope]}</Descriptions.Item>
        <Descriptions.Item label="人工门">{HUMAN_GATE_LABELS[run.human_gate_mode]}</Descriptions.Item>
        <Descriptions.Item label="导出格式">
          {run.export_format ? EXPORT_FORMAT_LABELS[run.export_format] : '—'}
        </Descriptions.Item>
        <Descriptions.Item label="创建时间">{formatTimestamp(run.created_at)}</Descriptions.Item>
        <Descriptions.Item label="更新时间">{formatTimestamp(run.updated_at)}</Descriptions.Item>
        {run.next_attempt_at ? (
          <Descriptions.Item label="预计重试时间">
            <Text type="warning">{formatTimestamp(run.next_attempt_at)}</Text>
          </Descriptions.Item>
        ) : null}
        <Descriptions.Item label="活动后台任务">{run.active_background_task_id ?? '—'}</Descriptions.Item>
      </Descriptions>
      {errorDescription ? (
        <Alert style={{ marginTop: 12 }} type="error" showIcon message="运行已记录错误" description={errorDescription} />
      ) : null}
    </Card>
  );
};

const StepTimeline = ({ steps }: { steps: NovelAutopilotStepRun[] }) => {
  const columns = [
    {
      title: '步骤',
      dataIndex: 'step_type',
      key: 'step_type',
      width: 220,
      render: (value: NovelAutopilotStepType, step: NovelAutopilotStepRun) => {
        const isChapterScoped = step.chapter_number !== null;
        const stepLabel = (
          <Text strong style={{ display: 'block', cursor: isChapterScoped ? 'help' : undefined }}>
            {STEP_LABELS[value]}
          </Text>
        );
        return (
          <div style={{ minWidth: 0 }}>
            {isChapterScoped ? <Tooltip title={step.step_key}>{stepLabel}</Tooltip> : stepLabel}
            {!isChapterScoped ? (
              <Text
                type="secondary"
                ellipsis={{ tooltip: step.step_key }}
                style={{ display: 'block', fontSize: 12 }}
              >
                {step.step_key}
              </Text>
            ) : null}
          </div>
        );
      },
    },
    {
      title: '章节',
      dataIndex: 'chapter_number',
      key: 'chapter_number',
      width: 72,
      align: 'center' as const,
      render: (value: number | null) => (
        <Text style={{ whiteSpace: 'nowrap' }}>{value ? `第${value}章` : '—'}</Text>
      ),
    },
    {
      title: '尝试',
      dataIndex: 'attempt',
      key: 'attempt',
      width: 64,
      align: 'center' as const,
      render: (value: number) => <Text style={{ whiteSpace: 'nowrap' }}>{value}次</Text>,
    },
    {
      title: '状态',
      dataIndex: 'status',
      key: 'status',
      width: 92,
      align: 'center' as const,
      render: (value: NovelAutopilotStepStatus) => {
        const meta = STEP_STATUS_META[value];
        return (
          <Tag color={meta.color} style={{ marginInlineEnd: 0, whiteSpace: 'nowrap' }}>
            {meta.label}
          </Tag>
        );
      },
    },
    {
      title: '质量决定',
      dataIndex: 'quality_decision',
      key: 'quality_decision',
      width: 104,
      align: 'center' as const,
      render: (value: NovelAutopilotQualityDecision | null) => {
        if (!value) {
          return '—';
        }
        const meta = QUALITY_DECISION_META[value];
        return (
          <Tag color={meta.color} style={{ marginInlineEnd: 0, whiteSpace: 'nowrap' }}>
            {meta.label}
          </Tag>
        );
      },
    },
    {
      title: '错误代码',
      dataIndex: 'error_code',
      key: 'error_code',
      width: 140,
      align: 'left' as const,
      render: (value: string | null) => (
        <Text
          type={value ? 'danger' : 'secondary'}
          ellipsis={value ? { tooltip: value } : false}
          style={{ display: 'block', width: '100%', whiteSpace: 'nowrap' }}
        >
          {value ?? '—'}
        </Text>
      ),
    },
    {
      title: '更新时间',
      dataIndex: 'updated_at',
      key: 'updated_at',
      width: 156,
      render: (value: string) => (
        <Text style={{ fontSize: 12, whiteSpace: 'nowrap' }}>{formatTimestamp(value)}</Text>
      ),
    },
  ];

  return (
    <Card size="small" title={`步骤时间线（${steps.length}）`}>
      <Table
        rowKey="id"
        size="small"
        columns={columns}
        dataSource={steps}
        pagination={{ pageSize: 10, hideOnSinglePage: true }}
        scroll={{ x: 848 }}
        tableLayout="fixed"
      />
    </Card>
  );
};

const CreateRunPanel = ({
  disabled,
  loading,
  onCreate,
}: {
  disabled: boolean;
  loading: boolean;
  onCreate: (request: CreateNovelAutopilotRunRequest) => Promise<void>;
}) => {
  const [form] = Form.useForm<CreateRunFormValues>();
  const executionScope = Form.useWatch('execution_scope', form);
  const humanGateMode = Form.useWatch('human_gate_mode', form);

  return (
    <Collapse
      defaultActiveKey={disabled ? [] : ['create']}
      items={[{
        key: 'create',
        label: '启动新的完整成书 Run',
        children: (
          <Form<CreateRunFormValues>
            form={form}
            layout="vertical"
            initialValues={DEFAULT_FORM_VALUES}
            disabled={disabled || loading}
            onFinish={(values) => onCreate(buildCreateRequest(values))}
          >
            {disabled ? (
              <Alert
                type="info"
                showIcon
                style={{ marginBottom: 16 }}
                message="项目已有活动 Run"
                description="创建接口会幂等返回现有 Run。请先完成或取消当前 Run，再启动新配置。"
              />
            ) : null}
            <Alert
              type="info"
              showIcon
              style={{ marginBottom: 16 }}
              message="当前只生成缺失章节"
              description="自动创作会保留已有正文，仅补齐尚未生成的章节；当前不支持覆盖或重新生成既有章节，最终导出格式固定为 TXT。"
            />
            <Row gutter={16}>
              <Col xs={24} md={8}>
                <Form.Item name="execution_scope" label="执行范围" rules={[{ required: true }]}>
                  <Select options={Object.entries(EXECUTION_SCOPE_LABELS).map(([value, label]) => ({ value, label }))} />
                </Form.Item>
              </Col>
              <Col xs={24} md={8}>
                <Form.Item name="total_chapters" label="目标总章节数" rules={[{ required: true, type: 'number', min: 1 }]}>
                  <InputNumber min={1} max={10_000} style={{ width: '100%' }} />
                </Form.Item>
              </Col>
              <Col xs={24} md={8}>
                <Form.Item name="human_gate_mode" label="人工门策略" rules={[{ required: true }]}>
                  <Select
                    options={CREATE_HUMAN_GATE_MODES.map((value) => ({
                      value,
                      label: HUMAN_GATE_LABELS[value],
                    }))}
                  />
                </Form.Item>
              </Col>
            </Row>
            <Row gutter={16}>
              {executionScope === 'next_n_chapters' ? (
                <Col xs={24} md={8}>
                  <Form.Item name="next_chapter_count" label="接下来生成章节数" rules={[{ required: true, type: 'number', min: 1 }]}>
                    <InputNumber min={1} style={{ width: '100%' }} />
                  </Form.Item>
                </Col>
              ) : null}
              {humanGateMode === 'every_n_chapters' ? (
                <Col xs={24} md={8}>
                  <Form.Item name="gate_interval" label="人工门间隔章节" rules={[{ required: true, type: 'number', min: 1 }]}>
                    <InputNumber min={1} style={{ width: '100%' }} />
                  </Form.Item>
                </Col>
              ) : null}
              <Col xs={24} md={8}>
                <Form.Item name="max_chapters" label="最大章节预算" rules={[{ required: true, type: 'number', min: 1 }]}>
                  <InputNumber min={1} max={10_000} style={{ width: '100%' }} />
                </Form.Item>
              </Col>
              <Col xs={24} md={8}>
                <Form.Item name="max_tokens" label="最大估算输出 Token 预算" rules={[{ required: true, type: 'number', min: 1 }]}>
                  <InputNumber min={1} step={100_000} style={{ width: '100%' }} />
                </Form.Item>
              </Col>
            </Row>
            <Row gutter={16}>
              <Col xs={24} md={8}>
                <Form.Item
                  name="max_estimated_cost"
                  label="最大预估成本（USD，可留空）"
                  extra="当前版本没有统一 Provider 计价来源；填写后会安全地进入人工门，不会使用伪造价格继续运行。"
                  rules={[{ type: 'number', min: 0.0001 }]}
                >
                  <InputNumber min={0.0001} precision={4} style={{ width: '100%' }} />
                </Form.Item>
              </Col>
              <Col xs={24} md={8}>
                <Form.Item name="max_runtime_seconds" label="最大运行秒数" rules={[{ required: true, type: 'number', min: 1 }]}>
                  <InputNumber min={1} step={3600} style={{ width: '100%' }} />
                </Form.Item>
              </Col>
              <Col xs={8} md={4}>
                <Form.Item name="max_step_attempts" label="单步尝试" rules={[{ required: true, type: 'number', min: 1 }]}>
                  <InputNumber min={1} style={{ width: '100%' }} />
                </Form.Item>
              </Col>
              <Col xs={8} md={4}>
                <Form.Item name="max_consecutive_provider_failures" label="Provider 失败" rules={[{ required: true, type: 'number', min: 1 }]}>
                  <InputNumber min={1} style={{ width: '100%' }} />
                </Form.Item>
              </Col>
              <Col xs={8} md={4}>
                <Form.Item name="max_consecutive_quality_failures" label="质量失败" rules={[{ required: true, type: 'number', min: 1 }]}>
                  <InputNumber min={1} style={{ width: '100%' }} />
                </Form.Item>
              </Col>
            </Row>
            <Space size="large" wrap>
              <Form.Item name="run_book_review" valuePropName="checked" noStyle><Switch /> </Form.Item><Text>执行全书审查</Text>
              <Form.Item name="run_book_polish" valuePropName="checked" noStyle><Switch /> </Form.Item><Text>执行全书润色</Text>
              <Tag icon={<DownloadOutlined />} color="blue">导出格式：TXT</Tag>
            </Space>
            <Divider />
            <Button type="primary" htmlType="submit" icon={<RobotOutlined />} loading={loading} disabled={disabled}>
              启动自动创作
            </Button>
          </Form>
        ),
      }]}
    />
  );
};

const HumanGatePanel = ({
  run,
  steps,
  loading,
  onDecision,
  onGuidance,
}: {
  run: NovelAutopilotRun;
  steps: NovelAutopilotStepRun[];
  loading: boolean;
  onDecision: (decision: NovelAutopilotHumanDecision, guidance?: string) => Promise<void>;
  onGuidance: (guidance: string) => Promise<void>;
}) => {
  const [guidance, setGuidance] = useState('');
  if (run.status !== 'waiting_human' && run.status !== 'paused') {
    return null;
  }

  const latestErrorCode = run.last_error_code ?? '';
  const candidateId = steps.find((step) => step.candidate_id)?.candidate_id ?? null;
  const waitingHumanMessage = candidateId
    ? '候选已保存，等待人工复核'
    : latestErrorCode && isProviderFailureWaitingHumanCode(latestErrorCode)
      ? '模型 Provider 调用失败，未生成可供人工接受的候选'
      : WAITING_HUMAN_RESULT_INVALID_CODES.has(latestErrorCode)
        ? '模型返回的章节结果无效，未生成可供人工接受的候选'
        : WAITING_HUMAN_CONTEXT_INVALID_CODES.has(latestErrorCode)
          ? '章节上下文无效，未生成可供人工接受的候选'
      : '自动流程正在等待人工决定';
  const showCandidateActions = run.status === 'waiting_human' && candidateId !== null;

  const submitGuidance = async () => {
    if (!guidance.trim()) {
      message.warning('请输入后续创作指导');
      return;
    }
    await onGuidance(guidance.trim());
    setGuidance('');
  };

  return (
    <Card size="small" title={run.status === 'waiting_human' ? '人工质量门' : '暂停期间指导'}>
      <Alert
        type={run.status === 'waiting_human' ? 'warning' : 'info'}
        showIcon
        message={run.status === 'waiting_human' ? waitingHumanMessage : '流程已暂停，可以补充后续创作指导'}
        description="指导文本只应进入受控生成输入，不应出现在公开 Run、日志或审计结果中。"
        style={{ marginBottom: 12 }}
      />
      <Input.TextArea
        value={guidance}
        onChange={(event) => setGuidance(event.target.value)}
        maxLength={4_000}
        showCount
        autoSize={{ minRows: 3, maxRows: 8 }}
        placeholder="例如：后续三章降低战斗密度，加强人物关系推进，并保持既有世界观约束。"
      />
      <Space wrap style={{ marginTop: 12 }}>
        {run.status === 'paused' ? (
          <Button loading={loading} onClick={() => void submitGuidance()}>保存后续指导</Button>
        ) : showCandidateActions ? (
          <>
            <Button type="primary" icon={<CheckCircleOutlined />} loading={loading} onClick={() => void onDecision('accept', guidance)}>接受并继续</Button>
            <Button icon={<ReloadOutlined />} loading={loading} onClick={() => void onDecision('retry', guidance)}>重试</Button>
            <Button icon={<CaretRightOutlined />} loading={loading} onClick={() => void onDecision('repair', guidance)}>返修</Button>
            <Popconfirm title="停止后将把当前 Run 标记为已取消，确定继续？" onConfirm={() => void onDecision('stop', guidance)}>
              <Button danger icon={<StopOutlined />} loading={loading}>停止</Button>
            </Popconfirm>
          </>
        ) : (
          <>
            <Button icon={<ReloadOutlined />} loading={loading} onClick={() => void onDecision('retry', guidance)}>重试</Button>
            <Button icon={<CaretRightOutlined />} loading={loading} onClick={() => void onDecision('repair', guidance)}>返修</Button>
            <Popconfirm title="停止后将把当前 Run 标记为已取消，确定继续？" onConfirm={() => void onDecision('stop', guidance)}>
              <Button danger icon={<StopOutlined />} loading={loading}>停止</Button>
            </Popconfirm>
          </>
        )}
      </Space>
    </Card>
  );
};

const ExportSummary = ({ run }: { run: NovelAutopilotRun }) => {
  if (!run.final_export_ref) {
    return null;
  }
  const descriptor = parseExportDescriptor(run.final_export_ref);
  return (
    <Card size="small" title="最终导出产物" extra={<Tag color={descriptor ? 'success' : 'warning'}>{descriptor ? '已校验描述符' : '无法解析描述符'}</Tag>}>
      {descriptor ? (
        <Descriptions size="small" column={{ xs: 1, md: 2 }}>
          <Descriptions.Item label="文件名">{descriptor.filename}</Descriptions.Item>
          <Descriptions.Item label="格式">{descriptor.format.toUpperCase()}</Descriptions.Item>
          <Descriptions.Item label="章节数">{descriptor.chapter_count}</Descriptions.Item>
          <Descriptions.Item label="总字数">{formatNumber(descriptor.total_word_count)}</Descriptions.Item>
          <Descriptions.Item label="内容摘要" span={2}><Text code copyable>{descriptor.content_digest}</Text></Descriptions.Item>
        </Descriptions>
      ) : (
        <Alert type="warning" showIcon message="后端返回了导出引用，但不是受支持的 project-export-artifact/v1 描述符。" />
      )}
    </Card>
  );
};

export const NovelAutopilotWorkbench = ({ projectId }: NovelAutopilotWorkbenchProps) => {
  const { token } = theme.useToken();
  const { state, refresh, createRun, pause, resume, cancel, updateGuidance, submitDecision } = useNovelAutopilotWorkbench(projectId);
  const run = state.run;
  const output = useBackgroundTaskOutputStream(run?.active_background_task_id ?? null, Boolean(run?.active_background_task_id));
  const [showRuntimeStatus, setShowRuntimeStatus] = useState(() => readBooleanPreference(SHOW_RUNTIME_STATUS_KEY, true));
  const [showProviderReasoning, setShowProviderReasoning] = useState(() => readBooleanPreference(SHOW_REASONING_KEY, false));
  const [showGeneratedContent, setShowGeneratedContent] = useState(() => readBooleanPreference(SHOW_GENERATED_CONTENT_KEY, true));

  const hasActiveRun = Boolean(run && !isNovelAutopilotRunTerminal(run.status));
  const controls = useMemo(() => {
    if (!run) return null;
    return {
      canPause: run.status === 'running' || run.status === 'queued',
      canResume: run.status === 'paused',
      canCancel: !isNovelAutopilotRunTerminal(run.status),
    };
  }, [run]);

  const updatePreference = (key: string, setter: (value: boolean) => void) => (value: boolean) => {
    setter(value);
    writeBooleanPreference(key, value);
  };

  const handleCreate = async (request: CreateNovelAutopilotRunRequest) => {
    const response = await createRun(request);
    message.success(response.created ? '自动创作 Run 已创建' : '已连接项目中现有的活动 Run');
  };

  const handleAction = async (operation: () => Promise<unknown>, successText: string) => {
    await operation();
    message.success(successText);
  };

  if (state.loading) {
    return <div style={{ minHeight: 280, display: 'grid', placeItems: 'center' }}><Spin size="large" /></div>;
  }

  return (
    <div
      data-testid="novel-autopilot-workbench"
      style={{ minHeight: '100%', display: 'flex', flexDirection: 'column', overflow: 'visible', gap: 16 }}
    >
      <Card
        styles={{ body: { padding: 16 } }}
        style={{ borderColor: token.colorBorderSecondary }}
      >
        <Row gutter={[16, 16]} align="middle" justify="space-between">
          <Col flex="auto">
            <Title level={3} style={{ margin: 0 }}>自动创作工作台</Title>
            <Paragraph type="secondary" style={{ margin: '6px 0 0' }}>
              持久化串联设定、大纲、逐章生成、质量闭环、全书审查、润色与真实导出；每个 Tick 最多推进一个 Step。
            </Paragraph>
          </Col>
          <Col>
            <Space wrap>
              <PreferenceSwitch label="运行状态" checked={showRuntimeStatus} onChange={updatePreference(SHOW_RUNTIME_STATUS_KEY, setShowRuntimeStatus)} />
              <PreferenceSwitch label="Provider 思考" checked={showProviderReasoning} onChange={updatePreference(SHOW_REASONING_KEY, setShowProviderReasoning)} />
              <PreferenceSwitch label="生成内容" checked={showGeneratedContent} onChange={updatePreference(SHOW_GENERATED_CONTENT_KEY, setShowGeneratedContent)} />
              <Button icon={<ReloadOutlined />} loading={state.refreshing} onClick={() => void refresh()}>刷新</Button>
            </Space>
          </Col>
        </Row>
      </Card>

      {state.error ? <Alert type="error" showIcon closable message="自动创作操作失败" description={state.error} /> : null}

      {run && showRuntimeStatus ? (
        <div style={{ position: 'sticky', top: 0, zIndex: 5, display: 'grid', gap: 12, background: token.colorBgLayout, paddingBottom: 4 }}>
          <RunSummary run={run} />
          <RunMetrics run={run} steps={state.steps} />
        </div>
      ) : null}

      {run && controls ? (
        <Card size="small" title="运行控制">
          <Space wrap>
            <Button icon={<PauseCircleOutlined />} disabled={!controls.canPause} loading={state.mutating} onClick={() => void handleAction(pause, '已请求暂停自动创作')}>暂停</Button>
            <Button type="primary" icon={<PlayCircleOutlined />} disabled={!controls.canResume} loading={state.mutating} onClick={() => void handleAction(resume, '已恢复自动创作')}>恢复</Button>
            <Popconfirm title="取消后当前 Run 不可恢复，已提交的业务数据不会删除。确定取消？" onConfirm={() => void handleAction(cancel, '自动创作已取消')}>
              <Button danger icon={<CloseCircleOutlined />} disabled={!controls.canCancel} loading={state.mutating}>取消</Button>
            </Popconfirm>
            <Text type="secondary">控制命令使用 expected_version CAS，避免旧页面覆盖新状态。</Text>
          </Space>
        </Card>
      ) : null}

      {run ? (
        <HumanGatePanel
          run={run}
          steps={state.steps}
          loading={state.mutating}
          onGuidance={async (guidance) => {
            await updateGuidance(guidance);
            message.success('后续指导已更新');
          }}
          onDecision={async (decision, guidance) => {
            await submitDecision(decision, guidance);
            message.success('人工决定已提交');
          }}
        />
      ) : null}

      {run ? (
        <ModelOutputSections
          reasoningContent={output.reasoningContent}
          generatedContent={output.generatedContent}
          reasoningTruncated={output.reasoningTruncated}
          contentTruncated={output.contentTruncated}
          taskStatus={toOutputTaskStatus(run.status)}
          showReasoning={showProviderReasoning}
          showGeneratedContent={showGeneratedContent}
        />
      ) : null}

      {!run ? (
        <Card><Empty description="当前项目还没有自动创作 Run，请配置后启动。" /></Card>
      ) : null}

      <CreateRunPanel disabled={hasActiveRun} loading={state.mutating} onCreate={handleCreate} />

      {run ? <StepTimeline steps={state.steps} /> : null}
      {run ? <ExportSummary run={run} /> : null}
    </div>
  );
};

export default NovelAutopilotWorkbench;
