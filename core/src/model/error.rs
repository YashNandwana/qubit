#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("Failed to initialize ClickHouse schema: {0}")]
    SchemaInitializationFailed(String),

    #[error("Failed to add event: {0}")]
    EventAdditionFailed(String),

    #[error("Failed to fetch events: {0}")]
    EventFetchingFailed(String),

    #[error("Failed to record ebpf event: {0}")]
    EbpfEventRecordingFailed(String),

    #[error("Envoy admin API request failed: {0}")]
    EnvoyRequestFailed(String),
}
