use crate::common::ServiceInstance;
use crate::discovery::Discovery;
use crate::etcd::EtcdConf;
use etcd_client::{Client, EventType, GetOptions, KeyValue, WatchOptions};
use std::str::FromStr;
use tokio::sync::mpsc::Sender;
use tonic::transport::Endpoint;
use tower::discover::Change;

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct ClientConf {
    #[serde(rename = "Model")]
    pub model: String,
    #[serde(rename = "Etcd")]
    pub etcd_conf: EtcdConf,
}

pub struct EtcdDiscovery {
    etcd_client: Client,
}

impl EtcdDiscovery {
    async fn load_balance(
        &self,
        event_type: EventType,
        key_value: &KeyValue,
        service_name: &str,
        sender: &Sender<Change<String, Endpoint>>,
    ) {
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
                sender.send(Change::Insert(key, endpoint)).await.unwrap();
            }
            EventType::Delete => sender
                .send(Change::Remove(key_value.key_str().unwrap().to_string()))
                .await
                .unwrap(),
        }
    }
}

#[tonic::async_trait]
impl Discovery for EtcdDiscovery {
    async fn get_server(&mut self, service_name: &str, sender: Sender<Change<String, Endpoint>>) {
        let options = GetOptions::new().with_prefix();
        let response = self
            .etcd_client
            .get(service_name, Some(options))
            .await
            .unwrap();
        for kv in response.kvs() {
            self.load_balance(EventType::Put, kv, service_name, &sender)
                .await;
        }
    }

    async fn watch(&mut self, service_name: &str, sender: Sender<Change<String, Endpoint>>) {
        let (mut watcher, mut watch_stream) = self
            .etcd_client
            .watch(service_name, Some(WatchOptions::new().with_prefix()))
            .await
            .unwrap();
        while let Some(watch_response) = watch_stream.message().await.unwrap() {
            for event in watch_response.events() {
                self.load_balance(
                    event.event_type(),
                    event.kv().unwrap(),
                    service_name,
                    &sender,
                )
                .await;
            }
        }
        watcher.cancel().await.unwrap();
    }
}

impl EtcdDiscovery {
    pub fn new(etcd_client: Client) -> Self {
        Self { etcd_client }
    }
}
