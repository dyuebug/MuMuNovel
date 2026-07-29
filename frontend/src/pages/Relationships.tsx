import { Suspense, lazy, useState, useEffect, useMemo, useCallback, useRef } from 'react';
import { useParams, useNavigate } from 'react-router-dom';
import { Card, Tag, Button, Space, message, Modal, Form, Select, Slider, Input, Tabs, AutoComplete, theme, Typography, Row, Col, Divider } from 'antd';
import { PlusOutlined, UserOutlined, EditOutlined } from '@ant-design/icons';
import { useShallow } from 'zustand/react/shallow';
import { useStore } from '../store';
import { useCharacterSync } from '../store/hooks';
import axios from 'axios';
import { useDeferredMount } from '../hooks/useDeferredMount';
import InlineDeferredPanel from '../components/InlineDeferredPanel';
import { designDisplayFont } from '../theme/themeConfig';

const { TextArea } = Input;
const { Title, Paragraph, Text } = Typography;

const LazyDeferredAntdTable = lazy(() => import('../components/DeferredAntdTable'));

interface Relationship {
  id: string;
  character_from_id: string;
  character_to_id: string;
  relationship_name: string;
  intimacy_level: number;
  status: string;
  description?: string;
  source: string;
}

interface RelationshipType {
  id: number;
  name: string;
  category: string;
  reverse_name?: string;
  icon?: string;
}

const categoryLabels: Record<string, string> = {
  family: '家族关系',
  social: '社交关系',
  professional: '职业关系',
  hostile: '敌对关系'
};

