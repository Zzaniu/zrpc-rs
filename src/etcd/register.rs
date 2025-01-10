use crate::common::ServiceInstance;
use crate::error::ZrpcError;
use crate::etcd::EtcdConf;
use crate::register::Register;
use anyhow::anyhow;
use etcd_client::{Client, PutOptions};
use std::time::Duration;
use tool::log::trace_log::info;

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct ServerConf {
    #[serde(rename = "ServerName")]
    server_name: String,
    #[serde(rename = "Model")]
    model: String,
    #[serde(rename = "Endpoint")]
    endpoint: String,
    #[serde(rename = "Etcd")]
    etcd_conf: EtcdConf,
}

impl ServerConf {
    pub fn get_endpoint(&self) -> &str {
        self.endpoint.as_str()
    }

    pub fn get_server_name(&self) -> &str {
        self.server_name.as_str()
    }

    pub fn get_model(&self) -> &str {
        self.model.as_str()
    }

    pub fn get_etcd_conf(&self) -> &EtcdConf {
        &self.etcd_conf
    }
}

pub struct EtcdRegister {
    etcd_client: Client,
    ttl: i64, // ttl 秒
    interval: Duration,
}

impl EtcdRegister {
    pub async fn new(etcd_conf: impl AsRef<EtcdConf>, ttl: i64) -> Self {
        Self {
            etcd_client: etcd_conf
                .as_ref()
                .new_etcd_client()
                .await
                .expect("new etcd client failed"),
            ttl,
            interval: Duration::from_millis((1000 * ttl / 2) as u64),
        }
    }

    async fn register_with_kv(
        &mut self,
        key: impl Into<Vec<u8>>,
        value: impl Into<Vec<u8>>,
        lease_id: i64,
    ) -> Result<(), ZrpcError> {
        self.etcd_client
            .put(key, value, Some(PutOptions::new().with_lease(lease_id)))
            .await?;
        Ok(())
    }
}

#[tonic::async_trait]
impl Register for EtcdRegister {
    async fn register(mut self, server_instance: &ServiceInstance) -> Result<(), ZrpcError> {
        let lease_response = self.etcd_client.lease_grant(self.ttl, None).await?;
        let lease_id = lease_response.id();
        self.register_with_kv(
            &*server_instance.key,
            serde_json::to_vec(server_instance)?,
            lease_id,
        )
        .await?;
        let (mut lease_keeper, mut lease_keep_stream) =
            self.etcd_client.lease_keep_alive(lease_id).await?;
        use tokio::sync::mpsc;
        let (cancel_tx, mut cancel_rx) = mpsc::unbounded_channel();
        tokio::spawn(async move {
            loop {
                tokio::select! {
                    _ = cancel_rx.recv() => {
                        info!("cancel keep_alive");
                        return;
                    }
                    _ = tokio::time::sleep(self.interval) => {
                        lease_keeper.keep_alive().await.map_err(|e| anyhow!("lease_keeper keep_alive error: {e}")).unwrap();
                    }
                }
            }
        });
        while let Ok(res) = lease_keep_stream.message().await {
            if let Some(resp) = res {
                if resp.ttl() <= 0 {
                    info!("租约已经过期, 需要重新注册");
                    cancel_tx
                        .send(())
                        .map_err(|e| anyhow!("cancel_tx send error: {e}"))?;
                    break;
                } // 说明已经过期, 可能需要重新注册
                continue;
            }
            info!("keep_alive stream over");
            cancel_tx
                .send(())
                .map_err(|e| anyhow!("cancel_tx send error: {e}"))?;
            break;
        }
        Ok(())
    }
}
