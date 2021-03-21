use crate::bytestream::{ByteStreamRequest, ByteStreamResponse};
use crate::iotask::{IoHandler, IoTask};
use crate::Error;
use async_trait::async_trait;
use std::net::SocketAddr;
use tokio::net::TcpStream;
use tokio::time::{delay_for, Duration};

#[derive(Clone)]
pub struct Instrument {
    inner: IoTask<Handler>,
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
            TcpStream::connect(&self.addr.clone())
                .await
                .map_err(Error::io)?
        };
        let ret = crate::bytestream::handle(&mut stream, req.clone()).await;
        match ret {
            Ok(ret) => {
                // stream was ok, reinsert back
                self.stream.replace(stream);
                Ok(ret)
            }
            Err(err) => {
                drop(stream);
                if err.should_retry() {
                    delay_for(Duration::from_millis(100)).await;
                    let mut stream = TcpStream::connect(&self.addr.clone())
                        .await
                        .map_err(Error::io)?;
                    let ret = crate::bytestream::handle(&mut stream, req).await;
                    if ret.is_ok() {
                        // this time we succeeded, reinsert stream
                        self.stream.replace(stream);
                    }
                    ret
                } else {
                    Err(err)
                }
            }
        }
    }
}

impl Instrument {
    pub fn new(addr: SocketAddr) -> Self {
        let handler = Handler { stream: None, addr };
        Self {
            inner: IoTask::new(handler),
        }
    }

    pub async fn request(&mut self, req: ByteStreamRequest) -> crate::Result<ByteStreamResponse> {
        self.inner.request(req).await
    }

    pub fn disconnect(mut self) {
        self.inner.disconnect()
    }
}
