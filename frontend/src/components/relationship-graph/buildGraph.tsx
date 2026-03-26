import { ApartmentOutlined, UserOutlined } from '@ant-design/icons';
import type { CSSProperties } from 'react';
import type { Edge, Node } from '@xyflow/react';

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


export interface BuildGraphThemeToken {
  colorBgContainer: string;
  colorBorder: string;
  colorError: string;
  colorFillSecondary: string;
  colorInfo: string;
  colorPrimary: string;
  colorSuccess: string;
  colorText: string;
  colorTextBase: string;
  colorTextSecondary: string;
  colorTextTertiary: string;
  colorWarning: string;
}

interface BuildFlowEdgeOptions {
  dashed?: boolean;
  animated?: boolean;
  layoutWeight?: number;
}

interface BuildRelationshipGraphInput {
  projectId: string;
  graphData: GraphData;
  characters: CharacterDetail[];
  careersData: CareerListResponse;
  relationshipTypes: RelationshipType[];
  token: BuildGraphThemeToken;
  getLayoutedElements: (nodes: Node[], edges: Edge[]) => { nodes: Node[]; edges: Edge[] };
}

interface BuildRelationshipGraphResult {
  graphData: GraphData;
  characterDetailMap: Record<string, CharacterDetail>;
  mainCareers: CareerItem[];
  subCareers: CareerItem[];
  nodes: Node[];
  edges: Edge[];
}

const GROUP_MAIN_CAREER_NODE_ID = '__career_group_main__';
const GROUP_SUB_CAREER_NODE_ID = '__career_group_sub__';
const graphBuildCache = new Map<string, BuildRelationshipGraphResult>();

const alphaColor = (color: string, alpha: number) =>
  `color-mix(in srgb, ${color} ${(alpha * 100).toFixed(0)}%, transparent)`;

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

const getCategoryColor = (
  relationshipName: string,
  isActive: boolean,
  relationshipTypes: RelationshipType[],
  token: BuildGraphThemeToken,
) => {
  const inactiveColor = token.colorBorder;

  if (relationshipName.startsWith('?????')) {
    return isActive ? token.colorPrimary : inactiveColor;
  }

  if (relationshipName.startsWith('????')) {
    return isActive ? token.colorWarning : inactiveColor;
  }

  if (relationshipName.startsWith('????')) {
    return isActive ? token.colorInfo : inactiveColor;
  }

  if (relationshipName.startsWith('?????')) {
    return isActive ? token.colorTextTertiary : inactiveColor;
  }

  const relType = relationshipTypes.find((item) => item.name === relationshipName);
  const category = relType?.category || 'default';
  const categoryColors: Record<string, string> = {
    family: token.colorWarning,
    hostile: token.colorError,
    professional: token.colorInfo,
    social: token.colorSuccess,
    default: token.colorTextTertiary,
  };

  return isActive ? (categoryColors[category] || categoryColors.default) : inactiveColor;
};

const getCharacterNodeStyle = (roleType: string): CSSProperties => {
  const roleColorMap: Record<string, string> = {
    protagonist: 'var(--ant-color-error)',
    antagonist: 'var(--ant-color-primary)',
    supporting: 'var(--ant-color-info)',
  };

  const baseColor = roleColorMap[roleType] || 'var(--ant-color-info)';

  return {
    width: 130,
    height: 130,
    border: `2px solid ${baseColor}`,
    borderRadius: '50%',
    background: `linear-gradient(135deg, var(--ant-color-bg-container), color-mix(in srgb, ${baseColor} 12%, var(--ant-color-bg-container)))`,
    boxShadow: `0 4px 16px color-mix(in srgb, ${baseColor} 25%, transparent)`,
    display: 'flex',
    alignItems: 'center',
    justifyContent: 'center',
    padding: 0,
    transition: 'all 0.3s ease',
  };
};

