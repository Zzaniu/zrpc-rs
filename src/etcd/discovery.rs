use crate::common::ServiceInstance;
use crate::discovery::Discovery;
use crate::etcd::EtcdConf;
use anyhow::anyhow;
use etcd_client::{Client, EventType, GetOptions, KeyValue, WatchOptions, Watcher};
use std::str::FromStr;
use tokio::sync::mpsc::Sender;
use tonic::transport::Endpoint;
use tower::discover::Change;

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct ClientConf {
    #[serde(rename = "Model")]
    model: String,
    #[serde(rename = "Etcd")]
    etcd_conf: EtcdConf,
}

pub struct EtcdDiscovery {
    etcd_client: Client,
    sender: Sender<Change<String, Endpoint>>,
}

impl EtcdDiscovery {
    async fn load_balance(&self, event_type: EventType, key_value: &KeyValue, service_name: &str) {
        match event_type {
            EventType::Put => {
                let ServiceInstance {
                    name,
                    key,
                    endpoint,
                } = serde_json::from_slice::<ServiceInstance>(key_value.value()).unwrap();
                // 如果元信息的服务名不匹配，则跳过
                if name != service_name {
                    return;
                }
                let endpoint = Endpoint::from_str(format!("http://{}", endpoint).as_str()).unwrap();
                self.sender
                    .send(Change::Insert(key, endpoint))
                    .await
                    .unwrap();
            }
            EventType::Delete => self
                .sender
                .send(Change::Remove(key_value.key_str().unwrap().to_string()))
                .await
                .unwrap(),
        }
    }
}

#[tonic::async_trait]
impl Discovery for EtcdDiscovery {
    async fn get_server(&mut self, service_name: &str) {
        let options = GetOptions::new().with_prefix();
        let response = self
            .etcd_client
            .get(service_name, Some(options))
            .await
            .unwrap();
        for kv in response.kvs() {
            self.load_balance(EventType::Put, kv, service_name).await;
        }
    }

    async fn watch(&mut self, service_name: &str) {
        let options = WatchOptions::new().with_prefix();

        struct WatcherWrapper(Watcher);

        impl Drop for WatcherWrapper {
            fn drop(&mut self) {
                if let Ok(handle) = tokio::runtime::Handle::try_current() {
                    handle
                        .block_on(self.0.cancel())
                        .map_err(|e| anyhow!("canal watch error: {e}"))
                        .unwrap();
                }
            }
        }

        let (watcher, mut watch_stream) = self
            .etcd_client
            .watch(service_name, Some(options))
            .await
            .unwrap();
        let _watcher_wrapper = WatcherWrapper(watcher);
        while let Some(watch_response) = watch_stream.message().await.unwrap() {
            for event in watch_response.events() {
                self.load_balance(event.event_type(), event.kv().unwrap(), service_name)
                    .await;
            }
        }
    }
}

impl EtcdDiscovery {
    pub fn new(etcd_client: Client, sender: Sender<Change<String, Endpoint>>) -> Self {
        Self {
            etcd_client,
            sender,
        }
    }
}
