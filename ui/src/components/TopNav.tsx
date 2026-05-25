import { RefreshCw } from 'lucide-react';

export type Tab = 'topology' | 'events' | 'network';

interface TopNavProps {
  active: Tab;
  onChange: (t: Tab) => void;
  coreStatus: { ok: boolean; lastChecked: Date | null };
  onRefresh: () => void;
}

export function TopNav({ active, onChange, coreStatus, onRefresh }: TopNavProps) {
  return (
    <header className="flex h-12 items-center border-b border-border bg-surface px-4 flex-shrink-0">
      {/* Logo */}
      <div className="flex w-56 items-center gap-2">
        <span className="relative flex h-2 w-2">
          <span className={`qubit-ping absolute inline-flex h-full w-full rounded-full ${coreStatus.ok ? 'bg-success' : 'bg-error'}`} />
          <span className={`qubit-pulse relative inline-flex h-2 w-2 rounded-full ${coreStatus.ok ? 'bg-success' : 'bg-error'}`} />
        </span>
        <span className="font-sans text-sm font-bold tracking-[0.18em] text-foreground">
          QUBIT
        </span>
        <span className="font-mono text-[10px] uppercase tracking-wider text-muted-foreground">
          eBPF · live
        </span>
      </div>

      {/* Tabs */}
      <nav className="flex flex-1 justify-center gap-1">
        <TabButton label="Topology" active={active === 'topology'} onClick={() => onChange('topology')} />
        <TabButton label="K8s Events" active={active === 'events'} onClick={() => onChange('events')} />
        <TabButton label="Network" active={active === 'network'} onClick={() => onChange('network')} />
      </nav>

      {/* Right: status + refresh */}
      <div className="flex w-56 items-center justify-end gap-2">
        <div className="flex items-center gap-1.5 border border-border bg-background px-2 py-1">
          <span className={`h-1.5 w-1.5 rounded-full ${coreStatus.ok ? 'bg-success' : 'bg-error'}`} />
          <span className="font-mono text-[10px] uppercase tracking-wider text-mono">
            Core · {coreStatus.ok ? 'connected' : 'offline'}
          </span>
        </div>
        <button
          aria-label="Refresh"
          onClick={onRefresh}
          className="flex h-7 w-7 items-center justify-center border border-border bg-background text-muted-foreground hover:border-border-strong hover:text-foreground"
        >
          <RefreshCw className="h-3.5 w-3.5" />
        </button>
      </div>
    </header>
  );
}

function TabButton({ label, active, onClick }: { label: string; active: boolean; onClick: () => void }) {
  return (
    <button
      onClick={onClick}
      className={`relative h-12 px-4 font-sans text-sm transition-colors ${
        active ? 'text-foreground' : 'text-muted-foreground hover:text-foreground'
      }`}
    >
      {label}
      {active && (
        <span className="absolute inset-x-3 bottom-0 h-[2px] bg-primary" />
      )}
    </button>
  );
}
