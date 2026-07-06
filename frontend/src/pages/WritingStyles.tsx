import { type ReactNode, useState, useEffect, useCallback, useRef } from 'react';
import {
  Button,
  Modal,
  Form,
  Input,
  message,
  Card,
  Space,
  Tag,
  Popconfirm,
  Empty,
  Typography,
  Row,
  Col,
  Divider,
  theme,
} from 'antd';
import {
  PlusOutlined,
  EditOutlined,
  DeleteOutlined,
  StarOutlined,
  StarFilled,
  BookOutlined,
} from '@ant-design/icons';
import { useStore } from '../store';
import { writingStyleApi } from '../services/modularApi';
import type { WritingStyle, WritingStyleCreate, WritingStyleUpdate } from '../types';
import { designDisplayFont } from '../theme/themeConfig';

const { TextArea } = Input;
const { Text, Paragraph, Title } = Typography;

export default function WritingStyles() {
  const { currentProject } = useStore();
  const [styles, setStyles] = useState<WritingStyle[]>([]);
  const [loading, setLoading] = useState(false);
  const [isCreateModalOpen, setIsCreateModalOpen] = useState(false);
  const [isEditModalOpen, setIsEditModalOpen] = useState(false);
  const [editingStyle, setEditingStyle] = useState<WritingStyle | null>(null);
  const [createForm] = Form.useForm();
  const [editForm] = Form.useForm();
  const activeProjectIdRef = useRef<string | null>(currentProject?.id ?? null);
  const styleRequestIdRef = useRef(0);

  const { token } = theme.useToken();

  const isMobile = window.innerWidth <= 768;
  
  // 卡片网格配置
  const gridConfig = {
    gutter: isMobile ? 8 : 16, // 卡片之间的间距
    xs: 24,
    sm: 24,
    md: 12,
    lg: 8,
    xl: 6,
  };

  useEffect(() => {
    activeProjectIdRef.current = currentProject?.id ?? null;
  }, [currentProject?.id]);

  // 加载风格列表 - 如果有项目则加载项目风格（包含默认标记），否则加载用户风格
  useEffect(() => {
    loadStyles();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [currentProject?.id]);

  const loadStyles = useCallback(async () => {
    const requestId = ++styleRequestIdRef.current;
    const targetProjectId = currentProject?.id ?? null;
    try {
      setLoading(true);
      // 如果有当前项目，使用项目API获取（包含is_default标记）
      // 否则使用用户API获取（所有风格的is_default都是false）
      const response = currentProject?.id
        ? await writingStyleApi.getProjectStyles(currentProject.id)
        : await writingStyleApi.getUserStyles();
      
      // 排序：默认风格优先显示
      const sortedStyles = (response.styles || []).sort((a, b) => {
        // 默认风格排在最前面
        if (a.is_default && !b.is_default) return -1;
        if (!a.is_default && b.is_default) return 1;
        // 其他按原有顺序（order_index）
        return 0;
      });

      if (activeProjectIdRef.current !== targetProjectId || styleRequestIdRef.current !== requestId) {
        return;
      }

      setStyles(sortedStyles);
    } catch {
      message.error('加载风格列表失败');
    } finally {
      if (styleRequestIdRef.current === requestId) {
        setLoading(false);
      }
    }
  }, [currentProject?.id]);

  const handleCreate = async (values: { name: string; description?: string; prompt_content: string }) => {
    try {
      const createData: WritingStyleCreate = {
        name: values.name,
        style_type: 'custom',
        description: values.description,
        prompt_content: values.prompt_content,
      };

      await writingStyleApi.createStyle(createData);
      message.success('创建成功');
      setIsCreateModalOpen(false);
      createForm.resetFields();
      await loadStyles();
    } catch {
      message.error('创建失败');
    }
  };

  const handleEdit = (style: WritingStyle) => {
    setEditingStyle(style);
    editForm.setFieldsValue({
      name: style.name,
      description: style.description,
      prompt_content: style.prompt_content,
    });
    setIsEditModalOpen(true);
  };

  const handleUpdate = async (values: WritingStyleUpdate) => {
    if (!editingStyle) return;

    try {
      await writingStyleApi.updateStyle(editingStyle.id, values);
      message.success('更新成功');
      setIsEditModalOpen(false);
      editForm.resetFields();
      setEditingStyle(null);
      await loadStyles();
    } catch {
      message.error('更新失败');
    }
  };

  const handleDelete = async (styleId: number) => {
    try {
      await writingStyleApi.deleteStyle(styleId);
      message.success('删除成功');
      await loadStyles();
    } catch {
      message.error('删除失败');
    }
  };

  const handleSetDefault = async (styleId: number) => {
    if (!currentProject?.id) {
      message.warning('请先选择项目');
      return;
    }
    
    try {
      await writingStyleApi.setDefaultStyle(styleId, currentProject.id);
      message.success('设置默认风格成功');
      await loadStyles();
    } catch {
      message.error('设置失败');
    }
  };

  const showCreateModal = () => {
    createForm.resetFields();
    setIsCreateModalOpen(true);
  };

  const getStyleTypeColor = (styleType: string) => {
    return styleType === 'preset' ? 'blue' : 'purple';
  };

  const getStyleTypeLabel = (styleType: string) => {
    return styleType === 'preset' ? '预设' : '自定义';
  };

  const heroBackground = `linear-gradient(135deg,
    color-mix(in srgb, ${token.colorPrimary} 78%, #6f3d2f 22%) 0%,
    color-mix(in srgb, ${token.colorInfo} 34%, #1f262e 66%) 100%)`;
  const editorialInk = '#fff9f0';
  const actionButtonStyle = {
    borderRadius: 999,
    height: 42,
    paddingInline: 16,
    borderColor: 'rgba(255,255,255,0.18)',
    background: 'rgba(255,255,255,0.08)',
    color: editorialInk,
    boxShadow: 'none',
  } as const;
  const panelBackground = `linear-gradient(180deg,
    color-mix(in srgb, ${token.colorBgContainer} 94%, white 6%) 0%,
    color-mix(in srgb, ${token.colorFillAlter} 50%, ${token.colorBgContainer} 50%) 100%)`;
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
  const totalStyles = styles.length;
  const customCount = styles.filter((style) => style.style_type === 'custom').length;
  const presetCount = styles.filter((style) => style.style_type === 'preset').length;
  const defaultStyle = styles.find((style) => style.is_default);
  const statItems: Array<{ label: string; value: number | string; accent: string; compact?: boolean }> = [
    { label: '风格总数', value: totalStyles, accent: editorialInk },
    { label: '自定义', value: customCount, accent: token.colorSuccess },
    { label: '预设', value: presetCount, accent: token.colorInfo },
    { label: '默认方案', value: defaultStyle?.name ?? '未设置', accent: editorialInk, compact: true },
  ];
  const styleGuideSteps = [
    '先确认当前是在项目风格库还是个人风格库，再判断默认风格会落到哪条创作链路上。',
    '再区分预设与自定义卡片，优先把稳定基线和项目专项语气分开整理。',
    '最后再创建、编辑或调整默认方案，避免在还没看清现状时直接改动全局入口风格。',
  ];
  const createGuideSteps = [
    '先明确这次要补的是项目专项语气还是个人长期可复用的基线卡，避免命名过泛。',
    '再用风格描述和提示词正文把语气、节奏和禁忌写清楚，把它当作一张可复用的编辑卡片。',
    '最后提交创建，原有保存逻辑和风格刷新流程保持不变，这里只增强阅读顺序与焦点提示。',
  ];
  const editGuideSteps = [
    '先确认正在修订的是哪张风格卡，再判断这次是微调描述还是重写整段提示词。',
    '再按名称、描述、提示词正文的顺序逐项校准，避免只改一处导致卡片语义失衡。',
    '最后保存修改，原有更新逻辑和默认风格排序保持不变，这里只强化编辑视角。',
  ];
  const styleFocus = loading
    ? {
        title: '等待风格卡片同步',
        note: '列表正在刷新，稍后就能继续设置默认风格或编辑当前项目常用语气。',
      }
    : totalStyles === 0
      ? {
          title: '先建立第一张基线卡片',
          note: '当前风格库还是空的，优先创建一张稳定可复用的基础语气卡，再逐步细分其他风格。',
        }
      : !defaultStyle
        ? {
            title: '补齐默认写作方案',
            note: '当前已有风格卡片但还没有默认方案，适合先指定一张常用基线，保证创作入口调用一致。',
          }
        : {
            title: currentProject ? '校准项目专属语气' : '整理个人风格书架',
            note: currentProject
              ? '当前更适合检查默认风格是否真正贴合这个项目的叙事气质，并把专项语气和通用基线区分开。'
              : '当前可以把个人常用风格按用途整理清楚，保留一张稳定默认卡，再逐步扩展实验型语气。',
          };
  const createModalFocus = currentProject
    ? {
        title: `当前正在为项目「${currentProject.title}」补充一张新的风格卡`,
        note: '更适合先明确这张卡要覆盖的叙事气质，再把可复用提示词写完整，避免和默认基线混在一起。',
        tags: [
          { label: '项目语气', color: 'processing' },
          { label: '新增卡片', color: 'gold' },
        ],
      }
    : {
        title: '当前正在建立个人风格库中的新卡片',
        note: '建议先补一张长期稳定的个人基线卡，再逐步拆分实验型和题材型语气。',
        tags: [
          { label: '个人风格库', color: 'blue' },
          { label: '待创建', color: 'gold' },
        ],
      };
  const editModalFocus = editingStyle
    ? {
        title: `当前正在修订风格「${editingStyle.name}」`,
        note: editingStyle.is_default
          ? '这是一张默认风格卡，修改时更适合优先校准整体语气和适用边界，避免影响主要创作入口。'
          : '这次更适合把它当作一次局部修订：先看名称与描述，再回头校准整段提示词。',
        tags: [
          { label: editingStyle.style_type === 'preset' ? '预设风格' : '自定义风格', color: editingStyle.style_type === 'preset' ? 'blue' : 'purple' },
          editingStyle.is_default ? { label: '当前默认', color: 'gold' } : { label: '非默认', color: 'default' },
          editingStyle.user_id === null ? { label: '只读来源', color: 'default' } : { label: '可编辑', color: 'green' },
        ],
      }
    : {
        title: '等待选中的风格卡载入',
        note: '风格数据载入后，这里会继续显示当前修订焦点，原有编辑表单提交逻辑保持不变。',
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
  const renderWorkspacePanel = (label: string, title: string, description: string, children: ReactNode) => (
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

  return (
    <div
      style={{
        display: 'flex',
        flexDirection: 'column',
        height: '100%',
        gap: 16,
        overflow: 'hidden',
        paddingBottom: 24,
      }}
    >
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
        styles={{ body: { padding: isMobile ? 20 : 24 } }}
      >
        <div
          style={{
            position: 'absolute',
            top: -52,
            right: -28,
            width: 168,
            height: 168,
            borderRadius: '50%',
            background: 'rgba(255,255,255,0.09)',
            pointerEvents: 'none',
          }}
        />
        <div
          style={{
            position: 'absolute',
            bottom: -26,
            left: isMobile ? '58%' : '30%',
            width: 118,
            height: 118,
            borderRadius: '50%',
            background: 'rgba(255,255,255,0.05)',
            pointerEvents: 'none',
          }}
        />
        <Row gutter={[24, 18]} align="middle" style={{ position: 'relative', zIndex: 1 }}>
          <Col xs={24} lg={14}>
            <Space direction="vertical" size={8} style={{ width: '100%' }}>
              <Text
                style={{
                  color: 'rgba(255,255,255,0.72)',
                  fontSize: 11,
                  letterSpacing: '0.18em',
                  textTransform: 'uppercase',
                }}
              >
                Style Library
              </Text>
              <Title
                level={2}
                style={{
                  margin: 0,
                  color: editorialInk,
                  fontFamily: designDisplayFont,
                  letterSpacing: '-0.03em',
                }}
              >
                写作风格管理
              </Title>
              <Paragraph
                style={{
                  margin: 0,
                  color: 'rgba(255,255,255,0.82)',
                  fontSize: 15,
                  lineHeight: 1.8,
                }}
              >
                这里像你的写作语气库。把项目默认风格、长期可复用的提示词和一次性实验方案放在同一处，既保留工作区秩序，也保留创作中的编辑手感。
              </Paragraph>
              <Space wrap size={[10, 10]}>
                <Tag
                  style={{
                    borderRadius: 999,
                    paddingInline: 12,
                    border: '1px solid rgba(255,255,255,0.12)',
                    background: 'rgba(255,255,255,0.08)',
                    color: editorialInk,
                  }}
                >
                  {currentProject ? `当前项目：${currentProject.title}` : '当前处于个人风格库'}
                </Tag>
                <Tag
                  style={{
                    borderRadius: 999,
                    paddingInline: 12,
                    border: '1px solid rgba(255,255,255,0.12)',
                    background: 'rgba(255,255,255,0.08)',
                    color: editorialInk,
                  }}
                >
                  默认风格将优先用于创作入口
                </Tag>
              </Space>
            </Space>
          </Col>
          <Col xs={24} lg={10}>
            <Row gutter={[12, 12]}>
              {statItems.map((item) => (
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
                    <Text style={{ color: 'rgba(255,255,255,0.72)', fontSize: 12, display: 'block' }}>
                      {item.label}
                    </Text>
                    <Text
                      style={{
                        color: item.accent,
                        fontWeight: 700,
                        fontSize: item.compact ? 15 : 24,
                        lineHeight: 1.2,
                        wordBreak: 'break-word',
                      }}
                    >
                      {item.value}
                    </Text>
                  </div>
                </Col>
              ))}
            </Row>
          </Col>
        </Row>
        <Space wrap size={[10, 10]} style={{ marginTop: 20, position: 'relative', zIndex: 1 }}>
          <Button icon={<BookOutlined />} style={actionButtonStyle}>
            风格提示词工作台
          </Button>
          <Button
            type="primary"
            icon={<PlusOutlined />}
            onClick={showCreateModal}
            style={{ borderRadius: 999, paddingInline: 16 }}
          >
            创建自定义风格
          </Button>
        </Space>
      </Card>

      <Card
        variant="borderless"
        style={{
          borderRadius: 22,
          background: `linear-gradient(135deg, color-mix(in srgb, ${token.colorPrimary} 10%, white 90%) 0%, color-mix(in srgb, ${token.colorWarning} 10%, white 90%) 100%)`,
          border: `1px solid color-mix(in srgb, ${token.colorPrimary} 16%, white 84%)`,
          boxShadow: `0 18px 36px color-mix(in srgb, ${token.colorText} 8%, transparent)`,
        }}
        styles={{ body: { padding: isMobile ? 16 : 18 } }}
      >
        <Row gutter={[16, 16]}>
          <Col xs={24} lg={15}>
            <Space direction="vertical" size={8} style={{ width: '100%' }}>
              <Text style={{ color: token.colorTextTertiary, fontSize: 12, letterSpacing: '0.12em', textTransform: 'uppercase' }}>
                Style Guide
              </Text>
              <Paragraph style={{ margin: 0, color: token.colorText, lineHeight: 1.75 }}>
                这个页面更像写作语气书架与默认方案控制台。原有卡片排序、默认设置和编辑逻辑都保持不变，这里只把你进入风格库后的阅读顺序和优先级说清楚。
              </Paragraph>
              <div style={{ display: 'flex', flexWrap: 'wrap', gap: 8 }}>
                {styleGuideSteps.map((item, index) => (
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
                padding: isMobile ? '14px 14px 12px' : '16px 18px 14px',
                background: `linear-gradient(180deg, ${token.colorBgContainer} 0%, ${token.colorFillAlter} 100%)`,
                border: `1px solid ${token.colorBorderSecondary}`,
              }}
            >
              <Text style={{ display: 'block', color: token.colorTextTertiary, fontSize: 12, letterSpacing: '0.12em', textTransform: 'uppercase' }}>
                当前维护焦点
              </Text>
              <Title level={5} style={{ margin: '8px 0 6px', color: token.colorTextBase, fontFamily: designDisplayFont }}>
                {styleFocus.title}
              </Title>
              <Paragraph style={{ margin: 0, color: token.colorTextSecondary, lineHeight: 1.75 }}>
                {styleFocus.note}
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
        styles={{ body: { height: '100%', padding: isMobile ? 16 : 20 } }}
      >
        <Space direction="vertical" size={16} style={{ width: '100%', height: '100%' }}>
          <div
            style={{
              display: 'flex',
              justifyContent: 'space-between',
              alignItems: isMobile ? 'flex-start' : 'center',
              gap: 12,
              flexDirection: isMobile ? 'column' : 'row',
            }}
          >
            <Space direction="vertical" size={4}>
              <Text style={{ fontSize: 12, letterSpacing: '0.12em', textTransform: 'uppercase', color: token.colorTextTertiary }}>
                Workspace Shelf
              </Text>
              <Title level={4} style={{ margin: 0, fontFamily: designDisplayFont, color: token.colorTextBase }}>
                风格卡片库
              </Title>
              <Paragraph style={{ margin: 0, color: token.colorTextSecondary }}>
                预设风格偏稳定，适合作为基线；自定义风格适合为当前项目建立更明确的语气指令。
              </Paragraph>
            </Space>
            <Tag color="gold" style={{ borderRadius: 999, paddingInline: 12 }}>
              默认风格会在列表顶部展示
            </Tag>
          </div>

          <Divider style={{ margin: 0, borderColor: token.colorBorderSecondary }} />

          <div style={{ flex: 1, overflowY: 'auto', paddingRight: isMobile ? 0 : 4 }}>
            {styles.length === 0 ? (
              <Card
                variant="borderless"
                style={{
                  borderRadius: 22,
                  background: `linear-gradient(180deg, ${token.colorBgContainer} 0%, ${token.colorFillAlter} 100%)`,
                  border: `1px dashed ${token.colorBorder}`,
                  minHeight: 280,
                  display: 'flex',
                  alignItems: 'center',
                  justifyContent: 'center',
                }}
                styles={{ body: { width: '100%' } }}
              >
                <Empty
                  description="还没有风格卡片，先创建一份你常用的写作语气。"
                  image={Empty.PRESENTED_IMAGE_SIMPLE}
                >
                  <Button type="primary" icon={<PlusOutlined />} onClick={showCreateModal}>
                    创建第一份风格
                  </Button>
                </Empty>
              </Card>
            ) : (
              <Row
                gutter={[0, gridConfig.gutter]}
                style={{ marginLeft: 0, marginRight: 0 }}
              >
                {styles.map((style) => (
                  <Col
                    xs={gridConfig.xs}
                    sm={gridConfig.sm}
                    md={gridConfig.md}
                    lg={gridConfig.lg}
                    xl={gridConfig.xl}
                    key={style.id}
                    style={{
                      paddingLeft: 0,
                      paddingRight: gridConfig.gutter / 2,
                      marginBottom: gridConfig.gutter,
                    }}
                  >
                    <Card
                      hoverable
                      style={{
                        height: '100%',
                        display: 'flex',
                        flexDirection: 'column',
                        borderRadius: 22,
                        overflow: 'hidden',
                        background: style.is_default
                          ? `linear-gradient(180deg, color-mix(in srgb, ${token.colorPrimary} 9%, ${token.colorBgContainer} 91%) 0%, ${token.colorBgContainer} 100%)`
                          : token.colorBgContainer,
                        border: style.is_default
                          ? `1px solid color-mix(in srgb, ${token.colorPrimary} 44%, white 56%)`
                          : `1px solid ${token.colorBorderSecondary}`,
                        boxShadow: style.is_default
                          ? `0 18px 32px color-mix(in srgb, ${token.colorPrimary} 18%, transparent)`
                          : `0 14px 28px color-mix(in srgb, ${token.colorText} 6%, transparent)`,
                      }}
                      styles={{
                        body: {
                          flex: 1,
                          display: 'flex',
                          flexDirection: 'column',
                          padding: 18,
                        },
                        actions: {
                          background: 'transparent',
                          borderTop: `1px solid ${token.colorBorderSecondary}`,
                        },
                      }}
                      actions={[
                        <span
                          key="default"
                          onClick={() => !style.is_default && handleSetDefault(style.id)}
                          style={{ cursor: style.is_default ? 'default' : 'pointer' }}
                        >
                          {style.is_default ? (
                            <StarFilled style={{ color: token.colorWarning, fontSize: 18 }} />
                          ) : (
                            <StarOutlined style={{ fontSize: 18 }} />
                          )}
                        </span>,
                        <EditOutlined
                          key="edit"
                          onClick={() => style.user_id !== null && handleEdit(style)}
                          style={{
                            fontSize: 18,
                            cursor: style.user_id === null ? 'not-allowed' : 'pointer',
                            color: style.user_id === null ? token.colorTextQuaternary : undefined,
                          }}
                        />,
                        <Popconfirm
                          key="delete"
                          title="确定删除这个风格吗？"
                          description={style.is_default ? '这是默认风格，删除后需要设置新的默认风格' : undefined}
                          onConfirm={() => handleDelete(style.id)}
                          okText="确定"
                          cancelText="取消"
                          disabled={style.user_id === null}
                        >
                          <DeleteOutlined
                            style={{
                              fontSize: 18,
                              color: style.user_id === null ? token.colorTextQuaternary : undefined,
                              cursor: style.user_id === null ? 'not-allowed' : 'pointer',
                            }}
                          />
                        </Popconfirm>,
                      ]}
                    >
                      <div style={{ flex: 1, display: 'flex', flexDirection: 'column', gap: 12 }}>
                        <Space align="start" style={{ justifyContent: 'space-between', width: '100%' }}>
                          <Space direction="vertical" size={4} style={{ maxWidth: 'calc(100% - 48px)' }}>
                            <Text strong style={{ fontSize: 17, lineHeight: 1.3 }}>
                              {style.name}
                            </Text>
                            <Space wrap size={[8, 8]}>
                              <Tag color={getStyleTypeColor(style.style_type)}>{getStyleTypeLabel(style.style_type)}</Tag>
                              {style.is_default && <Tag color="gold">默认</Tag>}
                              {style.user_id === null && <Tag bordered={false}>只读预设</Tag>}
                            </Space>
                          </Space>
                          <div
                            style={{
                              width: 38,
                              height: 38,
                              borderRadius: 14,
                              display: 'flex',
                              alignItems: 'center',
                              justifyContent: 'center',
                              background: style.is_default
                                ? `color-mix(in srgb, ${token.colorPrimary} 14%, white 86%)`
                                : token.colorFillAlter,
                              color: style.is_default ? token.colorPrimary : token.colorTextSecondary,
                              flexShrink: 0,
                            }}
                          >
                            <EditOutlined />
                          </div>
                        </Space>

                        {style.description ? (
                          <Paragraph
                            type="secondary"
                            style={{ fontSize: 13, marginBottom: 0, minHeight: 44 }}
                            ellipsis={{
                              rows: 2,
                              expandable: 'collapsible',
                              symbol: (expanded) => (expanded ? '收起' : '展开'),
                              tooltip: style.description,
                            }}
                          >
                            {style.description}
                          </Paragraph>
                        ) : (
                          <Text type="secondary" style={{ fontSize: 13 }}>
                            这份风格卡片还没有补充额外说明，可直接查看下方提示词内容。
                          </Text>
                        )}

                        <div
                          style={{
                            borderRadius: 16,
                            background: `linear-gradient(180deg, ${token.colorFillAlter} 0%, ${token.colorBgContainer} 100%)`,
                            border: `1px solid ${token.colorBorderSecondary}`,
                            padding: 12,
                            flex: 1,
                            display: 'flex',
                            flexDirection: 'column',
                            gap: 8,
                          }}
                        >
                          <Text style={{ fontSize: 12, letterSpacing: '0.08em', textTransform: 'uppercase', color: token.colorTextTertiary }}>
                            Prompt Excerpt
                          </Text>
                          <Paragraph
                            type="secondary"
                            style={{
                              fontSize: 12,
                              marginBottom: 0,
                              flex: 1,
                              minHeight: 78,
                              whiteSpace: 'pre-wrap',
                            }}
                            ellipsis={{
                              rows: 4,
                              expandable: 'collapsible',
                              symbol: (expanded) => (expanded ? '收起' : '展开'),
                              tooltip: style.prompt_content,
                            }}
                          >
                            {style.prompt_content}
                          </Paragraph>
                        </div>
                      </div>
                    </Card>
                  </Col>
                ))}
              </Row>
            )}
          </div>
        </Space>
      </Card>

      {/* 创建自定义风格 Modal */}
      <Modal
        title={null}
        open={isCreateModalOpen}
        onCancel={() => {
          setIsCreateModalOpen(false);
          createForm.resetFields();
        }}
        footer={null}
        centered
        width={isMobile ? 'calc(100vw - 32px)' : 600}
        style={isMobile ? { maxWidth: 'calc(100vw - 32px)', margin: '0 16px' } : undefined}
        styles={modalSurfaceStyles}
      >
        {renderModalHero(
          'Create Style',
          '创建一张新的写作风格卡',
          '这里保留原有创建逻辑，只把进入表单前的阅读顺序说清楚：先界定用途，再补语气说明，最后提交为可复用风格。'
        )}
        {renderGuidePanel(
          'Create Guide',
          '新增时先定义用途，再写提示词正文',
          '这个弹窗更像一张风格建卡工作台，不是一次性备注框。原有字段、保存逻辑和列表刷新流程都保持不变。',
          createGuideSteps,
          createModalFocus.title,
          createModalFocus.note,
          createModalFocus.tags,
        )}
        {renderWorkspacePanel(
          'Create Workspace',
          '风格卡片编辑区',
          '按名称、描述、提示词正文的顺序完成这张卡片，提交后仍然沿用现有创建与刷新逻辑。',
          <Form
            form={createForm}
            layout="vertical"
            onFinish={handleCreate}
            style={{ marginTop: 4 }}
          >
            <Form.Item
              label="风格名称"
              name="name"
              rules={[{ required: true, message: '请输入风格名称' }]}
            >
              <Input placeholder="如：武侠风、科幻风" />
            </Form.Item>

            <Form.Item label="风格描述" name="description">
              <TextArea rows={2} placeholder="简要描述这个风格的特点..." />
            </Form.Item>

            <Form.Item
              label="提示词内容"
              name="prompt_content"
              rules={[{ required: true, message: '请输入提示词内容' }]}
            >
              <TextArea
                rows={6}
                placeholder="输入风格的提示词，用于引导AI生成符合该风格的内容..."
              />
            </Form.Item>

            <Form.Item style={{ marginBottom: 0 }}>
              <Space style={{ width: '100%', justifyContent: 'flex-end' }}>
                <Button
                  onClick={() => {
                    setIsCreateModalOpen(false);
                    createForm.resetFields();
                  }}
                >
                  取消
                </Button>
                <Button type="primary" htmlType="submit" loading={loading}>
                  创建
                </Button>
              </Space>
            </Form.Item>
          </Form>,
        )}
      </Modal>

      {/* 编辑风格 Modal */}
      <Modal
        title={null}
        open={isEditModalOpen}
        onCancel={() => {
          setIsEditModalOpen(false);
          editForm.resetFields();
          setEditingStyle(null);
        }}
        footer={null}
        centered
        width={isMobile ? 'calc(100vw - 32px)' : 600}
        style={isMobile ? { maxWidth: 'calc(100vw - 32px)', margin: '0 16px' } : undefined}
        styles={modalSurfaceStyles}
      >
        {renderModalHero(
          'Edit Style',
          '修订现有写作风格卡',
          '这里仍然沿用原有编辑提交流程，只补一层导览语言，帮助你先看清当前卡片角色，再决定是微调描述还是重写提示词正文。'
        )}
        {renderGuidePanel(
          'Edit Guide',
          '编辑时先校准定位，再调整字段顺序',
          '这个弹窗更像一次风格卡审校台。原有表单字段、保存逻辑与默认风格排序保持不变，这里只强化修订焦点。',
          editGuideSteps,
          editModalFocus.title,
          editModalFocus.note,
          editModalFocus.tags,
        )}
        {renderWorkspacePanel(
          'Edit Workspace',
          '风格卡片修订区',
          '先确认名称和适用边界，再回头校准描述与提示词正文；保存后仍然沿用当前更新逻辑。',
          <Form form={editForm} layout="vertical" onFinish={handleUpdate} style={{ marginTop: 4 }}>
            <Form.Item
              label="风格名称"
              name="name"
              rules={[{ required: true, message: '请输入风格名称' }]}
            >
              <Input placeholder="输入风格名称" />
            </Form.Item>

            <Form.Item label="风格描述" name="description">
              <TextArea rows={2} placeholder="简要描述这个风格的特点..." />
            </Form.Item>

            <Form.Item
              label="提示词内容"
              name="prompt_content"
              rules={[{ required: true, message: '请输入提示词内容' }]}
            >
              <TextArea
                rows={6}
                placeholder="输入风格的提示词..."
              />
            </Form.Item>

            <Form.Item style={{ marginBottom: 0 }}>
              <Space style={{ width: '100%', justifyContent: 'flex-end' }}>
                <Button
                  onClick={() => {
                    setIsEditModalOpen(false);
                    editForm.resetFields();
                    setEditingStyle(null);
                  }}
                >
                  取消
                </Button>
                <Button type="primary" htmlType="submit" loading={loading}>
                  保存
                </Button>
              </Space>
            </Form.Item>
          </Form>,
        )}
      </Modal>
    </div>
  );
}
