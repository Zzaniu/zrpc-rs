use crate::common::ServiceInstance;
use crate::discovery::Discovery;
use crate::etcd::EtcdConf;
use etcd_client::{Client, EventType, GetOptions, KeyValue, WatchOptions, Watcher};
use std::str::FromStr;
use tokio::sync::mpsc::Sender;
use tonic::transport::Endpoint;
use tool::log::trace_log::{error, info};
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
                if let Ok(ServiceInstance {
                    name,
                    key,
                    endpoint,
                }) = serde_json::from_slice::<ServiceInstance>(key_value.value())
                {
                    // 如果元信息的服务名不匹配，则跳过
                    if name != service_name {
                        return;
                    }
                    if let Ok(endpoint) =
                        Endpoint::from_str(format!("http://{}", endpoint).as_str())
                    {
                        sender.send(Change::Insert(key, endpoint)).await.unwrap();
                    } else {
                        error!("invalid endpoint: {}", endpoint)
                    }
                } else {
                    error!(
                        "invalid service instance: {}",
                        String::from_utf8_lossy(key_value.value())
                    );
                }
            }
            EventType::Delete => {
                if let Ok(key) = key_value.key_str() {
                    sender.send(Change::Remove(key.to_owned())).await.unwrap()
                }
            }
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
            .expect("etcd get server failed");
        for kv in response.kvs() {
            self.load_balance(EventType::Put, kv, service_name, &sender)
                .await;
        }
    }

    async fn watch(&mut self, service_name: &str, sender: Sender<Change<String, Endpoint>>) {
        struct WatcherWrapper(Option<Watcher>);

        impl Drop for WatcherWrapper {
            fn drop(&mut self) {
                if let Some(mut watcher) = self.0.take() {
                    if let Ok(handler) = tokio::runtime::Handle::try_current() {
                        handler.spawn(async move {
                            // 取消失败了也无所谓, 反正客户端都挂了
                            watcher.cancel().await.unwrap_or_default();
                        });
                    }
                }
            }
        }

        // 启动 watch 任务失败的话, 直接挂了就行
        let (watcher, mut watch_stream) = self
            .etcd_client
            .watch(service_name, Some(WatchOptions::new().with_prefix()))
            .await
            .unwrap();

        // 自动取消 watch 任务
        let _watcher_wrapper = WatcherWrapper(Some(watcher));

        while let Ok(Some(watch_response)) = watch_stream.message().await {
            for event in watch_response.events() {
                if let Some(key_value) = event.kv() {
                    self.load_balance(event.event_type(), key_value, service_name, &sender)
                        .await;
                }
            }
        }
        info!("etcd watch server exit");
    }
}

impl EtcdDiscovery {
    pub fn new(etcd_client: Client) -> Self {
        Self { etcd_client }
    }
}
