use std::sync::Arc;

use k8s_openapi::api::core::v1::Event;

use super::informer::EventHandler;
use crate::service::ClusterAggregator;

/// Dedicated handler for K8s Event resources. Unlike GenericHandler, this
/// extracts debugging-critical fields: reason, message, type, and the
/// involved object (what resource this event is about).
///
/// These are the events the AI agent cares most about during debugging:
/// "CrashLoopBackOff", "OOMKilled", "FailedScheduling", "Unhealthy", etc.
pub struct K8sEventHandler {
    aggregator: Arc<ClusterAggregator>,
}

impl K8sEventHandler {
    pub fn new(aggregator: Arc<ClusterAggregator>) -> Self {
        Self { aggregator }
    }
}

impl EventHandler<Event> for K8sEventHandler {
    fn on_apply(&self, event: &Event) {
        let name = event.metadata.name.clone().unwrap_or_default();
        let namespace = event.metadata.namespace.clone().unwrap_or_default();
        let labels = event.metadata.labels.clone().unwrap_or_default();

        let resource_data = build_event_data(event);
        let aggregator = self.aggregator.clone();
        tokio::spawn(async move {
            if let Err(e) = aggregator
                .send_k8s_resource_event(
                    "Event".to_string(),
                    name.clone(),
                    namespace,
                    crate::proto::qubit::K8sEventType::Applied,
                    labels.into_iter().collect(),
                    resource_data,
                )
                .await
            {
                log::error!("Failed to send Event (name={}): {}", name, e);
            }
        });
    }

    fn on_delete(&self, _event: &Event) {
        // K8s Events are ephemeral — deletions aren't meaningful for debugging
    }

    fn on_init_apply(&self, event: &Event) {
        self.on_apply(event);
    }

    fn on_init_done(&self) {
        log::info!("Event initial sync complete");
    }
}

/// Builds a JSON string with the debugging-relevant fields from a K8s Event.
fn build_event_data(event: &Event) -> String {
    let reason = event.reason.as_deref().unwrap_or("");
    let message = event.message.as_deref().unwrap_or("");
    let event_type = event.type_.as_deref().unwrap_or("Normal");
    let count = event.count.unwrap_or(1);

    // involved_object tells us WHAT resource this event is about
    let obj = &event.involved_object;
    let involved_kind = obj.kind.as_deref().unwrap_or("");
    let involved_name = obj.name.as_deref().unwrap_or("");
    let involved_namespace = obj.namespace.as_deref().unwrap_or("");

    format!(
        r#"{{"reason":"{}","message":"{}","type":"{}","count":{},"involved_kind":"{}","involved_name":"{}","involved_namespace":"{}"}}"#,
        escape_json(reason),
        escape_json(message),
        escape_json(event_type),
        count,
        escape_json(involved_kind),
        escape_json(involved_name),
        escape_json(involved_namespace),
    )
}

/// Minimal JSON string escaping for hand-built JSON.
fn escape_json(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
}
