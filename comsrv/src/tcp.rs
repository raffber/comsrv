use crate::iotask::{IoContext, IoHandler, IoTask};
use crate::{inventory, Error};
use async_trait::async_trait;
use comsrv_protocol::{ByteStreamRequest, ByteStreamResponse, TcpAddress, TcpOptions};
use std::net::ToSocketAddrs;
use tokio::task::{self, JoinHandle};

use anyhow::anyhow;
use std::net::SocketAddr;
use tokio::net::TcpStream;
use tokio::time::{sleep, timeout, Duration, Instant};

const DEFAULT_DROP_DELAY: Duration = Duration::from_secs(100);
const DEFAULT_CONNECTION_TIMEOUT: Duration = Duration::from_millis(500);

#[derive(Clone)]
pub struct Instrument {
    inner: IoTask<Handler>,
}

struct Handler {
    addr: SocketAddr,
    stream: Option<TcpStream>,
    last_request: Instant,
    drop_delay: Duration,
    connection_timeout: Duration,
    drop_delay_task: Option<JoinHandle<()>>,
}

#[derive(Clone)]
pub enum TcpRequest {
    SetOptions(TcpOptions),
    Bytes {
        request: ByteStreamRequest,
        options: Option<TcpOptions>,
    },
    DropCheck,
}

impl TcpRequest {
    fn options(&self) -> Option<&TcpOptions> {
        match self {
            TcpRequest::SetOptions(x) => Some(x),
            TcpRequest::Bytes { options, .. } => options.clone(),
            TcpRequest::DropCheck => None,
        }
    }
}

pub enum TcpResponse {
    Bytes(ByteStreamResponse),
    Nope,
}

impl Handler {
    async fn handle_request(
        &mut self,
        mut stream: TcpStream,
        req: TcpRequest,
    ) -> (crate::Result<TcpResponse>, TcpStream) {
        match req {
            TcpRequest::Bytes { request, .. } => {
                let ret = crate::bytestream::handle(&mut stream, req).await;
                (ret.map(TcpResponse::Bytes), stream)
            }
            TcpRequest::DropCheck => {
                log::error!("This bit of code should not be reachable.!");
                (Err(crate::Error::internal("Unreachable code.")), stream)
            }
            TcpRequest::SetOptions(_) => {}
        }
    }

    fn set_options(&mut self, opts: &TcpOptions) {
        if let Some(drop_delay) = opts.auto_drop {
            self.drop_delay = drop_delay.into()
        }
        if let Some(connection_timeout) = opts.connection_timeout {
            self.connection_timeout = connection_timeout.into();
        }
    }
}

async fn connect_tcp_stream(
    addr: SocketAddr,
    connection_timeout: Duration,
) -> crate::Result<TcpStream> {
    let fut = async move { TcpStream::connect(&addr).await.map_err(Error::io) };
    match timeout(connection_timeout, fut).await {
        Ok(Ok(x)) => Ok(x),
        Ok(Err(x)) => Err(x),
        Err(_) => Err(crate::Error::Timeout),
    }
}

#[async_trait]
impl IoHandler for Handler {
    type Request = TcpRequest;
    type Response = TcpResponse;

    async fn handle(
        &mut self,
        ctx: &mut IoContext<Self>,
        req: Self::Request,
    ) -> crate::Result<Self::Response> {
        if let Some(opts) = req.options() {
            self.set_options(opts);
        }
        if let TcpRequest::DropCheck = &req {
            let now = Instant::now();
            if now - self.last_request > self.drop_delay {
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
                match connect_tcp_stream(addr, self.connection_timeout).await {
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
                    let ctx = ctx.clone();
                    self.drop_delay_task = Some(task::spawn(async move {
                        sleep(self.drop_delay + Duration::from_millis(100)).await;
                        let _ = ctx.send(TcpRequest::DropCheck);
                    }));
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
        let handler = Handler {
            stream: None,
            addr,
            last_request: Instant::now(),
            drop_delay: DEFAULT_DROP_DELAY,
            connection_timeout: DEFAULT_CONNECTION_TIMEOUT,
            drop_delay_task: None,
        };
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

impl inventory::Instrument for Instrument {
    type Address = TcpAddress;

    fn connect(server: &crate::app::Server, addr: &Self::Address) -> crate::Result<Self> {
        let addr = (&addr.host as &str, addr.port).to_socket_addrs();
        let iter = addr.map_err(|x| crate::Error::argument)?;
        if let Some(x) = iter.next() {
            Ok(Instrument::new(x))
        } else {
            Err(crate::Error::argument(anyhow!(
                "Invalid tcp socket address: {:?}",
                addr
            )))
        }
    }
}
