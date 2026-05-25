import type {
  QubitTopology,
  ServiceNode,
  ServiceConnection,
  TransformedTopology,
  FlowNode,
  FlowEdge,
} from '@/types/topology';

// ── transformTopologyData ─────────────────────────────────────────────────────
//
// Converts the Qubit /api/topology response into the normalised graph model
// that the React Flow components expect.
//
// Key differences from ringmaster's version:
//   1. Qubit nodes have {serviceName, namespace, ip} — no layer or domain field.
//      Layer is computed here via BFS from a chosen root node.
//   2. Flow source/target are bare service names, not full "namespace/name" IDs.
//      We build a serviceName → nodeId lookup to resolve them.
//   3. The same A→B flow appears in both upstream[B] and downstream[A].
//      We add both types (different visual styles) but deduplicate identical pairs.

export function transformTopologyData(data: QubitTopology | null): TransformedTopology {
  if (!data || !data.nodes) {
    return { services: [], connections: [], rootService: undefined };
  }

  // serviceName → "namespace/serviceName" id — used to resolve flow endpoints
  const nameToId = new Map<string, string>();
  Object.entries(data.nodes).forEach(([id, node]) => {
    nameToId.set(node.applicationName, id);
  });

  // Build connections, deduplicating via a seen-set per type
  const connections: ServiceConnection[] = [];
  const seen = new Set<string>();

  const addConnection = (
    sourceService: string,
    destService: string,
    type: 'upstream' | 'downstream'
  ) => {
    const sourceId = nameToId.get(sourceService);
    const destId = nameToId.get(destService);
    if (!sourceId || !destId) return;

    const key = `${type}:${sourceId}→${destId}`;
    if (seen.has(key)) return;
    seen.add(key);
    connections.push({ source: sourceId, target: destId, type });
  };

  if (data.downstream) {
    Object.values(data.downstream).forEach(({ flows }) => {
      flows.forEach(f => addConnection(f.sourceApplication, f.destinationApplication, 'downstream'));
    });
  }

  if (data.upstream) {
    Object.values(data.upstream).forEach(({ flows }) => {
      flows.forEach(f => addConnection(f.sourceApplication, f.destinationApplication, 'upstream'));
    });
  }

  // Choose root: the node with the most outgoing downstream connections.
  // Falls back to the first node alphabetically if the graph has no edges.
  const outDegree = new Map<string, number>();
  connections
    .filter(c => c.type === 'downstream')
    .forEach(c => outDegree.set(c.source, (outDegree.get(c.source) ?? 0) + 1));

  const rootId =
    [...outDegree.entries()].sort((a, b) => b[1] - a[1])[0]?.[0] ??
    Object.keys(data.nodes).sort()[0];

  // BFS from root over downstream edges to assign layers.
  // Any node not reachable from root (e.g. in a disconnected component) gets layer 1.
  const layerMap = new Map<string, number>();
  layerMap.set(rootId, 0);
  const queue = [rootId];
  while (queue.length > 0) {
    const current = queue.shift()!;
    const currentLayer = layerMap.get(current)!;
    connections
      .filter(c => c.source === current && c.type === 'downstream')
      .forEach(c => {
        if (!layerMap.has(c.target)) {
          layerMap.set(c.target, currentLayer + 1);
          queue.push(c.target);
        }
      });
  }

  const services: ServiceNode[] = Object.entries(data.nodes).map(([id, node]) => ({
    id,
    name: node.applicationName,
    namespace: node.namespace,
    domain: node.namespace, // namespace doubles as domain in Qubit
    layer: layerMap.get(id) ?? 1,
  }));

  return { services, connections, rootService: rootId };
}

// ── Everything below is copied verbatim from ringmaster's spectraUtils.ts ─────
// Layout algorithm, colour schemes, statistics helpers — none of them depend on
// the API format, so they work unchanged.

