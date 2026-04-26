use clickhouse::Client;
use clickhouse::insert::Insert;
use futures::TryFutureExt;
use std::sync::Arc;

use crate::config::QubitConfig;
use crate::model::{EbpfNetworkEvent, K8sResourceEvent, Error};

pub struct DAO {
    config: Arc<QubitConfig>,
    client: Client,
}

impl DAO {
    pub fn new(config: Arc<QubitConfig>) -> Result<Self, String> {
        let clickhouse_url = format!("http://{}:{}", config.db.host, config.db.port);

        let client = Client::default()
            .with_url(&clickhouse_url)
            .with_database("default")
            .with_user(&config.db.user)
            .with_password(&config.db.password);

        Ok(Self { config, client }) // Store client in the struct
    }

    pub async fn initialize_schema(&self) -> Result<(), Error> {
        let create_ebpf_table = format!(
            "CREATE TABLE IF NOT EXISTS {} ({})
            ENGINE = MergeTree()
            ORDER BY (timestamp_ns, src_service, dst_service)",
            self.config.db.table.ebpf_network_events,
            EbpfNetworkEvent::CREATE_TABLE_SCHEMA
        );

        self.client
            .query(&create_ebpf_table)
            .execute()
            .await
            .map_err(|e| Error::SchemaInitializationFailed(e.to_string()))?;

        // K8s resource events table with 1 day TTL.
        // ClickHouse automatically drops expired rows during background merges.
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

    pub async fn add_event(&self,
        event: EbpfNetworkEvent) -> Result<(), Error> {
        let mut insert: Insert<EbpfNetworkEvent> = self
            .client
            .insert(&self.config.db.table.ebpf_network_events)
            .await
            .map_err(|e| Error::EventAdditionFailed(e.to_string()))?;

        insert.write(&event).await.map_err(|e| Error::EventAdditionFailed(e.to_string()))?;

        insert.end().await.map_err(|e| Error::EventAdditionFailed(e.to_string()))?;

        Ok(())
    }

    pub async fn add_events(&self,
        events: Vec<EbpfNetworkEvent>) -> Result<(), Error> {
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

    pub async fn fetch_events_by_service(&self, service_name: String) -> Result<Vec<EbpfNetworkEvent>, Error> {
        let query_str = format!(
            "SELECT * FROM {} WHERE service_name = '{}'",
            self.config.db.table.ebpf_network_events, service_name
        );

        let events: Vec<EbpfNetworkEvent> = self
            .client
            .query(&query_str)
            .fetch_all()
            .await
            .map_err(|e| Error::EventFetchingFailed(e.to_string()))?;

        Ok(events)
    }

    pub async fn get_ebpf_events_in_range(&self,
        start_time: u64,
        end_time: u64) -> Result<Vec<EbpfNetworkEvent>, Error> {
        let query_str = format!(
            "SELECT * FROM {} WHERE timestamp_ns >= {} AND timestamp_ns <= {}",
            self.config.db.table.ebpf_network_events, start_time, end_time
        );

        let events: Vec<EbpfNetworkEvent> = self
            .client
            .query(&query_str)
            .fetch_all()
            .await
            .map_err(|e| Error::EventFetchingFailed(e.to_string()))?;

        Ok(events)
    }
}
