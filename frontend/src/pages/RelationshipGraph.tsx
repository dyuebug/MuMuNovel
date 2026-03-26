import { Suspense, lazy, useState, useEffect, useCallback, useMemo, type CSSProperties } from 'react';
import { useNavigate, useParams } from 'react-router-dom';
import { Card, Tag, Button, Space, message, Typography, theme } from 'antd';
import {
  ArrowLeftOutlined,
  ApartmentOutlined,
  UserOutlined,
  TeamOutlined,
  TrophyOutlined,
} from '@ant-design/icons';
import axios from 'axios';
import type { Node, Edge } from '@xyflow/react';
import type { BuildGraphThemeToken } from '../components/relationship-graph/buildGraph';

const { Text } = Typography;
const RelationshipGraphCanvas = lazy(() => import('../components/relationship-graph/RelationshipGraphCanvas'));

interface GraphNode {
  id: string;
  name: string;
  type: string;
  role_type: string;
  avatar: string | null;
}

interface GraphLink {
  source: string;
  target: string;
  relationship: string;
  intimacy: number;
  status: string;
}

interface GraphData {
  nodes: GraphNode[];
  links: GraphLink[];
}

interface RelationshipType {
  id: number;
  name: string;
  category: string;
  reverse_name: string;
  intimacy_range: string;
  icon: string;
  description: string;
  created_at: string;
}

interface CharacterDetail {
  id: string;
  project_id: string;
  name: string;
  age: string;
  gender: string;
  is_organization: boolean;
  role_type: string;
  personality: string;
  background: string;
  appearance: string;
  organization_type: string;
  organization_purpose: string;
  organization_members: string;
  traits: string;
  avatar_url: string;
  power_level: number;
  location: string;
  motto: string;
  color: string;
  main_career_id?: string;
  main_career_stage?: number;
  sub_careers?: Array<{ career_id: string; stage?: number }> | string | null;
}

interface CareerItem {
  id: string;
  name: string;
  type: 'main' | 'sub';
  max_stage: number;
}

interface CareerListResponse {
  main_careers?: CareerItem[];
  sub_careers?: CareerItem[];
}

interface CharacterListResponse {
  items?: CharacterDetail[];
}

const GROUP_MAIN_CAREER_NODE_ID = '__career_group_main__';
const GROUP_SUB_CAREER_NODE_ID = '__career_group_sub__';

type EdgeColorPreset = 'primary' | 'warning' | 'info' | 'textTertiary' | 'error' | 'success';

const EDGE_CATEGORY_META: Record<string, { label: string; colorPreset: EdgeColorPreset; order: number }> = {
  organization: { label: '组织成员', colorPreset: 'primary', order: 1 },
  career_main: { label: '主职业关联', colorPreset: 'warning', order: 2 },
  career_sub: { label: '副职业关联', colorPreset: 'info', order: 3 },
  career_group: { label: '职业分类', colorPreset: 'textTertiary', order: 4 },
  family: { label: '亲属关系', colorPreset: 'warning', order: 5 },
  hostile: { label: '敌对关系', colorPreset: 'error', order: 6 },
  professional: { label: '职业关系', colorPreset: 'info', order: 7 },
  social: { label: '社交关系', colorPreset: 'success', order: 8 },
  default: { label: '其他关系', colorPreset: 'textTertiary', order: 99 },
};

const getEdgeCategory = (edge: Edge) =>
  typeof edge.data?.category === 'string' ? edge.data.category : 'default';

const getEdgeCategoryMeta = (category: string) =>
  EDGE_CATEGORY_META[category] || {
    label: `${category}关系`,
    colorPreset: 'textTertiary' as const,
    order: 999,
  };

