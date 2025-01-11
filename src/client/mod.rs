use crate::discovery::Discovery;
use tonic::transport::Channel;

pub struct Client<D> {
    discovery: D,
    balance_channel_capacity: usize,
}

impl<D> Client<D>
where
    D: Discovery + Send + 'static,
{
    pub fn new(discovery: D, balance_channel_capacity: usize) -> Client<D> {
        Client {
            discovery,
            balance_channel_capacity,
        }
    }

    pub async fn new_balance_client<S, F>(self, f: F) -> S
    where
        F: Fn(Channel) -> S,
    {
        let Self {
            mut discovery,
            balance_channel_capacity,
        } = self;
        let (channel, sender) = Channel::balance_channel(balance_channel_capacity);
        discovery
            .get_server("Dev168/test.rpc", sender.clone())
            .await;
        tokio::spawn(async move {
            discovery.watch("Dev168/test.rpc", sender).await;
        });
        f(channel)
    }
}
