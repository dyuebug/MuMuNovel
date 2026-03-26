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
    <div style={{ flex: 1, minHeight: 0 }} className="relationship-graph-flow">
      <style>
        {`
          .relationship-graph-flow .react-flow__handle {
            opacity: 0 !important;
            background: transparent !important;
            border: none !important;
            pointer-events: none !important;
          }

          .relationship-graph-flow .react-flow__node {
            outline: 1px solid var(--ant-color-border-secondary);
            outline-offset: 0;
          }

          .relationship-graph-flow .react-flow__controls {
            border: 1px solid var(--ant-color-border-secondary);
            border-radius: var(--ant-border-radius-lg);
            overflow: hidden;
            background: var(--ant-color-bg-elevated);
            box-shadow: 0 6px 16px color-mix(in srgb, var(--ant-color-text) 12%, transparent);
          }

          .relationship-graph-flow .react-flow__controls-button {
            background: var(--ant-color-bg-elevated);
            border-bottom: 1px solid var(--ant-color-border-secondary);
            color: var(--ant-color-text);
          }

          .relationship-graph-flow .react-flow__controls-button:last-child {
            border-bottom: none;
          }

          .relationship-graph-flow .react-flow__controls-button:hover {
            background: var(--ant-color-fill-secondary);
          }

          .relationship-graph-flow .react-flow__controls-button:disabled {
            background: var(--ant-color-fill-quaternary);
            color: var(--ant-color-text-quaternary);
          }

          .relationship-graph-flow .react-flow__controls-button svg {
            fill: currentColor;
          }

          .relationship-graph-flow .react-flow__attribution {
            background: var(--ant-color-bg-elevated);
            border: 1px solid var(--ant-color-border-secondary);
            border-radius: var(--ant-border-radius-sm);
            box-shadow: 0 2px 8px color-mix(in srgb, var(--ant-color-text) 10%, transparent);
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
        <Background variant={BackgroundVariant.Dots} gap={20} size={1} />
        <Controls position="top-left" />
      </ReactFlow>
    </div>
  );
}

export default memo(RelationshipGraphCanvas);
