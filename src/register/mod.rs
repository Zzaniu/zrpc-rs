use crate::common::ServiceInstance;
use crate::error::ZrpcError;

#[tonic::async_trait]
pub trait Register {
    async fn register(mut self, server_instance: &ServiceInstance) -> Result<(), ZrpcError>;
    // fn deregister();
}
