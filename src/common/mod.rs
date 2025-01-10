use crate::etcd::register::ServerConf;
use chrono::Local;
use uuid::Uuid;

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct ServiceInstance {
    #[serde(rename = "name")]
    pub name: String,
    #[allow(unused)]
    #[serde(rename = "key")]
    pub key: String,
    #[serde(rename = "endpoint")]
    pub endpoint: String,
    // TODO: 后续可以考虑加入权重、版本等信息
}

impl AsRef<ServiceInstance> for ServiceInstance {
    fn as_ref(&self) -> &ServiceInstance {
        self
    }
}

impl ServiceInstance {
    pub fn new(
        name_space: impl AsRef<str>,
        name: impl AsRef<str>,
        endpoint: String,
    ) -> ServiceInstance {
        ServiceInstance {
            name: format!("{}/{}", name_space.as_ref(), name.as_ref()),
            key: format!(
                "{}/{}/{}/{}",
                name_space.as_ref(),
                name.as_ref(),
                Local::now().timestamp(),
                Uuid::new_v4()
            ),
            endpoint,
        }
    }
}

impl From<&ServerConf> for ServiceInstance {
    fn from(value: &ServerConf) -> Self {
        Self::new(
            value.get_model(),
            value.get_server_name(),
            value.get_endpoint().to_owned(),
        )
    }
}