const getOrganizationNodeStyle = (): CSSProperties => ({
  width: 160,
  height: 90,
  border: '2px solid var(--ant-color-success)',
  borderRadius: 12,
  background: 'linear-gradient(135deg, var(--ant-color-bg-container), color-mix(in srgb, var(--ant-color-success) 12%, var(--ant-color-bg-container)))',
  boxShadow: '0 4px 16px color-mix(in srgb, var(--ant-color-success) 15%, transparent)',
  display: 'flex',
  alignItems: 'center',
  justifyContent: 'center',
  padding: 0,
  transition: 'all 0.3s ease',
});

const getCareerNodeStyle = (type: 'main' | 'sub'): CSSProperties => {
  const color = type === 'main' ? 'var(--ant-color-warning)' : 'var(--ant-color-info)';

  return {
    width: 150,
    height: 72,
    border: `2px solid ${color}`,
    borderRadius: 12,
    background: `linear-gradient(135deg, var(--ant-color-bg-container), color-mix(in srgb, ${color} 12%, var(--ant-color-bg-container)))`,
    boxShadow: `0 4px 12px color-mix(in srgb, ${color} 20%, transparent)`,
    display: 'flex',
    alignItems: 'center',
    justifyContent: 'center',
    padding: 0,
    transition: 'all 0.3s ease',
  };
};

const getCareerGroupStyle = (type: 'main' | 'sub'): CSSProperties => {
  const color = type === 'main' ? 'var(--ant-color-warning)' : 'var(--ant-color-info)';

  return {
    width: 130,
    height: 52,
    border: `2px dashed ${color}`,
    borderRadius: 26,
    backgroundColor: 'var(--ant-color-bg-container)',
    boxShadow: `0 2px 8px color-mix(in srgb, ${color} 15%, transparent)`,
    display: 'flex',
    alignItems: 'center',
    justifyContent: 'center',
    fontWeight: 600,
    fontSize: 13,
    color,
    padding: 0,
  };
};

const buildFlowEdge = (
  edgeId: string,
  source: string,
  target: string,
  relationship: string,
  status: string,
  intimacy: number,
  relationshipTypes: RelationshipType[],
  token: BuildGraphThemeToken,
  opts?: BuildFlowEdgeOptions,
): Edge => {
  const edgeColor = getCategoryColor(relationship, status === 'active', relationshipTypes, token);
  const isOrgMemberLink = relationship.startsWith('?????');
  const isCareerMainLink = relationship.startsWith('????');
  const isCareerSubLink = relationship.startsWith('????');
  const isCareerClassLink = relationship.startsWith('?????');

  return {
    id: edgeId,
    source,
    target,
    label: relationship,
    type: 'smoothstep',
    animated: opts?.animated,
    style: {
      stroke: edgeColor,
      strokeWidth: isCareerClassLink ? 1.5 : 2,
      strokeDasharray: opts?.dashed || isOrgMemberLink || isCareerSubLink ? '6 3' : undefined,
      opacity: isCareerClassLink ? 0.5 : isCareerMainLink || isCareerSubLink ? 0.6 : 1,
    },
    labelStyle: {
      fill: token.colorTextSecondary,
      fontSize: 10,
      fontWeight: isCareerMainLink || isCareerSubLink ? 600 : 500,
    },
    labelBgStyle: {
      fill: token.colorBgContainer,
      fillOpacity: 0.9,
    },
    markerEnd: {
      type: 'arrowclosed',
      color: edgeColor,
    },
    data: {
      intimacy,
      status,
      layoutWeight: opts?.layoutWeight ?? 1,
      category: isOrgMemberLink
        ? 'organization'
        : isCareerMainLink
          ? 'career_main'
          : isCareerSubLink
            ? 'career_sub'
            : isCareerClassLink
              ? 'career_group'
              : relationshipTypes.find((item) => item.name === relationship)?.category || 'social',
    },
  };
};

const cloneNode = (node: Node): Node => ({
  ...node,
  position: { ...node.position },
  data: node.data ? { ...node.data } : node.data,
  style: node.style ? { ...node.style } : node.style,
});

