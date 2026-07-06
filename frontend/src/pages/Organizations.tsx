import { Suspense, lazy, useState, useEffect, useCallback, useMemo, useRef } from 'react';
import { useParams } from 'react-router-dom';
import { Card, Tag, Button, Space, message, Modal, Form, Select, InputNumber, Input, Descriptions, Drawer, theme, Typography, Row, Col, Empty } from 'antd';
import { PlusOutlined, UserOutlined, EditOutlined, DeleteOutlined, UnorderedListOutlined, BankOutlined } from '@ant-design/icons';
import { useShallow } from 'zustand/react/shallow';
import { useStore } from '../store';
import { useCharacterSync } from '../store/hooks';
import axios from 'axios';
import { useDeferredMount } from '../hooks/useDeferredMount';
import InlineDeferredPanel from '../components/InlineDeferredPanel';
import { designDisplayFont } from '../theme/themeConfig';

const LazyDeferredAntdTable = lazy(() => import('../components/DeferredAntdTable'));

interface Organization {
  id: string;
  character_id: string;
  name: string;
  type: string;
  purpose: string;
  member_count: number;
  power_level: number;
  location?: string;
  motto?: string;
  color?: string;
}

interface OrganizationMember {
  id: string;
  character_id: string;
  character_name: string;
  position: string;
  rank: number;
  loyalty: number;
  contribution: number;
  status: string;
  joined_at?: string;
  left_at?: string;
  notes?: string;
}

const { Title, Paragraph, Text } = Typography;

