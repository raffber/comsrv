use crate::Rpc;
use comsrv_protocol::{Request, Response};
use async_trait::async_trait;
use std::time::Duration;

#[derive(Clone)]
pub struct HttpRpc {

}

#[async_trait]
impl Rpc for HttpRpc {
    async fn request(&mut self, request: Request, timeout: Duration) -> crate::Result<Response> {
        todo!()
    }
}