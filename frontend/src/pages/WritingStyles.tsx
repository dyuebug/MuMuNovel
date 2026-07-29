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
        minHeight: '100%',
        gap: 16,
        overflow: 'visible',
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
          flexShrink: 0,
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
                管理项目默认风格、自定义提示词和可复用语气方案。
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
          flex: '1 0 auto',
          overflow: 'hidden',
          background: panelBackground,
          borderRadius: 24,
          border: panelBorder,
          boxShadow: `0 18px 36px color-mix(in srgb, ${token.colorText} 8%, transparent)`,
        }}
        styles={{ body: { padding: isMobile ? 16 : 20 } }}
      >
        <Space direction="vertical" size={16} style={{ width: '100%' }}>
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
