import { Suspense, lazy, useState, useEffect, useCallback, useMemo, useRef } from 'react';
import type { MutableRefObject } from 'react';
import { useNavigate, useParams } from 'react-router-dom';
import { Card, Tag, Button, Space, message, Typography, theme, Row, Col } from 'antd';
import { ArrowLeftOutlined } from '@ant-design/icons';
import axios from 'axios';
import type { Node, Edge } from '@xyflow/react';
import {
  buildCareerNameMap,
  buildEdgeCategoryOptions,
  getEdgeCategory,
} from '../components/relationship-graph/selectors';
import {
  GROUP_MAIN_CAREER_NODE_ID,
  GROUP_SUB_CAREER_NODE_ID,
  type CareerItem,
  type CareerListResponse,
  type CharacterDetail,
  type CharacterListResponse,
  type GraphData,
  type RelationshipGraphThemeToken,
  type RelationshipType,
} from '../components/relationship-graph/types';
import WorkflowEntryFallback from '../components/WorkflowEntryFallback';
import { designDisplayFont } from '../theme/themeConfig';

const { Text, Title, Paragraph } = Typography;
const RelationshipGraphCanvas = lazy(() => import('../components/relationship-graph/RelationshipGraphCanvas'));
const RelationshipGraphDetailPanel = lazy(() => import('../components/relationship-graph/RelationshipGraphDetailPanel'));

