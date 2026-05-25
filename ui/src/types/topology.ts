// Qubit Core /api/topology response — mirrors the gRPC GetTopologyResponse shape.
export interface QubitTopology {
  nodes?: {
    [key: string]: {       // key = "namespace/service_name"
      applicationName: string;
      namespace: string;
      ip: string;
    };
  };
  upstream?: {
    [key: string]: {       // key = "namespace/service_name" (destination)
      flows: Array<{
        sourceApplication: string;
        destinationApplication: string;
        method: string;
        path: string;
      }>;
    };
  };
  downstream?: {
    [key: string]: {       // key = "namespace/service_name" (source)
      flows: Array<{
        sourceApplication: string;
        destinationApplication: string;
        method: string;
        path: string;
      }>;
    };
  };
}

// ── Shared graph model ────────────────────────────────────────────────────────
// These match the ringmaster types exactly so all layout/rendering logic is
// reused verbatim.

export interface ServiceNode {
  id: string;
  name: string;
  namespace: string;
  domain: string;   // Qubit uses namespace as domain (no team/domain concept)
  layer: number;    // Computed via BFS from root; 0 = root
}

export interface ServiceConnection {
  source: string;
  target: string;
  type: 'upstream' | 'downstream';
}

export interface TransformedTopology {
  services: ServiceNode[];
  connections: ServiceConnection[];
  rootService?: string;
}

export interface FlowNode {
  id: string;
  type: 'serviceNode';
  position: { x: number; y: number };
  data: ServiceNodeData;
}

export type ServiceNodeData = Omit<ServiceNode, 'id'> & {
  isSelected?: boolean;
};

export interface FlowEdge {
  id: string;
  source: string;
  target: string;
  type: 'upstreamEdge' | 'downstreamEdge';
  animated: boolean;
  style?: {
    stroke: string;
    strokeWidth: number;
    strokeDasharray?: string;
  };
  data?: Record<string, unknown>;
}