const cloneEdge = (edge: Edge): Edge => ({
  ...edge,
  data: edge.data ? { ...edge.data } : edge.data,
  style: edge.style ? { ...edge.style } : edge.style,
  labelStyle: edge.labelStyle ? { ...edge.labelStyle } : edge.labelStyle,
  labelBgStyle: edge.labelBgStyle ? { ...edge.labelBgStyle } : edge.labelBgStyle,
  markerEnd:
    edge.markerEnd && typeof edge.markerEnd === 'object'
      ? { ...edge.markerEnd }
      : edge.markerEnd,
});

const cloneBuildResult = (result: BuildRelationshipGraphResult): BuildRelationshipGraphResult => ({
  graphData: {
    nodes: result.graphData.nodes.map((node) => ({ ...node })),
    links: result.graphData.links.map((link) => ({ ...link })),
  },
  characterDetailMap: Object.fromEntries(
    Object.entries(result.characterDetailMap).map(([key, value]) => [key, { ...value }]),
  ),
  mainCareers: result.mainCareers.map((item) => ({ ...item })),
  subCareers: result.subCareers.map((item) => ({ ...item })),
  nodes: result.nodes.map(cloneNode),
  edges: result.edges.map(cloneEdge),
});

const buildCacheKey = ({
  projectId,
  graphData,
  characters,
  careersData,
  relationshipTypes,
  token,
}: Omit<BuildRelationshipGraphInput, 'getLayoutedElements'>) =>
  JSON.stringify({
    projectId,
    graphData,
    relationshipTypes: relationshipTypes.map((item) => ({
      id: item.id,
      name: item.name,
      category: item.category,
      reverse_name: item.reverse_name,
    })),
    characters: characters.map((item) => ({
      id: item.id,
      name: item.name,
      is_organization: item.is_organization,
      role_type: item.role_type,
      avatar_url: item.avatar_url,
      organization_type: item.organization_type,
      main_career_id: item.main_career_id,
      main_career_stage: item.main_career_stage,
      sub_careers: safeParseSubCareers(item.sub_careers),
    })),
    careersData,
    token,
  });

