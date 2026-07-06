import { memo, useEffect } from 'react';
import {
  Background,
  BackgroundVariant,
  Controls,
  ReactFlow,
  useEdgesState,
  useNodesState,
  type Edge,
  type Node,
  type NodeMouseHandler,
} from '@xyflow/react';
import '@xyflow/react/dist/style.css';

interface RelationshipGraphCanvasProps {
  nodes: Node[];
  edges: Edge[];
  onNodeClick: NodeMouseHandler<Node>;
}

function RelationshipGraphCanvas({ nodes, edges, onNodeClick }: RelationshipGraphCanvasProps) {
  const [flowNodes, setFlowNodes, onNodesChange] = useNodesState<Node>(nodes);
  const [flowEdges, setFlowEdges, onEdgesChange] = useEdgesState<Edge>(edges);

  useEffect(() => {
    setFlowNodes(nodes);
  }, [nodes, setFlowNodes]);

  useEffect(() => {
    setFlowEdges(edges);
  }, [edges, setFlowEdges]);

  return (
    <div
      style={{
        flex: 1,
        minHeight: 0,
        position: 'relative',
        overflow: 'hidden',
        borderRadius: 18,
        border: '1px solid color-mix(in srgb, var(--ant-color-border-secondary) 84%, transparent)',
        background: 'linear-gradient(180deg, color-mix(in srgb, var(--ant-color-bg-container) 96%, white 4%) 0%, color-mix(in srgb, var(--ant-color-fill-quaternary) 44%, var(--ant-color-bg-container) 56%) 100%)',
        boxShadow: 'inset 0 1px 0 rgba(255,255,255,0.65), 0 18px 40px color-mix(in srgb, var(--ant-color-text) 8%, transparent)',
      }}
      className="relationship-graph-flow"
    >
      <div
        style={{
          position: 'absolute',
          inset: 'auto -10% 68% auto',
          width: 240,
          height: 240,
          borderRadius: '50%',
          background: 'radial-gradient(circle, color-mix(in srgb, var(--ant-color-primary) 16%, transparent) 0%, transparent 72%)',
          pointerEvents: 'none',
          zIndex: 0,
        }}
      />
      <style>
        {`
          .relationship-graph-flow .react-flow {
            background:
              radial-gradient(circle at top right, color-mix(in srgb, var(--ant-color-info) 10%, transparent) 0%, transparent 32%),
              linear-gradient(180deg, color-mix(in srgb, var(--ant-color-bg-container) 94%, white 6%) 0%, color-mix(in srgb, var(--ant-color-fill-quaternary) 40%, var(--ant-color-bg-container) 60%) 100%);
          }

          .relationship-graph-flow .react-flow__handle {
            opacity: 0 !important;
            background: transparent !important;
            border: none !important;
            pointer-events: none !important;
          }

          .relationship-graph-flow .react-flow__pane {
            cursor: grab;
          }

          .relationship-graph-flow .react-flow__pane:active {
            cursor: grabbing;
          }

          .relationship-graph-flow .react-flow__node {
            outline: 1px solid color-mix(in srgb, var(--ant-color-border-secondary) 88%, transparent);
            outline-offset: 0;
            border-radius: 18px;
            box-shadow: 0 18px 38px color-mix(in srgb, var(--ant-color-text) 10%, transparent);
          }

          .relationship-graph-flow .react-flow__node.selected,
          .relationship-graph-flow .react-flow__node:focus-visible {
            outline: 1px solid color-mix(in srgb, var(--ant-color-primary) 46%, transparent);
            box-shadow:
              0 20px 44px color-mix(in srgb, var(--ant-color-primary) 16%, transparent),
              0 0 0 4px color-mix(in srgb, var(--ant-color-primary) 10%, transparent);
          }

          .relationship-graph-flow .react-flow__edge-path {
            stroke-width: 2.2px;
            stroke: color-mix(in srgb, var(--ant-color-text-secondary) 42%, var(--ant-color-primary) 58%);
            opacity: 0.88;
          }

          .relationship-graph-flow .react-flow__edge.selected .react-flow__edge-path {
            stroke: color-mix(in srgb, var(--ant-color-primary) 82%, white 18%);
            opacity: 1;
          }

          .relationship-graph-flow .react-flow__controls {
            border: 1px solid color-mix(in srgb, var(--ant-color-border-secondary) 86%, transparent);
            border-radius: 16px;
            overflow: hidden;
            background: color-mix(in srgb, var(--ant-color-bg-elevated) 92%, white 8%);
            box-shadow: 0 14px 30px color-mix(in srgb, var(--ant-color-text) 10%, transparent);
            backdrop-filter: blur(16px);
          }

          .relationship-graph-flow .react-flow__controls-button {
            background: transparent;
            border-bottom: 1px solid color-mix(in srgb, var(--ant-color-border-secondary) 86%, transparent);
            color: var(--ant-color-text);
          }

          .relationship-graph-flow .react-flow__controls-button:last-child {
            border-bottom: none;
          }

          .relationship-graph-flow .react-flow__controls-button:hover {
            background: color-mix(in srgb, var(--ant-color-primary) 8%, var(--ant-color-bg-elevated) 92%);
          }

          .relationship-graph-flow .react-flow__controls-button:disabled {
            background: var(--ant-color-fill-quaternary);
            color: var(--ant-color-text-quaternary);
          }

          .relationship-graph-flow .react-flow__controls-button svg {
            fill: currentColor;
          }

          .relationship-graph-flow .react-flow__attribution {
            background: color-mix(in srgb, var(--ant-color-bg-elevated) 94%, white 6%);
            border: 1px solid color-mix(in srgb, var(--ant-color-border-secondary) 84%, transparent);
            border-radius: 999px;
            padding: 2px 8px;
            box-shadow: 0 8px 18px color-mix(in srgb, var(--ant-color-text) 8%, transparent);
          }

          .relationship-graph-flow .react-flow__attribution a {
            color: var(--ant-color-text-secondary);
          }

          .relationship-graph-flow .react-flow__attribution a:hover {
            color: var(--ant-color-primary);
          }
        `}
      </style>
      <ReactFlow
        nodes={flowNodes}
        edges={flowEdges}
        onNodesChange={onNodesChange}
        onEdgesChange={onEdgesChange}
        onNodeClick={onNodeClick}
        fitView
        fitViewOptions={{ padding: 0.2 }}
        attributionPosition="bottom-left"
        onlyRenderVisibleElements
      >
        <Background
          variant={BackgroundVariant.Dots}
          gap={22}
          size={1.2}
          color="color-mix(in srgb, var(--ant-color-border-secondary) 72%, transparent)"
        />
        <Controls position="top-left" />
      </ReactFlow>
    </div>
  );
}

export default memo(RelationshipGraphCanvas);
