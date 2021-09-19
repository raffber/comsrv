use crate::Rpc;
use comsrv_protocol::{Request, Response};
use std::time::Duration;
use std::io;
use async_trait::async_trait;

type Client = wsrpc::client::Client<Request, Response>;

pub struct WsRpc {
    client: Client,
}

impl WsRpc {
    pub async fn connect<A>(url: A, duration: Duration) -> io::Result<Self>
        where
            A: Into<Url>,
    {
        let client = Client::connect(url, duration)?;
        Ok(WsRpc { client })
    }
}

#[async_trait]
impl Rpc for WsRpc {
    async fn request(&mut self, request: Request, timeout: Duration) -> std::io::Result<Response> {
        self.client.query()
        todo!()
    }
}