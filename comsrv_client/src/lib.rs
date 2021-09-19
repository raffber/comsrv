mod ws;
mod http;
mod bytestream;

use comsrv_protocol::{Response, Request};
use std::io;
use std::time::Duration;

#[async_trait]
trait Rpc {
    async fn request(&mut self, request: Request, timeout: Duration) -> io::Result<Response>;
}