const resolveEdgePresetColor = (
  colorPreset: EdgeColorPreset,
  token: {
    colorPrimary: string;
    colorWarning: string;
    colorInfo: string;
    colorTextTertiary: string;
    colorError: string;
    colorSuccess: string;
  },
) => {
  const colorMap: Record<EdgeColorPreset, string> = {
    primary: token.colorPrimary,
    warning: token.colorWarning,
    info: token.colorInfo,
    textTertiary: token.colorTextTertiary,
    error: token.colorError,
    success: token.colorSuccess,
  };

  return colorMap[colorPreset];
};

const clampTextStyle = (rows: number): CSSProperties => ({
  margin: '4px 0 0',
  color: 'var(--ant-color-text-secondary)',
  fontSize: 14,
  lineHeight: '22px',
  display: '-webkit-box',
  WebkitBoxOrient: 'vertical',
  WebkitLineClamp: rows,
  overflow: 'hidden',
  textOverflow: 'ellipsis',
  wordBreak: 'break-word',
});

const safeParseStringArray = (raw: unknown): string[] => {
  if (!raw) return [];

  if (Array.isArray(raw)) {
    return raw.map((item) => String(item)).filter(Boolean);
  }

  if (typeof raw === 'string') {
    try {
      const parsed = JSON.parse(raw) as unknown;
      if (Array.isArray(parsed)) {
        return parsed.map((item) => String(item)).filter(Boolean);
      }
    } catch {
      return raw
        .split(/[，,、]/)
        .map((item) => item.trim())
        .filter(Boolean);
    }
  }

  return [];
};

const safeParseSubCareers = (raw: CharacterDetail['sub_careers']) => {
  if (!raw) return [] as Array<{ career_id: string; stage?: number }>;

  if (Array.isArray(raw)) {
    return raw.filter((item) => item?.career_id);
  }

  if (typeof raw === 'string') {
    try {
      const parsed = JSON.parse(raw) as Array<{ career_id: string; stage?: number }>;
      if (Array.isArray(parsed)) {
        return parsed.filter((item) => item?.career_id);
      }
    } catch {
      return [];
    }
  }

  return [];
};

const InfoField = ({
  label,
  value,
  rows = 2,
}: {
  label: string;
  value?: string | null;
  rows?: number;
}) => {
  if (!value) return null;

  return (
    <div
      style={{
        marginBottom: 12,
        padding: '12px 14px',
        borderRadius: 12,
        background: 'var(--ant-color-fill-quaternary)',
        border: '1px solid var(--ant-color-border-secondary)',
        boxShadow: '0 2px 4px color-mix(in srgb, var(--ant-color-text) 6%, transparent)',
      }}
    >
      <Text strong style={{ fontSize: 14, color: 'var(--ant-color-text)' }}>
        {label}
      </Text>
      <div style={clampTextStyle(rows)}>{value}</div>
    </div>
  );
};

