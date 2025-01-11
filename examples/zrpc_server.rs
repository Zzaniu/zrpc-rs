use crate::pb::user::{
    user_server, AddUserRequest, AddUserResponse, GetUserRequest, GetUserResponse,
};
use chrono::Local;
use std::sync::Mutex;
use tonic::metadata::MetadataValue;
use tonic::{Code, Request, Response, Status};
use tool::log::trace_log::{info, tracing_subscriber};
use zrpc::etcd::register::ServerConf;
use zrpc::sre_breaker::ServerSreBreaker;
use zrpc::Server;

mod pb;

#[derive(Debug, Default)]
pub struct UserServer {} // 这里面可以放一些 db, cache 之类的东西

pub type UserResult<T> = Result<Response<T>, Status>;

static COUNT: Mutex<usize> = Mutex::new(0);

#[tonic::async_trait]
impl user_server::User for UserServer {
    async fn add(&self, request: Request<AddUserRequest>) -> UserResult<AddUserResponse> {
        let mut count = COUNT.lock().unwrap();
        *count = count.checked_add(1).unwrap();
        info!("count = {:?}", *count);
        let user_request = request.into_inner();
        info!("user_request = {:?}", user_request);
        return Err(Status::invalid_argument("name is empty".to_owned()));
        if user_request.name.is_empty() {}
        let user_response = AddUserResponse { id: 1 };
        Ok(Response::new(user_response))
    }

    async fn get(&self, request: Request<GetUserRequest>) -> UserResult<GetUserResponse> {
        let user_request = request.into_inner();
        info!("user_request = {:?}", user_request);
        if let Some(name) = user_request.name {
            info!("name = {}", name);
            return Err(Status::new(Code::NotFound, "not found".to_string()));
        };
        let user_response = GetUserResponse {
            name: "张三".to_string(),
            age: 30,
            hobby: vec!["抽烟".to_owned()],
            timestamp: Local::now().timestamp_millis(),
        };
        Ok(Response::new(user_response))
    }
}

#[derive(Debug, serde::Deserialize)]
pub struct Config {
    #[serde(rename = "ServerConf")]
    server_conf: ServerConf,
}

fn check_auth(req: Request<()>) -> Result<Request<()>, Status> {
    let token: MetadataValue<_> = "Bearer some-secret-token".parse().unwrap();

    match req.metadata().get("authorization") {
        Some(t) if token == t => Ok(req),
        _ => Err(Status::unauthenticated("No valid auth token")),
    }
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();
    let conf_data = std::fs::read("cfg/conf.yaml").unwrap();
    let config: Config = serde_yaml::from_slice(conf_data.as_slice()).unwrap();
    let service_instance = (&config.server_conf).into();
    let register =
        zrpc::etcd::register::EtcdRegister::new(&config.server_conf.get_etcd_conf(), 10).await;

    let zrpc_server = Server::new(register, service_instance);
    zrpc_server
        .serve(|server| {
            server
                .layer(ServerSreBreaker)
                // .add_service(user_server::UserServer::with_interceptor(
                //     UserServer::default(),
                //     check_auth,
                // ))
                .add_service(user_server::UserServer::new(UserServer::default()))
        })
        .await;
}
