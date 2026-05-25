import { useState, useEffect, useCallback } from 'react';
import { Search, ChevronDown, ChevronLeft, ChevronRight, RefreshCw } from 'lucide-react';

const PAGE_SIZE = 50;
const REFRESH_INTERVAL_MS = 10_000;

type EventType = 'ADDED' | 'MODIFIED' | 'DELETED';

// Shape of a K8sResourceEvent as serialised by the Rust backend (snake_case).
// event_time is epoch seconds (u32); event_type is "Applied" or "Deleted".
interface RawK8sEvent {
  event_time: number;
  resource_type: string;
  name: string;
  namespace: string;
  event_type: string;
  labels: string;
  resource_data: string;
}

interface PagedResponse {
  items: RawK8sEvent[];
  total: number;
  page: number;
  page_size: number;
}

interface K8sEvent {
  time: string;
  type: EventType;
  kind: string;
  namespace: string;
  name: string;
  message: string;
}

const TYPE_STYLES: Record<EventType, string> = {
  ADDED:    'bg-success/10 text-success border-success/30',
  MODIFIED: 'bg-warning/15 text-[#8a6d00] border-warning/40',
  DELETED:  'bg-error/10 text-error border-error/30',
};

function normalizeEventType(raw: string): EventType {
  const u = raw.toUpperCase();
  // Backend sends "Applied"/"Deleted" from the K8sEventType gRPC enum.
  if (u === 'APPLIED' || u === 'ADDED') return 'ADDED';
  if (u === 'DELETED') return 'DELETED';
  return 'MODIFIED';
}

function normalize(raw: RawK8sEvent): K8sEvent {
  // event_time is epoch seconds; multiply by 1000 for JS Date constructor.
  const time = raw.event_time
    ? new Date(raw.event_time * 1000).toLocaleTimeString()
    : '—';

  // resource_data is a JSON string — try to extract a human-readable message.
  let message = '';
  try {
    const data = JSON.parse(raw.resource_data);
    message = data.reason ?? data.message ?? raw.resource_data;
  } catch {
    message = raw.resource_data ?? '';
  }

  return {
    time,
    type: normalizeEventType(raw.event_type),
    kind: raw.resource_type,
    namespace: raw.namespace,
    name: raw.name,
    message,
  };
}

interface Props {
  onStatusChange: (s: { ok: boolean; lastChecked: Date | null }) => void;
  refreshKey: number;
}

