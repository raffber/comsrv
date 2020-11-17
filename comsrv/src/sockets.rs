use crate::iotask::{IoTask, IoHandler};
use tokio::net::TcpStream;
use crate::bytestream::{ByteStreamRequest, ByteStreamResponse};
use async_std::net::SocketAddr;
use crate::Error;
use async_trait::async_trait;

#[derive(Clone)]
pub struct Instrument {
    inner: IoTask<Handler>
}

struct Handler {
    addr: SocketAddr,
    stream: Option<TcpStream>,
}

#[async_trait]
impl IoHandler for Handler {
    type Request = ByteStreamRequest;
    type Response = ByteStreamResponse;

    async fn handle(&mut self, req: Self::Request) -> crate::Result<Self::Response> {
        let mut stream = if let Some(stream) = self.stream.take() {
            stream
        } else {
            TcpStream::connect(&self.addr.clone()).await.map_err(Error::io)?
        };
        let ret = crate::bytestream::handle(&mut stream, req).await;
        self.stream.replace(stream);
        ret
    }
}
