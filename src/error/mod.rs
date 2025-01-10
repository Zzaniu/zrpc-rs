#[derive(Debug, thiserror::Error)]
pub enum ZrpcError {
    #[error("Serde Error: {0}")]
    SerdeError(#[from] serde_json::Error),
    #[error("ETCD Error: {0}")]
    EtcdError(#[from] etcd_client::Error),
    #[error(transparent)]
    Unexpected(#[from] anyhow::Error),
}