export function K8sEventsView({ onStatusChange, refreshKey }: Props) {
  const [events, setEvents] = useState<K8sEvent[]>([]);
  const [total, setTotal] = useState(0);
  const [page, setPage] = useState(0);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [search, setSearch] = useState('');
  const [typeFilter, setTypeFilter] = useState('All');
  const [nsFilter, setNsFilter] = useState('All');
  // Collect all seen namespaces across pages so the dropdown stays stable.
  const [allNamespaces, setAllNamespaces] = useState<string[]>([]);

  const fetchEvents = useCallback(async (targetPage: number) => {
    try {
      const res = await fetch(`/api/k8s-events?page=${targetPage}&page_size=${PAGE_SIZE}`);
      if (!res.ok) throw new Error(`HTTP ${res.status}`);
      const data: PagedResponse = await res.json();
      const normalised = data.items.map(normalize);
      setEvents(normalised);
      setTotal(data.total);
      setAllNamespaces(prev => {
        const merged = new Set([...prev, ...normalised.map(e => e.namespace)]);
        return Array.from(merged).sort();
      });
      setError(null);
      onStatusChange({ ok: true, lastChecked: new Date() });
    } catch (e) {
      setError(e instanceof Error ? e.message : 'fetch failed');
      onStatusChange({ ok: false, lastChecked: new Date() });
    } finally {
      setLoading(false);
    }
  }, [onStatusChange]);

  // Re-fetch when the page changes, or the nav refresh button is clicked.
  useEffect(() => { fetchEvents(page); }, [fetchEvents, page, refreshKey]);

  // Auto-refresh at current page.
  useEffect(() => {
    const id = setInterval(() => fetchEvents(page), REFRESH_INTERVAL_MS);
    return () => clearInterval(id);
  }, [fetchEvents, page]);

  // When the user changes a filter, jump back to page 0 so they don't end up
  // on a page that has no matching rows.
  const handleSearchChange = (v: string) => { setSearch(v); setPage(0); };
  const handleTypeChange = (v: string) => { setTypeFilter(v); setPage(0); };
  const handleNsChange = (v: string) => { setNsFilter(v); setPage(0); };

  const nsOptions = ['All', ...allNamespaces];
  const typeOptions = ['All', 'ADDED', 'MODIFIED', 'DELETED'];

  const filtered = events.filter(e => {
    const q = search.toLowerCase();
    const matchSearch = q === '' ||
      [e.kind, e.namespace, e.name, e.message].some(v => v.toLowerCase().includes(q));
    const matchType = typeFilter === 'All' || e.type === typeFilter;
    const matchNs = nsFilter === 'All' || e.namespace === nsFilter;
    return matchSearch && matchType && matchNs;
  });

  const totalPages = Math.max(1, Math.ceil(total / PAGE_SIZE));

  return (
    <div className="flex h-full flex-col">
      {/* Toolbar */}
      <div className="flex items-center gap-3 border-b border-border bg-surface px-6 py-3 flex-shrink-0">
        <div className="relative flex-1 max-w-sm">
          <Search className="absolute left-2.5 top-1/2 h-3.5 w-3.5 -translate-y-1/2 text-muted-foreground" />
          <input
            value={search}
            onChange={e => handleSearchChange(e.target.value)}
            placeholder="Search events…"
            className="w-full border border-border bg-background py-1.5 pl-8 pr-3 font-mono text-xs text-foreground placeholder:text-muted-foreground focus:border-primary focus:outline-none"
          />
        </div>
        <FilterDropdown label="Type" value={typeFilter} options={typeOptions} onChange={handleTypeChange} />
        <FilterDropdown label="Namespace" value={nsFilter} options={nsOptions} onChange={handleNsChange} />
        <button
          onClick={() => fetchEvents(page)}
          className="flex items-center gap-1.5 border border-border bg-background px-3 py-1.5 font-mono text-xs text-foreground hover:border-border-strong"
        >
          <RefreshCw className="h-3 w-3" />
          Refresh
        </button>
        <div className="border border-border bg-background px-2.5 py-1 font-mono text-[11px] text-muted-foreground ml-auto">
          {total.toLocaleString()} total
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
        ) : filtered.length === 0 ? (
          <div className="flex h-full flex-col items-center justify-center gap-2">
            <span className="font-sans text-sm font-semibold text-foreground">No events</span>
            <span className="font-mono text-xs text-muted-foreground">
              {total === 0
                ? 'Waiting for cluster-agent to forward K8s events'
                : 'No events on this page match the current filters'}
            </span>
          </div>
        ) : (
          <table className="w-full border-collapse text-left text-sm">
            <thead className="sticky top-0 bg-surface">
              <tr className="border-b border-border">
                <Th>Time</Th><Th>Type</Th><Th>Kind</Th><Th>Namespace</Th><Th>Name</Th><Th>Message</Th>
              </tr>
            </thead>
            <tbody>
              {filtered.map((e, i) => (
                <tr key={i} className="border-b border-border hover:bg-background">
                  <Td mono muted>{e.time}</Td>
                  <Td>
                    <span className={`inline-block border px-1.5 py-0.5 font-mono text-[10px] font-medium uppercase tracking-wider ${TYPE_STYLES[e.type]}`}>
                      {e.type}
                    </span>
                  </Td>
                  <Td>{e.kind}</Td>
                  <Td mono>{e.namespace}</Td>
                  <Td mono>{e.name}</Td>
                  <Td muted>{e.message}</Td>
                </tr>
              ))}
            </tbody>
          </table>
        )}
      </div>

      {/* Pagination footer */}
      <div className="flex items-center justify-between border-t border-border bg-surface px-6 py-2.5 flex-shrink-0">
        <span className="font-mono text-[11px] text-muted-foreground">
          Page {page + 1} of {totalPages} · {total.toLocaleString()} events
        </span>
        <div className="flex items-center gap-1">
          <PageButton
            onClick={() => setPage(0)}
            disabled={page === 0}
            label="«"
          />
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
          <PageButton
            onClick={() => setPage(totalPages - 1)}
            disabled={page >= totalPages - 1}
            label="»"
          />
        </div>
      </div>
    </div>
  );
}

// ── Small components ──────────────────────────────────────────────────────────

function Th({ children }: { children: React.ReactNode }) {
  return <th className="px-6 py-2.5 font-mono text-[10px] font-medium uppercase tracking-wider text-muted-foreground">{children}</th>;
}

function Td({ children, mono, muted }: { children: React.ReactNode; mono?: boolean; muted?: boolean }) {
  return (
    <td className={`px-6 py-2.5 text-xs ${mono ? 'font-mono' : ''} ${muted ? 'text-muted-foreground' : 'text-mono'}`}>
      {children}
    </td>
  );
}

function FilterDropdown({ label, value, options, onChange }: {
  label: string; value: string; options: string[]; onChange: (v: string) => void;
}) {
  return (
    <div className="flex items-center gap-1 border border-border bg-background px-3 py-1.5">
      <span className="font-mono text-xs text-muted-foreground">{label}:</span>
      <select
        value={value}
        onChange={e => onChange(e.target.value)}
        className="bg-transparent font-mono text-xs text-foreground focus:outline-none cursor-pointer"
      >
        {options.map(o => <option key={o} value={o}>{o}</option>)}
      </select>
      <ChevronDown className="h-3 w-3 text-muted-foreground flex-shrink-0" />
    </div>
  );
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
