import { useState, useEffect, useCallback, useRef } from 'react';
import { Button, Modal, Form, Input, Select, message, Row, Col, Empty, Tabs, Card, Tag, Space, Divider, Typography, InputNumber, theme } from 'antd';
import { ThunderboltOutlined, PlusOutlined, EditOutlined, DeleteOutlined, TrophyOutlined } from '@ant-design/icons';
import { useParams } from 'react-router-dom';
import { api } from '../services/modularApi';
import { backgroundTaskApi } from '../services/modularApi';
import { invalidateProjectCareers, loadProjectCareers } from '../services/projectCareers';
import SSEProgressModal from '../components/SSEProgressModal';
import { isActiveBackgroundTask, useBackgroundTaskStore } from '../store/backgroundTasks';
import { formatBackgroundTaskError } from '../utils/taskPolling';
import { useRestorableBackgroundTaskPolling } from '../hooks/useRestorableBackgroundTaskPolling';
import { isRequestCancelledError } from '../services/core/httpClient';
import { designDisplayFont } from '../theme/themeConfig';

const { TextArea } = Input;
const { Title, Text, Paragraph } = Typography;
const CAREER_TASK_REFRESH_KEY_PREFIX = 'background-task-refresh:careers:';
const CAREER_TASK_REFRESH_RETRY_DELAY_MS = 2000;

const hasCareerTaskRefreshBeenHandled = (taskId: string): boolean => {
    try {
        return sessionStorage.getItem(`${CAREER_TASK_REFRESH_KEY_PREFIX}${taskId}`) === '1';
    } catch {
        return false;
    }
};

const markCareerTaskRefreshHandled = (taskId: string) => {
    try {
        sessionStorage.setItem(`${CAREER_TASK_REFRESH_KEY_PREFIX}${taskId}`, '1');
    } catch {
        // ignore sessionStorage failures
    }
};

const createCareerRefreshTaskLock = () => {
    const inFlightTaskIds = new Set<string>();

    return {
        acquire(taskId: string) {
            if (!taskId || inFlightTaskIds.has(taskId)) {
                return false;
            }
            inFlightTaskIds.add(taskId);
            return true;
        },
        release(taskId: string) {
            if (!taskId) {
                return;
            }
            inFlightTaskIds.delete(taskId);
        },
    };
};

const selectActiveCareerTask = (
    tasks: Record<string, import('../store/backgroundTasks').TrackedBackgroundTask>,
    projectId?: string,
) => {
    if (!projectId) {
        return null;
    }

    return Object.values(tasks)
        .filter(
            (task) => task.projectId === projectId
                && (task.taskType === 'careers_generate_system' || task.taskType === 'wizard_career_system')
                && isActiveBackgroundTask(task)
        )
        .sort((left, right) => right.updatedAt - left.updatedAt)[0] ?? null;
};

const selectCompletedCareerTaskRefreshSignature = (
    tasks: Record<string, import('../store/backgroundTasks').TrackedBackgroundTask>,
    projectId?: string,
): string => {
    if (!projectId) {
        return '';
    }

    const completedTask = Object.values(tasks)
        .filter(
            (task) => task.projectId === projectId
                && (task.taskType === 'careers_generate_system' || task.taskType === 'wizard_career_system')
                && task.status === 'completed'
                && !hasCareerTaskRefreshBeenHandled(task.taskId)
        )
        .sort((left, right) => (right.completedAt ?? right.updatedAt) - (left.completedAt ?? left.updatedAt))[0];

    if (!completedTask) {
        return '';
    }

    return `${completedTask.taskId}:${completedTask.completedAt ?? completedTask.updatedAt}`;
};

interface CareerStage {
    level: number;
    name: string;
    description?: string;
}

interface Career {
    id: string;
    project_id: string;
    name: string;
    type: 'main' | 'sub';
    description?: string;
    category?: string;
    stages: CareerStage[];
    max_stage: number;
    requirements?: string;
    special_abilities?: string;
    worldview_rules?: string;
    source: string;
}