export default function RelationshipGraph() {
  const { projectId } = useParams<{ projectId: string }>();
  const navigate = useNavigate();
  const { token } = theme.useToken();
  const alphaColor = (color: string, alpha: number) =>
    `color-mix(in srgb, ${color} ${(alpha * 100).toFixed(0)}%, transparent)`;

  const graphTheme = useMemo<BuildGraphThemeToken>(
    () => ({
      colorBgContainer: token.colorBgContainer,
      colorBorder: token.colorBorder,
      colorError: token.colorError,
      colorFillSecondary: token.colorFillSecondary,
      colorInfo: token.colorInfo,
      colorPrimary: token.colorPrimary,
      colorSuccess: token.colorSuccess,
      colorText: token.colorText,
      colorTextBase: token.colorTextBase,
      colorTextSecondary: token.colorTextSecondary,
      colorTextTertiary: token.colorTextTertiary,
      colorWarning: token.colorWarning,
    }),
    [
      token.colorBgContainer,
      token.colorBorder,
      token.colorError,
      token.colorFillSecondary,
      token.colorInfo,
      token.colorPrimary,
      token.colorSuccess,
      token.colorText,
      token.colorTextBase,
      token.colorTextSecondary,
      token.colorTextTertiary,
      token.colorWarning,
    ],
  );

  const [graphData, setGraphData] = useState<GraphData | null>(null);
  const [loading, setLoading] = useState(false);
  const [selectedNodeId, setSelectedNodeId] = useState<string | null>(null);
  const [nodeDetail, setNodeDetail] = useState<CharacterDetail | null>(null);
  const [, setDetailLoading] = useState(false);
  const [relationshipTypes, setRelationshipTypes] = useState<RelationshipType[]>([]);
  const [characterDetailMap, setCharacterDetailMap] = useState<Record<string, CharacterDetail>>({});
  const [mainCareers, setMainCareers] = useState<CareerItem[]>([]);
  const [subCareers, setSubCareers] = useState<CareerItem[]>([]);

  const [nodes, setNodes] = useState<Node[]>([]);
  const [edges, setEdges] = useState<Edge[]>([]);
  const [edgeVisibilityMap, setEdgeVisibilityMap] = useState<Record<string, boolean>>({});

  const careerNameMap = useMemo(() => {
    const map: Record<string, CareerItem> = {};
    [...mainCareers, ...subCareers].forEach((career) => {
      map[career.id] = career;
    });
    return map;
  }, [mainCareers, subCareers]);

  const edgeCategoryOptions = useMemo(() => {
    const counter = new Map<string, number>();

    edges.forEach((edge) => {
      const category = getEdgeCategory(edge);
      counter.set(category, (counter.get(category) || 0) + 1);
    });

    return Array.from(counter.entries())
      .map(([category, count]) => {
        const meta = getEdgeCategoryMeta(category);
        return {
          category,
          count,
          label: meta.label,
          color: resolveEdgePresetColor(meta.colorPreset, token),
          order: meta.order,
        };
      })
      .sort((a, b) => a.order - b.order || a.label.localeCompare(b.label, 'zh-CN'));
  }, [edges, token]);

  useEffect(() => {
    if (edgeCategoryOptions.length === 0) {
      return;
    }

    setEdgeVisibilityMap((prev) => {
      const next: Record<string, boolean> = {};
      edgeCategoryOptions.forEach((option) => {
        next[option.category] = prev[option.category] ?? true;
      });
      return next;
    });
  }, [edgeCategoryOptions]);

  const visibleEdges = useMemo(
    () => edges.filter((edge) => edgeVisibilityMap[getEdgeCategory(edge)] !== false),
    [edges, edgeVisibilityMap],
  );

  const toggleEdgeCategoryVisibility = (category: string) => {
    setEdgeVisibilityMap((prev) => ({
      ...prev,
      [category]: !(prev[category] ?? true),
    }));
  };

  useEffect(() => {
    if (projectId) {
      void loadRelationshipTypes();
    }
  }, [projectId]);

  const loadRelationshipTypes = async () => {
    try {
      const res = await axios.get('/api/relationships/types');
      setRelationshipTypes(res.data || []);
    } catch (error) {
      console.error('加载关系类型失败', error);
    }
  };

  const loadGraphData = useCallback(async () => {
    if (!projectId || relationshipTypes.length === 0) return;

    setLoading(true);
    try {
      const [graphRes, charactersRes, careersRes, layoutModule, graphBuilderModule] = await Promise.all([
        axios.get(`/api/relationships/graph/${projectId}`),
        axios.get('/api/characters', { params: { project_id: projectId } }),
        axios.get('/api/careers', { params: { project_id: projectId } }),
        import('../components/relationship-graph/layout'),
        import('../components/relationship-graph/buildGraph'),
      ]);

      const data = graphRes.data as GraphData;
      const characters = (charactersRes.data as CharacterListResponse)?.items || [];
      const careersData = (careersRes.data as CareerListResponse) || {};
      const buildResult = graphBuilderModule.buildRelationshipGraph({
        projectId,
        graphData: data,
        characters,
        careersData,
        relationshipTypes,
        token: graphTheme,
        getLayoutedElements: layoutModule.getLayoutedElements,
      });

      setMainCareers(buildResult.mainCareers);
      setSubCareers(buildResult.subCareers);
      setCharacterDetailMap(buildResult.characterDetailMap);
      setNodes(buildResult.nodes);
      setEdges(buildResult.edges);
      setGraphData(buildResult.graphData);
    } catch (error) {
      message.error('加载关系图谱失败');
      console.error(error);
    } finally {
      setLoading(false);
    }
  }, [projectId, relationshipTypes, graphTheme]);

  // ? relationshipTypes ???????????
  useEffect(() => {
    void loadGraphData();
  }, [loadGraphData]);

  const loadNodeDetail = async (nodeId: string) => {
    if (!projectId) return;

    // 职业分组节点不展示详情
    if (nodeId === GROUP_MAIN_CAREER_NODE_ID || nodeId === GROUP_SUB_CAREER_NODE_ID) {
      return;
    }

    // 职业节点不展示详情
    if (nodeId.startsWith('career-main-') || nodeId.startsWith('career-sub-')) {
      return;
    }

    const cached = characterDetailMap[nodeId];
    if (cached) {
      setNodeDetail(cached);
      return;
    }

    setDetailLoading(true);
    try {
      const res = await axios.get(`/api/characters/${nodeId}`);
      setNodeDetail(res.data as CharacterDetail);
    } catch (error) {
      message.error('加载详情失败');
      console.error(error);
    } finally {
      setDetailLoading(false);
    }
  };

  const handleNodeClick = (_: unknown, node: { id: string }) => {
    setSelectedNodeId(node.id);

    const shouldShowDetail =
      node.id !== GROUP_MAIN_CAREER_NODE_ID &&
      node.id !== GROUP_SUB_CAREER_NODE_ID &&
      !node.id.startsWith('career-main-') &&
      !node.id.startsWith('career-sub-');

    setNodeDetail(null);

    if (shouldShowDetail) {
      void loadNodeDetail(node.id);
    }
  };

  const handleCloseDetail = () => {
    setSelectedNodeId(null);
    setNodeDetail(null);
  };

  const goBack = () => {
    if (projectId) {
      navigate(`/project/${projectId}/relationships`);
      return;
    }
    navigate('/projects');
  };

  const renderCareerTags = () => {
    if (!nodeDetail || nodeDetail.is_organization) return null;

    const subCareerData = safeParseSubCareers(nodeDetail.sub_careers);

    return (
      <div
        style={{
          marginBottom: 12,
          padding: '12px 14px',
          borderRadius: 12,
          background: token.colorFillQuaternary,
          border: `1px solid ${token.colorBorderSecondary}`,
          boxShadow: `0 2px 4px ${alphaColor(token.colorTextBase, 0.06)}`,
        }}
      >
        <Text strong style={{ fontSize: 14, color: token.colorText }}>
          职业体系
        </Text>
        <div style={{ marginTop: 10, display: 'flex', flexDirection: 'column', gap: 8 }}>
          {nodeDetail.main_career_id ? (
            <div style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
              <Tag color="gold" style={{ margin: 0, borderRadius: 12, padding: '0 10px', fontWeight: 500 }}>主职业</Tag>
              <span style={{ fontSize: 14, color: token.colorText }}>
                {careerNameMap[nodeDetail.main_career_id]?.name || nodeDetail.main_career_id}
                {nodeDetail.main_career_stage ? <span style={{ color: token.colorTextTertiary, marginLeft: 4 }}>第{nodeDetail.main_career_stage}阶</span> : ''}
              </span>
            </div>
          ) : (
            <div style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
              <Tag style={{ margin: 0, borderRadius: 12, padding: '0 10px' }}>主职业</Tag>
              <span style={{ fontSize: 14, color: token.colorTextTertiary }}>未设置</span>
            </div>
          )}

          {subCareerData.length > 0 ? (
            <div style={{ display: 'flex', alignItems: 'flex-start', gap: 8 }}>
               <Tag color="cyan" style={{ margin: 0, borderRadius: 12, padding: '0 10px', fontWeight: 500 }}>副职业</Tag>
               <div style={{ display: 'flex', flexWrap: 'wrap', gap: 6, flex: 1 }}>
                {subCareerData.map((sub, index) => (
                  <span key={`${sub.career_id}-${index}`} style={{ fontSize: 14, color: token.colorText, background: token.colorBgContainer, border: `1px solid ${token.colorBorderSecondary}`, borderRadius: token.borderRadiusSM, padding: '0 6px' }}>
                    {careerNameMap[sub.career_id]?.name || sub.career_id}
                    {sub.stage ? <span style={{ color: token.colorTextTertiary, marginLeft: 4 }}>阶{sub.stage}</span> : ''}
                  </span>
                ))}
               </div>
            </div>
          ) : (
            <div style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
              <Tag style={{ margin: 0, borderRadius: 12, padding: '0 10px' }}>副职业</Tag>
              <span style={{ fontSize: 14, color: token.colorTextTertiary }}>未设置</span>
            </div>
          )}
        </div>
      </div>
    );
  };

  const traitList = safeParseStringArray(nodeDetail?.traits);
  const orgMembers = safeParseStringArray(nodeDetail?.organization_members);

  const renderGraphCanvasPlaceholder = (messageText: string) => (
    <div
      style={{
        flex: 1,
        minHeight: 0,
        display: 'flex',
        alignItems: 'center',
        justifyContent: 'center',
        borderRadius: 12,
        background: token.colorFillQuaternary,
      }}
    >
      <Text type="secondary">{messageText}</Text>
    </div>
  );

  return (
    <div
      style={{
        height: '100%',
        minHeight: 0,
        display: 'flex',
        flexDirection: 'column',
        backgroundColor: token.colorBgLayout,
        overflow: 'hidden',
      }}
    >
      <Card
        size="small"
        style={{
          flex: 1,
          minHeight: 0,
          display: 'flex',
          flexDirection: 'column',
        }}
        bodyStyle={{
          flex: 1,
          minHeight: 0,
          display: 'flex',
          flexDirection: 'column',
          overflow: 'hidden',
          padding: 12,
        }}
        title={
          <Space>
            <Button type="text" icon={<ArrowLeftOutlined />} onClick={goBack}>
              返回
            </Button>
            <span>关系图谱</span>
            <Tag color="processing" style={{ marginInlineStart: 4 }}>
              {graphData?.nodes?.length || 0} 节点 / {graphData?.links?.length || 0} 关系
            </Tag>
          </Space>
        }
        extra={
          <Space direction="vertical" size={6} style={{ alignItems: 'flex-end' }}>
            <div style={{ display: 'flex', justifyContent: 'flex-end', alignItems: 'center', gap: 14, fontSize: 12, flexWrap: 'wrap' }}>
              {/* 节点图例 */}
              <div style={{ display: 'flex', alignItems: 'center', gap: 4 }}>
                <span style={{ color: token.colorInfo, fontWeight: 'bold' }}>●</span>
                <span>角色（圆形）</span>
              </div>
              <div style={{ display: 'flex', alignItems: 'center', gap: 4 }}>
                <span style={{ color: token.colorSuccess, fontWeight: 'bold' }}>■</span>
                <span>组织（方形）</span>
              </div>
              <div style={{ display: 'flex', alignItems: 'center', gap: 4 }}>
                <span style={{ color: token.colorWarning, fontWeight: 'bold' }}>▭</span>
                <span>主职业</span>
              </div>
              <div style={{ display: 'flex', alignItems: 'center', gap: 4 }}>
                <span style={{ color: token.colorInfo, fontWeight: 'bold' }}>▭</span>
                <span>副职业</span>
              </div>

              <span style={{ color: token.colorBorder }}>|</span>

              {/* 连线图例 */}
              <div style={{ display: 'flex', alignItems: 'center', gap: 4 }}>
                <span style={{ color: token.colorPrimary, fontWeight: 'bold' }}>- -</span>
                <span>组织成员</span>
              </div>
              <div style={{ display: 'flex', alignItems: 'center', gap: 4 }}>
                <span style={{ color: token.colorWarning, fontWeight: 'bold' }}>—</span>
                <span>主职业关联</span>
              </div>
              <div style={{ display: 'flex', alignItems: 'center', gap: 4 }}>
                <span style={{ color: token.colorInfo, fontWeight: 'bold' }}>- -</span>
                <span>副职业关联</span>
              </div>
            </div>

            {edgeCategoryOptions.length > 0 && (
              <div style={{ display: 'flex', justifyContent: 'flex-end', alignItems: 'center', gap: 8, flexWrap: 'wrap' }}>
                <Text type="secondary" style={{ fontSize: 12 }}>
                  连线显示：
                </Text>
                {edgeCategoryOptions.map((option) => {
                  const isVisible = edgeVisibilityMap[option.category] !== false;
                  return (
                    <Button
                      key={option.category}
                      size="small"
                      type={isVisible ? 'primary' : 'default'}
                      onClick={() => toggleEdgeCategoryVisibility(option.category)}
                      style={
                        isVisible
                          ? { backgroundColor: option.color, borderColor: option.color, color: token.colorWhite }
                          : { color: token.colorTextSecondary }
                      }
                    >
                      {option.label}（{option.count}）
                    </Button>
                  );
                })}
              </div>
            )}
          </Space>
        }
      >
        {loading ? (
          renderGraphCanvasPlaceholder('关系图谱加载中...')
        ) : graphData && nodes.length > 0 ? (
          <Suspense fallback={renderGraphCanvasPlaceholder('图谱引擎加载中...')}>
            <RelationshipGraphCanvas
              nodes={nodes}
              edges={visibleEdges}
              onNodeClick={handleNodeClick}
            />
          </Suspense>
        ) : (
          renderGraphCanvasPlaceholder('暂无可渲染的关系图谱数据')
        )}
      </Card>

      {selectedNodeId && nodeDetail && (
<div
  style={{
    position: 'fixed',
    right: 24,
    top: 80,
    width: 400,
    height: 'calc(100vh - 100px)',
    maxHeight: 700,
    zIndex: 1000,
    display: 'flex',
    flexDirection: 'column',
  }}
>
  <Card
    size="small"
    style={{
      width: '100%',
      flex: 1,
      borderRadius: 16,
      boxShadow: `0 12px 32px ${alphaColor(token.colorTextBase, 0.22)}`,
      overflow: 'hidden',
      display: 'flex',
      flexDirection: 'column',
    }}
    bodyStyle={{
      flex: 1,
      overflow: 'hidden',
      padding: '12px 16px',
      display: 'flex',
      flexDirection: 'column',
    }}
            title={
              <Space>
                {nodeDetail.is_organization ? <ApartmentOutlined /> : <UserOutlined />}
                <span>{nodeDetail.is_organization ? '组织详情' : '角色详情'}</span>
              </Space>
            }
            extra={
              <Button type="text" size="small" onClick={handleCloseDetail}>
                ×
              </Button>
            }
          >
            <div style={{ flex: 1, display: 'flex', flexDirection: 'column', overflow: 'hidden' }}>
              <div
                style={{
                  textAlign: 'center',
                  marginBottom: 16,
                  padding: '8px 12px 0',
                  minHeight: 140,
                  display: 'flex',
                  flexDirection: 'column',
                  alignItems: 'center',
                }}
              >
                <div style={{ position: 'relative', width: 84, height: 84, marginBottom: 12 }}>
                  {nodeDetail.avatar_url ? (
                    <img
                      src={nodeDetail.avatar_url}
                      alt={nodeDetail.name}
                      style={{
                        width: '100%',
                        height: '100%',
                        borderRadius: '50%',
                        objectFit: 'cover',
                        border: `3px solid ${token.colorBgContainer}`,
                        boxShadow: `0 4px 12px ${alphaColor(token.colorTextBase, 0.18)}`,
                      }}
                    />
                  ) : (
                    <div
                      style={{
                        width: '100%',
                        height: '100%',
                        borderRadius: '50%',
                        backgroundColor: nodeDetail.color || (nodeDetail.is_organization ? token.colorSuccess : token.colorPrimary),
                        display: 'flex',
                        alignItems: 'center',
                        justifyContent: 'center',
                        fontSize: 32,
                        color: token.colorWhite,
                        border: `3px solid ${token.colorBgContainer}`,
                        boxShadow: `0 4px 12px ${alphaColor(token.colorTextBase, 0.18)}`,
                      }}
                    >
                      {nodeDetail.is_organization ? <TeamOutlined /> : <UserOutlined />}
                    </div>
                  )}
                  <div
                    style={{
                      position: 'absolute',
                      bottom: -4,
                      right: -4,
                      background: nodeDetail.is_organization ? token.colorSuccess : (nodeDetail.role_type === 'protagonist' ? token.colorError : nodeDetail.role_type === 'antagonist' ? token.colorPrimary : token.colorInfo),
                      borderRadius: '50%',
                      width: 28,
                      height: 28,
                      display: 'flex',
                      alignItems: 'center',
                      justifyContent: 'center',
                      border: `2px solid ${token.colorBgContainer}`,
                      color: token.colorWhite,
                      boxShadow: `0 2px 6px ${alphaColor(token.colorTextBase, 0.22)}`,
                    }}
                  >
                    {nodeDetail.is_organization ? <ApartmentOutlined style={{ fontSize: 14 }} /> : <UserOutlined style={{ fontSize: 14 }} />}
                  </div>
                </div>

                <div style={{ fontSize: 20, fontWeight: 600, color: token.colorText, marginBottom: 8 }}>{nodeDetail.name}</div>
                <Space size={6} wrap style={{ justifyContent: 'center' }}>
                  {!nodeDetail.is_organization && (
                    <Tag
                      color={
                        nodeDetail.role_type === 'protagonist'
                          ? 'red'
                          : nodeDetail.role_type === 'antagonist'
                            ? 'purple'
                            : 'blue'
                      }
                      style={{ borderRadius: 12, padding: '0 10px', fontWeight: 500 }}
                    >
                      {nodeDetail.role_type === 'protagonist'
                        ? '主角'
                        : nodeDetail.role_type === 'antagonist'
                          ? '反派'
                          : '配角'}
                    </Tag>
                  )}
                  {nodeDetail.gender && !nodeDetail.is_organization && <Tag style={{ borderRadius: 12, padding: '0 10px' }}>{nodeDetail.gender}</Tag>}
                  {nodeDetail.age && !nodeDetail.is_organization && <Tag style={{ borderRadius: 12, padding: '0 10px' }}>{nodeDetail.age}岁</Tag>}
                </Space>
              </div>

              <div style={{ flex: 1, overflowY: 'auto', paddingRight: 8, paddingLeft: 4, paddingBottom: 16 }}>
                {!nodeDetail.is_organization ? (
                  <>
                    {renderCareerTags()}
                    <InfoField label="外貌特征" value={nodeDetail.appearance} rows={2} />
                    <InfoField label="性格特点" value={nodeDetail.personality} rows={3} />
                    <InfoField label="背景故事" value={nodeDetail.background} rows={4} />

                    {traitList.length > 0 && (
                      <div
                        style={{
                          marginBottom: 12,
                          padding: '12px 14px',
                          borderRadius: 12,
                          background: token.colorFillQuaternary,
                          border: `1px solid ${token.colorBorderSecondary}`,
                          boxShadow: `0 2px 4px ${alphaColor(token.colorTextBase, 0.06)}`,
                        }}
                      >
                        <Text strong style={{ fontSize: 14, color: token.colorText }}>
                          特征标签
                        </Text>
                        <Space size={[6, 8]} wrap style={{ marginTop: 10 }}>
                          {traitList.slice(0, 12).map((trait, index) => (
                            <Tag key={`${trait}-${index}`} color="blue" style={{ borderRadius: 12, padding: '0 10px', margin: 0 }}>
                              {trait}
                            </Tag>
                          ))}
                        </Space>
                      </div>
                    )}
                  </>
                ) : (
                  <>
                    <InfoField label="组织类型" value={nodeDetail.organization_type} rows={2} />
                    <InfoField label="组织目的" value={nodeDetail.organization_purpose} rows={3} />
                    <InfoField label="所在地" value={nodeDetail.location} rows={2} />
                    <InfoField label="组织格言" value={nodeDetail.motto} rows={2} />

                    {nodeDetail.power_level !== undefined && nodeDetail.power_level !== null && (
                      <div
                        style={{
                          marginBottom: 12,
                          padding: '12px 14px',
                          borderRadius: 12,
                          background: token.colorFillQuaternary,
                          border: `1px solid ${token.colorBorderSecondary}`,
                          boxShadow: `0 2px 4px ${alphaColor(token.colorTextBase, 0.06)}`,
                        }}
                      >
                        <Text strong style={{ fontSize: 14, color: token.colorText }}>
                          势力等级
                        </Text>
                        <div style={{ ...clampTextStyle(1), fontSize: 18, color: token.colorWarning, fontWeight: 'bold' }}>
                          {nodeDetail.power_level}<span style={{ fontSize: 14, color: token.colorTextTertiary, fontWeight: 'normal' }}>/100</span>
                        </div>
                      </div>
                    )}

                    {orgMembers.length > 0 && (
                      <div
                        style={{
                          marginBottom: 12,
                          padding: '12px 14px',
                          borderRadius: 12,
                          background: token.colorFillQuaternary,
                          border: `1px solid ${token.colorBorderSecondary}`,
                          boxShadow: `0 2px 4px ${alphaColor(token.colorTextBase, 0.06)}`,
                        }}
                      >
                        <Text strong style={{ fontSize: 14, color: token.colorText }}>
                          组织成员
                        </Text>
                        <Space size={[6, 8]} wrap style={{ marginTop: 10 }}>
                          {orgMembers.slice(0, 16).map((member, index) => (
                            <Tag key={`${member}-${index}`} color="green" style={{ borderRadius: 12, padding: '0 10px', margin: 0 }}>
                              {member}
                            </Tag>
                          ))}
                        </Space>
                      </div>
                    )}
                  </>
                )}
              </div>
            </div>
          </Card>
        </div>
      )}

      {/* 职业节点点击提示（不展示详情卡时） */}
      {selectedNodeId && !nodeDetail && (selectedNodeId.startsWith('career-main-') || selectedNodeId.startsWith('career-sub-')) && (
        <div
          style={{
            position: 'fixed',
            right: 20,
            top: 80,
            zIndex: 1000,
          }}
        >
          <Card size="small" style={{ width: 300, borderRadius: 10, boxShadow: `0 6px 18px ${alphaColor(token.colorTextBase, 0.2)}` }}>
            <Space align="start">
              <TrophyOutlined style={{ color: token.colorWarning, marginTop: 4 }} />
              <div>
                <Text strong>职业节点</Text>
                <p style={{ ...clampTextStyle(2), marginTop: 2 }}>
                  职业节点用于展示主/副职业分组及其与角色的关联关系，不显示角色详情卡。
                </p>
              </div>
            </Space>
          </Card>
        </div>
      )}
    </div>
  );
}
