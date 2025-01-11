use crate::pb::user;
use crate::pb::user::user_client;
use std::time::Duration;
use tonic::Request;
use tower::ServiceBuilder;
use zrpc::etcd::discovery::{ClientConf, EtcdDiscovery};
use zrpc::Client;

mod pb;

#[derive(Debug, serde::Deserialize)]
struct ClientRpcConf {
    #[serde(rename = "ClientConf")]
    conf: ClientConf,
}

#[tokio::main]
async fn main() {
    let conf_data = std::fs::read("examples/cfg/client_conf.yaml").unwrap();
    let client_conf = serde_yaml::from_slice::<ClientRpcConf>(conf_data.as_slice()).unwrap();
    let etcd_client = client_conf.conf.etcd_conf.new_etcd_client().await.unwrap();
    let discovery = EtcdDiscovery::new(etcd_client);
    let client = Client::new(discovery, 50);
    let mut user_rpc_client = client
        .new_balance_client(|channel| {
            let channel = ServiceBuilder::new()
                // Interceptors can be also be applied as middleware
                .timeout(Duration::from_secs(3))
                // .layer_fn(MyMiddleware::new)
                .service(channel);
            user_client::UserClient::new(channel)
        })
        .await;
    for _ in 0..100 {
        let request = Request::new(user::AddUserRequest {
            name: "张三".to_string(),
            age: 32,
            hobby: vec!["抽烟".to_string(), "喝酒".to_string()],
            score: Default::default(),
        });
        let add_response = user_rpc_client.add(request).await;
        match add_response {
            Ok(res) => {
                println!("res = {:?}", res);
            }
            Err(err) => {
                println!("err = {:?}", err);
            }
        }
        tokio::time::sleep(Duration::from_millis(1)).await;
    }
}
