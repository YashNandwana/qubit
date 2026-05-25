import { useState, useCallback, useRef } from 'react';
import { TopNav, type Tab } from '@/components/TopNav';
import { TopologyView } from '@/views/TopologyView';
import { K8sEventsView } from '@/views/K8sEventsView';
import { NetworkEventsView } from '@/views/NetworkEventsView';

interface CoreStatus {
  ok: boolean;
  lastChecked: Date | null;
}

export default function App() {
  const [activeView, setActiveView] = useState<Tab>('topology');
  const [coreStatus, setCoreStatus] = useState<CoreStatus>({ ok: true, lastChecked: null });
  const [refreshKey, setRefreshKey] = useState(0);

  // Stable callback — inline arrow would create a new reference every render,
  // which would retrigger fetch effects in child views (the infinite-loop bug).
  const handleStatusChange = useCallback((status: CoreStatus) => setCoreStatus(status), []);
  const handleNavRefresh = useCallback(() => setRefreshKey(k => k + 1), []);

  // Kept as a ref so TopNav always calls the latest version without needing
  // handleNavRefresh to be in any effect deps.
  const refreshKeyRef = useRef(refreshKey);
  refreshKeyRef.current = refreshKey;

  return (
    <div className="flex h-full flex-col overflow-hidden bg-background">
      <TopNav
        active={activeView}
        onChange={setActiveView}
        coreStatus={coreStatus}
        onRefresh={handleNavRefresh}
      />
      <main className="flex-1 min-h-0 overflow-hidden">
        {activeView === 'topology' && (
          <TopologyView
            key="topology"
            onStatusChange={handleStatusChange}
            refreshKey={refreshKey}
          />
        )}
        {activeView === 'events' && (
          <K8sEventsView
            key="k8s-events"
            onStatusChange={handleStatusChange}
            refreshKey={refreshKey}
          />
        )}
        {activeView === 'network' && (
          <NetworkEventsView
            key="network-events"
            onStatusChange={handleStatusChange}
            refreshKey={refreshKey}
          />
        )}
      </main>
    </div>
  );
}
