use tokio::sync::mpsc::Sender;
use tonic::transport::Endpoint;
use tower::discover::Change;

#[tonic::async_trait]
pub trait Discovery {
    async fn get_server(&mut self, service_name: &str, sender: Sender<Change<String, Endpoint>>);
    async fn watch(&mut self, service_name: &str, sender: Sender<Change<String, Endpoint>>);
}
