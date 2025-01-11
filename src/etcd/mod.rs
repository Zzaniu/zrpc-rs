pub mod discovery;
pub mod register;

use etcd_client::{Client, ConnectOptions};
use std::time::Duration;

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct EtcdConf {
    #[serde(rename = "Hosts")]
    pub hosts: String,
    #[serde(rename = "User", skip_serializing_if = "Option::is_none")]
    pub user: Option<String>,
    #[serde(rename = "Pass", skip_serializing_if = "Option::is_none")]
    pub pass: Option<String>,
}

impl AsRef<EtcdConf> for EtcdConf {
    fn as_ref(&self) -> &EtcdConf {
        self
    }
}

impl EtcdConf {
    pub async fn new_etcd_client(&self) -> anyhow::Result<Client> {
        let conn_option = ConnectOptions::new().with_connect_timeout(Duration::from_secs(1));
        let endpoint: Vec<&str> = self.hosts.split(",").collect();
        let etcd_client = Client::connect(endpoint, Some(conn_option)).await?;
        Ok(etcd_client)
    }
}
