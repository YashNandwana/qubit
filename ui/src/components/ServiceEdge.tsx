import { EdgeProps, getBezierPath } from '@xyflow/react';

export function UpstreamEdge({
  id,
  sourceX,
  sourceY,
  targetX,
  targetY,
  sourcePosition,
  targetPosition,
  style = {},
}: EdgeProps) {
  const [edgePath] = getBezierPath({
    sourceX,
    sourceY,
    sourcePosition,
    targetX,
    targetY,
    targetPosition,
  });

  return (
    <>
      <defs>
        <marker
          id={`upstream-arrow-${id}`}
          markerWidth="10"
          markerHeight="10"
          refX="5"
          refY="3"
          orient="auto"
          markerUnits="strokeWidth"
        >
          <path d="M0,0 L0,6 L9,3 z" fill="#3B82F6" stroke="#3B82F6" />
        </marker>
      </defs>

      <path
        id={id}
        style={{
          ...style,
          stroke: '#3B82F6',
          strokeWidth: 2,
          strokeDasharray: '8,8',
          animation: 'flow 1s linear infinite',
        }}
        className="react-flow__edge-path"
        d={edgePath}
        markerEnd={`url(#upstream-arrow-${id})`}
      />

      <text
        x={(sourceX + targetX) / 2}
        y={(sourceY + targetY) / 2 - 10}
        className="fill-blue-600 text-xs font-medium"
        textAnchor="middle"
      >
        upstream
      </text>
    </>
  );
}

export function DownstreamEdge({
  id,
  sourceX,
  sourceY,
  targetX,
  targetY,
  sourcePosition,
  targetPosition,
  style = {},
}: EdgeProps) {
  const [edgePath] = getBezierPath({
    sourceX,
    sourceY,
    sourcePosition,
    targetX,
    targetY,
    targetPosition,
  });

  return (
    <>
      <defs>
        <marker
          id={`downstream-arrow-${id}`}
          markerWidth="10"
          markerHeight="10"
          refX="5"
          refY="3"
          orient="auto"
          markerUnits="strokeWidth"
        >
          <path d="M0,0 L0,6 L9,3 z" fill="#10B981" stroke="#10B981" />
        </marker>
      </defs>

      <path
        id={id}
        style={{
          ...style,
          stroke: '#10B981',
          strokeWidth: 2,
          strokeDasharray: '8,8',
          animation: 'flow 1s linear infinite',
        }}
        className="react-flow__edge-path"
        d={edgePath}
        markerEnd={`url(#downstream-arrow-${id})`}
      />

      <text
        x={(sourceX + targetX) / 2}
        y={(sourceY + targetY) / 2 + 20}
        className="fill-green-600 text-xs font-medium"
        textAnchor="middle"
      >
        downstream
      </text>
    </>
  );
}
