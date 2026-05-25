import { useState, useEffect, useCallback } from 'react';
import { Pause, Play, ChevronLeft, ChevronRight } from 'lucide-react';

const PAGE_SIZE = 100;
const LIVE_REFRESH_MS = 5_000;

// Shape of EbpfNetworkEvent as serialised by the Rust backend (snake_case).
interface RawNetworkEvent {
  timestamp_ns: number;
  src_service: string;
  src_namespace: string;
  dst_service: string;
  dst_namespace: string;
  src_port: number;
  dst_port: number;
  method: string;
  path: string;
  host: string;
}

interface PagedResponse {
  items: RawNetworkEvent[];
  total: number;
  page: number;
  page_size: number;
}

interface NetworkRow {
  time: string;
  method: string;
  src: string;
  dst: string;
  path: string;
}

const METHOD_STYLES: Record<string, string> = {
  GET:    'bg-primary/10 text-primary border-primary/30',
  POST:   'bg-success/10 text-success border-success/30',
  PUT:    'bg-warning/15 text-[#8a6d00] border-warning/40',
  PATCH:  'bg-purple/10 text-purple border-purple/30',
  DELETE: 'bg-error/10 text-error border-error/30',
};

function normalize(raw: RawNetworkEvent): NetworkRow {
  const d = new Date(raw.timestamp_ns / 1_000_000);
  const ms = String(d.getMilliseconds()).padStart(3, '0');
  const time = `${d.toLocaleTimeString([], { hour12: false })}.${ms}`;
  // Show "namespace/service" so the user can tell apart same-named services in
  // different namespaces at a glance.
  const src = raw.src_namespace && raw.src_namespace !== 'unknown'
    ? `${raw.src_namespace}/${raw.src_service}`
    : raw.src_service;
  const dst = raw.dst_namespace && raw.dst_namespace !== 'unknown'
    ? `${raw.dst_namespace}/${raw.dst_service}`
    : raw.dst_service;
  return { time, method: raw.method.toUpperCase(), src, dst, path: raw.path };
}

interface Props {
  onStatusChange: (s: { ok: boolean; lastChecked: Date | null }) => void;
  refreshKey: number;
}

