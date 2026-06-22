import { useMemo, useState, useEffect, useCallback } from 'react';
import {
  ReactFlow,
  Controls,
  MiniMap,
  Node,
  useNodesState,
  useEdgesState,
  ReactFlowProvider,
  NodeMouseHandler,
} from '@xyflow/react';

import { ServiceNode } from './ServiceNode';
import { UpstreamEdge, DownstreamEdge } from './ServiceEdge';
import {
  convertToFlowData,
  getLayerColorScheme,
  getConnectedNodeIdsAtDepth,
} from '@/utils/topologyUtils';
import type { ServiceNode as ServiceNodeType, ServiceConnection, FlowEdge } from '@/types/topology';

const nodeTypes = { serviceNode: ServiceNode } as any;
const edgeTypes = { upstreamEdge: UpstreamEdge, downstreamEdge: DownstreamEdge };

interface ServiceTopologyGraphProps {
  services: ServiceNodeType[];
  connections: ServiceConnection[];
  rootService?: string;
  depth?: number;
  hideUnconnectedNodes?: boolean;
  selectedNodeId?: string;
  onNodeSelect?: (nodeId: string) => void;
  /// When true the backend already BFS-filtered the data; skip client-side depth filtering.
  isSubgraphMode?: boolean;
}

function ServiceTopologyGraphInner({
  services,
  connections,
  rootService,
  depth = 1,
  hideUnconnectedNodes = false,
  selectedNodeId: propSelectedNodeId,
  onNodeSelect,
  isSubgraphMode = false,
}: ServiceTopologyGraphProps) {
  const [internalSelectedNodeId, setInternalSelectedNodeId] = useState<string | null>(
    rootService ?? null
  );

  const selectedNodeId = propSelectedNodeId ?? internalSelectedNodeId;

  useEffect(() => {
    if (!propSelectedNodeId) setInternalSelectedNodeId(rootService ?? null);
  }, [rootService, propSelectedNodeId]);

  const { nodes: initialNodes } = useMemo(
    () => convertToFlowData(services, connections, rootService),
    [services, connections, rootService]
  );

  // BFS from selected node up to `depth` hops. When no node is selected, or when the
  // backend already filtered the data (isSubgraphMode), all nodes are reachable.
  const reachableNodeIds = useMemo(() => {
    if (!selectedNodeId || isSubgraphMode) return new Set(initialNodes.map(n => n.id));
    return getConnectedNodeIdsAtDepth(selectedNodeId, connections, depth);
  }, [selectedNodeId, connections, depth, initialNodes, isSubgraphMode]);

  // Which nodes are visible. Without hideUnconnectedNodes this is always all nodes;
  // with it, trimmed to the reachable set.
  const connectedNodeIds = useMemo(() => {
    if (!hideUnconnectedNodes || !selectedNodeId) return new Set(initialNodes.map(n => n.id));
    return reachableNodeIds;
  }, [hideUnconnectedNodes, selectedNodeId, reachableNodeIds, initialNodes]);

  // Edges: show all edges whose both endpoints are in the reachable set.
  // When no node is selected, show all edges so "Focus: all" is meaningful.
  const filteredEdges = useMemo(() => {
    let edgeIndex = 0;
    return connections
      .filter(c => reachableNodeIds.has(c.source) && reachableNodeIds.has(c.target))
      .map(c => ({
        id: `edge-${c.type === 'downstream' ? 'downstream' : 'upstream'}-${edgeIndex++}`,
        source: c.source,
        target: c.target,
        type: c.type === 'downstream' ? 'downstreamEdge' : 'upstreamEdge',
        animated: !!selectedNodeId,
      }));
  }, [selectedNodeId, connections, reachableNodeIds]);

  const nodesWithSelection = useMemo(
    () =>
      initialNodes
        .filter(node => connectedNodeIds.has(node.id))
        .map(node => ({
          ...node,
          data: { ...node.data, isSelected: node.id === selectedNodeId },
        })),
    [initialNodes, selectedNodeId, connectedNodeIds]
  );

  const [nodes, setNodes, onNodesChange] = useNodesState(nodesWithSelection);
  const [edges, setEdges, onEdgesChange] = useEdgesState(filteredEdges);

  // Full position reset ONLY when the underlying node list changes (new API data).
  // Everything else uses functional updates so drag positions are never clobbered.
  // eslint-disable-next-line react-hooks/exhaustive-deps
  useEffect(() => {
    setNodes(
      initialNodes.map(n => ({
        ...n,
        data: { ...n.data, isSelected: n.id === selectedNodeId },
      }))
    );
  }, [initialNodes]); // intentionally omit selectedNodeId — handled below

  // Visibility (hide-unconnected) + selection: functional update preserves drag positions.
  useEffect(() => {
    setNodes(prev =>
      prev
        .filter(n => connectedNodeIds.has(n.id))
        .map(n => ({ ...n, data: { ...n.data, isSelected: n.id === selectedNodeId } }))
    );
  }, [connectedNodeIds, selectedNodeId]);

  useEffect(() => { setEdges(filteredEdges); }, [filteredEdges]);

  const onNodeClick: NodeMouseHandler = useCallback(
    (_event, node) => {
      if (onNodeSelect) onNodeSelect(node.id);
      else setInternalSelectedNodeId(node.id);
    },
    [onNodeSelect]
  );

  const proOptions = { hideAttribution: true };

  return (
    <div className="w-full h-full">
      <style>{`
        @keyframes flow {
          0%   { stroke-dashoffset: 16; }
          100% { stroke-dashoffset: 0; }
        }
      `}</style>

      <ReactFlow
        nodes={nodes}
        edges={edges}
        onNodesChange={onNodesChange}
        onEdgesChange={onEdgesChange}
        onNodeClick={onNodeClick}
        nodeTypes={nodeTypes}
        edgeTypes={edgeTypes}
        nodesConnectable={false}
        fitView
        fitViewOptions={{ padding: 0.15, includeHiddenNodes: false, minZoom: 0.1, maxZoom: 1.5 }}
        proOptions={proOptions}
        defaultViewport={{ x: 0, y: 0, zoom: 0.8 }}
        minZoom={0.1}
        maxZoom={2}
      >
        <Controls
          showInteractive={false}
          style={{
            background: 'var(--surface)',
            border: '1px solid var(--border)',
            boxShadow: 'none',
            borderRadius: '2px',
          }}
        />
        <MiniMap
          nodeColor={(node: Node) => {
            const layer = typeof node.data?.layer === 'number' ? node.data.layer : 0;
            return getLayerColorScheme(layer).primary;
          }}
          maskColor="rgba(247, 246, 243, 0.7)"
          style={{
            background: 'var(--surface)',
            border: '1px solid var(--border)',
            boxShadow: 'none',
          }}
        />
      </ReactFlow>
    </div>
  );
}

export function ServiceTopologyGraph(props: ServiceTopologyGraphProps) {
  return (
    <ReactFlowProvider>
      <ServiceTopologyGraphInner {...props} />
    </ReactFlowProvider>
  );
}