export function convertToFlowData(
  services: ServiceNode[],
  connections: ServiceConnection[],
  rootService?: string
): { nodes: FlowNode[]; edges: FlowEdge[] } {
  const nodes: FlowNode[] = [];
  const edges: FlowEdge[] = [];

  const layerGroups = new Map<number, ServiceNode[]>();
  services.forEach(service => {
    const layer = service.layer;
    if (!layerGroups.has(layer)) layerGroups.set(layer, []);
    layerGroups.get(layer)?.push(service);
  });

  const centerX = 400;
  const centerY = 400;
  let baseRadius = 200;
  const radiusIncrement = 300;

  const rootNode = services.find(service => service.id === rootService);
  if (rootNode) {
    nodes.push({
      id: rootNode.id,
      type: 'serviceNode',
      position: { x: centerX, y: centerY },
      data: {
        name: rootNode.name,
        namespace: rootNode.namespace,
        domain: rootNode.domain,
        layer: rootNode.layer,
      },
    });
    layerGroups.delete(rootNode.layer);
  }

  for (let layerIndex = 1; layerIndex <= layerGroups.size; layerIndex++) {
    const layerNodes = layerGroups.get(layerIndex);
    if (!layerNodes || layerNodes.length === 0) continue;

    const sortedLayerNodes = layerNodes.sort((a, b) => {
      const aHasDownstream = connections.some(
        conn => conn.source === a.id && conn.type === 'downstream'
      );
      const aHasUpstream = connections.some(
        conn => conn.target === a.id && conn.type === 'upstream'
      );
      const bHasDownstream = connections.some(
        conn => conn.source === b.id && conn.type === 'downstream'
      );
      const bHasUpstream = connections.some(
        conn => conn.target === b.id && conn.type === 'upstream'
      );
      if (aHasDownstream && !bHasDownstream) return -1;
      if (!aHasDownstream && bHasDownstream) return 1;
      if (aHasUpstream && !bHasUpstream) return -1;
      if (!aHasUpstream && bHasUpstream) return 1;
      return a.name.localeCompare(b.name);
    });

    const maxNodesPerSublayer = 70;
    let sublayerGapRatio = 0.15;

    const sublayers: ServiceNode[][] = [];
    for (let i = 0; i < sortedLayerNodes.length; i += maxNodesPerSublayer) {
      sublayers.push(sortedLayerNodes.slice(i, i + maxNodesPerSublayer));
    }

    const sublayerRadii: number[] = [];
    const baseLayerRadius = baseRadius + radiusIncrement;
    let currentRadius = baseLayerRadius;

    sublayers.forEach((sublayerNodes, sublayerIndex) => {
      const nodeCount = sublayerNodes.length;
      const nodeWidth = 200;
      const nodePadding = 60;
      const effectiveNodeSpace = nodeWidth + nodePadding;
      const circumference = nodeCount * effectiveNodeSpace;
      const minRadiusForNodes = circumference / (2 * Math.PI);

      if (sublayerIndex === 0) {
        currentRadius = Math.max(baseLayerRadius, minRadiusForNodes);
      } else {
        const previousRadius = sublayerRadii[sublayerIndex - 1];
        const minSublayerGap = 40;
        const dynamicGap = Math.max(minSublayerGap, previousRadius * sublayerGapRatio);
        currentRadius = Math.max(previousRadius + dynamicGap, minRadiusForNodes);
      }
      sublayerRadii.push(currentRadius);
    });

    baseRadius = currentRadius;

    sublayers.forEach((sublayerNodes, sublayerIndex) => {
      const adjustedRadius = sublayerRadii[sublayerIndex];
      const nodeCount = sublayerNodes.length;
      const angleStep = (2 * Math.PI) / nodeCount;

      const downstreamNodeCount = sublayerNodes.filter(service =>
        connections.some(conn => conn.source === service.id && conn.type === 'downstream')
      ).length;

      const downstreamAngleSpan = (downstreamNodeCount / 2) * angleStep;
      const angleOffset = Math.PI / 2 + downstreamAngleSpan;
      sublayerGapRatio -= 0.04;

      sublayerNodes.forEach((service, index) => {
        const angle = index * angleStep - angleOffset;
        const x = centerX + adjustedRadius * Math.cos(angle);
        const y = centerY + adjustedRadius * Math.sin(angle);

        nodes.push({
          id: service.id,
          type: 'serviceNode',
          position: { x, y },
          data: {
            name: service.name,
            namespace: service.namespace,
            domain: service.domain,
            layer: service.layer,
          },
        });
      });
    });
  }

  connections.forEach((connection, index) => {
    edges.push({
      id: `edge-${connection.type}-${index}`,
      source: connection.source,
      target: connection.target,
      type: connection.type === 'upstream' ? 'upstreamEdge' : 'downstreamEdge',
      animated: false,
    });
  });

  return { nodes, edges };
}

export function extractEnvironment(namespace: string): string {
  const parts = namespace.split('/');
  return parts[0] || 'unknown';
}

export function getLayerStatistics(services: ServiceNode[]): {
  layers: Map<number, ServiceNode[]>;
  maxLayer: number;
  totalLayers: number;
} {
  const layers = new Map<number, ServiceNode[]>();
  services.forEach(service => {
    const layer = service.layer;
    if (!layers.has(layer)) layers.set(layer, []);
    layers.get(layer)?.push(service);
  });

  const layerNumbers = Array.from(layers.keys());
  const maxLayer = layerNumbers.length > 0 ? Math.max(...layerNumbers) : 0;
  const totalLayers = layers.size;

  return { layers, maxLayer, totalLayers };
}

