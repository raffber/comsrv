use crate::bytestream::{ByteStreamRequest, ByteStreamResponse};
use crate::iotask::{IoHandler, IoTask};
use crate::Error;
use async_trait::async_trait;
use std::net::SocketAddr;
use tokio::net::TcpStream;
use tokio::time::{delay_for, Duration};
use crate::modbus::{ModBusRequest, ModBusResponse, handle_modbus_request};
use crate::clonable_channel::ClonableChannel;
use tokio_modbus::client::Context;
use tokio_modbus::prelude::Slave;

#[derive(Clone)]
pub struct Instrument {
    inner: IoTask<Handler>,
}

struct Handler {
    addr: SocketAddr,
    stream: Option<TcpStream>,
}

#[derive(Clone)]
pub enum TcpRequest {
    Bytes(ByteStreamRequest),
    ModBus{
        slave_id: u8,
        req: ModBusRequest,
    },
}

pub enum TcpResponse {
    Bytes(ByteStreamResponse),
    ModBus(ModBusResponse),
}

impl Handler {
    async fn handle_request(&mut self, mut stream: TcpStream, req: TcpRequest) -> (crate::Result<TcpResponse>, TcpStream) {
        match req {
            TcpRequest::Bytes(req) => {
                let ret = crate::bytestream::handle(&mut stream, req).await;
                (ret.map(TcpResponse::Bytes), stream)
            }
            TcpRequest::ModBus { slave_id, req } => {
                let cloned = ClonableChannel::new(stream);
                let ret = tokio_modbus::client::rtu::connect_slave(cloned.clone(), Slave(slave_id)).await.map_err(Error::io);
                match ret {
                    Ok(mut ctx) => {
                        let ret = handle_modbus_request(&mut ctx, req).await;
                        (ret.map(TcpResponse::ModBus), cloned.take().unwrap())
                    }
                    Err(err) => (Err(err), cloned.take().unwrap())
                }
            }
        }

    }
}

#[async_trait]
impl IoHandler for Handler {
    type Request = TcpRequest;
    type Response = TcpResponse;

    async fn handle(&mut self, req: Self::Request) -> crate::Result<Self::Response> {
        let mut stream = if let Some(stream) = self.stream.take() {
            stream
        } else {
            TcpStream::connect(&self.addr.clone())
                .await
                .map_err(Error::io)?
        };
        let (ret, stream) = self.handle_request(stream, req.clone()).await;
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
                    let (ret, stream) = self.handle_request(stream, req).await;
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

    pub async fn request(&mut self, req: TcpRequest) -> crate::Result<TcpResponse> {
        self.inner.request(req).await
    }

    pub fn disconnect(mut self) {
        self.inner.disconnect()
    }
}