export default function RelationshipGraph() {
  const { projectId } = useParams<{ projectId: string }>();
  const navigate = useNavigate();
  const { token } = theme.useToken();

  const graphTheme = useMemo<RelationshipGraphThemeToken>(
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
  const mountedRef = useRef(false);
  const relationshipTypesRequestIdRef = useRef(0);
  const graphRequestIdRef = useRef(0);
  const detailRequestIdRef = useRef(0);

  const careerNameMap = useMemo(
    () => buildCareerNameMap(mainCareers, subCareers),
    [mainCareers, subCareers],
  );

  const edgeCategoryOptions = useMemo(
    () => buildEdgeCategoryOptions(edges, graphTheme),
    [edges, graphTheme],
  );

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

  const applyGraphBuildResult = useCallback((buildResult: {
    mainCareers: CareerItem[];
    subCareers: CareerItem[];
    characterDetailMap: Record<string, CharacterDetail>;
    nodes: Node[];
    edges: Edge[];
    graphData: GraphData;
  }) => {
    setMainCareers(buildResult.mainCareers);
    setSubCareers(buildResult.subCareers);
    setCharacterDetailMap(buildResult.characterDetailMap);
    setNodes(buildResult.nodes);
    setEdges(buildResult.edges);
    setGraphData(buildResult.graphData);
  }, []);

  const beginRequest = (requestRef: MutableRefObject<number>) => {
    const nextRequestId = requestRef.current + 1;
    requestRef.current = nextRequestId;
    return nextRequestId;
  };

  const isRequestActive = (requestRef: MutableRefObject<number>, requestId: number) => (
    mountedRef.current && requestRef.current === requestId
  );

  useEffect(() => {
    mountedRef.current = true;

    return () => {
      mountedRef.current = false;
      relationshipTypesRequestIdRef.current += 1;
      graphRequestIdRef.current += 1;
      detailRequestIdRef.current += 1;
    };
  }, []);

  useEffect(() => {
    if (projectId) {
      void loadRelationshipTypes();
    }
  }, [projectId]);

  const loadRelationshipTypes = async () => {
    const requestId = beginRequest(relationshipTypesRequestIdRef);

    try {
      const res = await axios.get('/api/relationships/types');
      if (!isRequestActive(relationshipTypesRequestIdRef, requestId)) {
        return;
      }
      setRelationshipTypes(res.data || []);
    } catch (error) {
      if (!isRequestActive(relationshipTypesRequestIdRef, requestId)) {
        return;
      }
      console.error('加载关系类型失败', error);
    }
  };

  const loadGraphData = useCallback(async () => {
    if (!projectId || relationshipTypes.length === 0) return;

    const requestId = beginRequest(graphRequestIdRef);
    setLoading(true);
    try {
      const auxiliaryDataPromise = Promise.allSettled([
        axios.get('/api/characters', { params: { project_id: projectId } }),
        axios.get('/api/careers', { params: { project_id: projectId } }),
      ]);

      const [graphRes, layoutModule, graphBuilderModule] = await Promise.all([
        axios.get(`/api/relationships/graph/${projectId}`),
        import('../components/relationship-graph/layout'),
        import('../components/relationship-graph/buildGraph'),
      ]);

      if (!isRequestActive(graphRequestIdRef, requestId)) {
        return;
      }

      const data = graphRes.data as GraphData;
      const buildGraph = graphBuilderModule.buildRelationshipGraph;
      applyGraphBuildResult(buildGraph({
        projectId,
        graphData: data,
        characters: [],
        careersData: {},
        relationshipTypes,
        token: graphTheme,
        getLayoutedElements: layoutModule.getLayoutedElements,
      }));

      const [charactersResult, careersResult] = await auxiliaryDataPromise;

      if (!isRequestActive(graphRequestIdRef, requestId)) {
        return;
      }

      const characters = charactersResult.status === 'fulfilled'
        ? ((charactersResult.value.data as CharacterListResponse)?.items || [])
        : [];
      const careersData = careersResult.status === 'fulfilled'
        ? ((careersResult.value.data as CareerListResponse) || {})
        : {};

      if (charactersResult.status === 'rejected') {
        console.error('加载关系图谱角色详情失败，已降级为空数据:', charactersResult.reason);
      }

      if (careersResult.status === 'rejected') {
        console.error('加载关系图谱职业数据失败，已降级为空数据:', careersResult.reason);
      }

      applyGraphBuildResult(buildGraph({
        projectId,
        graphData: data,
        characters,
        careersData,
        relationshipTypes,
        token: graphTheme,
        getLayoutedElements: layoutModule.getLayoutedElements,
      }));
    } catch (error) {
      if (!isRequestActive(graphRequestIdRef, requestId)) {
        return;
      }
      message.error('加载关系图谱失败');
      console.error(error);
    } finally {
      if (isRequestActive(graphRequestIdRef, requestId)) {
        setLoading(false);
      }
    }
  }, [applyGraphBuildResult, projectId, relationshipTypes, graphTheme]);

  // 当 relationshipTypes 变化时重新加载图谱数据
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
      detailRequestIdRef.current += 1;
      setNodeDetail(cached);
      return;
    }

    const requestId = beginRequest(detailRequestIdRef);
    setDetailLoading(true);
    try {
      const res = await axios.get(`/api/characters/${nodeId}`);
      if (!isRequestActive(detailRequestIdRef, requestId)) {
        return;
      }
      setNodeDetail(res.data as CharacterDetail);
    } catch (error) {
      if (!isRequestActive(detailRequestIdRef, requestId)) {
        return;
      }
      message.error('加载详情失败');
      console.error(error);
    } finally {
      if (isRequestActive(detailRequestIdRef, requestId)) {
        setDetailLoading(false);
      }
    }
  };

  const handleNodeClick = (_: unknown, node: { id: string }) => {
    detailRequestIdRef.current += 1;
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
    detailRequestIdRef.current += 1;
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

  const editorialInk = token.colorText;
  const heroBackground = `linear-gradient(135deg, #171411 0%, color-mix(in srgb, #171411 68%, ${token.colorPrimary} 32%) 100%)`;
  const panelBackground = `linear-gradient(180deg, color-mix(in srgb, ${token.colorBgContainer} 96%, ${token.colorPrimary} 4%) 0%, color-mix(in srgb, ${token.colorBgContainer} 92%, ${token.colorWarning} 8%) 100%)`;
  const quietPanelBackground = `linear-gradient(180deg, color-mix(in srgb, ${token.colorBgContainer} 98%, ${token.colorBgLayout} 2%) 0%, color-mix(in srgb, ${token.colorBgContainer} 92%, ${token.colorBgLayout} 8%) 100%)`;
  const panelBorder = `1px solid color-mix(in srgb, ${token.colorPrimary} 12%, ${token.colorBorder} 88%)`;
  const outlineButtonStyle = {
    borderRadius: 999,
    background: 'color-mix(in srgb, var(--ant-color-bg-container) 14%, transparent)',
    border: '1px solid color-mix(in srgb, var(--ant-color-bg-container) 20%, transparent)',
    color: editorialInk,
    boxShadow: `0 10px 18px color-mix(in srgb, ${token.colorText} 18%, transparent)`,
    backdropFilter: 'blur(8px)',
  } as const;
  return (
    <div
      style={{
        minHeight: '100%',
        display: 'flex',
        flexDirection: 'column',
        backgroundColor: token.colorBgLayout,
        overflow: 'visible',
        gap: 16,
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
        styles={{ body: { padding: 24 } }}
      >
        <div style={{ position: 'absolute', top: -56, right: -40, width: 180, height: 180, borderRadius: '50%', background: 'rgba(255,255,255,0.08)', pointerEvents: 'none' }} />
        <div style={{ position: 'absolute', bottom: -30, left: '24%', width: 110, height: 110, borderRadius: '50%', background: 'rgba(255,255,255,0.05)', pointerEvents: 'none' }} />
        <Row gutter={[24, 18]} align="middle" style={{ position: 'relative', zIndex: 1 }}>
          <Col xs={24} lg={15}>
            <Space direction="vertical" size={8} style={{ width: '100%' }}>
              <Text style={{ color: 'rgba(255,255,255,0.72)', fontSize: 11, letterSpacing: '0.18em', textTransform: 'uppercase' }}>
                Relationship Atlas
              </Text>
              <Title level={2} style={{ margin: 0, color: editorialInk, fontFamily: designDisplayFont, letterSpacing: '-0.03em' }}>
                关系图谱
              </Title>
              <Paragraph style={{ margin: 0, color: 'rgba(255,255,255,0.82)', fontSize: 15, lineHeight: 1.8 }}>
                把角色、组织与职业体系放进同一张关系地图里浏览。这里更强调可读性和筛选感，让你既能俯瞰结构，又能从节点侧栏回到具体人物与组织设定。
              </Paragraph>
            </Space>
          </Col>
          <Col xs={24} lg={9}>
            <Space direction="vertical" size={12} style={{ width: '100%' }}>
              {[
                { label: '节点数', value: `${graphData?.nodes?.length || 0}` },
                { label: '关系数', value: `${graphData?.links?.length || 0}` },
                { label: '已选节点', value: selectedNodeId ? '查看中' : '未选择' },
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
          <Button icon={<ArrowLeftOutlined />} onClick={goBack} style={outlineButtonStyle}>
            返回
          </Button>
          <Tag color="processing" style={{ marginInlineEnd: 0, borderRadius: 999, paddingInline: 12, lineHeight: '28px' }}>
            {graphData?.nodes?.length || 0} 节点 / {graphData?.links?.length || 0} 关系
          </Tag>
        </Space>
      </Card>

      <Card
        variant="borderless"
        style={{
          flex: '1 1 auto',
          minHeight: 640,
          display: 'flex',
          flexDirection: 'column',
          background: panelBackground,
          borderRadius: 24,
          border: panelBorder,
          boxShadow: `0 18px 36px color-mix(in srgb, ${token.colorText} 8%, transparent)`,
        }}
        styles={{
          body: {
            flex: 1,
            minHeight: 600,
            display: 'flex',
            flexDirection: 'column',
            overflow: 'visible',
            padding: 16,
          }
        }}
      >
        <div style={{ display: 'flex', flexDirection: 'column', gap: 16, width: '100%', flex: 1, minHeight: 600 }}>
          <Card
            variant="borderless"
            style={{
              borderRadius: 20,
              background: quietPanelBackground,
              border: `1px solid ${token.colorBorderSecondary}`,
              flexShrink: 0,
            }}
            styles={{ body: { padding: 16 } }}
          >
            <Space direction="vertical" size={12} style={{ width: '100%' }}>
              <div style={{ display: 'flex', justifyContent: 'space-between', gap: 16, flexWrap: 'wrap' }}>
                <div style={{ display: 'flex', alignItems: 'center', gap: 14, fontSize: 12, flexWrap: 'wrap' }}>
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
                </div>
                <div style={{ display: 'flex', alignItems: 'center', gap: 14, fontSize: 12, flexWrap: 'wrap' }}>
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
              </div>

              {edgeCategoryOptions.length > 0 && (
                <div style={{ display: 'flex', alignItems: 'center', gap: 8, flexWrap: 'wrap' }}>
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
                            ? { backgroundColor: option.color, borderColor: option.color, color: token.colorWhite, borderRadius: 999 }
                            : { color: token.colorTextSecondary, borderRadius: 999 }
                        }
                      >
                        {option.label}（{option.count}）
                      </Button>
                    );
                  })}
                </div>
              )}
            </Space>
          </Card>

          <Card
            variant="borderless"
            style={{
              flex: '1 1 360px',
              minHeight: 520,
              display: 'flex',
              flexDirection: 'column',
              overflow: 'hidden',
              borderRadius: 20,
              background: quietPanelBackground,
              border: `1px solid ${token.colorBorderSecondary}`,
            }}
            styles={{
              body: {
                flex: 1,
                minHeight: 500,
                display: 'flex',
                overflow: 'hidden',
                padding: 12,
              }
            }}
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
        </div>
      </Card>

      {selectedNodeId ? (
        <Suspense
          fallback={(
            <WorkflowEntryFallback
              eyebrow="Graph Detail"
              title="正在展开关系图谱详情侧板"
              message="系统正在恢复节点详情、职业标签与关闭入口，原有选点、详情计算和侧板逻辑保持不变。"
              tags={[
                { label: '节点详情', color: 'geekblue' },
                { label: '侧板恢复中', color: 'processing' },
                { label: '图谱逻辑保持原样', color: 'green' },
              ]}
            />
          )}
        >
          <RelationshipGraphDetailPanel
            selectedNodeId={selectedNodeId}
            nodeDetail={nodeDetail}
            careerNameMap={careerNameMap}
            onClose={handleCloseDetail}
          />
        </Suspense>
      ) : null}
    </div>
  );
}
