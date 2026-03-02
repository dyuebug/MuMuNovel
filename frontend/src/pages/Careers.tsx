import { useState, useEffect, useCallback, useRef } from 'react';
import { Button, Modal, Form, Input, Select, message, Row, Col, Empty, Tabs, Card, Tag, Space, Divider, Typography, InputNumber } from 'antd';
import { ThunderboltOutlined, PlusOutlined, EditOutlined, DeleteOutlined, TrophyOutlined } from '@ant-design/icons';
import { useParams } from 'react-router-dom';
import api, { backgroundTaskApi } from '../services/api';
import SSEProgressModal from '../components/SSEProgressModal';

const { TextArea } = Input;
const { Title, Text, Paragraph } = Typography;

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
    const aiTaskPollTimerRef = useRef<number | null>(null);
    const aiTaskIdRef = useRef<string | null>(null);

    const fetchCareers = useCallback(async () => {
        try {
            setLoading(true);
            const response = await api.get('/careers', {
                params: { project_id: projectId }
            }) as { main_careers?: Career[]; sub_careers?: Career[] };
            setMainCareers(response.main_careers || []);
            setSubCareers(response.sub_careers || []);
        } catch (error: unknown) {
            console.error('获取职业列表失败:', error);
        } finally {
            setLoading(false);
        }
    }, [projectId]);

    useEffect(() => {
        if (projectId) {
            fetchCareers();
        }
    }, [projectId, fetchCareers]);

    const stopAiTaskPolling = useCallback(() => {
        if (aiTaskPollTimerRef.current) {
            window.clearInterval(aiTaskPollTimerRef.current);
            aiTaskPollTimerRef.current = null;
        }
    }, []);

    useEffect(() => {
        return () => {
            stopAiTaskPolling();
            aiTaskIdRef.current = null;
        };
    }, [stopAiTaskPolling]);

    const startAiTaskPolling = useCallback((taskId: string) => {
        stopAiTaskPolling();
        aiTaskIdRef.current = taskId;

        const poll = async () => {
            try {
                const task = await backgroundTaskApi.getTaskStatus(taskId);
                setAiProgress(task.progress || 0);
                setAiMessage(task.message || '');

                if (task.status === 'completed') {
                    stopAiTaskPolling();
                    aiTaskIdRef.current = null;
                    setAiGenerating(false);
                    message.success('AI新职业生成完成！');
                    void fetchCareers();
                    return;
                }

                if (task.status === 'failed') {
                    stopAiTaskPolling();
                    aiTaskIdRef.current = null;
                    setAiGenerating(false);
                    message.error(task.error || task.message || '生成失败');
                    return;
                }

                if (task.status === 'cancelled') {
                    stopAiTaskPolling();
                    aiTaskIdRef.current = null;
                    setAiGenerating(false);
                    message.info(task.message || '后台任务已取消');
                }
            } catch (error) {
                console.error('轮询后台任务状态失败:', error);
            }
        };

        void poll();
        aiTaskPollTimerRef.current = window.setInterval(() => {
            void poll();
        }, 1500);
    }, [fetchCareers, stopAiTaskPolling]);

    const handleAIGenerateBackground = async (values: { main_career_count: number; sub_career_count: number }) => {
        if (aiGenerating) {
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
                    enable_mcp: false
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
            fetchCareers();
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
                    fetchCareers();
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
                        {career.source === 'ai' ? 'AI生成' : '手动创建'}
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
            style={{ marginBottom: 16 }}
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

    return (
        <>
            {contextHolder}
            <div style={{
            height: '100%',
            display: 'flex',
            flexDirection: 'column',
            overflow: 'hidden'
        }}>
            {/* 固定头部 */}
            <div style={{
                padding: '16px 16px 0 16px',
                flexShrink: 0
            }}>
                <div style={{
                    marginBottom: 16,
                    display: 'flex',
                    justifyContent: 'space-between',
                    alignItems: 'center',
                    flexWrap: 'wrap',
                    gap: '12px'
                }}>
                    <Title level={3} style={{ margin: 0 }}>
                        <TrophyOutlined style={{ marginRight: 8 }} />
                        职业管理
                    </Title>
                    <Space wrap>
                        <Button
                            type="dashed"
                            icon={<ThunderboltOutlined />}
                            onClick={() => {
                                aiForm.resetFields();
                                setIsAIModalOpen(true);
                            }}
                            loading={aiGenerating}
                        >
                            AI生成新职业
                        </Button>
                        <Button
                            type="primary"
                            icon={<PlusOutlined />}
                            onClick={() => handleOpenModal()}
                        >
                            新增职业
                        </Button>
                    </Space>
                </div>
            </div>

            {/* 可滚动的内容区域 */}
            <div style={{
                flex: 1,
                overflow: 'auto',
                padding: '0 16px 16px 16px'
            }}>
                <Tabs items={tabItems} />
            </div>

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
                title="AI生成新职业（增量式）"
                open={isAIModalOpen}
                onCancel={() => setIsAIModalOpen(false)}
                footer={null}
            >
                <Form form={aiForm} layout="vertical" onFinish={handleAIGenerate}>
                    <Paragraph type="secondary">
                        AI将分析当前世界观和已有职业，智能生成新的补充职业。
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
                title="AI生成新职业中..."
                blocking={false}
                onCancel={handleCancelAIGenerate}
            />
            </div>
        </>
    );
}
