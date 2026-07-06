import { useState, useEffect, useCallback } from 'react';
import { Card, Button, Modal, Form, Select, InputNumber, Input, message, Progress, Tag, Space, Divider, Typography, theme } from 'antd';
import { EditOutlined, PlusOutlined, DeleteOutlined, TrophyOutlined } from '@ant-design/icons';
import { api } from '../services/core/httpClient';
import { designDisplayFont } from '../theme/themeConfig';

const { TextArea } = Input;
const { Text, Paragraph, Title } = Typography;

interface CareerDetail {
    id: string;
    character_id: string;
    career_id: string;
    career_name: string;
    career_type: 'main' | 'sub';
    current_stage: number;
    stage_name: string;
    stage_description?: string;
    stage_progress: number;
    max_stage: number;
    started_at?: string;
    reached_current_stage_at?: string;
    notes?: string;
}

interface Career {
    id: string;
    name: string;
    type: 'main' | 'sub';
    max_stage: number;
}

interface Props {
    characterId: string;
    projectId: string;
    editable?: boolean;
    onUpdate?: () => void;
}

export const CharacterCareerCard: React.FC<Props> = ({
    characterId,
    projectId,
    editable = false,
    onUpdate
}) => {
    const { token } = theme.useToken();
    const alphaColor = (color: string, alpha: number) =>
        `color-mix(in srgb, ${color} ${(alpha * 100).toFixed(0)}%, transparent)`;
    const [mainCareer, setMainCareer] = useState<CareerDetail | null>(null);
    const [subCareers, setSubCareers] = useState<CareerDetail[]>([]);
    const [allCareers, setAllCareers] = useState<Career[]>([]);
    const [loading, setLoading] = useState(true);

    const [isMainModalOpen, setIsMainModalOpen] = useState(false);
    const [isSubModalOpen, setIsSubModalOpen] = useState(false);
    const [isProgressModalOpen, setIsProgressModalOpen] = useState(false);
    const [selectedCareer, setSelectedCareer] = useState<CareerDetail | null>(null);

    const [mainForm] = Form.useForm();
    const [subForm] = Form.useForm();
    const [progressForm] = Form.useForm();
    const [modal, contextHolder] = Modal.useModal();
    const heroBackground = `linear-gradient(135deg,
        color-mix(in srgb, ${token.colorPrimary} 78%, #6f3d2f 22%) 0%,
        color-mix(in srgb, ${token.colorInfo} 34%, #1f262e 66%) 100%)`;
    const panelBorder = `1px solid color-mix(in srgb, ${token.colorBorderSecondary} 88%, white 12%)`;
    const quietPanelBackground = `linear-gradient(180deg,
        color-mix(in srgb, ${token.colorBgContainer} 96%, white 4%) 0%,
        color-mix(in srgb, ${token.colorFillAlter} 36%, ${token.colorBgContainer} 64%) 100%)`;
    const modalSurfaceStyles = {
        header: {
            padding: '18px 24px 0',
            borderBottom: 'none',
            background: quietPanelBackground,
        },
        body: {
            padding: 20,
            background: quietPanelBackground,
        },
        footer: {
            padding: '0 24px 20px',
            borderTop: 'none',
            background: quietPanelBackground,
        },
        content: {
            borderRadius: 24,
            overflow: 'hidden',
            border: panelBorder,
            boxShadow: `0 24px 52px color-mix(in srgb, ${token.colorText} 12%, transparent)`,
        },
    } as const;

    const fetchCharacterCareers = useCallback(async () => {
        try {
            setLoading(true);
            const response = await api.get(
                `/careers/character/${characterId}/careers`
            ) as { main_career: CareerDetail | null; sub_careers: CareerDetail[] };
            setMainCareer(response.main_career || null);
            setSubCareers(response.sub_careers || []);
        } catch (error: unknown) {
            const axiosError = error as { response?: { data?: { detail?: string } } };
            message.error(axiosError.response?.data?.detail || '获取职业信息失败');
        } finally {
            setLoading(false);
        }
    }, [characterId]);

    const fetchAllCareers = useCallback(async () => {
        try {
            const response = await api.get('/careers', {
                params: { project_id: projectId }
            }) as { main_careers: Career[]; sub_careers: Career[] };
            const main = response.main_careers || [];
            const sub = response.sub_careers || [];
            setAllCareers([...main, ...sub]);
        } catch (error: unknown) {
            console.error('获取职业列表失败:', error);
        }
    }, [projectId]);

    useEffect(() => {
        fetchCharacterCareers();
        if (editable) {
            fetchAllCareers();
        }
    }, [characterId, editable, fetchCharacterCareers, fetchAllCareers]);

    const handleSetMainCareer = async (values: { career_id: string; current_stage?: number; started_at?: string }) => {
        try {
            await api.post(
                `/careers/character/${characterId}/careers/main`,
                values
            );
            message.success('主职业设置成功');
            setIsMainModalOpen(false);
            mainForm.resetFields();
            fetchCharacterCareers();
            onUpdate?.();
        } catch (error: unknown) {
            const axiosError = error as { response?: { data?: { detail?: string } } };
            message.error(axiosError.response?.data?.detail || '设置主职业失败');
        }
    };

    const handleAddSubCareer = async (values: { career_id: string; current_stage?: number; started_at?: string }) => {
        try {
            await api.post(
                `/careers/character/${characterId}/careers/sub`,
                values
            );
            message.success('副职业添加成功');
            setIsSubModalOpen(false);
            subForm.resetFields();
            fetchCharacterCareers();
            onUpdate?.();
        } catch (error: unknown) {
            const axiosError = error as { response?: { data?: { detail?: string } } };
            message.error(axiosError.response?.data?.detail || '添加副职业失败');
        }
    };

    const handleUpdateProgress = async (values: { current_stage: number; stage_progress: number; reached_current_stage_at?: string; notes?: string }) => {
        if (!selectedCareer) return;

        try {
            await api.put(
                `/careers/character/${characterId}/careers/${selectedCareer.career_id}/stage`,
                values
            );
            message.success('职业阶段更新成功');
            setIsProgressModalOpen(false);
            progressForm.resetFields();
            fetchCharacterCareers();
            onUpdate?.();
        } catch (error: unknown) {
            const axiosError = error as { response?: { data?: { detail?: string } } };
            message.error(axiosError.response?.data?.detail || '更新职业阶段失败');
        }
    };

    const handleRemoveSubCareer = (careerId: string) => {
        modal.confirm({
            title: '确认删除',
            content: '确定要移除这个副职业吗？',
            centered: true,
            onOk: async () => {
                try {
                    await api.delete(
                        `/careers/character/${characterId}/careers/${careerId}`
                    );
                    message.success('副职业删除成功');
                    fetchCharacterCareers();
                    onUpdate?.();
                } catch (error: unknown) {
                    const axiosError = error as { response?: { data?: { detail?: string } } };
                    message.error(axiosError.response?.data?.detail || '删除副职业失败');
                }
            }
        });
    };

    const openEditProgress = (career: CareerDetail) => {
        setSelectedCareer(career);
        progressForm.setFieldsValue({
            current_stage: career.current_stage,
            stage_progress: career.stage_progress,
            reached_current_stage_at: career.reached_current_stage_at || '',
            notes: career.notes || ''
        });
        setIsProgressModalOpen(true);
    };

    const mainCareerGuideSteps = [
        '先确认这次是在为角色补第一条主职业，而不是直接更新阶段进度。',
        '再选择职业与初始阶段，把它当作角色成长基线，而不是临时备注项。',
        '最后提交设置，原有创建主职业与列表刷新逻辑保持不变。',
    ];
    const subCareerGuideSteps = [
        '先判断这条副职业是否真的需要独立记录，避免把短期技能误记为长期职业线。',
        '再选择副职业和初始阶段，优先把角色的副线身份整理清楚。',
        '最后提交添加，原有副职业创建与刷新逻辑保持不变。',
    ];
    const progressGuideSteps = [
        '先确认当前编辑的是哪条职业线，再区分这次是升阶段还是只更新进度与备注。',
        '再按阶段、进度、到达时间的顺序补齐信息，把它当作成长记录而不是随手注释。',
        '最后提交更新，原有阶段更新与刷新逻辑保持不变。',
    ];
    const mainCareerFocus = mainCareer
        ? {
            title: `角色已经有主职业「${mainCareer.career_name}」，当前更适合审阅而不是重复创建`,
            note: '如果你仍打开这个弹窗，建议先确认是否真的需要替换主职业基线，避免重复录入。',
            tags: [
                { label: '已有主职业', color: 'gold' },
                { label: `${mainCareer.current_stage}/${mainCareer.max_stage} 阶段`, color: 'blue' },
            ],
        }
        : {
            title: '当前正在为角色建立第一条主职业基线',
            note: '这一步更像人物成长主线建档，适合先确定职业方向，再补初始阶段和时间锚点。',
            tags: [
                { label: '主职业建档', color: 'processing' },
                { label: '角色核心成长线', color: 'purple' },
            ],
        };
    const subCareerFocus = {
        title: subCareers.length > 0
            ? `当前角色已有 ${subCareers.length} 条副职业记录，先判断是否真的需要继续扩展`
            : '当前正在补充角色的第一条副职业记录',
        note: subCareers.length > 0
            ? '更适合先确认这条副职业是否承担长期身份或剧情功能，再决定是否新增。'
            : '这一步更适合作为角色副线身份建档入口，避免把短期经历直接写进职业库。',
        tags: [
            { label: `已记录副职业 ${subCareers.length} 条`, color: 'blue' },
            { label: '上限 5 条', color: 'default' },
        ],
    };
    const progressFocus = selectedCareer
        ? {
            title: `当前正在更新「${selectedCareer.career_name}」的成长阶段`,
            note: '建议先判断这次是阶段跃迁还是进度修订，再补时间和备注，能让职业成长线更连贯。',
            tags: [
                { label: `${selectedCareer.current_stage}/${selectedCareer.max_stage} 阶段`, color: 'processing' },
                { label: `${selectedCareer.stage_progress}% 进度`, color: 'green' },
                { label: selectedCareer.career_type === 'main' ? '主职业' : '副职业', color: selectedCareer.career_type === 'main' ? 'gold' : 'blue' },
            ],
        }
        : {
            title: '等待职业记录载入后再继续更新阶段',
            note: '选中的职业数据准备好之后，这里会继续显示当前成长焦点，原有更新逻辑保持不变。',
            tags: [{ label: '等待数据', color: 'default' }],
        };

    const renderModalHero = (eyebrow: string, title: string, description: string) => (
        <Card
            bordered={false}
            style={{
                marginBottom: 16,
                borderRadius: 20,
                overflow: 'hidden',
                background: heroBackground,
            }}
            styles={{ body: { padding: 20 } }}
        >
            <Text style={{ color: 'color-mix(in srgb, #ffffff 68%, transparent)', letterSpacing: '0.14em', textTransform: 'uppercase' }}>
                {eyebrow}
            </Text>
            <Title level={5} style={{ margin: '8px 0 10px', color: '#f7f1e8', fontFamily: designDisplayFont, letterSpacing: '-0.03em' }}>
                {title}
            </Title>
            <Paragraph style={{ margin: 0, color: 'color-mix(in srgb, #ffffff 82%, transparent)', lineHeight: 1.7 }}>
                {description}
            </Paragraph>
        </Card>
    );

    const renderGuidePanel = (
        guideLabel: string,
        guideTitle: string,
        guideDescription: string,
        guideSteps: string[],
        focusTitle: string,
        focusNote: string,
        focusTags: Array<{ label: string; color: string }>,
    ) => (
        <Card
            bordered={false}
            style={{
                marginBottom: 16,
                borderRadius: 18,
                background: quietPanelBackground,
                border: panelBorder,
            }}
            styles={{ body: { padding: 18 } }}
        >
            <div
                style={{
                    display: 'grid',
                    gridTemplateColumns: 'repeat(auto-fit, minmax(220px, 1fr))',
                    gap: 16,
                }}
            >
                <div>
                    <Text style={{ fontSize: 12, letterSpacing: '0.12em', textTransform: 'uppercase', color: token.colorTextTertiary }}>
                        {guideLabel}
                    </Text>
                    <Title level={5} style={{ margin: '6px 0 8px', fontFamily: designDisplayFont }}>
                        {guideTitle}
                    </Title>
                    <Paragraph style={{ margin: 0, color: token.colorTextSecondary, lineHeight: 1.75 }}>
                        {guideDescription}
                    </Paragraph>
                    <div style={{ display: 'flex', flexWrap: 'wrap', gap: 8, marginTop: 12 }}>
                        {guideSteps.map((item, index) => (
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
                                    color: token.colorTextSecondary,
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
                        borderRadius: 16,
                        padding: '16px 18px',
                        background: token.colorBgContainer,
                        border: `1px solid ${token.colorBorderSecondary}`,
                    }}
                >
                    <Text style={{ display: 'block', fontSize: 12, letterSpacing: '0.12em', textTransform: 'uppercase', color: token.colorTextTertiary }}>
                        当前工作焦点
                    </Text>
                    <Title level={5} style={{ margin: '8px 0 6px', fontFamily: designDisplayFont }}>
                        {focusTitle}
                    </Title>
                    <Paragraph style={{ margin: 0, color: token.colorTextSecondary, lineHeight: 1.75 }}>
                        {focusNote}
                    </Paragraph>
                    <Space wrap size={[8, 8]} style={{ marginTop: 12 }}>
                        {focusTags.map((tag) => (
                            <Tag key={`${tag.color}-${tag.label}`} color={tag.color} style={{ margin: 0, borderRadius: 999, paddingInline: 10 }}>
                                {tag.label}
                            </Tag>
                        ))}
                    </Space>
                </div>
            </div>
        </Card>
    );

    const renderWorkspacePanel = (label: string, title: string, description: string, children: React.ReactNode) => (
        <Card
            bordered={false}
            style={{
                borderRadius: 18,
                background: token.colorBgContainer,
                border: panelBorder,
            }}
            styles={{ body: { padding: 18 } }}
        >
            <div style={{ marginBottom: 14 }}>
                <Text style={{ fontSize: 12, letterSpacing: '0.12em', textTransform: 'uppercase', color: token.colorTextTertiary }}>
                    {label}
                </Text>
                <Title level={5} style={{ margin: '6px 0 8px', fontFamily: designDisplayFont }}>
                    {title}
                </Title>
                <Paragraph style={{ margin: 0, color: token.colorTextSecondary, lineHeight: 1.7 }}>
                    {description}
                </Paragraph>
            </div>
            {children}
        </Card>
    );

    const renderCareerInfo = (career: CareerDetail, isMain: boolean = false) => (
        <div
            key={career.id}
            style={{
                marginBottom: 16,
                padding: '16px 16px 14px',
                borderRadius: 18,
                background: token.colorBgContainer,
                border: `1px solid ${token.colorBorderSecondary}`,
            }}
        >
            <Space style={{ width: '100%', justifyContent: 'space-between', alignItems: 'flex-start' }}>
                <Space align="start">
                    <div
                        style={{
                            width: 36,
                            height: 36,
                            borderRadius: 12,
                            display: 'inline-flex',
                            alignItems: 'center',
                            justifyContent: 'center',
                            background: alphaColor(isMain ? token.colorPrimary : token.colorInfo, 0.14),
                            color: isMain ? token.colorPrimary : token.colorInfo,
                            flexShrink: 0,
                        }}
                    >
                        <TrophyOutlined />
                    </div>
                    <div>
                        <Text style={{ display: 'block', fontSize: 12, letterSpacing: '0.12em', textTransform: 'uppercase', color: token.colorTextTertiary }}>
                            {isMain ? 'Main Career' : 'Sub Career'}
                        </Text>
                        <Title level={5} style={{ margin: '4px 0 0', fontFamily: designDisplayFont }}>
                            {career.career_name}
                        </Title>
                    </div>
                </Space>
                <Space wrap size={[8, 8]}>
                    {isMain ? <Tag color="blue" style={{ margin: 0, borderRadius: 999 }}>主职业</Tag> : null}
                    <Tag color={isMain ? 'gold' : 'cyan'} style={{ margin: 0, borderRadius: 999 }}>
                        第 {career.current_stage}/{career.max_stage} 阶段
                    </Tag>
                    <Tag color="green" style={{ margin: 0, borderRadius: 999 }}>
                        {career.stage_progress}% 进度
                    </Tag>
                </Space>
            </Space>

            <div style={{ marginTop: 12 }}>
                <Text type="secondary">
                    {career.stage_name}（第{career.current_stage}/{career.max_stage}阶段）
                </Text>
                {career.stage_description && (
                    <Paragraph type="secondary" style={{ fontSize: 12, marginTop: 6, marginBottom: 0 }}>
                        {career.stage_description}
                    </Paragraph>
                )}
                <Progress
                    percent={career.stage_progress}
                    size="small"
                    style={{ marginTop: 10 }}
                    format={(percent) => `${percent}%`}
                />
                {career.started_at || career.notes ? (
                    <div
                        style={{
                            marginTop: 10,
                            padding: '10px 12px',
                            borderRadius: 14,
                            background: quietPanelBackground,
                            border: `1px solid ${token.colorBorderSecondary}`,
                        }}
                    >
                        {career.started_at ? (
                            <Text type="secondary" style={{ display: 'block', fontSize: 12 }}>
                                开始时间：{career.started_at}
                            </Text>
                        ) : null}
                        {career.notes ? (
                            <Paragraph type="secondary" style={{ fontSize: 12, margin: career.started_at ? '6px 0 0' : 0 }}>
                                备注：{career.notes}
                            </Paragraph>
                        ) : null}
                    </div>
                ) : null}
            </div>

            {editable && (
                <Space style={{ width: '100%', justifyContent: 'flex-end', marginTop: 12 }}>
                    <Button size="small" icon={<EditOutlined />} onClick={() => openEditProgress(career)}>
                        更新进度
                    </Button>
                    {!isMain && (
                        <Button
                            size="small"
                            danger
                            icon={<DeleteOutlined />}
                            onClick={() => handleRemoveSubCareer(career.career_id)}
                        >
                            移除
                        </Button>
                    )}
                </Space>
            )}
        </div>
    );

    if (loading) {
        return <Card loading />;
    }

    const totalCareerCount = (mainCareer ? 1 : 0) + subCareers.length;
    const careerFocusTitle = mainCareer
        ? `当前角色已整理 ${totalCareerCount} 条职业线，主职业是「${mainCareer.career_name}」`
        : '当前角色还没有建立职业基线';
    const careerFocusNote = mainCareer
        ? '更适合先审阅主职业成长阶段，再决定是否扩展副职业或修订进度；原有职业创建、更新和删除逻辑保持不变。'
        : '建议先补主职业，把它当作角色成长主线，再决定是否扩展副职业记录。';

    return (
        <>
            {contextHolder}
            <Card
                bordered={false}
                style={{
                    borderRadius: 22,
                    background: token.colorBgContainer,
                    border: panelBorder,
                    boxShadow: `0 20px 42px ${alphaColor(token.colorText, 0.08)}`,
                }}
                styles={{ body: { padding: 18 } }}
            >
                <Card
                    bordered={false}
                    style={{
                        marginBottom: 16,
                        borderRadius: 20,
                        overflow: 'hidden',
                        background: heroBackground,
                    }}
                    styles={{ body: { padding: 20 } }}
                >
                    <Text style={{ color: 'color-mix(in srgb, #ffffff 68%, transparent)', letterSpacing: '0.14em', textTransform: 'uppercase' }}>
                        Career Workspace
                    </Text>
                    <Title level={4} style={{ margin: '8px 0 10px', color: '#f7f1e8', fontFamily: designDisplayFont, letterSpacing: '-0.03em' }}>
                        职业成长档案
                    </Title>
                    <Paragraph style={{ margin: 0, color: 'color-mix(in srgb, #ffffff 82%, transparent)', lineHeight: 1.7 }}>
                        这里把角色的主副职业、阶段进度和成长备注收拢成一张职业档案卡。当前只重组阅读顺序和信息层级，不改变职业接口、阶段更新和刷新链路。
                    </Paragraph>
                </Card>

                <Card
                    bordered={false}
                    style={{
                        marginBottom: 16,
                        borderRadius: 18,
                        background: quietPanelBackground,
                        border: panelBorder,
                    }}
                    styles={{ body: { padding: 18 } }}
                >
                    <div
                        style={{
                            display: 'grid',
                            gridTemplateColumns: 'repeat(auto-fit, minmax(220px, 1fr))',
                            gap: 16,
                        }}
                    >
                        <div>
                            <Text style={{ fontSize: 12, letterSpacing: '0.12em', textTransform: 'uppercase', color: token.colorTextTertiary }}>
                                Career Guide
                            </Text>
                            <Title level={5} style={{ margin: '6px 0 8px', fontFamily: designDisplayFont }}>
                                先看主线，再扩展副线
                            </Title>
                            <Paragraph style={{ margin: 0, color: token.colorTextSecondary, lineHeight: 1.75 }}>
                                建议先确认主职业成长基线，再审阅副职业数量和阶段进度；这里不改变任何职业行为，只让阅读顺序更清晰。
                            </Paragraph>
                            <div style={{ display: 'flex', flexWrap: 'wrap', gap: 8, marginTop: 12 }}>
                                <span style={{ display: 'inline-flex', alignItems: 'center', gap: 8, padding: '6px 12px', borderRadius: 999, background: token.colorBgContainer, border: `1px solid ${token.colorBorderSecondary}`, color: token.colorTextSecondary, fontSize: 12 }}>
                                    <span style={{ color: token.colorPrimary, fontWeight: 700 }}>1</span>
                                    先确认主职业是否已建档
                                </span>
                                <span style={{ display: 'inline-flex', alignItems: 'center', gap: 8, padding: '6px 12px', borderRadius: 999, background: token.colorBgContainer, border: `1px solid ${token.colorBorderSecondary}`, color: token.colorTextSecondary, fontSize: 12 }}>
                                    <span style={{ color: token.colorPrimary, fontWeight: 700 }}>2</span>
                                    再审阅阶段和成长进度
                                </span>
                                <span style={{ display: 'inline-flex', alignItems: 'center', gap: 8, padding: '6px 12px', borderRadius: 999, background: token.colorBgContainer, border: `1px solid ${token.colorBorderSecondary}`, color: token.colorTextSecondary, fontSize: 12 }}>
                                    <span style={{ color: token.colorPrimary, fontWeight: 700 }}>3</span>
                                    最后决定是否扩展副职业
                                </span>
                            </div>
                        </div>
                        <div
                            style={{
                                borderRadius: 16,
                                padding: '16px 18px',
                                background: token.colorBgContainer,
                                border: `1px solid ${token.colorBorderSecondary}`,
                            }}
                        >
                            <Text style={{ display: 'block', fontSize: 12, letterSpacing: '0.12em', textTransform: 'uppercase', color: token.colorTextTertiary }}>
                                当前工作焦点
                            </Text>
                            <Title level={5} style={{ margin: '8px 0 6px', fontFamily: designDisplayFont }}>
                                {careerFocusTitle}
                            </Title>
                            <Paragraph style={{ margin: 0, color: token.colorTextSecondary, lineHeight: 1.75 }}>
                                {careerFocusNote}
                            </Paragraph>
                            <Space wrap size={[8, 8]} style={{ marginTop: 12 }}>
                                <Tag color={mainCareer ? 'gold' : 'processing'} style={{ margin: 0, borderRadius: 999, paddingInline: 10 }}>
                                    {mainCareer ? '主职业已建立' : '等待主职业建档'}
                                </Tag>
                                <Tag color="blue" style={{ margin: 0, borderRadius: 999, paddingInline: 10 }}>
                                    副职业 {subCareers.length} 条
                                </Tag>
                                <Tag color="green" style={{ margin: 0, borderRadius: 999, paddingInline: 10 }}>
                                    总职业线 {totalCareerCount} 条
                                </Tag>
                            </Space>
                        </div>
                    </div>
                </Card>

                <Card
                    bordered={false}
                    style={{
                        borderRadius: 18,
                        background: token.colorBgContainer,
                        border: panelBorder,
                    }}
                    styles={{ body: { padding: 18 } }}
                >
                    <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'flex-start', gap: 12, marginBottom: 16 }}>
                        <div>
                            <Text style={{ fontSize: 12, letterSpacing: '0.12em', textTransform: 'uppercase', color: token.colorTextTertiary }}>
                                Career Archive
                            </Text>
                            <Title level={5} style={{ margin: '6px 0 8px', fontFamily: designDisplayFont }}>
                                主副职业记录区
                            </Title>
                            <Paragraph style={{ margin: 0, color: token.colorTextSecondary, lineHeight: 1.7 }}>
                                先看主职业，再顺序浏览副职业与成长进度；原有编辑、删除和新增动作保持不变。
                            </Paragraph>
                        </div>
                        {editable && !mainCareer && (
                            <Button
                                size="small"
                                icon={<PlusOutlined />}
                                onClick={() => {
                                    mainForm.resetFields();
                                    setIsMainModalOpen(true);
                                }}
                            >
                                设置主职业
                            </Button>
                        )}
                    </div>

                {mainCareer ? (
                    <>
                        {renderCareerInfo(mainCareer, true)}

                        {subCareers.length > 0 && (
                            <>
                                <Divider />
                                <Text type="secondary">副职业</Text>
                                <div style={{ marginTop: 8 }}>
                                    {subCareers.map(career => renderCareerInfo(career, false))}
                                </div>
                            </>
                        )}

                        {editable && subCareers.length < 5 && (
                            <div style={{ textAlign: 'center', marginTop: 16 }}>
                                <Button
                                    size="small"
                                    icon={<PlusOutlined />}
                                    onClick={() => {
                                        subForm.resetFields();
                                        setIsSubModalOpen(true);
                                    }}
                                >
                                    添加副职业
                                </Button>
                            </div>
                        )}
                    </>
                ) : (
                    <Text type="secondary" style={{ display: 'block', textAlign: 'center', padding: '20px 0' }}>
                        暂无职业信息
                    </Text>
                )}
                </Card>
            </Card>

            {/* 设置主职业 */}
            <Modal
                title={null}
                open={isMainModalOpen}
                onCancel={() => setIsMainModalOpen(false)}
                footer={null}
                styles={modalSurfaceStyles}
            >
                {renderModalHero(
                    'Main Career',
                    '为角色建立主职业基线',
                    '这里保留原有主职业创建逻辑，只补一层导览语言，帮助你先确认角色成长主线，再录入职业与起始阶段。'
                )}
                {renderGuidePanel(
                    'Main Guide',
                    '先定成长主线，再补初始阶段',
                    '这个弹窗更像角色成长基线建档台，不是随手补一条备注。原有字段、提交流程与刷新逻辑保持不变。',
                    mainCareerGuideSteps,
                    mainCareerFocus.title,
                    mainCareerFocus.note,
                    mainCareerFocus.tags,
                )}
                {renderWorkspacePanel(
                    'Main Workspace',
                    '主职业设置区',
                    '按职业、阶段、开始时间的顺序完成建档，提交后仍然沿用现有创建与刷新逻辑。',
                    <Form form={mainForm} layout="vertical" onFinish={handleSetMainCareer}>
                        <Form.Item label="选择主职业" name="career_id" rules={[{ required: true }]}>
                            <Select placeholder="选择职业">
                                {allCareers.filter(c => c.type === 'main').map(career => (
                                    <Select.Option key={career.id} value={career.id}>
                                        {career.name}（{career.max_stage}个阶段）
                                    </Select.Option>
                                ))}
                            </Select>
                        </Form.Item>
                        <Form.Item label="当前阶段" name="current_stage" initialValue={1}>
                            <InputNumber min={1} style={{ width: '100%' }} />
                        </Form.Item>
                        <Form.Item label="开始时间" name="started_at">
                            <Input placeholder="如：修仙历3000年" />
                        </Form.Item>
                        <Form.Item style={{ marginBottom: 0 }}>
                            <Space style={{ width: '100%', justifyContent: 'flex-end' }}>
                                <Button onClick={() => setIsMainModalOpen(false)}>取消</Button>
                                <Button type="primary" htmlType="submit">确定</Button>
                            </Space>
                        </Form.Item>
                    </Form>
                )}
            </Modal>

            {/* 添加副职业 */}
            <Modal
                title={null}
                open={isSubModalOpen}
                onCancel={() => setIsSubModalOpen(false)}
                footer={null}
                styles={modalSurfaceStyles}
            >
                {renderModalHero(
                    'Sub Career',
                    '补充角色的副职业身份',
                    '这里仍然沿用原有副职业创建逻辑，只增强阅读顺序和焦点卡，帮助你把角色的副线身份整理得更清楚。'
                )}
                {renderGuidePanel(
                    'Sub Guide',
                    '先判断副线身份，再补初始阶段',
                    '这个弹窗更像角色副线身份建档区。原有字段、提交流程与上限规则保持不变，这里只补导览层。',
                    subCareerGuideSteps,
                    subCareerFocus.title,
                    subCareerFocus.note,
                    subCareerFocus.tags,
                )}
                {renderWorkspacePanel(
                    'Sub Workspace',
                    '副职业设置区',
                    '优先确认这条副职业的长期意义，再按职业、阶段和时间顺序录入；提交后仍沿用现有创建与刷新逻辑。',
                    <Form form={subForm} layout="vertical" onFinish={handleAddSubCareer}>
                        <Form.Item label="选择副职业" name="career_id" rules={[{ required: true }]}>
                            <Select placeholder="选择职业">
                                {allCareers.filter(c => c.type === 'sub').map(career => (
                                    <Select.Option key={career.id} value={career.id}>
                                        {career.name}（{career.max_stage}个阶段）
                                    </Select.Option>
                                ))}
                            </Select>
                        </Form.Item>
                        <Form.Item label="当前阶段" name="current_stage" initialValue={1}>
                            <InputNumber min={1} style={{ width: '100%' }} />
                        </Form.Item>
                        <Form.Item label="开始时间" name="started_at">
                            <Input placeholder="如：修仙历3000年" />
                        </Form.Item>
                        <Form.Item style={{ marginBottom: 0 }}>
                            <Space style={{ width: '100%', justifyContent: 'flex-end' }}>
                                <Button onClick={() => setIsSubModalOpen(false)}>取消</Button>
                                <Button type="primary" htmlType="submit">添加</Button>
                            </Space>
                        </Form.Item>
                    </Form>
                )}
            </Modal>

            {/* 更新职业进度 */}
            <Modal
                title={null}
                open={isProgressModalOpen}
                onCancel={() => setIsProgressModalOpen(false)}
                footer={null}
                styles={modalSurfaceStyles}
            >
                {selectedCareer && (
                    <>
                        {renderModalHero(
                            'Career Progress',
                            '更新职业成长阶段',
                            '这里保留原有阶段更新逻辑，只把编辑顺序和当前焦点说清楚，帮助你把职业成长线记录得更完整。'
                        )}
                        {renderGuidePanel(
                            'Progress Guide',
                            '先判断阶段变化，再补时间与备注',
                            '这个弹窗更像职业成长记录台，而不是简单的数字改动入口。原有阶段更新、刷新与角色回调逻辑保持不变。',
                            progressGuideSteps,
                            progressFocus.title,
                            progressFocus.note,
                            progressFocus.tags,
                        )}
                        {renderWorkspacePanel(
                            'Progress Workspace',
                            '成长阶段修订区',
                            '先确认职业对象，再按阶段、进度、时间和备注的顺序更新；提交后仍然沿用现有更新逻辑。',
                            <Form form={progressForm} layout="vertical" onFinish={handleUpdateProgress}>
                                <Text>职业：{selectedCareer.career_name}</Text>
                                <Divider style={{ margin: '12px 0' }} />
                                <Form.Item label="当前阶段" name="current_stage" rules={[{ required: true }]}>
                                    <InputNumber min={1} max={selectedCareer.max_stage} style={{ width: '100%' }} />
                                </Form.Item>
                                <Form.Item label="阶段进度（0-100）" name="stage_progress" rules={[{ required: true }]}>
                                    <InputNumber min={0} max={100} style={{ width: '100%' }} />
                                </Form.Item>
                                <Form.Item label="到达时间" name="reached_current_stage_at">
                                    <Input placeholder="如：修仙历3001年" />
                                </Form.Item>
                                <Form.Item label="备注" name="notes">
                                    <TextArea rows={2} placeholder="如：突破至金丹期" />
                                </Form.Item>
                                <Form.Item style={{ marginBottom: 0 }}>
                                    <Space style={{ width: '100%', justifyContent: 'flex-end' }}>
                                        <Button onClick={() => setIsProgressModalOpen(false)}>取消</Button>
                                        <Button type="primary" htmlType="submit">更新</Button>
                                    </Space>
                                </Form.Item>
                            </Form>
                        )}
                    </>
                )}
            </Modal>
        </>
    );
};

export default CharacterCareerCard;
