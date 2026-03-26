import dagre from 'dagre';
import type { Edge, Node } from '@xyflow/react';

const GROUP_MAIN_CAREER_NODE_ID = '__career_group_main__';
const GROUP_SUB_CAREER_NODE_ID = '__career_group_sub__';
const MAIN_GRAPH_FIXED_X_GAP = 220;
const MAIN_GRAPH_FIXED_Y_GAP = 180;
const MAIN_GRAPH_MAX_PER_ROW = 6;
const MAIN_GRAPH_GROUP_Y_GAP = 140;

const getNodeSize = (node: Node) => {
  const width =
    typeof node.style?.width === 'number'
      ? node.style.width
      : Number(node.style?.width ?? 140) || 140;
  const height =
    typeof node.style?.height === 'number'
      ? node.style.height
      : Number(node.style?.height ?? 60) || 60;

  return { width, height };
};

const layoutNodesInWrappedRows = (
  rowNodes: Node[],
  startX: number,
  startY: number,
  maxPerRow: number,
  columnGap: number,
  rowGap: number,
): Node[] => {
  if (rowNodes.length === 0) {
    return [];
  }

  const sorted = [...rowNodes].sort((a, b) => a.position.x - b.position.x);

  return sorted.map((node, index) => {
    const col = index % maxPerRow;
    const row = Math.floor(index / maxPerRow);
    return {
      ...node,
      position: {
        ...node.position,
        x: startX + col * columnGap,
        y: startY + row * rowGap,
      },
    };
  });
};

export const getLayoutedElements = (nodes: Node[], edges: Edge[]) => {
  const dagreGraph = new dagre.graphlib.Graph();
  dagreGraph.setDefaultEdgeLabel(() => ({}));
  dagreGraph.setGraph({ rankdir: 'TB', nodesep: 160, ranksep: 180, edgesep: 80, marginx: 40, marginy: 40 });

  const careerNodeIds = new Set(
    nodes.filter((node) => node.id.startsWith('career-') || node.id.startsWith('__career_group')).map((node) => node.id),
  );

  const layoutNodes = nodes.filter((node) => !careerNodeIds.has(node.id));
  const careerNodes = nodes.filter((node) => careerNodeIds.has(node.id));

  layoutNodes.forEach((node) => {
    const { width, height } = getNodeSize(node);
    dagreGraph.setNode(node.id, { width, height });
  });

  dagreGraph.setNode('__dummy_root', { width: 1, height: 1 });
  layoutNodes.forEach((node) => {
    if (node.data?.type === 'organization') {
      dagreGraph.setEdge('__dummy_root', node.id, { weight: 100, minlen: 1 });
      return;
    }
    dagreGraph.setEdge('__dummy_root', node.id, { weight: 1, minlen: 2 });
  });

  edges.forEach((edge) => {
    if (!careerNodeIds.has(edge.source) && !careerNodeIds.has(edge.target)) {
      dagreGraph.setEdge(edge.source, edge.target, {
        weight: edge.data?.layoutWeight ?? 1,
        minlen: 1,
      });
    }
  });

  dagre.layout(dagreGraph);

  const layoutedNodes = layoutNodes.map((node) => {
    const nodeWithPosition = dagreGraph.node(node.id);
    const { width, height } = getNodeSize(node);

    return {
      ...node,
      position: {
        x: nodeWithPosition.x - width / 2,
        y: nodeWithPosition.y - height / 2,
      },
    };
  });

  const organizationNodes = layoutedNodes.filter((node) => node.data?.type === 'organization');
  const characterNodes = layoutedNodes.filter((node) => node.data?.type !== 'organization');

  const baseStartX = layoutedNodes.reduce(
    (min, node) => (node.position.x < min ? node.position.x : min),
    Infinity,
  );
  const alignedStartX = Number.isFinite(baseStartX) ? baseStartX : 0;

  const orgStartYRaw = organizationNodes.reduce(
    (min, node) => (node.position.y < min ? node.position.y : min),
    Infinity,
  );
  const orgStartY = Number.isFinite(orgStartYRaw) ? orgStartYRaw : 0;

  const orgRows = Math.ceil(organizationNodes.length / MAIN_GRAPH_MAX_PER_ROW);
  const orgMaxHeight = organizationNodes.reduce(
    (max, node) => Math.max(max, getNodeSize(node).height),
    0,
  );
  const organizationBottomY =
    orgStartY +
    Math.max(orgRows - 1, 0) * MAIN_GRAPH_FIXED_Y_GAP +
    orgMaxHeight;

  const characterStartYRaw = characterNodes.reduce(
    (min, node) => (node.position.y < min ? node.position.y : min),
    Infinity,
  );
  const characterStartYBase = Number.isFinite(characterStartYRaw)
    ? characterStartYRaw
    : organizationBottomY + MAIN_GRAPH_GROUP_Y_GAP;
  const characterStartY = Math.max(characterStartYBase, organizationBottomY + MAIN_GRAPH_GROUP_Y_GAP);

  const wrappedOrganizations = layoutNodesInWrappedRows(
    organizationNodes,
    alignedStartX,
    orgStartY,
    MAIN_GRAPH_MAX_PER_ROW,
    MAIN_GRAPH_FIXED_X_GAP,
    MAIN_GRAPH_FIXED_Y_GAP,
  );

  const wrappedCharacters = layoutNodesInWrappedRows(
    characterNodes,
    alignedStartX,
    characterStartY,
    MAIN_GRAPH_MAX_PER_ROW,
    MAIN_GRAPH_FIXED_X_GAP,
    MAIN_GRAPH_FIXED_Y_GAP,
  );

  const normalizedMap = new Map<string, Node>(
    [...wrappedOrganizations, ...wrappedCharacters].map((node) => [node.id, node]),
  );

  const normalizedLayoutedNodes = layoutedNodes.map((node) => normalizedMap.get(node.id) || node);

  const { minX, minY } = normalizedLayoutedNodes.reduce(
    (acc, node) => ({
      minX: Math.min(acc.minX, node.position.x),
      minY: Math.min(acc.minY, node.position.y),
    }),
    { minX: Infinity, minY: Infinity },
  );

  const safeMinX = Number.isFinite(minX) ? minX : 0;
  const safeMinY = Number.isFinite(minY) ? minY : 0;
  const careerStartX = safeMinX - 460;
  let currentY = safeMinY;

  const placedCareerNodes: Node[] = [];
  const placeNode = (nodeId: string, xOffset = 0) => {
    const node = careerNodes.find((candidate) => candidate.id === nodeId);
    if (!node) {
      return;
    }

    placedCareerNodes.push({
      ...node,
      position: { x: careerStartX + xOffset, y: currentY },
    });
    const { height } = getNodeSize(node);
    currentY += height + 30;
  };

  placeNode(GROUP_MAIN_CAREER_NODE_ID, -180);
  careerNodes.filter((node) => node.data?.type === 'career_main').forEach((node) => placeNode(node.id));

  currentY += 20;

  placeNode(GROUP_SUB_CAREER_NODE_ID, -180);
  careerNodes.filter((node) => node.data?.type === 'career_sub').forEach((node) => placeNode(node.id));

  return { nodes: [...normalizedLayoutedNodes, ...placedCareerNodes], edges };
};
