use crate::Rpc;
use comsrv_protocol::{Request, Response};
use async_trait::async_trait;
use std::time::Duration;

pub struct HttpRpc {

}

#[async_trait]
impl Rpc for HttpRpc {
    async fn request(&mut self, request: Request, timeout: Duration) -> std::io::Result<Response> {
        todo!()
    }
}