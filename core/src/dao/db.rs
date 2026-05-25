use clickhouse::insert::Insert;
use clickhouse::Client;
use clickhouse::Row;
use serde::Deserialize;
use std::sync::Arc;

use crate::config::QubitConfig;
use crate::model::{EbpfNetworkEvent, K8sResourceEvent, Error};

pub struct DAO {
    config: Arc<QubitConfig>,
    client: Client,
}

// Used only to deserialize the single-row COUNT result from ClickHouse.
// ClickHouse's count() function returns a UInt64; we alias the column to `c`
// so serde knows which field to populate.
#[derive(Row, Deserialize)]
struct CountRow {
    c: u64,
}

impl DAO {
    pub fn new(config: Arc<QubitConfig>) -> Result<Self, String> {
        let clickhouse_url = format!("http://{}:{}", config.db.host, config.db.port);

        let client = Client::default()
            .with_url(&clickhouse_url)
            .with_database("default")
            .with_user(&config.db.user)
            .with_password(&config.db.password);

        Ok(Self { config, client })
    }

    pub async fn initialize_schema(&self) -> Result<(), Error> {
        // eBPF network events — 7-day TTL.
        //
        // ClickHouse MergeTree reserves ~1 MiB of disk per insert even for small
        // batches. Without a TTL the table grows unboundedly and eventually that
        // reservation fails (NOT_ENOUGH_SPACE, code 243).
        //
        // TTL expression: convert nanosecond timestamp → ClickHouse DateTime, add
        // 7 days. Rows are dropped during background merges; to reclaim space
        // immediately run:  OPTIMIZE TABLE <name> FINAL
        let create_ebpf_table = format!(
            "CREATE TABLE IF NOT EXISTS {} ({})
            ENGINE = MergeTree()
            ORDER BY (timestamp_ns, src_service, dst_service)
            TTL toDateTime(intDiv(timestamp_ns, 1000000000)) + INTERVAL 7 DAY",
            self.config.db.table.ebpf_network_events,
            EbpfNetworkEvent::CREATE_TABLE_SCHEMA
        );

        self.client
            .query(&create_ebpf_table)
            .execute()
            .await
            .map_err(|e| Error::SchemaInitializationFailed(e.to_string()))?;

        // ALTER TABLE is idempotent — it replaces the TTL rule in place.
        // This handles tables that were created before TTL was added so they
        // get the same cleanup schedule without needing a DROP + recreate.
        let alter_ebpf_ttl = format!(
            "ALTER TABLE {} MODIFY TTL \
             toDateTime(intDiv(timestamp_ns, 1000000000)) + INTERVAL 7 DAY",
            self.config.db.table.ebpf_network_events
        );
        self.client
            .query(&alter_ebpf_ttl)
            .execute()
            .await
            .map_err(|e| Error::SchemaInitializationFailed(e.to_string()))?;

        log::info!(
            "eBPF events TTL: 7 days on {}",
            self.config.db.table.ebpf_network_events
        );

        // K8s resource events — 1-day TTL (unchanged).
        let create_k8s_table = format!(
            "CREATE TABLE IF NOT EXISTS {} ({})
            ENGINE = MergeTree()
            ORDER BY (event_time, namespace, resource_type)
            TTL event_time + INTERVAL 1 DAY",
            self.config.db.table.k8s_resource_events,
            K8sResourceEvent::CREATE_TABLE_SCHEMA
        );

        self.client
            .query(&create_k8s_table)
            .execute()
            .await
            .map_err(|e| Error::SchemaInitializationFailed(e.to_string()))?;

        Ok(())
    }

    pub async fn add_event(&self, event: EbpfNetworkEvent) -> Result<(), Error> {
        let mut insert: Insert<EbpfNetworkEvent> = self
            .client
            .insert(&self.config.db.table.ebpf_network_events)
            .await
            .map_err(|e| Error::EventAdditionFailed(e.to_string()))?;

        insert.write(&event).await.map_err(|e| Error::EventAdditionFailed(e.to_string()))?;
        insert.end().await.map_err(|e| Error::EventAdditionFailed(e.to_string()))?;
        Ok(())
    }