export default function Organizations() {
  const { projectId } = useParams<{ projectId: string }>();
  const projectCharacters = useStore(
    useShallow((state) => state.characters.filter((character) => character.project_id === projectId)),
  );
  const { refreshCharacters } = useCharacterSync();
  const [organizations, setOrganizations] = useState<Organization[]>([]);
  const [selectedOrg, setSelectedOrg] = useState<Organization | null>(null);
  const [members, setMembers] = useState<OrganizationMember[]>([]);
  const [loading, setLoading] = useState(false);
  const [isAddMemberModalOpen, setIsAddMemberModalOpen] = useState(false);
  const [isEditMemberModalOpen, setIsEditMemberModalOpen] = useState(false);
  const [isEditOrgModalOpen, setIsEditOrgModalOpen] = useState(false);
  const [editingMember, setEditingMember] = useState<OrganizationMember | null>(null);
  const [form] = Form.useForm();
  const [editMemberForm] = Form.useForm();
  const [editOrgForm] = Form.useForm();
  const [isMobile, setIsMobile] = useState(window.innerWidth <= 768);
  const [modal, contextHolder] = Modal.useModal();
  const [orgListVisible, setOrgListVisible] = useState(false);
  const selectedOrgIdRef = useRef<string | null>(null);
  const activeProjectIdRef = useRef<string | null>(projectId ?? null);
  const organizationsRequestIdRef = useRef(0);
  const membersRequestIdRef = useRef(0);
  const membersTableReady = useDeferredMount(!!selectedOrg);
  const { token } = theme.useToken();

  useEffect(() => {
    const handleResize = () => {
      setIsMobile(window.innerWidth <= 768);
    };

    window.addEventListener('resize', handleResize);
    return () => window.removeEventListener('resize', handleResize);
  }, []);

  useEffect(() => {
    selectedOrgIdRef.current = selectedOrg?.id ?? null;
  }, [selectedOrg]);

  useEffect(() => {
    activeProjectIdRef.current = projectId ?? null;
  }, [projectId]);

  const loadOrganizations = useCallback(async () => {
    if (!projectId) return;

    const requestId = ++organizationsRequestIdRef.current;
    const targetProjectId = projectId;
    setLoading(true);
    try {
      const res = await axios.get(`/api/organizations/project/${projectId}`);
      if (activeProjectIdRef.current !== targetProjectId || organizationsRequestIdRef.current !== requestId) {
        return;
      }
      const nextOrganizations = res.data as Organization[];
      setOrganizations(nextOrganizations);

      const currentSelectedOrgId = selectedOrgIdRef.current;
      const nextSelectedOrg = currentSelectedOrgId
        ? nextOrganizations.find((org) => org.id === currentSelectedOrgId) ?? nextOrganizations[0] ?? null
        : nextOrganizations[0] ?? null;

      setSelectedOrg(nextSelectedOrg);

      if (!nextSelectedOrg) {
        setMembers([]);
        return;
      }

      if (nextSelectedOrg.id !== currentSelectedOrgId) {
        void loadMembers(nextSelectedOrg.id);
      }
    } catch (error) {
      message.error('加载组织列表失败');
      console.error(error);
    } finally {
      if (organizationsRequestIdRef.current === requestId) {
        setLoading(false);
      }
    }
  }, [projectId]);

  useEffect(() => {
    if (!projectId) {
      return;
    }

    void loadOrganizations();

    const { currentProject: cachedProject, characters: cachedCharacters } = useStore.getState();
    if (cachedProject?.id !== projectId || cachedCharacters.length === 0) {
      void refreshCharacters(projectId);
    }
  }, [projectId, loadOrganizations, refreshCharacters]);

  const loadMembers = async (orgId: string) => {
    const requestId = ++membersRequestIdRef.current;
    const targetProjectId = activeProjectIdRef.current;
    try {
      const res = await axios.get(`/api/organizations/${orgId}/members`);
      if (activeProjectIdRef.current !== targetProjectId || membersRequestIdRef.current !== requestId) {
        return;
      }
      setMembers(res.data);
    } catch (error) {
      message.error('加载成员列表失败');
      console.error(error);
    }
  };

  const handleSelectOrganization = (org: Organization) => {
    setSelectedOrg(org);
    loadMembers(org.id);
  };

  const handleAddMember = async (values: Record<string, unknown>) => {
    if (!selectedOrg) return;

    try {
      await axios.post(`/api/organizations/${selectedOrg.id}/members`, values);
      message.success('成员添加成功');
      setIsAddMemberModalOpen(false);
      form.resetFields();
      loadMembers(selectedOrg.id);
      loadOrganizations(); // 刷新成员计数
    } catch (error) {
      message.error('添加成员失败');
      console.error(error);
    }
  };

  const handleRemoveMember = async (memberId: string) => {
    modal.confirm({
      title: '确认移除',
      content: '确定要移除该成员吗？',
      centered: true,
      okText: '移除',
      okType: 'danger',
      cancelText: '取消',
      onOk: async () => {
        try {
          await axios.delete(`/api/organizations/members/${memberId}`);
          message.success('成员移除成功');
          if (selectedOrg) {
            loadMembers(selectedOrg.id);
            loadOrganizations(); // 刷新成员计数
          }
        } catch (error) {
          message.error('移除失败');
          console.error(error);
        }
      }
    });
  };

  const handleEditMember = (member: OrganizationMember) => {
    setEditingMember(member);
    editMemberForm.setFieldsValue({
      position: member.position,
      rank: member.rank,
      loyalty: member.loyalty,
      contribution: member.contribution,
      status: member.status,
      notes: member.notes,
      joined_at: member.joined_at
    });
    setIsEditMemberModalOpen(true);
  };

  const handleUpdateMember = async (values: Record<string, unknown>) => {
    if (!editingMember) return;

    try {
      await axios.put(`/api/organizations/members/${editingMember.id}`, values);
      message.success('成员信息更新成功');
      setIsEditMemberModalOpen(false);
      editMemberForm.resetFields();
      setEditingMember(null);
      if (selectedOrg) {
        loadMembers(selectedOrg.id);
      }
    } catch (error) {
      message.error('更新失败');
      console.error(error);
    }
  };

  const getStatusColor = (status: string) => {
    const colors: Record<string, string> = {
      active: 'green',
      retired: 'default',
      expelled: 'red',
      deceased: 'black'
    };
    return colors[status] || 'default';
  };

  const getStatusText = (status: string) => {
    const texts: Record<string, string> = {
      active: '在职',
      retired: '退休',
      expelled: '除名',
      deceased: '已故'
    };
    return texts[status] || status;
  };

  const memberColumns = [
    {
      title: '姓名',
      dataIndex: 'character_name',
      key: 'name',
      render: (name: string) => (
        <Space>
          <UserOutlined />
          <span>{name}</span>
        </Space>
      ),
      width: isMobile ? 100 : undefined,
    },
    {
      title: '职位',
      dataIndex: 'position',
      key: 'position',
      render: (position: string, record: OrganizationMember) => (
        <Tag color="blue">{position} {!isMobile && `(级别 ${record.rank})`}</Tag>
      ),
      width: isMobile ? 120 : undefined,
    },
    {
      title: '忠诚度',
      dataIndex: 'loyalty',
      key: 'loyalty',
      render: (loyalty: number) => (
        <span style={{ color: loyalty >= 70 ? 'green' : loyalty >= 40 ? 'orange' : 'red' }}>
          {loyalty}%
        </span>
      ),
      width: isMobile ? 80 : undefined,
    },
    {
      title: '贡献度',
      dataIndex: 'contribution',
      key: 'contribution',
      render: (contribution: number) => `${contribution}%`,
      width: isMobile ? 80 : undefined,
    },
    {
      title: '状态',
      dataIndex: 'status',
      key: 'status',
      render: (status: string) => (
        <Tag color={getStatusColor(status)}>{getStatusText(status)}</Tag>
      ),
      width: isMobile ? 80 : undefined,
    },
    {
      title: '加入时间',
      dataIndex: 'joined_at',
      key: 'joined_at',
      render: (time: string) => time || '-',
      width: isMobile ? 120 : undefined,
    },
    {
      title: '操作',
      key: 'action',
      render: (_: unknown, record: OrganizationMember) => (
        <Space size={isMobile ? 0 : 'small'}>
          <Button
            type="link"
            size="small"
            icon={<EditOutlined />}
            onClick={() => handleEditMember(record)}
            style={isMobile ? { padding: '4px' } : undefined}
          >
            {isMobile ? '' : '编辑'}
          </Button>
          <Button
            type="link"
            danger
            size="small"
            icon={<DeleteOutlined />}
            onClick={() => handleRemoveMember(record.id)}
            style={isMobile ? { padding: '4px' } : undefined}
          >
            {isMobile ? '' : '移除'}
          </Button>
        </Space>
      ),
      width: isMobile ? 50 : undefined,
      fixed: isMobile ? 'right' as const : undefined,
    },
  ];

  // 过滤掉已是成员的角色
  const availableCharacters = useMemo(() => {
    const memberCharacterIds = new Set(members.map((member) => member.character_id));
    return projectCharacters.filter((character) => !character.is_organization && !memberCharacterIds.has(character.id));
  }, [members, projectCharacters]);

  const availableCharacterOptions = useMemo(() => availableCharacters.map((character) => ({
    label: character.name,
    value: character.id,
  })), [availableCharacters]);

  const editorialInk = token.colorText;
  const heroBackground = `linear-gradient(135deg, #171411 0%, color-mix(in srgb, #171411 68%, ${token.colorPrimary} 32%) 100%)`;
  const quietPanelBackground = `linear-gradient(180deg, color-mix(in srgb, ${token.colorBgContainer} 98%, ${token.colorBgLayout} 2%) 0%, color-mix(in srgb, ${token.colorBgContainer} 92%, ${token.colorBgLayout} 8%) 100%)`;
  const panelBackground = `linear-gradient(180deg, color-mix(in srgb, ${token.colorBgContainer} 96%, ${token.colorPrimary} 4%) 0%, color-mix(in srgb, ${token.colorBgContainer} 92%, ${token.colorWarning} 8%) 100%)`;
  const panelBorder = `1px solid color-mix(in srgb, ${token.colorPrimary} 12%, ${token.colorBorder} 88%)`;
  const modalSurfaceStyles = {
    header: { padding: '22px 24px 0', borderBottom: 'none' },
    body: { padding: '0 24px 24px' },
    footer: { padding: '0 24px 24px', borderTop: 'none' },
  } as const;
  const actionButtonStyle = {
    borderRadius: 999,
    background: 'color-mix(in srgb, var(--ant-color-bg-container) 14%, transparent)',
    border: '1px solid color-mix(in srgb, var(--ant-color-bg-container) 20%, transparent)',
    color: editorialInk,
    boxShadow: `0 10px 18px color-mix(in srgb, ${token.colorText} 18%, transparent)`,
    backdropFilter: 'blur(8px)',
  } as const;
  const totalMemberCount = organizations.reduce((sum, org) => sum + org.member_count, 0);
  const organizationGuideItems = [
    {
      label: 'Step 1',
      title: '先选组织母体',
      description: '先确认你正在维护的是哪一个势力，再展开成员和组织设定的细节。',
    },
    {
      label: 'Step 2',
      title: '再看成员结构',
      description: '把职位、忠诚度、贡献度当作组织关系的操作台，而不是单纯的名单。',
    },
    {
      label: 'Step 3',
      title: '最后校准设定',
      description: '把总部、口号、代表色等信息统一成可复用的世界观档案。',
    },
  ];
  const organizationFocusItems = [
    {
      label: '当前组织',
      value: selectedOrg?.name || '未选择',
      detail: selectedOrg ? `类型：${selectedOrg.type}` : '从左侧目录或移动端抽屉中选择一个组织',
    },
    {
      label: '待分配角色',
      value: `${availableCharacters.length} 名`,
      detail: availableCharacters.length > 0 ? '可继续补充进当前组织' : '当前没有可新增的角色成员',
    },
    {
      label: '成员视图',
      value: `${members.length} 人`,
      detail: selectedOrg ? '用于维护职位、忠诚度与贡献度' : '选中组织后这里会同步更新',
    },
  ];

  return (
    <div style={{ display: 'flex', flexDirection: 'column', gap: 16, paddingBottom: 24 }}>
      {contextHolder}
      <Card
        variant="borderless"
        style={{
          background: heroBackground,
          borderRadius: isMobile ? 22 : 28,
          border: `1px solid color-mix(in srgb, ${token.colorBgContainer} 12%, transparent)`,
          boxShadow: `0 26px 52px color-mix(in srgb, ${token.colorText} 20%, transparent)`,
          overflow: 'hidden',
          position: 'relative',
        }}
        styles={{ body: { padding: isMobile ? 18 : 24 } }}
      >
        <div style={{ position: 'absolute', top: -56, right: -40, width: 180, height: 180, borderRadius: '50%', background: 'rgba(255,255,255,0.08)', pointerEvents: 'none' }} />
        <div style={{ position: 'absolute', bottom: -30, left: '24%', width: 110, height: 110, borderRadius: '50%', background: 'rgba(255,255,255,0.05)', pointerEvents: 'none' }} />
        <Row gutter={[24, 18]} align="middle" style={{ position: 'relative', zIndex: 1 }}>
          <Col xs={24} lg={15}>
            <Space direction="vertical" size={8} style={{ width: '100%' }}>
              <Text style={{ color: 'rgba(255,255,255,0.72)', fontSize: 11, letterSpacing: '0.18em', textTransform: 'uppercase' }}>
                Organization Ledger
              </Text>
              <Title level={isMobile ? 3 : 2} style={{ margin: 0, color: editorialInk, fontFamily: designDisplayFont, letterSpacing: '-0.03em' }}>
                <BankOutlined style={{ marginRight: 8, color: 'rgba(255,255,255,0.9)' }} />
                组织管理
              </Title>
              <Paragraph style={{ margin: 0, color: 'rgba(255,255,255,0.82)', fontSize: isMobile ? 13 : 15, lineHeight: 1.8 }}>
                把项目里的门派、帮会、机构与势力组织整理成一份可持续维护的工作台。左侧像目录，右侧像详情档案，成员关系与组织设定都集中在同一页完成。
              </Paragraph>
            </Space>
          </Col>
          <Col xs={24} lg={9}>
            <Space direction="vertical" size={12} style={{ width: '100%' }}>
              {[
                { label: '组织总数', value: `${organizations.length}` },
                { label: '成员总计', value: `${totalMemberCount}` },
                { label: '当前选中', value: selectedOrg?.name || '未选择' },
              ].map((item) => (
                <div
                  key={item.label}
                  style={{
                    display: 'flex',
                    justifyContent: 'space-between',
                    alignItems: 'center',
                    gap: 12,
                    borderRadius: 18,
                    padding: '12px 14px',
                    background: 'rgba(255,255,255,0.08)',
                    border: '1px solid rgba(255,255,255,0.1)',
                    backdropFilter: 'blur(10px)',
                  }}
                >
                  <Text style={{ color: 'rgba(255,255,255,0.72)', fontSize: 12 }}>{item.label}</Text>
                  <Text style={{ color: editorialInk, fontWeight: 600 }}>{item.value}</Text>
                </div>
              ))}
            </Space>
          </Col>
        </Row>
        <Space wrap size={[10, 10]} style={{ marginTop: 20, position: 'relative', zIndex: 1 }}>
          <Button
            icon={<UnorderedListOutlined />}
            onClick={() => {
              if (isMobile) {
                setOrgListVisible(true);
              }
            }}
            disabled={!isMobile || organizations.length === 0}
            style={actionButtonStyle}
          >
            打开组织列表
          </Button>
          <Button
            icon={<BankOutlined />}
            onClick={() => void loadOrganizations()}
            style={actionButtonStyle}
          >
            刷新组织
          </Button>
          {selectedOrg && (
            <Button
              icon={<EditOutlined />}
              onClick={() => {
                editOrgForm.setFieldsValue({
                  power_level: selectedOrg.power_level,
                  location: selectedOrg.location,
                  motto: selectedOrg.motto,
                  color: selectedOrg.color
                });
                setIsEditOrgModalOpen(true);
              }}
              style={actionButtonStyle}
            >
              编辑组织
            </Button>
          )}
          {selectedOrg && (
            <Button
              type="primary"
              icon={<PlusOutlined />}
              onClick={() => setIsAddMemberModalOpen(true)}
              disabled={availableCharacters.length === 0}
              style={{ borderRadius: 999, paddingInline: 16 }}
            >
              添加成员
            </Button>
          )}
        </Space>
      </Card>

      <div
        style={{
          display: 'grid',
          gridTemplateColumns: isMobile ? '1fr' : 'minmax(0, 1.15fr) minmax(300px, 0.9fr)',
          gap: 16,
        }}
      >
        <Card
          variant="borderless"
          style={{
            borderRadius: 22,
            background: quietPanelBackground,
            border: panelBorder,
            boxShadow: `0 18px 36px color-mix(in srgb, ${token.colorText} 8%, transparent)`,
          }}
          styles={{ body: { padding: 18 } }}
        >
          <Text style={{ fontSize: 11, letterSpacing: '0.18em', textTransform: 'uppercase', color: token.colorTextTertiary }}>
            Workspace Guide
          </Text>
          <Title level={4} style={{ margin: '8px 0 10px', fontFamily: designDisplayFont, letterSpacing: '-0.03em' }}>
            组织页维护顺序
          </Title>
          <Paragraph type="secondary" style={{ marginBottom: 14, lineHeight: 1.8 }}>
            这页更像势力档案与成员名单的混合工作台。优先维护“组织是谁”，再补“成员怎么协作”，最后统一世界观层的视觉与设定细节。
          </Paragraph>
          <div style={{ display: 'grid', gridTemplateColumns: 'repeat(auto-fit, minmax(180px, 1fr))', gap: 10 }}>
            {organizationGuideItems.map((item) => (
              <div
                key={item.label}
                style={{
                  borderRadius: 16,
                  padding: '12px 14px',
                  border: `1px solid ${token.colorBorderSecondary}`,
                  background: token.colorBgContainer,
                }}
              >
                <Text style={{ display: 'block', fontSize: 11, color: token.colorTextTertiary, textTransform: 'uppercase', letterSpacing: '0.08em' }}>
                  {item.label}
                </Text>
                <Text strong style={{ display: 'block', margin: '6px 0 4px' }}>
                  {item.title}
                </Text>
                <Text type="secondary" style={{ fontSize: 12, lineHeight: 1.7 }}>
                  {item.description}
                </Text>
              </div>
            ))}
          </div>
        </Card>

        <Card
          variant="borderless"
          style={{
            borderRadius: 22,
            background: quietPanelBackground,
            border: panelBorder,
            boxShadow: `0 18px 36px color-mix(in srgb, ${token.colorText} 8%, transparent)`,
          }}
          styles={{ body: { padding: 18 } }}
        >
          <Text style={{ fontSize: 11, letterSpacing: '0.18em', textTransform: 'uppercase', color: token.colorTextTertiary }}>
            Current Focus
          </Text>
          <Title level={4} style={{ margin: '8px 0 10px', fontFamily: designDisplayFont, letterSpacing: '-0.03em' }}>
            当前维护焦点
          </Title>
          <Space direction="vertical" size={10} style={{ width: '100%' }}>
            {organizationFocusItems.map((item) => (
              <div
                key={item.label}
                style={{
                  borderRadius: 16,
                  padding: '12px 14px',
                  background: token.colorBgContainer,
                  border: `1px solid ${token.colorBorderSecondary}`,
                }}
              >
                <Text style={{ display: 'block', marginBottom: 4, fontSize: 12, color: token.colorTextTertiary }}>
                  {item.label}
                </Text>
                <Text strong style={{ display: 'block', lineHeight: 1.7 }}>
                  {item.value}
                </Text>
                <Text type="secondary" style={{ fontSize: 12, lineHeight: 1.7 }}>
                  {item.detail}
                </Text>
              </div>
            ))}
          </Space>
        </Card>
      </div>

      {isMobile && (
        <Drawer
          title="组织列表"
          placement="left"
          onClose={() => setOrgListVisible(false)}
          open={orgListVisible}
          width="85%"
          styles={{ body: { padding: 0 } }}
        >
          {organizations.length === 0 ? (
            <div style={{ textAlign: 'center', padding: '40px 20px', color: token.colorTextTertiary }}>
              暂无组织
            </div>
          ) : (
            <Space direction="vertical" style={{ width: '100%', padding: '12px' }}>
              {organizations.map(org => (
                <Card
                  key={org.id}
                  size="small"
                  hoverable
                  style={{
                    cursor: 'pointer',
                    border: selectedOrg?.id === org.id ? `2px solid ${token.colorPrimary}` : `1px solid ${token.colorBorder}`,
                    background: selectedOrg?.id === org.id ? token.colorPrimaryBg : 'transparent'
                  }}
                  onClick={() => {
                    handleSelectOrganization(org);
                    setOrgListVisible(false);
                  }}
                >
                  <Space direction="vertical" size="small" style={{ width: '100%' }}>
                    <strong style={{ fontSize: 14 }}>{org.name}</strong>
                    <Tag color="blue">{org.type}</Tag>
                    <div style={{ fontSize: '12px', color: token.colorTextSecondary }}>
                      成员: {org.member_count} | 势力: {org.power_level}
                    </div>
                  </Space>
                </Card>
              ))}
            </Space>
          )}
        </Drawer>
      )}

      <Row gutter={[16, 16]} align="stretch">
        {!isMobile && (
          <Col xs={24} xl={8}>
            <Card
              variant="borderless"
              style={{
                height: '100%',
                background: quietPanelBackground,
                borderRadius: 24,
                border: panelBorder,
                boxShadow: `0 18px 36px color-mix(in srgb, ${token.colorText} 8%, transparent)`,
              }}
              styles={{ body: { padding: 14, maxHeight: 'calc(100vh - 320px)', overflowY: 'auto' } }}
              loading={loading}
            >
              <Space direction="vertical" size={12} style={{ width: '100%' }}>
                <div>
                  <Text style={{ fontSize: 11, letterSpacing: '0.18em', textTransform: 'uppercase', color: token.colorTextTertiary }}>
                    Organization Index
                  </Text>
                  <Title level={4} style={{ margin: '8px 0 0', fontFamily: designDisplayFont, letterSpacing: '-0.03em' }}>
                    组织列表
                  </Title>
                </div>
                {organizations.length === 0 ? (
                  <Empty description="暂无组织" style={{ padding: '36px 0 24px' }} />
                ) : (
                  organizations.map((org) => (
                    <Card
                      key={org.id}
                      size="small"
                      hoverable
                      style={{
                        cursor: 'pointer',
                        borderRadius: 18,
                        border: selectedOrg?.id === org.id ? `1px solid ${token.colorPrimary}` : `1px solid ${token.colorBorder}`,
                        background: selectedOrg?.id === org.id
                          ? `color-mix(in srgb, ${token.colorPrimary} 10%, ${token.colorBgContainer} 90%)`
                          : 'color-mix(in srgb, var(--ant-color-bg-container) 88%, var(--ant-color-bg-layout) 12%)',
                        boxShadow: selectedOrg?.id === org.id
                          ? `0 14px 30px color-mix(in srgb, ${token.colorPrimary} 16%, transparent)`
                          : 'none',
                      }}
                      onClick={() => handleSelectOrganization(org)}
                    >
                      <Space direction="vertical" size={8} style={{ width: '100%' }}>
                        <div style={{ display: 'flex', justifyContent: 'space-between', gap: 12, alignItems: 'center' }}>
                          <Text strong style={{ fontSize: 14 }}>{org.name}</Text>
                          <Tag color="blue" style={{ marginInlineEnd: 0 }}>{org.type}</Tag>
                        </div>
                        <Text type="secondary" style={{ fontSize: 12 }}>
                          成员 {org.member_count} 人 · 势力 {org.power_level}
                        </Text>
                      </Space>
                    </Card>
                  ))
                )}
              </Space>
            </Card>
          </Col>
        )}

        <Col xs={24} xl={isMobile ? 24 : 16}>
          <Card
            variant="borderless"
            style={{
              height: '100%',
              background: panelBackground,
              borderRadius: 24,
              border: panelBorder,
              boxShadow: `0 18px 36px color-mix(in srgb, ${token.colorText} 8%, transparent)`,
            }}
            styles={{ body: { padding: isMobile ? 14 : 18 } }}
          >
            {!selectedOrg ? (
              <Empty description="请选择一个组织查看详情" style={{ padding: '76px 0 64px' }}>
                <Paragraph type="secondary" style={{ maxWidth: 460, margin: '8px auto 20px', lineHeight: 1.8 }}>
                  先从左侧目录选择一个组织，或在移动端打开组织列表。进入详情后，你可以继续维护组织设定、成员名单与职位关系。
                </Paragraph>
                {isMobile && organizations.length > 0 && (
                  <Button
                    type="primary"
                    icon={<UnorderedListOutlined />}
                    onClick={() => setOrgListVisible(true)}
                  >
                    打开组织列表
                  </Button>
                )}
              </Empty>
            ) : (
              <Space direction="vertical" size={16} style={{ width: '100%' }}>
                <Card
                  variant="borderless"
                  style={{
                    borderRadius: 20,
                    background: 'color-mix(in srgb, var(--ant-color-bg-container) 88%, var(--ant-color-bg-layout) 12%)',
                    border: `1px solid ${token.colorBorderSecondary}`,
                  }}
                  styles={{ body: { padding: isMobile ? 16 : 20 } }}
                >
                  <Space direction="vertical" size={10} style={{ width: '100%' }}>
                    <div style={{ display: 'flex', justifyContent: 'space-between', gap: 12, alignItems: 'flex-start', flexWrap: 'wrap' }}>
                      <div>
                        <Text style={{ fontSize: 11, letterSpacing: '0.18em', textTransform: 'uppercase', color: token.colorTextTertiary }}>
                          Organization Profile
                        </Text>
                        <Title level={3} style={{ margin: '8px 0 6px', fontFamily: designDisplayFont, letterSpacing: '-0.03em' }}>
                          {selectedOrg.name}
                        </Title>
                        <Space wrap>
                          <Tag color="blue">{selectedOrg.type}</Tag>
                          <Tag color={selectedOrg.power_level >= 70 ? 'red' : selectedOrg.power_level >= 50 ? 'orange' : 'default'}>
                            势力等级 {selectedOrg.power_level}
                          </Tag>
                          <Tag>{selectedOrg.member_count} 名成员</Tag>
                        </Space>
                      </div>
                      {isMobile && (
                        <Button
                          icon={<UnorderedListOutlined />}
                          onClick={() => setOrgListVisible(true)}
                        >
                          切换组织
                        </Button>
                      )}
                    </div>
                    <Descriptions column={isMobile ? 1 : 2} size="small">
                      <Descriptions.Item label="组织名称">{selectedOrg.name}</Descriptions.Item>
                      <Descriptions.Item label="类型">{selectedOrg.type}</Descriptions.Item>
                      <Descriptions.Item label="成员数量">{selectedOrg.member_count}</Descriptions.Item>
                      <Descriptions.Item label="势力等级">{selectedOrg.power_level}</Descriptions.Item>
                      {selectedOrg.location && (
                        <Descriptions.Item label="所在地" span={isMobile ? 1 : 2}>
                          {selectedOrg.location}
                        </Descriptions.Item>
                      )}
                      {selectedOrg.color && (
                        <Descriptions.Item label="代表颜色">
                          {selectedOrg.color}
                        </Descriptions.Item>
                      )}
                      {selectedOrg.motto && (
                        <Descriptions.Item label="格言/口号" span={isMobile ? 1 : 2}>
                          {selectedOrg.motto}
                        </Descriptions.Item>
                      )}
                      <Descriptions.Item label="组织目的" span={isMobile ? 1 : 2}>
                        {selectedOrg.purpose}
                      </Descriptions.Item>
                    </Descriptions>
                  </Space>
                </Card>

                <Card
                  variant="borderless"
                  style={{
                    borderRadius: 20,
                    background: 'color-mix(in srgb, var(--ant-color-bg-container) 90%, var(--ant-color-bg-layout) 10%)',
                    border: `1px solid ${token.colorBorderSecondary}`,
                  }}
                  styles={{ body: { padding: isMobile ? 12 : 16 } }}
                >
                  <div style={{ display: 'flex', justifyContent: 'space-between', gap: 12, alignItems: 'center', marginBottom: 12, flexWrap: 'wrap' }}>
                    <div>
                      <Text style={{ display: 'block', marginBottom: 4, fontSize: 11, letterSpacing: '0.18em', textTransform: 'uppercase', color: token.colorTextTertiary }}>
                        Member Desk
                      </Text>
                      <Title level={4} style={{ margin: 0, fontFamily: designDisplayFont, letterSpacing: '-0.03em' }}>
                        组织成员
                      </Title>
                      <Text type="secondary" style={{ fontSize: 12 }}>
                        当前共 {members.length} 名成员
                      </Text>
                    </div>
                    <Button
                      type="primary"
                      size="small"
                      icon={<PlusOutlined />}
                      onClick={() => setIsAddMemberModalOpen(true)}
                      disabled={availableCharacters.length === 0}
                    >
                      添加成员
                    </Button>
                  </div>
                  {membersTableReady ? (
                    <Suspense
                      fallback={(
                        <InlineDeferredPanel
                          eyebrow="Member Table"
                          title="正在整理组织成员列表"
                          message="成员表格正在接入排序、分页和成员状态列。这里只补充轻量过渡说明，不改变表格数据与操作逻辑。"
                          tags={[
                            { label: '成员列表', color: 'blue' },
                            { label: '分页能力恢复中', color: 'processing' },
                          ]}
                        />
                      )}
                    >
                      <LazyDeferredAntdTable
                        columns={memberColumns}
                        dataSource={members}
                        rowKey="id"
                        pagination={
                          members.length > 5
                            ? {
                              defaultPageSize: 5,
                              showSizeChanger: true,
                              showQuickJumper: !isMobile,
                              showTotal: (total: number) => `共 ${total} 名成员`,
                              pageSizeOptions: [5, 10, 20],
                              simple: isMobile,
                              position: ['bottomCenter'],
                            }
                            : false
                        }
                        size="small"
                        scroll={{
                          x: isMobile ? 'max-content' : undefined,
                          y: members.length > 10 ? 500 : undefined,
                        }}
                      />
                    </Suspense>
                  ) : (
                    <InlineDeferredPanel
                      eyebrow="Member Workspace"
                      title="正在接管组织成员工作区"
                      message="系统正在准备成员名单、状态列与分页区域，原有成员查询、添加和编辑逻辑保持不变。"
                      minHeight={220}
                      tags={[
                        { label: '成员列表接管中', color: 'processing' },
                        { label: `当前 ${members.length} 名`, color: 'blue' },
                        { label: '成员逻辑保持原样', color: 'green' },
                      ]}
                    />
                  )}
                </Card>
              </Space>
            )}
          </Card>
        </Col>
      </Row>

      {/* 添加成员模态框 */}
      <Modal
        title={(
          <Space direction="vertical" size={2}>
            <Text style={{ fontSize: 11, letterSpacing: '0.18em', textTransform: 'uppercase', color: token.colorTextTertiary }}>
              Member Intake
            </Text>
            <Title level={4} style={{ margin: 0, fontFamily: designDisplayFont, letterSpacing: '-0.03em' }}>
              添加组织成员
            </Title>
            <Text type="secondary">
              为当前组织补充新成员时，优先明确职位与加入状态，让名单一开始就带有结构信息。
            </Text>
          </Space>
        )}
        open={isAddMemberModalOpen}
        onCancel={() => {
          setIsAddMemberModalOpen(false);
          form.resetFields();
        }}
        footer={null}
        centered={!isMobile}
        width={isMobile ? '100%' : 500}
        style={isMobile ? { top: 0, paddingBottom: 0, maxWidth: '100vw' } : undefined}
        styles={isMobile ? { ...modalSurfaceStyles, body: { ...modalSurfaceStyles.body, maxHeight: 'calc(100vh - 110px)', overflowY: 'auto' } } : modalSurfaceStyles}
      >
        <Card
          size="small"
          variant="borderless"
          style={{ marginBottom: 16, borderRadius: 14, background: 'color-mix(in srgb, var(--ant-color-info-bg) 82%, var(--ant-color-bg-container) 18%)' }}
        >
          <Text type="secondary">
            新增成员前，先确认该角色尚未加入当前组织。职位、等级和忠诚度会直接影响后续关系判断。
          </Text>
        </Card>
        <Form
          form={form}
          layout="vertical"
          onFinish={handleAddMember}
        >
          <Form.Item
            name="character_id"
            label="选择角色"
            rules={[{ required: true, message: '请选择角色' }]}
          >
            <Select
              placeholder="选择要加入的角色"
              showSearch
              filterOption={(input, option) =>
                (option?.label ?? '').toLowerCase().includes(input.toLowerCase())
              }
              options={availableCharacterOptions}
            />
          </Form.Item>

          <Form.Item
            name="position"
            label="职位"
            rules={[{ required: true, message: '请输入职位' }]}
          >
            <Input placeholder="如：掌门、长老、弟子" />
          </Form.Item>

          <Form.Item
            name="rank"
            label="职位等级"
            initialValue={5}
            tooltip="数字越大等级越高"
          >
            <InputNumber min={0} max={10} style={{ width: '100%' }} />
          </Form.Item>

          <Form.Item
            name="loyalty"
            label="初始忠诚度"
            initialValue={50}
          >
            <InputNumber min={0} max={100} style={{ width: '100%' }} addonAfter="%" />
          </Form.Item>

          <Form.Item
            name="status"
            label="状态"
            initialValue="active"
          >
            <Select>
              <Select.Option value="active">在职</Select.Option>
              <Select.Option value="retired">退休</Select.Option>
              <Select.Option value="expelled">除名</Select.Option>
            </Select>
          </Form.Item>

          <Form.Item
            name="joined_at"
            label="加入时间"
          >
            <Input placeholder="如：开山大典时、三年前、建立之初等" />
          </Form.Item>

          <Form.Item>
            <Space style={{ width: '100%', justifyContent: 'flex-end' }}>
              <Button onClick={() => setIsAddMemberModalOpen(false)}>取消</Button>
              <Button type="primary" htmlType="submit">
                添加
              </Button>
            </Space>
          </Form.Item>
        </Form>
      </Modal>

      {/* 编辑成员模态框 */}
      <Modal
        title={(
          <Space direction="vertical" size={2}>
            <Text style={{ fontSize: 11, letterSpacing: '0.18em', textTransform: 'uppercase', color: token.colorTextTertiary }}>
              Membership Record
            </Text>
            <Title level={4} style={{ margin: 0, fontFamily: designDisplayFont, letterSpacing: '-0.03em' }}>
              编辑成员信息
            </Title>
            <Text type="secondary">
              这里维护的是成员在组织中的位置，而不是角色本体资料。优先更新职位、贡献度与状态变化。
            </Text>
          </Space>
        )}
        open={isEditMemberModalOpen}
        onCancel={() => {
          setIsEditMemberModalOpen(false);
          editMemberForm.resetFields();
          setEditingMember(null);
        }}
        footer={null}
        centered={true}
        width={isMobile ? '90%' : 500}
        style={isMobile ? {
          maxWidth: '90vw',
          margin: '0 auto'
        } : undefined}
        styles={isMobile ? {
          ...modalSurfaceStyles,
          body: {
            ...modalSurfaceStyles.body,
            maxHeight: 'calc(80vh - 110px)',
            overflowY: 'auto',
            padding: '0 16px 20px'
          }
        } : modalSurfaceStyles}
      >
        <Card
          size="small"
          variant="borderless"
          style={{ marginBottom: 16, borderRadius: 14, background: 'color-mix(in srgb, var(--ant-color-warning-bg) 72%, var(--ant-color-bg-container) 28%)' }}
        >
          <Text type="secondary">
            当成员状态变化较大时，建议同步补充备注，方便后续理解组织结构变化的原因。
          </Text>
        </Card>
        <Form
          form={editMemberForm}
          layout="vertical"
          onFinish={handleUpdateMember}
        >
          <Form.Item
            name="position"
            label="职位"
            rules={[{ required: true, message: '请输入职位' }]}
          >
            <Input placeholder="如：掌门、长老、弟子" />
          </Form.Item>

          <Form.Item
            name="rank"
            label="职位等级"
            tooltip="数字越大等级越高"
          >
            <InputNumber min={0} max={10} style={{ width: '100%' }} />
          </Form.Item>

          <Form.Item
            name="loyalty"
            label="忠诚度"
          >
            <InputNumber min={0} max={100} style={{ width: '100%' }} addonAfter="%" />
          </Form.Item>

          <Form.Item
            name="contribution"
            label="贡献度"
          >
            <InputNumber min={0} max={100} style={{ width: '100%' }} addonAfter="%" />
          </Form.Item>

          <Form.Item
            name="status"
            label="状态"
          >
            <Select>
              <Select.Option value="active">在职</Select.Option>
              <Select.Option value="retired">退休</Select.Option>
              <Select.Option value="expelled">除名</Select.Option>
              <Select.Option value="deceased">已故</Select.Option>
            </Select>
          </Form.Item>

          <Form.Item
            name="joined_at"
            label="加入时间"
          >
            <Input placeholder="如：开山大典时、三年前、建立之初等" />
          </Form.Item>

          <Form.Item
            name="notes"
            label="备注"
          >
            <Input.TextArea rows={3} placeholder="成员相关的备注信息" />
          </Form.Item>

          <Form.Item>
            <Space style={{ width: '100%', justifyContent: 'flex-end' }}>
              <Button onClick={() => {
                setIsEditMemberModalOpen(false);
                editMemberForm.resetFields();
                setEditingMember(null);
              }}>
                取消
              </Button>
              <Button type="primary" htmlType="submit">
                保存
              </Button>
            </Space>
          </Form.Item>
        </Form>
      </Modal>

      {/* 编辑组织模态框 */}
      <Modal
        title={(
          <Space direction="vertical" size={2}>
            <Text style={{ fontSize: 11, letterSpacing: '0.18em', textTransform: 'uppercase', color: token.colorTextTertiary }}>
              Organization Profile
            </Text>
            <Title level={4} style={{ margin: 0, fontFamily: designDisplayFont, letterSpacing: '-0.03em' }}>
              编辑组织信息
            </Title>
            <Text type="secondary">
              把势力等级、地理位置、代表色和口号整理成统一档案，让组织设定在世界观里更稳定可读。
            </Text>
          </Space>
        )}
        open={isEditOrgModalOpen}
        onCancel={() => {
          setIsEditOrgModalOpen(false);
          editOrgForm.resetFields();
        }}
        footer={null}
        centered={!isMobile}
        width={isMobile ? '100%' : 500}
        style={isMobile ? { top: 0, paddingBottom: 0, maxWidth: '100vw' } : undefined}
        styles={isMobile ? { ...modalSurfaceStyles, body: { ...modalSurfaceStyles.body, maxHeight: 'calc(100vh - 110px)', overflowY: 'auto' } } : modalSurfaceStyles}
      >
        <Card
          size="small"
          variant="borderless"
          style={{ marginBottom: 16, borderRadius: 14, background: 'color-mix(in srgb, var(--ant-color-success-bg) 70%, var(--ant-color-bg-container) 30%)' }}
        >
          <Text type="secondary">
            这些字段会直接影响组织在详情页中的辨识度，适合沉淀为世界观层的长期信息。
          </Text>
        </Card>
        <Form
          form={editOrgForm}
          layout="vertical"
          onFinish={async (values) => {
            if (!selectedOrg) return;
            try {
              await axios.put(`/api/organizations/${selectedOrg.id}`, values);
              message.success('组织信息更新成功');
              setIsEditOrgModalOpen(false);
              editOrgForm.resetFields();

              // 重新获取更新后的组织列表
              const res = await axios.get(`/api/organizations/project/${projectId}`);
              setOrganizations(res.data);

              // 更新当前选中的组织详情
              const updatedOrg = res.data.find((org: Organization) => org.id === selectedOrg.id);
              if (updatedOrg) {
                setSelectedOrg(updatedOrg);
              }

              // 刷新全局 store
              await refreshCharacters(projectId);
            } catch (error) {
              message.error('更新失败');
              console.error(error);
            }
          }}
        >
          <Form.Item
            name="power_level"
            label="势力等级"
            rules={[{ required: true, message: '请输入势力等级' }]}
            tooltip="0-100的数值，表示组织的影响力"
          >
            <InputNumber min={0} max={100} style={{ width: '100%' }} />
          </Form.Item>

          <Form.Item
            name="location"
            label="所在地"
          >
            <Input placeholder="组织的主要活动区域或总部位置" />
          </Form.Item>

          <Form.Item
            name="motto"
            label="格言/口号"
          >
            <Input placeholder="组织的宗旨、格言或口号" />
          </Form.Item>

          <Form.Item
            name="color"
            label="代表颜色"
          >
            <Input placeholder="如：深红色、金色、黑色等" />
          </Form.Item>

          <Form.Item>
            <Space style={{ width: '100%', justifyContent: 'flex-end' }}>
              <Button onClick={() => setIsEditOrgModalOpen(false)}>取消</Button>
              <Button type="primary" htmlType="submit">
                保存
              </Button>
            </Space>
          </Form.Item>
        </Form>
      </Modal>
    </div>
  );
}
