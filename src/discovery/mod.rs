#[tonic::async_trait]
pub trait Discovery {
    async fn get_server(&mut self, service_name: &str);
    async fn watch(&mut self, service_name: &str);
}