export default function Careers() {
    const { token } = theme.useToken();
    const { projectId } = useParams<{ projectId: string }>();
    const [mainCareers, setMainCareers] = useState<Career[]>([]);
    const [subCareers, setSubCareers] = useState<Career[]>([]);
    const [, setLoading] = useState(true);
    const [isModalOpen, setIsModalOpen] = useState(false);
    const [isAIModalOpen, setIsAIModalOpen] = useState(false);
    const [editingCareer, setEditingCareer] = useState<Career | null>(null);
    const [form] = Form.useForm();
    const [aiForm] = Form.useForm();
    const [modal, contextHolder] = Modal.useModal();

    // AI生成状态
    const [aiGenerating, setAiGenerating] = useState(false);
    const [aiProgress, setAiProgress] = useState(0);
    const [aiMessage, setAiMessage] = useState('');
    const activeProjectIdRef = useRef<string | null>(projectId ?? null);
    const careerRequestIdRef = useRef(0);
    const completedCareerRefreshLockRef = useRef(createCareerRefreshTaskLock());
    const completedCareerRefreshRetryTimerRef = useRef<number | null>(null);
    const [completedCareerRefreshRetryTick, setCompletedCareerRefreshRetryTick] = useState(0);
    const activeTrackedCareerTask = useBackgroundTaskStore((state) => selectActiveCareerTask(state.tasks, projectId));
    const completedCareerTaskRefreshSignature = useBackgroundTaskStore(
        (state) => selectCompletedCareerTaskRefreshSignature(state.tasks, projectId)
    );

    useEffect(() => {
        activeProjectIdRef.current = projectId ?? null;
    }, [projectId]);

    const scheduleCompletedCareerRefreshRetry = useCallback(() => {
        if (completedCareerRefreshRetryTimerRef.current) {
            clearTimeout(completedCareerRefreshRetryTimerRef.current);
        }

        completedCareerRefreshRetryTimerRef.current = window.setTimeout(() => {
            completedCareerRefreshRetryTimerRef.current = null;
            setCompletedCareerRefreshRetryTick((value) => value + 1);
        }, CAREER_TASK_REFRESH_RETRY_DELAY_MS);
    }, []);

    const fetchCareers = useCallback(async () => {
        if (!projectId) {
            return;
        }

        const requestId = ++careerRequestIdRef.current;
        const targetProjectId = projectId;
        try {
            setLoading(true);
            const response = await loadProjectCareers(projectId) as { mainCareers: Career[]; subCareers: Career[] };
            if (activeProjectIdRef.current !== targetProjectId || careerRequestIdRef.current !== requestId) {
                return;
            }
            setMainCareers(response.mainCareers || []);
            setSubCareers(response.subCareers || []);
        } catch (error: unknown) {
            console.error('获取职业列表失败:', error);
        } finally {
            if (careerRequestIdRef.current === requestId) {
                setLoading(false);
            }
        }
    }, [projectId]);

    useEffect(() => {
        if (projectId) {
            invalidateProjectCareers(projectId);
            void fetchCareers();
        }
    }, [projectId, fetchCareers]);

    const { currentTaskIdRef: aiTaskIdRef, startTaskPolling: startAiTaskPolling, stopTaskPolling: stopAiTaskPolling } = useRestorableBackgroundTaskPolling({
        projectId,
        activeTrackedTask: activeTrackedCareerTask,
        isMatchingTask: (task) =>
            (task.task_type === 'careers_generate_system' || task.task_type === 'wizard_career_system')
            && (task.status === 'pending' || task.status === 'running'),
        onRestoreTask: ({ progress, message: taskMessage }) => {
            setAiGenerating(true);
            setAiProgress(progress || 0);
            setAiMessage(taskMessage || '正在恢复职业体系生成任务...');
        },
        createPollingOptions: () => ({
            pollTask: (currentPollingTaskId) => backgroundTaskApi.getTaskStatus(currentPollingTaskId),
            onTask: (task) => {
                setAiProgress(task.progress || 0);
                setAiMessage(task.message || '');
            },
            onCompleted: () => {
                stopAiTaskPolling();
                aiTaskIdRef.current = null;
                setAiGenerating(false);
                message.success('职业体系生成完成');
            },
            onFailed: (task) => {
                stopAiTaskPolling();
                aiTaskIdRef.current = null;
                setAiGenerating(false);
                message.error(formatBackgroundTaskError(task.error, task.message, '生成失败'));
            },
            onCancelled: (task) => {
                stopAiTaskPolling();
                aiTaskIdRef.current = null;
                setAiGenerating(false);
                message.info(task.message || '任务已取消');
            },
            onPollingError: (error) => {
                if (isRequestCancelledError(error)) {
                    return;
                }
                console.error('轮询职业生成任务失败:', error);
                stopAiTaskPolling();
                aiTaskIdRef.current = null;
                setAiGenerating(false);
                setAiMessage('职业生成状态同步失败，请刷新后重试');
                void fetchCareers();
                message.error('职业生成状态同步失败，请刷新后重试');
            },
        }),
    });

    useEffect(() => {
        if (!projectId || aiTaskIdRef.current || aiGenerating) {
            return;
        }

        if (!completedCareerTaskRefreshSignature) {
            return;
        }

        const [taskId] = completedCareerTaskRefreshSignature.split(':');
        if (!taskId) {
            return;
        }
        if (!completedCareerRefreshLockRef.current.acquire(taskId)) {
            return;
        }

        invalidateProjectCareers(projectId);
        void fetchCareers()
            .then(() => {
                markCareerTaskRefreshHandled(taskId);
            })
            .catch((error) => {
                console.error('刷新职业体系失败:', error);
                scheduleCompletedCareerRefreshRetry();
            })
            .finally(() => {
                completedCareerRefreshLockRef.current.release(taskId);
            });
    }, [
        aiGenerating,
        aiTaskIdRef,
        completedCareerRefreshRetryTick,
        completedCareerTaskRefreshSignature,
        fetchCareers,
        projectId,
        scheduleCompletedCareerRefreshRetry,
    ]);

    useEffect(() => {
        return () => {
            if (completedCareerRefreshRetryTimerRef.current) {
                clearTimeout(completedCareerRefreshRetryTimerRef.current);
                completedCareerRefreshRetryTimerRef.current = null;
            }
        };
    }, []);

    const handleAIGenerateBackground = async (values: { main_career_count: number; sub_career_count: number }) => {
        if (aiGenerating || activeTrackedCareerTask) {
            message.info('已有后台职业生成任务在运行，请稍后查看结果');
            return;
        }
        if (!projectId) {
            message.error('缺少项目ID');
            return;
        }

        setIsAIModalOpen(false);
        setAiGenerating(true);
        setAiProgress(0);
        setAiMessage('正在创建后台任务...');

        try {
            const task = await backgroundTaskApi.createTask({
                task_type: 'careers_generate_system',
                project_id: projectId,
                payload: {
                    main_career_count: values.main_career_count,
                    sub_career_count: values.sub_career_count,
                }
            });

            message.success('后台职业生成任务已创建，可继续进行其他操作');
            aiTaskIdRef.current = task.task_id;
            startAiTaskPolling(task.task_id);
        } catch (err: unknown) {
            stopAiTaskPolling();
            aiTaskIdRef.current = null;
            setAiGenerating(false);
            const error = err as Error;
            message.error(error.message || '启动生成失败');
        }
    };

    const handleCancelAIGenerate = async () => {
        const taskId = aiTaskIdRef.current;
        if (!taskId) {
            return;
        }

        try {
            await backgroundTaskApi.cancelTask(taskId);
            message.info('正在取消后台任务...');
        } catch (error) {
            console.error('取消职业生成任务失败:', error);
            message.error('取消任务失败，请重试');
        } finally {
            stopAiTaskPolling();
            aiTaskIdRef.current = null;
            setAiGenerating(false);
        }
    };

    const handleOpenModal = (career?: Career) => {
        if (career) {
            setEditingCareer(career);
            form.setFieldsValue({
                ...career,
                stages: career.stages.map(s => `${s.level}. ${s.name}${s.description ? ` - ${s.description}` : ''}`).join('\n')
            });
        } else {
            setEditingCareer(null);
            form.resetFields();
        }
        setIsModalOpen(true);
    };

    interface CareerFormValues {
        name: string;
        type: 'main' | 'sub';
        description?: string;
        category?: string;
        stages?: string;
        requirements?: string;
        special_abilities?: string;
        worldview_rules?: string;
    }

    const handleSubmit = async (values: CareerFormValues) => {
        try {
            // 解析阶段数据
            const stagesText = values.stages || '';
            const stages: CareerStage[] = stagesText.split('\n')
                .filter((line: string) => line.trim())
                .map((line: string, index: number) => {
                    const match = line.match(/^(\d+)\.\s*([^-]+)(?:\s*-\s*(.*))?$/);
                    if (match) {
                        return {
                            level: parseInt(match[1]),
                            name: match[2].trim(),
                            description: match[3]?.trim() || ''
                        };
                    }
                    return {
                        level: index + 1,
                        name: line.trim(),
                        description: ''
                    };
                });

            const data = {
                ...values,
                stages,
                max_stage: stages.length
            };

            if (editingCareer) {
                await api.put(`/careers/${editingCareer.id}`, data);
                message.success('职业更新成功');
            } else {
                await api.post('/careers', {
                    ...data,
                    project_id: projectId,
                    source: 'manual'
                });
                message.success('职业创建成功');
            }

            setIsModalOpen(false);
            form.resetFields();
            invalidateProjectCareers(projectId);
            void fetchCareers();
        } catch (error: unknown) {
            const axiosError = error as { response?: { data?: { detail?: string } } };
            message.error(axiosError.response?.data?.detail || '操作失败');
        }
    };

    const handleDelete = async (id: string) => {
        modal.confirm({
            title: '确认删除',
            content: '确定要删除这个职业吗？如果有角色使用了该职业，将无法删除。',
            centered: true,
            onOk: async () => {
                try {
                    await api.delete(`/careers/${id}`);
                    message.success('职业删除成功');
                    invalidateProjectCareers(projectId);
                    void fetchCareers();
                } catch (error: unknown) {
                    const axiosError = error as { response?: { data?: { detail?: string } } };
                    message.error(axiosError.response?.data?.detail || '删除失败');
                }
            }
        });
    };
    const handleAIGenerate = async (values: { main_career_count: number; sub_career_count: number }) => {
        return handleAIGenerateBackground(values);
    };

    const renderCareerCard = (career: Career) => (
        <Card
            key={career.id}
            title={
                <Space>
                    <TrophyOutlined />
                    {career.name}
                    <Tag color={career.source === 'ai' ? 'blue' : 'default'}>
                        {career.source === 'ai' ? '智能生成' : '手动创建'}
                    </Tag>
                    {career.category && <Tag>{career.category}</Tag>}
                </Space>
            }
            extra={
                <Space>
                    <Button size="small" icon={<EditOutlined />} onClick={() => handleOpenModal(career)} />
                    <Button size="small" danger icon={<DeleteOutlined />} onClick={() => handleDelete(career.id)} />
                </Space>
            }
            style={{
                marginBottom: 16,
                borderRadius: 22,
                border: career.source === 'ai'
                    ? `1px solid color-mix(in srgb, ${token.colorPrimary} 20%, white 80%)`
                    : `1px solid ${token.colorBorderSecondary}`,
                background: career.source === 'ai'
                    ? `linear-gradient(180deg, color-mix(in srgb, ${token.colorPrimary} 7%, ${token.colorBgContainer} 93%) 0%, ${token.colorBgContainer} 100%)`
                    : token.colorBgContainer,
                boxShadow: `0 14px 28px color-mix(in srgb, ${token.colorText} 6%, transparent)`,
            }}
        >
            <Paragraph ellipsis={{ rows: 2 }}>{career.description || '暂无描述'}</Paragraph>
            <Divider style={{ margin: '12px 0' }} />
            <Text strong>阶段体系（共{career.max_stage}个）：</Text>
            <div style={{ maxHeight: 120, overflowY: 'auto', marginTop: 8 }}>
                {career.stages.slice(0, 5).map(stage => (
                    <div key={stage.level} style={{ marginLeft: 16, marginBottom: 4 }}>
                        <Text type="secondary">{stage.level}. {stage.name}</Text>
                        {stage.description && <Text type="secondary" style={{ fontSize: 12 }}> - {stage.description}</Text>}
                    </div>
                ))}
                {career.stages.length > 5 && (
                    <Text type="secondary" style={{ marginLeft: 16 }}>...还有{career.stages.length - 5}个阶段</Text>
                )}
            </div>
            {career.special_abilities && (
                <>
                    <Divider style={{ margin: '12px 0' }} />
                    <Text strong>特殊能力：</Text>
                    <Paragraph ellipsis={{ rows: 2 }} style={{ marginTop: 4 }}>{career.special_abilities}</Paragraph>
                </>
            )}
        </Card>
    );

    const tabItems = [
        {
            key: 'main',
            label: `主职业 (${mainCareers.length})`,
            children: mainCareers.length > 0 ? (
                <div>{mainCareers.map(renderCareerCard)}</div>
            ) : (
                <Empty description="还没有主职业" />
            )
        },
        {
            key: 'sub',
            label: `副职业 (${subCareers.length})`,
            children: subCareers.length > 0 ? (
                <div>{subCareers.map(renderCareerCard)}</div>
            ) : (
                <Empty description="还没有副职业" />
            )
        }
    ];

    const heroBackground = `linear-gradient(135deg,
        color-mix(in srgb, ${token.colorPrimary} 74%, #6f4638 26%) 0%,
        color-mix(in srgb, ${token.colorInfo} 26%, #18242d 74%) 100%)`;
    const editorialInk = '#fff9f0';
    const totalCareers = mainCareers.length + subCareers.length;
    const aiCareerCount = [...mainCareers, ...subCareers].filter((career) => career.source === 'ai').length;
    const panelBackground = `linear-gradient(180deg,
        color-mix(in srgb, ${token.colorBgContainer} 95%, white 5%) 0%,
        color-mix(in srgb, ${token.colorFillAlter} 44%, ${token.colorBgContainer} 56%) 100%)`;
    const panelBorder = `1px solid color-mix(in srgb, ${token.colorBorderSecondary} 88%, white 12%)`;
    const summaryItems: Array<{ label: string; value: number | string; accent: string; compact?: boolean }> = [
        { label: '职业总数', value: totalCareers, accent: editorialInk },
        { label: '主职业', value: mainCareers.length, accent: token.colorSuccess },
        { label: '副职业', value: subCareers.length, accent: token.colorInfo },
        { label: 'AI 生成', value: aiCareerCount, accent: editorialInk },
    ];
    const careerGuideSteps = [
        '先看主职业、副职业和 AI 生成占比，确认这次是在补主干体系还是补充支线分工。',
        '再切到对应 Tab 审核职业描述、阶段和能力信息，避免主副职业混在一起修改。',
        '最后再决定新增、编辑或智能生成，把体系扩展放在已经看清现状之后。',
    ];
    const careerFocus = aiGenerating || activeTrackedCareerTask
        ? {
            title: '等待职业体系补全回流',
            note: '当前有一条职业生成任务正在执行，适合先观察进度，等结果回流后再统一整理职业卡片。',
        }
        : totalCareers === 0
            ? {
                title: '先搭主职业骨架',
                note: '当前还没有职业条目，优先建立主职业基线，再考虑副职业和阶段延展会更稳。',
            }
            : subCareers.length === 0
                ? {
                    title: '补充副职业分工',
                    note: '主职业已经存在，下一步更适合补齐副职业，让世界观里的辅助、生产和支线能力更完整。',
                }
                : {
                    title: '做一次职业体系巡检',
                    note: '当前主副职业都已成型，适合检查分类是否清晰、阶段是否连贯，以及 AI 生成内容是否需要人工收束。',
                };

    return (
        <>
            {contextHolder}
            <div style={{
            height: '100%',
            display: 'flex',
            flexDirection: 'column',
            overflow: 'hidden',
            gap: 16,
            paddingBottom: 24,
        }}>
            <Card
                variant="borderless"
                style={{
                    background: heroBackground,
                    borderRadius: 28,
                    border: `1px solid color-mix(in srgb, ${token.colorBgContainer} 12%, transparent)`,
                    boxShadow: `0 26px 52px color-mix(in srgb, ${token.colorText} 20%, transparent)`,
                    overflow: 'hidden',
                    position: 'relative',
                }}
                styles={{ body: { padding: 24 } }}
            >
                <div style={{ position: 'absolute', top: -56, right: -28, width: 172, height: 172, borderRadius: '50%', background: 'rgba(255,255,255,0.08)', pointerEvents: 'none' }} />
                <div style={{ position: 'absolute', bottom: -32, left: '28%', width: 120, height: 120, borderRadius: '50%', background: 'rgba(255,255,255,0.05)', pointerEvents: 'none' }} />
                <Row gutter={[24, 18]} align="middle" style={{ position: 'relative', zIndex: 1 }}>
                    <Col xs={24} lg={14}>
                        <Space direction="vertical" size={8} style={{ width: '100%' }}>
                            <Text style={{ color: 'rgba(255,255,255,0.72)', fontSize: 11, letterSpacing: '0.18em', textTransform: 'uppercase' }}>
                                Career Ledger
                            </Text>
                            <Title level={2} style={{ margin: 0, color: editorialInk, fontFamily: designDisplayFont, letterSpacing: '-0.03em' }}>
                                <TrophyOutlined style={{ marginRight: 8 }} />
                                职业管理
                            </Title>
                            <Paragraph style={{ margin: 0, color: 'rgba(255,255,255,0.82)', fontSize: 15, lineHeight: 1.8 }}>
                                在这里维护世界里的主职业、副职业与阶段体系。它更像职业设定台账：既要能补充新条目，也要能看见整套体系的密度与分工。
                            </Paragraph>
                            <Space wrap>
                                <Button
                                    type="dashed"
                                    icon={<ThunderboltOutlined />}
                                    onClick={() => {
                                        aiForm.resetFields();
                                        setIsAIModalOpen(true);
                                    }}
                                    loading={Boolean(aiGenerating || activeTrackedCareerTask)}
                                    style={{
                                        borderRadius: 999,
                                        borderColor: 'rgba(255,255,255,0.18)',
                                        background: 'rgba(255,255,255,0.08)',
                                        color: editorialInk,
                                    }}
                                >
                                    智能生成新职业
                                </Button>
                                <Button
                                    type="primary"
                                    icon={<PlusOutlined />}
                                    onClick={() => handleOpenModal()}
                                    style={{ borderRadius: 999, paddingInline: 16 }}
                                >
                                    新增职业
                                </Button>
                            </Space>
                        </Space>
                    </Col>
                    <Col xs={24} lg={10}>
                        <Row gutter={[12, 12]}>
                            {summaryItems.map((item) => (
                                <Col xs={12} key={item.label}>
                                    <div
                                        style={{
                                            minHeight: 92,
                                            borderRadius: 18,
                                            padding: '12px 14px',
                                            background: 'rgba(255,255,255,0.08)',
                                            border: '1px solid rgba(255,255,255,0.1)',
                                            backdropFilter: 'blur(10px)',
                                            display: 'flex',
                                            flexDirection: 'column',
                                            justifyContent: 'space-between',
                                        }}
                                    >
                                        <Text style={{ color: 'rgba(255,255,255,0.72)', fontSize: 12, display: 'block' }}>{item.label}</Text>
                                        <Text style={{ color: item.accent, fontWeight: 700, fontSize: 24 }}>{item.value}</Text>
                                    </div>
                                </Col>
                            ))}
                        </Row>
                    </Col>
                </Row>
            </Card>

            <Card
                variant="borderless"
                style={{
                    borderRadius: 22,
                    background: `linear-gradient(135deg, color-mix(in srgb, ${token.colorPrimary} 10%, white 90%) 0%, color-mix(in srgb, ${token.colorInfo} 10%, white 90%) 100%)`,
                    border: `1px solid color-mix(in srgb, ${token.colorPrimary} 16%, white 84%)`,
                    boxShadow: `0 18px 36px color-mix(in srgb, ${token.colorText} 8%, transparent)`,
                }}
                styles={{ body: { padding: 18 } }}
            >
                <Row gutter={[16, 16]}>
                    <Col xs={24} lg={15}>
                        <Space direction="vertical" size={8} style={{ width: '100%' }}>
                            <Text style={{ color: token.colorTextTertiary, fontSize: 12, letterSpacing: '0.12em', textTransform: 'uppercase' }}>
                                Career Guide
                            </Text>
                            <Paragraph style={{ margin: 0, color: token.colorText, lineHeight: 1.75 }}>
                                这个页面更像职业台账与体系校对台。原有 Tabs、AI 生成和编辑提交流程都保持不变，这里只把查看顺序和当前维护重点提炼出来，方便长期整理世界职业谱系。
                            </Paragraph>
                            <div style={{ display: 'flex', flexWrap: 'wrap', gap: 8 }}>
                                {careerGuideSteps.map((item, index) => (
                                    <span
                                        key={item}
                                        style={{
                                            display: 'inline-flex',
                                            alignItems: 'center',
                                            gap: 8,
                                            padding: '6px 12px',
                                            borderRadius: 999,
                                            background: token.colorBgContainer,
                                            border: `1px solid ${token.colorBorderSecondary}`,
                                            color: token.colorTextBase,
                                            fontSize: 12,
                                        }}
                                    >
                                        <span style={{ color: token.colorPrimary, fontWeight: 700 }}>{index + 1}</span>
                                        {item}
                                    </span>
                                ))}
                            </div>
                        </Space>
                    </Col>
                    <Col xs={24} lg={9}>
                        <div
                            style={{
                                height: '100%',
                                borderRadius: 18,
                                padding: '16px 18px 14px',
                                background: `linear-gradient(180deg, ${token.colorBgContainer} 0%, ${token.colorFillAlter} 100%)`,
                                border: `1px solid ${token.colorBorderSecondary}`,
                            }}
                        >
                            <Text style={{ display: 'block', color: token.colorTextTertiary, fontSize: 12, letterSpacing: '0.12em', textTransform: 'uppercase' }}>
                                当前维护焦点
                            </Text>
                            <Title level={5} style={{ margin: '8px 0 6px', color: token.colorTextBase, fontFamily: designDisplayFont }}>
                                {careerFocus.title}
                            </Title>
                            <Paragraph style={{ margin: 0, color: token.colorTextSecondary, lineHeight: 1.75 }}>
                                {careerFocus.note}
                            </Paragraph>
                        </div>
                    </Col>
                </Row>
            </Card>

            <Card
                variant="borderless"
                style={{
                    flex: 1,
                    overflow: 'hidden',
                    background: panelBackground,
                    borderRadius: 24,
                    border: panelBorder,
                    boxShadow: `0 18px 36px color-mix(in srgb, ${token.colorText} 8%, transparent)`,
                }}
                styles={{ body: { height: '100%', padding: 20 } }}
            >
                <Space direction="vertical" size={16} style={{ width: '100%', height: '100%' }}>
                    <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', gap: 12, flexWrap: 'wrap' }}>
                        <Space direction="vertical" size={4}>
                            <Text style={{ fontSize: 12, letterSpacing: '0.12em', textTransform: 'uppercase', color: token.colorTextTertiary }}>
                                Profession Workspace
                            </Text>
                            <Title level={4} style={{ margin: 0, fontFamily: designDisplayFont, color: token.colorTextBase }}>
                                主副职业工作区
                            </Title>
                            <Paragraph style={{ margin: 0, color: token.colorTextSecondary }}>
                                Tabs 继续保留原来的主副职业切换逻辑，只把外层承载改成更适合长期维护的编辑工作区。
                            </Paragraph>
                        </Space>
                        <Space wrap>
                            <Tag color="blue" style={{ borderRadius: 999, paddingInline: 12 }}>主职业 {mainCareers.length}</Tag>
                            <Tag color="purple" style={{ borderRadius: 999, paddingInline: 12 }}>副职业 {subCareers.length}</Tag>
                        </Space>
                    </div>

                    <Divider style={{ margin: 0, borderColor: token.colorBorderSecondary }} />

            <div style={{
                flex: 1,
                overflow: 'auto',
                paddingRight: 4
            }}>
                <Tabs items={tabItems} />
            </div>
                </Space>
            </Card>

            {/* 创建/编辑对话框 */}
            <Modal
                title={editingCareer ? '编辑职业' : '新增职业'}
                open={isModalOpen}
                onCancel={() => {
                    setIsModalOpen(false);
                    form.resetFields();
                }}
                footer={null}
                width={700}
            >
                <Form form={form} layout="vertical" onFinish={handleSubmit}>
                    <Row gutter={16}>
                        <Col span={16}>
                            <Form.Item label="职业名称" name="name" rules={[{ required: true }]}>
                                <Input placeholder="如：剑修、炼丹师" />
                            </Form.Item>
                        </Col>
                        <Col span={8}>
                            <Form.Item label="类型" name="type" rules={[{ required: true }]} initialValue="main">
                                <Select>
                                    <Select.Option value="main">主职业</Select.Option>
                                    <Select.Option value="sub">副职业</Select.Option>
                                </Select>
                            </Form.Item>
                        </Col>
                    </Row>

                    <Form.Item label="职业描述" name="description">
                        <TextArea rows={2} placeholder="描述这个职业..." />
                    </Form.Item>

                    <Form.Item label="职业分类" name="category">
                        <Input placeholder="如：战斗系、生产系、辅助系" />
                    </Form.Item>

                    <Form.Item label="职业阶段" name="stages" tooltip="每行一个阶段，格式：1. 阶段名 - 描述">
                        <TextArea
                            rows={8}
                            placeholder="示例：&#10;1. 炼气期 - 初窥门径&#10;2. 筑基期 - 根基稳固&#10;3. 金丹期 - 凝结金丹"
                        />
                    </Form.Item>

                    <Form.Item label="职业要求" name="requirements">
                        <TextArea rows={2} placeholder="需要什么条件才能修炼..." />
                    </Form.Item>

                    <Form.Item label="特殊能力" name="special_abilities">
                        <TextArea rows={2} placeholder="这个职业的特殊能力..." />
                    </Form.Item>

                    <Form.Item label="世界观规则" name="worldview_rules">
                        <TextArea rows={2} placeholder="如何融入世界观..." />
                    </Form.Item>

                    <Form.Item>
                        <Space style={{ width: '100%', justifyContent: 'flex-end' }}>
                            <Button onClick={() => setIsModalOpen(false)}>取消</Button>
                            <Button type="primary" htmlType="submit">
                                {editingCareer ? '更新' : '创建'}
                            </Button>
                        </Space>
                    </Form.Item>
                </Form>
            </Modal>

            {/* AI生成对话框 */}
            <Modal
                title="智能生成新职业（增量式）"
                open={isAIModalOpen}
                onCancel={() => setIsAIModalOpen(false)}
                footer={null}
            >
                <Form form={aiForm} layout="vertical" onFinish={handleAIGenerate}>
                    <Paragraph type="secondary">
                        系统将分析当前世界观和已有职业，智能生成新的补充职业。
                        <br />
                        💡 可以多次生成，逐步完善职业体系，不会替换已有职业。
                    </Paragraph>
                    <Divider style={{ margin: '12px 0' }} />
                    <Form.Item label="本次新增主职业数量" name="main_career_count" initialValue={3}>
                        <InputNumber min={1} max={10} style={{ width: '100%' }} />
                    </Form.Item>
                    <Form.Item label="本次新增副职业数量" name="sub_career_count" initialValue={5}>
                        <InputNumber min={0} max={15} style={{ width: '100%' }} />
                    </Form.Item>
                    <Form.Item>
                        <Space style={{ width: '100%', justifyContent: 'flex-end' }}>
                            <Button onClick={() => setIsAIModalOpen(false)}>取消</Button>
                            <Button type="primary" icon={<ThunderboltOutlined />} htmlType="submit">
                                开始生成
                            </Button>
                        </Space>
                    </Form.Item>
                </Form>
            </Modal>

            {/* AI生成进度 */}
            <SSEProgressModal
                visible={aiGenerating}
                progress={aiProgress}
                message={aiMessage}
                title="正在生成新职业..."
                blocking={false}
                onCancel={handleCancelAIGenerate}
            />
            </div>
        </>
    );
}
