use async_trait::async_trait;
use kube::Client;

#[async_trait]
pub trait Informer: Send + Sync {
    async fn start(&self, client: Client) -> Result<(), String>;

    fn event_handler(&self);

    fn process_add_event(&self);

    fn process_update_event(&self);

    fn process_delete_event(&self);
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InformerType {
    Service,
    ConfigMap,
}