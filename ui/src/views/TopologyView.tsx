import { useState, useEffect, useCallback, useMemo } from 'react';
import { ServiceTopologyGraph } from '@/components/ServiceTopologyGraph';
import { transformTopologyData } from '@/utils/topologyUtils';
import type { QubitTopology } from '@/types/topology';

const REFRESH_INTERVAL_MS = 30_000;

interface TopologyViewProps {
  onStatusChange: (status: { ok: boolean; lastChecked: Date | null }) => void;
  refreshKey: number;
}

function formatAge(date: Date | null): string {
  if (!date) return 'never';
  const diff = Math.floor((Date.now() - date.getTime()) / 1000);
  if (diff < 5) return 'just now';
  if (diff < 60) return `${diff}s ago`;
  if (diff < 3600) return `${Math.floor(diff / 60)}m ago`;
  return date.toLocaleTimeString();
}

export function TopologyView({ onStatusChange, refreshKey }: TopologyViewProps) {
  const [raw, setRaw] = useState<QubitTopology | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [lastUpdated, setLastUpdated] = useState<Date | null>(null);
  const [autoRefresh, setAutoRefresh] = useState(true);
  const [hideUnconnected, setHideUnconnected] = useState(false);
  const [selectedNodeId, setSelectedNodeId] = useState<string | undefined>(undefined);
  const [depth, setDepth] = useState<number>(1);

  const fetchTopology = useCallback(async () => {
    try {
      const res = await fetch('/api/topology');
      if (!res.ok) throw new Error(`Core returned ${res.status}`);
      const data: QubitTopology = await res.json();
      setRaw(data);
      setError(null);
      const now = new Date();
      setLastUpdated(now);
      onStatusChange({ ok: true, lastChecked: now });
    } catch (e) {
      setError(e instanceof Error ? e.message : 'Failed to fetch topology');
      onStatusChange({ ok: false, lastChecked: new Date() });
    } finally {
      setLoading(false);
    }
  }, [onStatusChange]);

  // Initial fetch + re-fetch on nav refresh button.
  // fetchTopology is stable (useCallback with stable deps), so this runs once
  // on mount and again each time refreshKey increments.
  useEffect(() => { fetchTopology(); }, [fetchTopology, refreshKey]);

  useEffect(() => {
    if (!autoRefresh) return;
    const id = setInterval(fetchTopology, REFRESH_INTERVAL_MS);
    return () => clearInterval(id);
  }, [autoRefresh, fetchTopology]);

  // Memoize on `raw` so services/connections are stable references between renders.
  // Without this, every TopologyView re-render creates new arrays → initialNodes
  // recomputes → position-reset effect fires → drag positions clobbered.
  const { services, connections, rootService } = useMemo(
    () => transformTopologyData(raw),
    [raw]
  );

  useEffect(() => {
    if (rootService && !selectedNodeId) setSelectedNodeId(rootService);
  }, [rootService, selectedNodeId]);

  const serviceOptions = [...services].sort((a, b) => a.name.localeCompare(b.name));

  return (
    <div
      className="relative h-full w-full overflow-hidden"
      style={{
        background: 'var(--background)',
        backgroundImage:
          'linear-gradient(var(--color-border) 1px, transparent 1px), linear-gradient(90deg, var(--color-border) 1px, transparent 1px)',
        backgroundSize: '32px 32px',
        backgroundPosition: '-1px -1px',
      }}
    >

      {/* ── Error banner ────────────────────────────────────────────────── */}
      {error && (
        <div className="absolute top-3 left-1/2 -translate-x-1/2 z-30 flex items-center gap-2 border border-error/30 bg-error/5 px-4 py-2 font-mono text-xs">
          <span className="font-semibold text-error">ERR</span>
          <span className="text-foreground">{error}</span>
          <span className="text-muted-foreground">— is Core running on :9000?</span>
        </div>
      )}

      {/* ── Loading ─────────────────────────────────────────────────────── */}
      {loading && (
        <div className="absolute inset-0 flex flex-col items-center justify-center gap-3 z-20">
          <div className="flex gap-1.5">
            {[0, 1, 2].map(i => (
              <div
                key={i}
                className="h-1.5 w-1.5 rounded-full bg-primary"
                style={{ animation: `qubit-pulse 1.4s ease-in-out ${i * 0.18}s infinite` }}
              />
            ))}
          </div>
          <span className="font-mono text-[10px] uppercase tracking-[0.2em] text-muted-foreground">
            Fetching topology
          </span>
        </div>
      )}

      {/* ── Empty state ─────────────────────────────────────────────────── */}
      {!loading && !error && services.length === 0 && (
        <div className="absolute inset-0 flex flex-col items-center justify-center gap-3 z-20">
          <span className="font-sans text-sm font-semibold text-foreground">No services discovered</span>
          <span className="font-mono text-xs text-muted-foreground text-center max-w-xs leading-relaxed">
            Ensure the eBPF loader and cluster-agent are running and generating traffic
          </span>
        </div>
      )}

      {/* ── Graph canvas ────────────────────────────────────────────────── */}
      {!loading && services.length > 0 && (
        <div className="absolute inset-0">
          <ServiceTopologyGraph
            services={services}
            connections={connections}
            rootService={rootService}
            depth={depth}
            hideUnconnectedNodes={hideUnconnected}
            selectedNodeId={selectedNodeId}
            onNodeSelect={setSelectedNodeId}
          />
        </div>
      )}

      {/* ── Legend (top-left) ───────────────────────────────────────────── */}
      {!loading && services.length > 0 && (
        <div className="absolute left-4 top-4 z-10 w-48 border border-border bg-surface p-3 shadow-sm">
          <div className="mb-2 font-mono text-[10px] font-medium uppercase tracking-wider text-muted-foreground">
            Connections
          </div>
          <div className="space-y-1.5 font-mono text-xs text-mono">
            <LegendEdge color="var(--color-primary)" label="Upstream" />
            <LegendEdge color="var(--color-success)" label="Downstream" />
          </div>
        </div>
      )}

      {/* ── Stats (top-right) ───────────────────────────────────────────── */}
      {!loading && services.length > 0 && (
        <div className="absolute right-4 top-4 z-10 flex gap-2">
          <StatCard label="Services" value={String(services.length)} />
          <StatCard label="Edges" value={String(connections.length)} />
        </div>
      )}

      {/* ── Floating control strip (bottom-center) ──────────────────────── */}
      {!loading && services.length > 0 && (
        <div className="absolute bottom-6 left-1/2 z-20 -translate-x-1/2">
          <div className="flex items-center gap-1 rounded-full border border-border bg-surface px-2 py-1.5 shadow-lg">

            {/* Focus select */}
            {serviceOptions.length > 0 && (
              <div className="flex items-center">
                <select
                  value={selectedNodeId ?? ''}
                  onChange={e => setSelectedNodeId(e.target.value || undefined)}
                  className="rounded-full bg-transparent px-3 py-1.5 font-mono text-xs text-foreground focus:outline-none cursor-pointer hover:bg-muted"
                >
                  <option value="">Focus: all</option>
                  {serviceOptions.map(s => (
                    <option key={s.id} value={s.id}>{s.name} ({s.namespace})</option>
                  ))}
                </select>
              </div>
            )}

            {/* Depth picker — only meaningful when a node is focused */}
            {selectedNodeId && (
              <>
                <Divider />
                <div className="flex items-center gap-0.5 px-1">
                  <span className="font-mono text-[10px] text-muted-foreground mr-1">depth</span>
                  {([1, 2, 3, Infinity] as const).map(d => (
                    <button
                      key={d}
                      onClick={() => setDepth(d)}
                      className={`rounded px-2 py-1 font-mono text-xs transition-colors ${
                        depth === d
                          ? 'bg-foreground text-background'
                          : 'text-foreground hover:bg-muted'
                      }`}
                    >
                      {d === Infinity ? '∞' : d}
                    </button>
                  ))}
                </div>
              </>
            )}

            <Divider />

            <ToggleChip
              label="Hide unconnected"
              active={hideUnconnected}
              onToggle={() => setHideUnconnected(v => !v)}
            />
            <ToggleChip
              label="Auto-refresh"
              active={autoRefresh}
              onToggle={() => setAutoRefresh(v => !v)}
            />

            <Divider />

            <span className="px-2 font-mono text-[11px] text-muted-foreground whitespace-nowrap">
              {lastUpdated ? `updated ${formatAge(lastUpdated)}` : 'fetching…'}
            </span>

            <button
              onClick={fetchTopology}
              className="rounded-full px-3 py-1.5 font-mono text-xs text-foreground hover:bg-muted"
            >
              Refresh
            </button>
          </div>
        </div>
      )}
    </div>
  );
}

function LegendEdge({ color, label }: { color: string; label: string }) {
  return (
    <div className="flex items-center gap-2">
      <span className="inline-block h-[2px] w-4 flex-shrink-0" style={{ backgroundColor: color }} />
      <span>{label}</span>
    </div>
  );
}

function StatCard({ label, value }: { label: string; value: string }) {
  return (
    <div className="border border-border bg-surface px-3 py-2 shadow-sm">
      <div className="font-mono text-[10px] uppercase tracking-wider text-muted-foreground">{label}</div>
      <div className="font-mono text-base font-semibold text-foreground">{value}</div>
    </div>
  );
}

function ToggleChip({ label, active, onToggle }: { label: string; active: boolean; onToggle: () => void }) {
  return (
    <button
      onClick={onToggle}
      className={`rounded-full px-3 py-1.5 font-mono text-xs transition-colors ${
        active ? 'bg-foreground text-background' : 'text-foreground hover:bg-muted'
      }`}
    >
      {label}
    </button>
  );
}

function Divider() {
  return <span className="h-5 w-px bg-border flex-shrink-0" />;
}