export interface LayerColorScheme {
  primary: string;
  background: string;
  backgroundEnd: string;
  text: string;
  badge: string;
  badgeText: string;
  handle: string;
  ring: string;
  name: string;
}

export const LAYER_COLOR_SCHEMES: Record<number, LayerColorScheme> = {
  0: {
    primary: '#ea580c', background: '#fed7aa', backgroundEnd: '#ffffff',
    text: '#9a3412', badge: '#fb923c', badgeText: '#9a3412',
    handle: '#ea580c', ring: '#fed7aa', name: 'Root Service',
  },
  1: {
    primary: '#2563eb', background: '#ffffff', backgroundEnd: '#f8fafc',
    text: '#1e3a8a', badge: '#dbeafe', badgeText: '#1e3a8a',
    handle: '#3b82f6', ring: '#93c5fd', name: 'Layer 1',
  },
  2: {
    primary: '#dc2626', background: '#fee2e2', backgroundEnd: '#ffffff',
    text: '#991b1b', badge: '#fca5a5', badgeText: '#991b1b',
    handle: '#ef4444', ring: '#fca5a5', name: 'Layer 2',
  },
  3: {
    primary: '#2563eb', background: '#dbeafe', backgroundEnd: '#ffffff',
    text: '#1e3a8a', badge: '#93c5fd', badgeText: '#1e3a8a',
    handle: '#3b82f6', ring: '#93c5fd', name: 'Layer 3',
  },
  4: {
    primary: '#16a34a', background: '#dcfce7', backgroundEnd: '#ffffff',
    text: '#14532d', badge: '#86efac', badgeText: '#14532d',
    handle: '#22c55e', ring: '#86efac', name: 'Layer 4',
  },
  5: {
    primary: '#9333ea', background: '#e9d5ff', backgroundEnd: '#ffffff',
    text: '#581c87', badge: '#c4b5fd', badgeText: '#581c87',
    handle: '#a855f7', ring: '#c4b5fd', name: 'Layer 5',
  },
  6: {
    primary: '#c2410c', background: '#ffedd5', backgroundEnd: '#ffffff',
    text: '#7c2d12', badge: '#fed7aa', badgeText: '#7c2d12',
    handle: '#f97316', ring: '#fed7aa', name: 'Layer 6',
  },
  7: {
    primary: '#be123c', background: '#ffe4e6', backgroundEnd: '#ffffff',
    text: '#881337', badge: '#fecdd3', badgeText: '#881337',
    handle: '#f43f5e', ring: '#fecdd3', name: 'Layer 7',
  },
};

const DEFAULT_COLOR_SCHEME: LayerColorScheme = {
  primary: '#6b7280', background: '#f9fafb', backgroundEnd: '#ffffff',
  text: '#374151', badge: '#e5e7eb', badgeText: '#374151',
  handle: '#9ca3af', ring: '#d1d5db', name: 'Layer',
};

export function getLayerColorScheme(layer: number): LayerColorScheme {
  return LAYER_COLOR_SCHEMES[layer] ?? { ...DEFAULT_COLOR_SCHEME, name: `Layer ${layer}` };
}

export function getLayerColorMapping(services: ServiceNode[]): Map<number, LayerColorScheme> {
  const uniqueLayers = new Set<number>();
  services.forEach(service => uniqueLayers.add(service.layer));

  const mapping = new Map<number, LayerColorScheme>();
  uniqueLayers.forEach(layer => mapping.set(layer, getLayerColorScheme(layer)));
  return mapping;
}

export function getConnectedNodeIds(
  nodeId: string,
  connections: ServiceConnection[]
): Set<string> {
  return getConnectedNodeIdsAtDepth(nodeId, connections, 1);
}

// BFS from nodeId following edges in both directions up to `depth` hops.
// depth=1 is equivalent to the original getConnectedNodeIds.
// depth=Infinity walks until no unvisited neighbours remain.
export function getConnectedNodeIdsAtDepth(
  nodeId: string,
  connections: ServiceConnection[],
  depth: number
): Set<string> {
  const visited = new Set<string>([nodeId]);
  let frontier = [nodeId];

  for (let d = 0; d < depth; d++) {
    if (frontier.length === 0) break;
    const next: string[] = [];
    for (const id of frontier) {
      connections.forEach(c => {
        const neighbor = c.source === id ? c.target : c.target === id ? c.source : null;
        if (neighbor && !visited.has(neighbor)) {
          visited.add(neighbor);
          next.push(neighbor);
        }
      });
    }
    frontier = next;
  }
  return visited;
}