export function NetworkEventsView({ onStatusChange, refreshKey }: Props) {
  const [rows, setRows] = useState<NetworkRow[]>([]);
  const [total, setTotal] = useState(0);
  const [page, setPage] = useState(0);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [paused, setPaused] = useState(false);

  const fetchEvents = useCallback(async (targetPage: number) => {
    try {
      const res = await fetch(`/api/network-events?page=${targetPage}&page_size=${PAGE_SIZE}`);
      if (!res.ok) throw new Error(`HTTP ${res.status}`);
      const data: PagedResponse = await res.json();
      setRows(data.items.map(normalize));
      setTotal(data.total);
      setError(null);
      onStatusChange({ ok: true, lastChecked: new Date() });
    } catch (e) {
      setError(e instanceof Error ? e.message : 'fetch failed');
      onStatusChange({ ok: false, lastChecked: new Date() });
    } finally {
      setLoading(false);
    }
  }, [onStatusChange]);

  // When nav refresh is clicked or the page changes, refetch.
  useEffect(() => { fetchEvents(page); }, [fetchEvents, page, refreshKey]);

  // Live mode: always poll page 0 so the latest events stay on screen.
  // Paused mode: no auto-refresh — the user is browsing history.
  useEffect(() => {
    if (paused) return;
    const id = setInterval(() => {
      setPage(0); // snap back to first page on each live tick
      fetchEvents(0);
    }, LIVE_REFRESH_MS);
    return () => clearInterval(id);
  }, [paused, fetchEvents]);

  // When the user unpauses, jump back to page 0 immediately.
  const handlePauseToggle = () => {
    setPaused(p => {
      if (p) setPage(0); // unpausing → go back to latest
      return !p;
    });
  };

  const totalPages = Math.max(1, Math.ceil(total / PAGE_SIZE));

  return (
    <div className="flex h-full flex-col">
      {/* Toolbar */}
      <div className="flex items-center gap-3 border-b border-border bg-surface px-6 py-3 flex-shrink-0">
        <div className="flex items-center gap-2">
          <span className="font-sans text-sm font-semibold text-foreground">Network</span>
          <span className="flex items-center gap-1.5 border border-border bg-background px-2 py-0.5">
            <span className="relative flex h-1.5 w-1.5">
              {!paused && (
                <span className="qubit-ping absolute inline-flex h-full w-full rounded-full bg-success" />
              )}
              <span className={`relative inline-flex h-1.5 w-1.5 rounded-full ${paused ? 'bg-muted-foreground' : 'bg-success qubit-pulse'}`} />
            </span>
            <span className="font-mono text-[10px] font-medium uppercase tracking-wider text-mono">
              {paused ? 'Paused' : 'Live'}
            </span>
          </span>
        </div>
        <div className="ml-auto flex items-center gap-3">
          <span className="font-mono text-[11px] text-muted-foreground">
            {total.toLocaleString()} requests total
          </span>
          <button
            onClick={handlePauseToggle}
            className="flex items-center gap-1.5 border border-border bg-background px-3 py-1.5 font-mono text-xs text-foreground hover:border-border-strong"
          >
            {paused ? <Play className="h-3 w-3" /> : <Pause className="h-3 w-3" />}
            {paused ? 'Resume' : 'Pause'}
          </button>
        </div>
      </div>

      {/* Error banner */}
      {error && (
        <div className="mx-6 mt-4 flex items-center gap-2 border border-error/30 bg-error/5 px-4 py-2 font-mono text-xs flex-shrink-0">
          <span className="font-semibold text-error">ERR {error}</span>
        </div>
      )}

      {/* Table */}
      <div className="flex-1 overflow-auto bg-surface">
        {loading ? (
          <div className="flex h-full items-center justify-center font-mono text-xs text-muted-foreground">Loading…</div>
        ) : rows.length === 0 ? (
          <div className="flex h-full flex-col items-center justify-center gap-2">
            <span className="font-sans text-sm font-semibold text-foreground">No traffic captured</span>
            <span className="font-mono text-xs text-muted-foreground">
              Ensure the eBPF loader is running and receiving HTTP traffic
            </span>
          </div>
        ) : (
          <table className="w-full border-collapse text-left text-sm">
            <thead className="sticky top-0 bg-surface">
              <tr className="border-b border-border">
                <Th>Time</Th>
                <Th>Method</Th>
                <Th>Source</Th>
                <Th>Destination</Th>
                <Th>Path</Th>
              </tr>
            </thead>
            <tbody>
              {rows.map((r, i) => (
                <tr key={i} className="border-b border-border hover:bg-background">
                  <td className="px-6 py-2.5 font-mono text-xs text-muted-foreground">{r.time}</td>
                  <td className="px-6 py-2.5">
                    <span className={`inline-block border px-1.5 py-0.5 font-mono text-[10px] font-medium uppercase tracking-wider ${METHOD_STYLES[r.method] ?? 'border-border text-mono'}`}>
                      {r.method}
                    </span>
                  </td>
                  <td className="px-6 py-2.5 font-mono text-xs text-mono">{r.src}</td>
                  <td className="px-6 py-2.5 font-mono text-xs text-mono">
                    <span className="text-muted-foreground">→ </span>{r.dst}
                  </td>
                  <td className="px-6 py-2.5 font-mono text-xs text-foreground">{r.path}</td>
                </tr>
              ))}
            </tbody>
          </table>
        )}
      </div>

      {/* Pagination footer */}
      <div className="flex items-center justify-between border-t border-border bg-surface px-6 py-2.5 flex-shrink-0">
        <span className="font-mono text-[11px] text-muted-foreground">
          {paused
            ? `Page ${page + 1} of ${totalPages} · ${total.toLocaleString()} total`
            : `Showing latest ${rows.length} · ${total.toLocaleString()} total`}
        </span>
        {paused && (
          <div className="flex items-center gap-1">
            <PageButton onClick={() => setPage(0)} disabled={page === 0} label="«" />
            <PageButton
              onClick={() => setPage(p => Math.max(0, p - 1))}
              disabled={page === 0}
              label="Prev"
              icon={<ChevronLeft className="h-3 w-3" />}
            />
            <PageButton
              onClick={() => setPage(p => Math.min(totalPages - 1, p + 1))}
              disabled={page >= totalPages - 1}
              label="Next"
              icon={<ChevronRight className="h-3 w-3" />}
              iconRight
            />
            <PageButton onClick={() => setPage(totalPages - 1)} disabled={page >= totalPages - 1} label="»" />
          </div>
        )}
      </div>
    </div>
  );
}

function Th({ children }: { children: React.ReactNode }) {
  return <th className="px-6 py-2.5 font-mono text-[10px] font-medium uppercase tracking-wider text-muted-foreground">{children}</th>;
}

function PageButton({ onClick, disabled, label, icon, iconRight }: {
  onClick: () => void; disabled: boolean; label: string; icon?: React.ReactNode; iconRight?: boolean;
}) {
  return (
    <button
      onClick={onClick}
      disabled={disabled}
      className="flex items-center gap-1 border border-border bg-background px-2.5 py-1 font-mono text-[11px] text-foreground hover:border-border-strong disabled:opacity-40 disabled:cursor-not-allowed"
    >
      {!iconRight && icon}
      {label}
      {iconRight && icon}
    </button>
  );
}
