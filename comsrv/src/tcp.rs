use crate::bytestream::read_all;
use crate::clonable_channel::ClonableChannel;
use crate::iotask::{IoHandler, IoTask};
use crate::modbus::handle_modbus_request_timeout;
use crate::Error;
use async_trait::async_trait;
use comsrv_protocol::{ByteStreamRequest, ByteStreamResponse, ModBusRequest, ModBusResponse};
use tokio::task;

use std::net::SocketAddr;
use tokio::net::TcpStream;
use tokio::time::{Duration, timeout, sleep, Instant};
use tokio_modbus::prelude::Slave;

const DROP_DELAY: Duration = Duration::from_secs(100);

#[derive(Clone)]
pub struct Instrument {
    inner: IoTask<Handler>,
}

struct Handler {
    addr: SocketAddr,
    stream: Option<TcpStream>,
    last_request: Instant,
}

#[derive(Clone)]
pub enum TcpRequest {
    Bytes(ByteStreamRequest),
    ModBus { slave_id: u8, req: ModBusRequest },
    DropCheck,
}

impl TcpRequest {
    fn timeout(&self) -> Duration {
        let default_timeout = Duration::from_millis(1000);
        match self {
            TcpRequest::Bytes(x) => x.timeout().unwrap_or(default_timeout),
            TcpRequest::ModBus { ..} => default_timeout,
            TcpRequest::DropCheck => default_timeout,
        } 
    }
}

pub enum TcpResponse {
    Bytes(ByteStreamResponse),
    ModBus(ModBusResponse),
    Nope,
}

impl Handler {
    async fn handle_request(
        &mut self,
        mut stream: TcpStream,
        req: TcpRequest,
    ) -> (crate::Result<TcpResponse>, TcpStream) {
        match req {
            TcpRequest::Bytes(req) => {
                let ret = crate::bytestream::handle(&mut stream, req).await;
                (ret.map(TcpResponse::Bytes), stream)
            }
            TcpRequest::ModBus { slave_id, req } => {
                let _ = read_all(&mut stream).await;
                let cloned = ClonableChannel::new(stream);
                let ret = tokio_modbus::client::rtu::connect_slave(cloned.clone(), Slave(slave_id))
                    .await
                    .map_err(Error::io);
                match ret {
                    Ok(mut ctx) => {
                        let timeout = Duration::from_millis(1000);
                        let ret = handle_modbus_request_timeout(&mut ctx, req, timeout).await;
                        (ret.map(TcpResponse::ModBus), cloned.take().unwrap())
                    }
                    Err(err) => (Err(err), cloned.take().unwrap()),
                }
            }
            _ => {unreachable!()}
        }
    }
}

async fn connect_tcp_stream(addr: SocketAddr, connection_timeout: Duration) -> crate::Result<TcpStream> {
    let fut = async move {
        TcpStream::connect(&addr)
            .await
            .map_err(Error::io)
    };
    match timeout(connection_timeout, fut).await {
        Ok(Ok(x)) => Ok(x),
        Ok(Err(x)) => Err(x),
        Err(_) => Err(crate::Error::Timeout)
    }
}

#[async_trait]
impl IoHandler for Handler {
    type Request = TcpRequest;
    type Response = TcpResponse;

    async fn handle(&mut self, req: Self::Request) -> crate::Result<Self::Response> {
        if let TcpRequest::DropCheck = &req {
            let now = Instant::now();
            if now - self.last_request > DROP_DELAY {
                self.stream.take();
            }
            return Ok(TcpResponse::Nope);
        }
        self.last_request = Instant::now();
        let mut tries = 0;
        let err = loop {
            tries += 1;
            let stream = if let Some(stream) = self.stream.take() {
                stream
            } else {
                let addr = self.addr.clone();
                match connect_tcp_stream(addr, req.timeout()).await {
                    Ok(stream) => stream,
                    Err(x) => {
                        if !x.should_retry() || tries > 3 {
                            break x;
                        }
                        continue;
                    }
                }
            };
            let (ret, stream) = self.handle_request(stream, req.clone()).await;
            match ret {
                Ok(ret) => {
                    self.stream.replace(stream);
                    return Ok(ret);
                }
                Err(x) => {
                    if !x.should_retry() || tries > 3 {
                        break x;
                    }
                }
            }
        };
        Err(err)
    }
}

impl Instrument {
    pub fn new(addr: SocketAddr) -> Self {
        let handler = Handler { stream: None, addr, last_request: Instant::now() };
        Self {
            inner: IoTask::new(handler),
        }
    }

    pub async fn request(&mut self, req: TcpRequest) -> crate::Result<TcpResponse> {
        let ret = self.inner.request(req).await;
        let mut copy = self.clone();
        task::spawn(async move {
            sleep(DROP_DELAY + Duration::from_millis(100)).await;
            let _ = copy.inner.request(TcpRequest::DropCheck).await;
        });
        ret
    }

    pub fn disconnect(mut self) {
        self.inner.disconnect()
    }
}