export const buildRelationshipGraph = ({
  projectId,
  graphData,
  characters,
  careersData,
  relationshipTypes,
  token,
  getLayoutedElements,
}: BuildRelationshipGraphInput): BuildRelationshipGraphResult => {
  const cacheKey = buildCacheKey({
    projectId,
    graphData,
    characters,
    careersData,
    relationshipTypes,
    token,
  });
  const cached = graphBuildCache.get(cacheKey);
  if (cached) {
    return cloneBuildResult(cached);
  }

  const mainCareers = careersData.main_careers || [];
  const subCareers = careersData.sub_careers || [];
  const detailMap: Record<string, CharacterDetail> = {};
  characters.forEach((item) => {
    detailMap[item.id] = item;
  });

  const roleColorMap: Record<string, string> = {
    protagonist: token.colorError,
    antagonist: token.colorPrimary,
    supporting: token.colorInfo,
  };

  const baseNodes: Node[] = graphData.nodes.map((node) => {
    const style = node.type === 'organization' ? getOrganizationNodeStyle() : getCharacterNodeStyle(node.role_type);
    const detail = detailMap[node.id];
    const baseColor = roleColorMap[node.role_type] || token.colorInfo;

    const labelContent = node.type === 'organization' ? (
      <div style={{ display: 'flex', flexDirection: 'column', alignItems: 'center', justifyContent: 'center', width: '100%', height: '100%' }}>
        <ApartmentOutlined style={{ fontSize: 24, color: token.colorSuccess, marginBottom: 4 }} />
        <div style={{ fontWeight: 600, fontSize: 14, color: token.colorText, maxWidth: '90%', overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>{node.name}</div>
        <div style={{ fontSize: 11, color: token.colorTextSecondary, marginTop: 2 }}>{detail?.organization_type || '??'}</div>
      </div>
    ) : (
      <div style={{ display: 'flex', flexDirection: 'column', alignItems: 'center', justifyContent: 'center', width: '100%', height: '100%' }}>
        {detail?.avatar_url ? (
          <img src={detail.avatar_url} alt={node.name} style={{ width: 56, height: 56, borderRadius: '50%', objectFit: 'cover', border: `2px solid ${token.colorBgContainer}`, boxShadow: `0 2px 6px ${alphaColor(token.colorTextBase, 0.18)}`, marginBottom: 6 }} />
        ) : (
          <div style={{ width: 56, height: 56, borderRadius: '50%', backgroundColor: token.colorFillSecondary, display: 'flex', alignItems: 'center', justifyContent: 'center', border: `2px solid ${token.colorBgContainer}`, boxShadow: `0 2px 6px ${alphaColor(token.colorTextBase, 0.18)}`, marginBottom: 6 }}>
            <UserOutlined style={{ fontSize: 28, color: baseColor }} />
          </div>
        )}
        <div style={{ fontWeight: 600, fontSize: 13, color: token.colorText, maxWidth: '90%', overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>{node.name}</div>
        <div style={{ fontSize: 11, color: baseColor, marginTop: 2, transform: 'scale(0.9)' }}>
          {node.role_type === 'protagonist' ? '??' : node.role_type === 'antagonist' ? '??' : '??'}
        </div>
      </div>
    );

    return {
      id: node.id,
      type: 'default',
      position: { x: 0, y: 0 },
      data: {
        label: labelContent,
        type: node.type,
        role_type: node.role_type,
      },
      style,
    };
  });

  const mainCareerNodes: Node[] = mainCareers.map((career) => ({
    id: `career-main-${career.id}`,
    type: 'default',
    position: { x: 0, y: 0 },
    data: {
      label: (
        <div style={{ display: 'flex', flexDirection: 'column', alignItems: 'center', justifyContent: 'center' }}>
          <div style={{ fontSize: 11, color: token.colorWarning, marginBottom: 2 }}>???</div>
          <div style={{ fontWeight: 600, fontSize: 13, color: token.colorText }}>{career.name}</div>
        </div>
      ),
      type: 'career_main',
    },
    style: getCareerNodeStyle('main'),
  }));

  const subCareerNodes: Node[] = subCareers.map((career) => ({
    id: `career-sub-${career.id}`,
    type: 'default',
    position: { x: 0, y: 0 },
    data: {
      label: (
        <div style={{ display: 'flex', flexDirection: 'column', alignItems: 'center', justifyContent: 'center' }}>
          <div style={{ fontSize: 11, color: token.colorInfo, marginBottom: 2 }}>???</div>
          <div style={{ fontWeight: 600, fontSize: 13, color: token.colorText }}>{career.name}</div>
        </div>
      ),
      type: 'career_sub',
    },
    style: getCareerNodeStyle('sub'),
  }));

  const careerGroupNodes: Node[] = [];
  if (mainCareerNodes.length > 0) {
    careerGroupNodes.push({
      id: GROUP_MAIN_CAREER_NODE_ID,
      type: 'default',
      position: { x: 0, y: 0 },
      data: {
        label: '?????',
        type: 'career_group',
      },
      style: getCareerGroupStyle('main'),
    });
  }
  if (subCareerNodes.length > 0) {
    careerGroupNodes.push({
      id: GROUP_SUB_CAREER_NODE_ID,
      type: 'default',
      position: { x: 0, y: 0 },
      data: {
        label: '?????',
        type: 'career_group',
      },
      style: getCareerGroupStyle('sub'),
    });
  }

  const allNodes: Node[] = [...baseNodes, ...careerGroupNodes, ...mainCareerNodes, ...subCareerNodes];
  const orgMemberLinks = graphData.links.filter((link) => link.relationship.startsWith('?????'));
  const memberRelationLinks = graphData.links.filter((link) => !link.relationship.startsWith('?????'));
  const orgMemberEdges: Edge[] = orgMemberLinks.map((link) =>
    buildFlowEdge(
      `${link.source}-${link.target}-${link.relationship}`,
      link.source,
      link.target,
      link.relationship,
      link.status,
      link.intimacy,
      relationshipTypes,
      token,
      { layoutWeight: 8 },
    ),
  );

  const careerGroupEdges: Edge[] = [
    ...mainCareerNodes.map((node) =>
      buildFlowEdge(
        `${GROUP_MAIN_CAREER_NODE_ID}-${node.id}`,
        GROUP_MAIN_CAREER_NODE_ID,
        node.id,
        '????????',
        'active',
        0,
        relationshipTypes,
        token,
        { dashed: true, layoutWeight: 4 },
      ),
    ),
    ...subCareerNodes.map((node) =>
      buildFlowEdge(
        `${GROUP_SUB_CAREER_NODE_ID}-${node.id}`,
        GROUP_SUB_CAREER_NODE_ID,
        node.id,
        '????????',
        'active',
        0,
        relationshipTypes,
        token,
        { dashed: true, layoutWeight: 4 },
      ),
    ),
  ];

  const mainCareerNodeIds = new Set(mainCareerNodes.map((node) => node.id));
  const subCareerNodeIds = new Set(subCareerNodes.map((node) => node.id));
  const careerNameMap: Record<string, string> = {};
  [...mainCareers, ...subCareers].forEach((career) => {
    careerNameMap[career.id] = career.name;
  });

  const careerToCharacterEdges: Edge[] = [];
  characters
    .filter((character) => !character.is_organization)
    .forEach((character) => {
      if (character.main_career_id) {
        const careerNodeId = `career-main-${character.main_career_id}`;
        if (mainCareerNodeIds.has(careerNodeId)) {
          const careerName = careerNameMap[character.main_career_id] || '????';
          careerToCharacterEdges.push(
            buildFlowEdge(
              `${careerNodeId}-${character.id}-main`,
              careerNodeId,
              character.id,
              `????${careerName}`,
              'active',
              100,
              relationshipTypes,
              token,
              { layoutWeight: 3 },
            ),
          );
        }
      }

      safeParseSubCareers(character.sub_careers).forEach((sub) => {
        const careerNodeId = `career-sub-${sub.career_id}`;
        if (subCareerNodeIds.has(careerNodeId)) {
          const careerName = careerNameMap[sub.career_id] || '?????';
          careerToCharacterEdges.push(
            buildFlowEdge(
              `${careerNodeId}-${character.id}-sub-${sub.stage || 1}`,
              careerNodeId,
              character.id,
              `????${careerName}`,
              'active',
              80,
              relationshipTypes,
              token,
              { dashed: true, layoutWeight: 2 },
            ),
          );
        }
      });
    });

  const memberRelationEdges: Edge[] = memberRelationLinks.map((link) =>
    buildFlowEdge(
      `${link.source}-${link.target}-${link.relationship}`,
      link.source,
      link.target,
      link.relationship,
      link.status,
      link.intimacy,
      relationshipTypes,
      token,
      { layoutWeight: 1 },
    ),
  );

  const layoutEdges = [...orgMemberEdges, ...careerGroupEdges, ...careerToCharacterEdges];
  const fallbackLayoutEdges = layoutEdges.length > 0 ? layoutEdges : memberRelationEdges;
  const layouted = getLayoutedElements(allNodes, fallbackLayoutEdges);

  const result: BuildRelationshipGraphResult = {
    graphData,
    characterDetailMap: detailMap,
    mainCareers,
    subCareers,
    nodes: layouted.nodes,
    edges: [...orgMemberEdges, ...careerGroupEdges, ...careerToCharacterEdges, ...memberRelationEdges],
  };

  if (graphBuildCache.size >= 12) {
    graphBuildCache.clear();
  }
  graphBuildCache.set(cacheKey, result);
  return cloneBuildResult(result);
};