export default function Relationships() {
  const { projectId } = useParams<{ projectId: string }>();
  const currentProject = useStore((state) => state.currentProject);
  const projectCharacters = useStore(
    useShallow((state) => state.characters.filter((character) => character.project_id === projectId)),
  );
  const { refreshCharacters } = useCharacterSync();
  const navigate = useNavigate();
  const [relationships, setRelationships] = useState<Relationship[]>([]);
  const [relationshipTypes, setRelationshipTypes] = useState<RelationshipType[]>([]);
  const [loading, setLoading] = useState(false);
  const [isModalOpen, setIsModalOpen] = useState(false);
  const [isEditMode, setIsEditMode] = useState(false);
  const [editingRelationship, setEditingRelationship] = useState<Relationship | null>(null);
  const [form] = Form.useForm();
  const [modal, contextHolder] = Modal.useModal();
  const { token } = theme.useToken();
  const [isMobile, setIsMobile] = useState(window.innerWidth <= 768);
  const [pageSize, setPageSize] = useState(10);
  const [currentPage, setCurrentPage] = useState(1);
  const [activeTabKey, setActiveTabKey] = useState('list');
  const activeProjectIdRef = useRef<string | null>(projectId ?? null);
  const relationshipRequestIdRef = useRef(0);
  const relationshipListReady = useDeferredMount(activeTabKey === 'list');

  useEffect(() => {
    const handleResize = () => {
      setIsMobile(window.innerWidth <= 768);
    };

    window.addEventListener('resize', handleResize);
    return () => window.removeEventListener('resize', handleResize);
  }, []);

  useEffect(() => {
    activeProjectIdRef.current = projectId ?? null;
  }, [projectId]);

  const loadData = useCallback(async () => {
    if (!projectId) {
      return;
    }

    const requestId = ++relationshipRequestIdRef.current;
    const targetProjectId = projectId;
    setLoading(true);
    try {
      const { currentProject: cachedProject, characters: cachedCharacters } = useStore.getState();
      const shouldRefreshCharacters = cachedProject?.id !== projectId || cachedCharacters.length === 0;
      const charactersPromise = shouldRefreshCharacters
        ? refreshCharacters(projectId).catch((error) => {
          console.error('刷新角色缓存失败:', error);
          return cachedCharacters;
        })
        : Promise.resolve(cachedCharacters);

      const [relsRes, typesRes] = await Promise.all([
        axios.get(`/api/relationships/project/${projectId}`),
        axios.get('/api/relationships/types'),
      ]);

      if (activeProjectIdRef.current !== targetProjectId || relationshipRequestIdRef.current !== requestId) {
        return;
      }

      setRelationships(relsRes.data);
      setRelationshipTypes(typesRes.data);
      void charactersPromise;
    } catch (error) {
      message.error('加载数据失败');
      console.error(error);
    } finally {
      if (relationshipRequestIdRef.current === requestId) {
        setLoading(false);
      }
    }
  }, [projectId, refreshCharacters]);

  useEffect(() => {
    void loadData();
  }, [loadData]);

  const handleCreateRelationship = async (values: {
    character_from_id: string;
    character_to_id: string;
    relationship_name: string;
    intimacy_level: number;
    status: string;
    description?: string;
  }) => {
    try {
      await axios.post('/api/relationships/', {
        project_id: projectId,
        ...values
      });
      message.success('关系创建成功');
      setIsModalOpen(false);
      form.resetFields();
      await loadData();
    } catch (error) {
      message.error('创建关系失败');
      console.error(error);
    }
  };

  const handleEditRelationship = useCallback((record: Relationship) => {
    setEditingRelationship(record);
    setIsEditMode(true);
    form.setFieldsValue({
      character_from_id: record.character_from_id,
      character_to_id: record.character_to_id,
      relationship_name: record.relationship_name,
      intimacy_level: record.intimacy_level,
      status: record.status,
      description: record.description,
    });
    setIsModalOpen(true);
  }, [form]);

  const handleUpdateRelationship = async (values: {
    character_from_id: string;
    character_to_id: string;
    relationship_name: string;
    intimacy_level: number;
    status: string;
    description?: string;
  }) => {
    if (!editingRelationship) return;

    try {
      await axios.put(`/api/relationships/${editingRelationship.id}`, {
        relationship_name: values.relationship_name,
        intimacy_level: values.intimacy_level,
        status: values.status,
        description: values.description,
      });
      message.success('关系更新成功');
      setIsModalOpen(false);
      setIsEditMode(false);
      setEditingRelationship(null);
      form.resetFields();
      await loadData();
    } catch (error) {
      message.error('更新关系失败');
      console.error(error);
    }
  };

  const handleDeleteRelationship = useCallback((id: string) => {
    modal.confirm({
      title: '确认删除',
      content: '确定要删除这条关系吗？',
      centered: true,
      okText: '删除',
      okType: 'danger',
      cancelText: '取消',
      onOk: async () => {
        try {
          await axios.delete(`/api/relationships/${id}`);
          message.success('关系删除成功');
          await loadData();
        } catch (error) {
          message.error('删除失败');
          console.error(error);
        }
      }
    });
  }, [loadData, modal]);

  const characterNameMap = useMemo(() => new Map(projectCharacters.map((character) => [character.id, character.name])), [projectCharacters]);

  const selectableCharacterOptions = useMemo(() => projectCharacters
    .filter((character) => !character.is_organization)
    .map((character) => ({ label: character.name, value: character.id })), [projectCharacters]);

  const relationshipTypeOptions = useMemo(() => relationshipTypes.map((type) => ({
    label: `${type.icon || ''} ${type.name} (${categoryLabels[type.category]})`,
    value: type.name,
  })), [relationshipTypes]);

  const getCharacterName = useCallback((id: string) => characterNameMap.get(id) || '未知', [characterNameMap]);

  const getIntimacyColor = (level: number) => {
    if (level >= 75) return 'green';
    if (level >= 50) return 'blue';
    if (level >= 25) return 'orange';
    if (level >= 0) return 'volcano';
    return 'red';
  };

  const getStatusColor = (status: string) => {
    const colors: Record<string, string> = {
      active: 'green',
      broken: 'red',
      past: 'default',
      complicated: 'orange'
    };
    return colors[status] || 'default';
  };

  const getCategoryColor = (category: string) => {
    const colors: Record<string, string> = {
      family: 'magenta',
      social: 'blue',
      hostile: 'red',
      professional: 'cyan'
    };
    return colors[category] || 'default';
  };

  const columns = useMemo(() => [
    {
      title: '角色A',
      dataIndex: 'character_from_id',
      key: 'from',
      render: (id: string) => (
        <Tag icon={<UserOutlined />} color="blue">
          {getCharacterName(id)}
        </Tag>
      ),
      width: 120,
    },
    {
      title: '关系',
      dataIndex: 'relationship_name',
      key: 'relationship',
      render: (name: string) => <strong>{name}</strong>,
      width: 120,
    },
    {
      title: '角色B',
      dataIndex: 'character_to_id',
      key: 'to',
      render: (id: string) => (
        <Tag icon={<UserOutlined />} color="purple">
          {getCharacterName(id)}
        </Tag>
      ),
      width: 120,
    },
    {
      title: '亲密度',
      dataIndex: 'intimacy_level',
      key: 'intimacy',
      render: (level: number) => (
        <Tag color={getIntimacyColor(level)}>{level}</Tag>
      ),
      width: 80,
    },
    {
      title: '状态',
      dataIndex: 'status',
      key: 'status',
      render: (status: string) => (
        <Tag color={getStatusColor(status)}>{status}</Tag>
      ),
      width: 80,
    },
    {
      title: '来源',
      dataIndex: 'source',
      key: 'source',
      render: (source: string) => (
        <Tag>{source === 'ai' ? '智能生成' : '手动创建'}</Tag>
      ),
      width: 100,
    },
    {
      title: '操作',
      key: 'action',
      render: (_: unknown, record: Relationship) => (
        <Space size="small">
          <Button
            type="link"
            size="small"
            icon={<EditOutlined />}
            onClick={() => handleEditRelationship(record)}
          >
            编辑
          </Button>
          <Button
            type="link"
            danger
            size="small"
            onClick={() => handleDeleteRelationship(record.id)}
          >
            删除
          </Button>
        </Space>
      ),
      width: 140,
      fixed: isMobile ? ('right' as const) : undefined,
    },
  ], [getCharacterName, handleDeleteRelationship, handleEditRelationship, isMobile]);

  const groupedTypes = useMemo(() => relationshipTypes.reduce((acc, type) => {
    if (!acc[type.category]) {
      acc[type.category] = [];
    }
    acc[type.category].push(type);
    return acc;
  }, {} as Record<string, RelationshipType[]>), [relationshipTypes]);

  const heroBackground = `linear-gradient(135deg,
    color-mix(in srgb, ${token.colorPrimary} 72%, #6f4537 28%) 0%,
    color-mix(in srgb, ${token.colorInfo} 32%, #18242d 68%) 100%)`;
  const editorialInk = '#fff9f0';
  const panelBackground = `linear-gradient(180deg,
    color-mix(in srgb, ${token.colorBgContainer} 94%, white 6%) 0%,
    color-mix(in srgb, ${token.colorFillAlter} 48%, ${token.colorBgContainer} 52%) 100%)`;
  const panelBorder = `1px solid color-mix(in srgb, ${token.colorBorderSecondary} 88%, white 12%)`;
  const modalSurfaceStyles = {
    header: { padding: '22px 24px 0', borderBottom: 'none' },
    body: { padding: '0 24px 24px' },
    footer: { padding: '0 24px 24px', borderTop: 'none' },
  } as const;
  const activeRelationships = relationships.filter((relationship) => relationship.status === 'active').length;
  const aiRelationships = relationships.filter((relationship) => relationship.source === 'ai').length;
  const averageIntimacy = relationships.length > 0
    ? Math.round(relationships.reduce((sum, relationship) => sum + relationship.intimacy_level, 0) / relationships.length)
    : 0;
  const categoryStats = Object.entries(groupedTypes).map(([category, types]) => ({
    category,
    label: categoryLabels[category] || category,
    count: types.length,
  }));
  const relationshipSummary = [
    { label: '关系总数', value: relationships.length, accent: editorialInk },
    { label: '活跃关系', value: activeRelationships, accent: token.colorSuccess },
    { label: '类型词典', value: relationshipTypes.length, accent: token.colorInfo },
    { label: '平均亲密度', value: averageIntimacy, accent: editorialInk },
  ] as const;

  return (
    <>
      {contextHolder}
      <div style={{ display: 'flex', flexDirection: 'column', minHeight: '100%', gap: 16, overflow: 'visible', paddingBottom: 24 }}>
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
          <div style={{ position: 'absolute', top: -52, right: -26, width: 168, height: 168, borderRadius: '50%', background: 'rgba(255,255,255,0.08)', pointerEvents: 'none' }} />
          <div style={{ position: 'absolute', bottom: -28, left: isMobile ? '56%' : '28%', width: 120, height: 120, borderRadius: '50%', background: 'rgba(255,255,255,0.05)', pointerEvents: 'none' }} />
          <Row gutter={[24, 18]} align="middle" style={{ position: 'relative', zIndex: 1 }}>
            <Col xs={24} lg={14}>
              <Space direction="vertical" size={8} style={{ width: '100%' }}>
                <Text style={{ color: 'rgba(255,255,255,0.72)', fontSize: 11, letterSpacing: '0.18em', textTransform: 'uppercase' }}>
                  Relationship Atlas
                </Text>
                <Title level={2} style={{ margin: 0, color: editorialInk, fontFamily: designDisplayFont, letterSpacing: '-0.03em' }}>
                  关系管理
                </Title>
                <Paragraph style={{ margin: 0, color: 'rgba(255,255,255,0.82)', fontSize: 15, lineHeight: 1.8 }}>
                  这一页更像人物关系的编辑台账。你能一边维护可操作的关系清单，一边保留关系类型词典和图谱入口，让角色网络保持结构清晰、叙事可追踪。
                </Paragraph>
                <Space wrap size={[10, 10]}>
                  <Tag style={{ borderRadius: 999, paddingInline: 12, border: '1px solid rgba(255,255,255,0.12)', background: 'rgba(255,255,255,0.08)', color: editorialInk }}>
                    {currentProject ? `当前项目：${currentProject.title}` : '当前项目未命名'}
                  </Tag>
                  <Tag style={{ borderRadius: 999, paddingInline: 12, border: '1px solid rgba(255,255,255,0.12)', background: 'rgba(255,255,255,0.08)', color: editorialInk }}>
                    AI 生成关系 {aiRelationships} 条
                  </Tag>
                </Space>
              </Space>
            </Col>
            <Col xs={24} lg={10}>
              <Row gutter={[12, 12]}>
                {relationshipSummary.map((item) => (
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
            borderRadius: 20,
            background: token.colorBgContainer,
            border: `1px solid ${token.colorBorderSecondary}`,
            boxShadow: `0 14px 28px color-mix(in srgb, ${token.colorText} 6%, transparent)`,
            flexShrink: 0,
          }}
          styles={{ body: { padding: 14 } }}
        >
          <Space wrap size={[10, 10]}>
            <Button
              onClick={() => projectId && navigate(`/project/${projectId}/relationships-graph`)}
              style={{
                borderRadius: 999,
                paddingInline: 16,
              }}
            >
              关系图谱
            </Button>
            <Button
              type="primary"
              icon={<PlusOutlined />}
              onClick={() => setIsModalOpen(true)}
              style={{ borderRadius: 999, paddingInline: 16 }}
            >
              {isMobile ? '添加关系' : '添加新关系'}
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
                  Network Workspace
                </Text>
                <Title level={4} style={{ margin: 0, fontFamily: designDisplayFont, color: token.colorTextBase }}>
                  关系台账与类型词典
                </Title>
                <Paragraph style={{ margin: 0, color: token.colorTextSecondary }}>
                  列表页适合逐条维护人物连线，类型页适合统一管理命名方式、反向称谓和关系分类。
                </Paragraph>
              </Space>
              <Space wrap size={[8, 8]}>
                {categoryStats.slice(0, 3).map((item) => (
                  <Tag key={item.category} color={getCategoryColor(item.category)} style={{ borderRadius: 999, paddingInline: 10 }}>
                    {item.label} {item.count}
                  </Tag>
                ))}
              </Space>
            </div>

            <Divider style={{ margin: 0, borderColor: token.colorBorderSecondary }} />

            <div style={{ flex: 1, minHeight: 0 }}>
              <Tabs
                activeKey={activeTabKey}
                onChange={setActiveTabKey}
                style={{ height: '100%' }}
                items={[
                  {
                    key: 'list',
                    label: `关系列表 (${relationships.length})`,
                    children: relationshipListReady ? (
                      <Suspense
                        fallback={(
                          <InlineDeferredPanel
                            eyebrow="Relationship Table"
                            title="正在整理关系列表工作区"
                            message="关系列表正在接入排序、分页与筛选列。这里只补充表格级过渡说明，不改变关系数据与编辑操作逻辑。"
                            tags={[
                              { label: '关系列表', color: 'blue' },
                              { label: '排序分页恢复中', color: 'processing' },
                            ]}
                          />
                        )}
                      >
                        <LazyDeferredAntdTable
                          columns={columns}
                          dataSource={relationships}
                          rowKey="id"
                          loading={loading}
                          pagination={{
                            current: currentPage,
                            pageSize: isMobile ? 10 : pageSize,
                            pageSizeOptions: ['10', '20', '50', '100'],
                            position: ['bottomCenter'],
                            showSizeChanger: !isMobile,
                            showQuickJumper: !isMobile,
                            showTotal: (total: number) => `共 ${total} 条`,
                            simple: isMobile,
                            onChange: (page: number, size: number) => {
                              setCurrentPage(page);
                              if (size !== pageSize) {
                                setPageSize(size);
                                setCurrentPage(1);
                              }
                            },
                            onShowSizeChange: (_: number, size: number) => {
                              setPageSize(size);
                              setCurrentPage(1);
                            },
                          }}
                          scroll={{
                            x: 700,
                            y: isMobile ? 'calc(100vh - 470px)' : 'calc(100vh - 560px)',
                          }}
                          size={isMobile ? 'small' : 'middle'}
                        />
                      </Suspense>
                    ) : (
                      <InlineDeferredPanel
                        eyebrow="Relationship Workspace"
                        title="正在接管关系列表工作区"
                        message="系统正在准备关系列表、类型分组与分页区域，原有关系列表查询、筛选与编辑逻辑保持不变。"
                        minHeight={220}
                        tags={[
                          { label: '关系列表接管中', color: 'processing' },
                          { label: `当前 ${relationships.length} 条`, color: 'blue' },
                          { label: '关系逻辑保持原样', color: 'green' },
                        ]}
                      />
                    ),
                  },
                  {
                    key: 'types',
                    label: `关系类型 (${relationshipTypes.length})`,
                    children: (
                      <div
                        style={{
                          display: 'grid',
                          gridTemplateColumns: isMobile ? '1fr' : 'repeat(auto-fill, minmax(220px, 1fr))',
                          gap: isMobile ? '12px' : '16px',
                          maxHeight: isMobile ? 'calc(100vh - 470px)' : 'calc(100vh - 470px)',
                          overflow: 'auto',
                          paddingRight: isMobile ? 0 : 4,
                        }}
                      >
                        {Object.entries(groupedTypes).map(([category, types]) => (
                          <Card
                            key={category}
                            size="small"
                            title={categoryLabels[category] || category}
                            style={{
                              borderRadius: 20,
                              border: `1px solid ${token.colorBorderSecondary}`,
                              boxShadow: `0 14px 24px color-mix(in srgb, ${token.colorText} 5%, transparent)`,
                            }}
                            headStyle={{
                              backgroundColor: token.colorFillAlter,
                              borderTopLeftRadius: 20,
                              borderTopRightRadius: 20,
                            }}
                          >
                            <Space direction="vertical" style={{ width: '100%' }}>
                              {types.map((type) => (
                                <Tag
                                  key={type.id}
                                  color={getCategoryColor(category)}
                                  style={{
                                    whiteSpace: 'normal',
                                    marginInlineEnd: 0,
                                    borderRadius: 999,
                                    padding: '4px 10px',
                                  }}
                                >
                                  {type.icon} {type.name}
                                  {type.reverse_name && ` ↔ ${type.reverse_name}`}
                                </Tag>
                              ))}
                            </Space>
                          </Card>
                        ))}
                      </div>
                    ),
                  },
                ]}
              />
            </div>
          </Space>
        </Card>

        <Modal
        title={(
          <Space direction="vertical" size={2}>
            <Text style={{ fontSize: 11, letterSpacing: '0.18em', textTransform: 'uppercase', color: token.colorTextTertiary }}>
              Relationship Editor
            </Text>
            <Title level={4} style={{ margin: 0, fontFamily: designDisplayFont, letterSpacing: '-0.03em' }}>
              {isEditMode ? '编辑关系' : '添加关系'}
            </Title>
            <Text type="secondary">
              这里维护的是人物连线本身。优先确定双方对象、关系名称和当前状态，再补亲密度与背景描述。
            </Text>
          </Space>
        )}
        open={isModalOpen}
        onCancel={() => {
          setIsModalOpen(false);
          setIsEditMode(false);
          setEditingRelationship(null);
          form.resetFields();
        }}
        footer={null}
        centered={!isMobile}
        width={isMobile ? '100%' : 600}
        style={isMobile ? { top: 0, paddingBottom: 0, maxWidth: '100vw' } : undefined}
        styles={isMobile ? { ...modalSurfaceStyles, body: { ...modalSurfaceStyles.body, maxHeight: 'calc(100vh - 110px)', overflowY: 'auto' } } : modalSurfaceStyles}
      >
        <Card
          size="small"
          variant="borderless"
          style={{ marginBottom: 16, borderRadius: 14, background: 'color-mix(in srgb, var(--ant-color-info-bg) 82%, var(--ant-color-bg-container) 18%)' }}
        >
          <Text type="secondary">
            如果你还不确定关系命名，先在“关系类型”面板确认已有词典，再回来创建或更新关系会更一致。
          </Text>
        </Card>
        <Form
          form={form}
          layout="vertical"
          onFinish={isEditMode ? handleUpdateRelationship : handleCreateRelationship}
        >
          <Form.Item
            name="character_from_id"
            label="角色A"
            rules={[{ required: true, message: '请选择角色A' }]}
          >
            <Select
              placeholder="选择角色"
              showSearch
              disabled={isEditMode}
              filterOption={(input, option) =>
                (option?.label ?? '').toLowerCase().includes(input.toLowerCase())
              }
              options={selectableCharacterOptions}
            />
          </Form.Item>

          <Form.Item
            name="relationship_name"
            label="关系类型"
            rules={[{ required: true, message: '请选择或输入关系类型' }]}
          >
            <AutoComplete
              placeholder="选择预定义类型或输入自定义关系"
              options={relationshipTypeOptions}
              filterOption={(inputValue, option) =>
                option!.value.toUpperCase().indexOf(inputValue.toUpperCase()) !== -1
              }
            />
          </Form.Item>

          <Form.Item
            name="character_to_id"
            label="角色B"
            rules={[{ required: true, message: '请选择角色B' }]}
          >
            <Select
              placeholder="选择角色"
              showSearch
              disabled={isEditMode}
              filterOption={(input, option) =>
                (option?.label ?? '').toLowerCase().includes(input.toLowerCase())
              }
              options={selectableCharacterOptions}
            />
          </Form.Item>

          <Form.Item
            name="intimacy_level"
            label="亲密度"
            initialValue={50}
          >
            <Slider
              min={-100}
              max={100}
              marks={{
                '-100': '-100',
                '-50': '-50',
                0: '0',
                50: '50',
                100: '100'
              }}
            />
          </Form.Item>

          <Form.Item
            name="status"
            label="状态"
            initialValue="active"
          >
            <Select>
              <Select.Option value="active">活跃</Select.Option>
              <Select.Option value="broken">破裂</Select.Option>
              <Select.Option value="past">过去</Select.Option>
              <Select.Option value="complicated">复杂</Select.Option>
            </Select>
          </Form.Item>

          <Form.Item name="description" label="关系描述">
            <TextArea rows={3} placeholder="描述这段关系的细节..." />
          </Form.Item>

          <Form.Item>
            <Space style={{ width: '100%', justifyContent: 'flex-end' }}>
              <Button onClick={() => {
                setIsModalOpen(false);
                setIsEditMode(false);
                setEditingRelationship(null);
                form.resetFields();
              }}>取消</Button>
              <Button type="primary" htmlType="submit">
                {isEditMode ? '更新' : '创建'}
              </Button>
            </Space>
          </Form.Item>
        </Form>
      </Modal>
      </div>
    </>
  );
}
