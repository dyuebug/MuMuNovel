import { Suspense, lazy, useState, useEffect, useCallback, useMemo } from 'react';
import { useNavigate, useParams } from 'react-router-dom';
import { Card, Tag, Button, Space, message, Typography, theme } from 'antd';
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

const { Text } = Typography;
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

      {selectedNodeId ? (
        <Suspense fallback={null}>
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
