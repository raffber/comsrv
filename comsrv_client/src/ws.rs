use crate::Rpc;
use async_trait::async_trait;
use comsrv_protocol::{Request, Response};
use std::io;
use std::time::Duration;
use url::Url;

type Client = wsrpc::client::Client<Request, Response>;

#[derive(Clone)]
pub struct WsRpc {
    client: Client,
}

impl WsRpc {
    pub async fn connect<A>(url: A, duration: Duration) -> io::Result<Self>
    where
        A: Into<Url>,
    {
        let client = Client::connect(url, duration).await?;
        Ok(WsRpc { client })
    }
}

#[async_trait]
impl Rpc for WsRpc {
    async fn request(&mut self, request: Request, timeout: Duration) -> crate::Result<Response> {
        Ok(self.client.query(request, timeout).await?)
    }
}
