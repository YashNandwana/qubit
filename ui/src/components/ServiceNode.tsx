import { Handle, Position } from '@xyflow/react';
import type { NodeProps } from '@xyflow/react';
import { extractEnvironment, getLayerColorScheme } from '@/utils/topologyUtils';
import type { ServiceNodeData } from '@/types/topology';

export function ServiceNode({ data }: NodeProps<any>) {
  const nodeData = data as ServiceNodeData;
  const isSelected = nodeData.isSelected ?? false;
  const colorScheme = getLayerColorScheme(nodeData.layer);

  // Root node (layer 0) — larger card with animated pulse ring
  if (nodeData.layer === 0) {
    return (
      <div
        className={`px-6 py-4 shadow-xl rounded-xl border-4 min-w-[220px] relative cursor-pointer transition-all duration-200 ${
          isSelected ? 'ring-4 ring-opacity-75' : ''
        }`}
        style={{
          borderColor: colorScheme.primary,
          background: `linear-gradient(to bottom right, ${colorScheme.background}, ${colorScheme.backgroundEnd})`,
          ...(isSelected && { '--tw-ring-color': colorScheme.ring }),
        } as React.CSSProperties}
      >
        <Handle
          type="target"
          position={Position.Top}
          className="w-4 h-4"
          style={{ backgroundColor: colorScheme.handle }}
        />
        <Handle
          type="source"
          position={Position.Bottom}
          className="w-4 h-4"
          style={{ backgroundColor: colorScheme.handle }}
        />

        <div
          className="absolute inset-0 rounded-xl border-2 animate-pulse"
          style={{
            borderColor: isSelected ? colorScheme.ring : colorScheme.primary,
            opacity: isSelected ? 0.8 : 0.5,
          }}
        />

        <div className="text-center relative z-10">
          <div className="font-bold text-lg mb-2" style={{ color: colorScheme.text }}>
            {nodeData.name}
          </div>

          <div className="flex justify-center mb-3">
            <span
              className="px-3 py-1 text-sm font-semibold rounded-full"
              style={{ backgroundColor: colorScheme.badge, color: colorScheme.badgeText }}
            >
              {extractEnvironment(nodeData.namespace)}
            </span>
          </div>

          <div className="text-sm space-y-2" style={{ color: colorScheme.text }}>
            {nodeData.domain && <div><strong>Domain:</strong> {nodeData.domain}</div>}
          </div>

          <div
            className="mt-3 text-xs font-bold uppercase tracking-wider"
            style={{ color: colorScheme.text }}
          >
            {colorScheme.name}
          </div>
        </div>
      </div>
    );
  }

  // Non-root node
  return (
    <div
      className={`px-4 py-3 shadow-lg rounded-lg border-2 min-w-[180px] cursor-pointer transition-all duration-200 ${
        isSelected ? 'ring-2 ring-opacity-50 shadow-xl' : 'hover:shadow-xl'
      }`}
      style={{
        borderColor: isSelected ? colorScheme.primary : '#d1d5db',
        background: `linear-gradient(to bottom right, ${colorScheme.background}, ${colorScheme.backgroundEnd})`,
        ...(isSelected && { '--tw-ring-color': colorScheme.ring }),
      } as React.CSSProperties}
    >
      <Handle
        type="target"
        position={Position.Top}
        className="w-3 h-3"
        style={{ backgroundColor: colorScheme.handle }}
      />
      <Handle
        type="source"
        position={Position.Bottom}
        className="w-3 h-3"
        style={{ backgroundColor: colorScheme.handle }}
      />

      <div className="text-center">
        <div className="font-semibold text-sm mb-1" style={{ color: colorScheme.text }}>
          {nodeData.name}
        </div>

        <div className="flex justify-center mb-2">
          <span
            className="px-2 py-1 text-xs rounded-full"
            style={{ backgroundColor: colorScheme.badge, color: colorScheme.badgeText }}
          >
            {extractEnvironment(nodeData.namespace)}
          </span>
        </div>

        {nodeData.domain && (
          <div className="text-xs space-y-1" style={{ color: colorScheme.text }}>
            <div><strong>Domain:</strong> {nodeData.domain}</div>
          </div>
        )}

        {nodeData.layer > 0 && (
          <div
            className="mt-2 text-xs font-medium uppercase tracking-wider"
            style={{ color: colorScheme.text, opacity: 0.8 }}
          >
            {colorScheme.name}
          </div>
        )}
      </div>
    </div>
  );
}