    pub async fn add_events(&self, events: Vec<EbpfNetworkEvent>) -> Result<(), Error> {
        let mut insert: Insert<EbpfNetworkEvent> = self
            .client
            .insert(&self.config.db.table.ebpf_network_events)
            .await
            .map_err(|e| Error::EventAdditionFailed(e.to_string()))?;
        for event in events {
            insert.write(&event).await.map_err(|e| Error::EventAdditionFailed(e.to_string()))?;
        }
        insert.end().await.map_err(|e| Error::EventAdditionFailed(e.to_string()))?;
        Ok(())
    }

    pub async fn add_k8s_resource_event(&self, event: K8sResourceEvent) -> Result<(), Error> {
        let mut insert: Insert<K8sResourceEvent> = self
            .client
            .insert(&self.config.db.table.k8s_resource_events)
            .await
            .map_err(|e| Error::EventAdditionFailed(e.to_string()))?;

        insert.write(&event).await.map_err(|e| Error::EventAdditionFailed(e.to_string()))?;
        insert.end().await.map_err(|e| Error::EventAdditionFailed(e.to_string()))?;
        Ok(())
    }

    pub async fn get_k8s_events_paginated(
        &self,
        page: u64,
        page_size: u64,
    ) -> Result<(Vec<K8sResourceEvent>, u64), Error> {
        let offset = page * page_size;

        let count_sql = format!(
            "SELECT count() AS c FROM {}",
            self.config.db.table.k8s_resource_events
        );
        let CountRow { c: total } = self
            .client
            .query(&count_sql)
            .fetch_one::<CountRow>()
            .await
            .map_err(|e| Error::EventFetchingFailed(e.to_string()))?;

        let data_sql = format!(
            "SELECT * FROM {} ORDER BY event_time DESC LIMIT {} OFFSET {}",
            self.config.db.table.k8s_resource_events, page_size, offset
        );
        let items = self
            .client
            .query(&data_sql)
            .fetch_all::<K8sResourceEvent>()
            .await
            .map_err(|e| Error::EventFetchingFailed(e.to_string()))?;

        Ok((items, total))
    }

    /// Returns the `page`-th page of eBPF network events, newest first,
    /// together with the total row count.
    pub async fn get_network_events_paginated(
        &self,
        page: u64,
        page_size: u64,
    ) -> Result<(Vec<EbpfNetworkEvent>, u64), Error> {
        let offset = page * page_size;

        let count_sql = format!(
            "SELECT count() AS c FROM {}",
            self.config.db.table.ebpf_network_events
        );
        let CountRow { c: total } = self
            .client
            .query(&count_sql)
            .fetch_one::<CountRow>()
            .await
            .map_err(|e| Error::EventFetchingFailed(e.to_string()))?;

        let data_sql = format!(
            "SELECT * FROM {} ORDER BY timestamp_ns DESC LIMIT {} OFFSET {}",
            self.config.db.table.ebpf_network_events, page_size, offset
        );
        let items = self
            .client
            .query(&data_sql)
            .fetch_all::<EbpfNetworkEvent>()
            .await
            .map_err(|e| Error::EventFetchingFailed(e.to_string()))?;

        Ok((items, total))
    }

    pub async fn fetch_events_by_service(&self, service_name: String) -> Result<Vec<EbpfNetworkEvent>, Error> {
        let query_str = format!(
            "SELECT * FROM {} WHERE src_service = '{}'",
            self.config.db.table.ebpf_network_events, service_name
        );
        let events = self
            .client
            .query(&query_str)
            .fetch_all::<EbpfNetworkEvent>()
            .await
            .map_err(|e| Error::EventFetchingFailed(e.to_string()))?;
        Ok(events)
    }

    pub async fn get_ebpf_events_in_range(
        &self,
        start_time: u64,
        end_time: u64,
    ) -> Result<Vec<EbpfNetworkEvent>, Error> {
        let query_str = format!(
            "SELECT * FROM {} WHERE timestamp_ns >= {} AND timestamp_ns <= {}",
            self.config.db.table.ebpf_network_events, start_time, end_time
        );
        let events = self
            .client
            .query(&query_str)
            .fetch_all::<EbpfNetworkEvent>()
            .await
            .map_err(|e| Error::EventFetchingFailed(e.to_string()))?;
        Ok(events)
    }
}
