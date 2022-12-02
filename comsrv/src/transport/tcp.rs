use crate::app::Server;
use crate::iotask::{IoContext, IoHandler, IoTask};
use crate::protocol::cobs_stream::CobsStream;
use crate::{inventory, Error};
use async_trait::async_trait;
use comsrv_protocol::cobs_stream::{CobsStreamRequest, CobsStreamResponse};
use comsrv_protocol::{
    ByteStreamInstrument, ByteStreamRequest, ByteStreamResponse, TcpAddress, TcpInstrument, TcpOptions,
};
use std::io;
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
    cobs_stream: Option<CobsStream>,
    cobs_stream_use_crc: bool,
    server: Server,
}

#[derive(Clone)]
pub enum TcpRequest {
    #[allow(dead_code)]
    SetOptions(TcpOptions),
    Bytes {
        request: ByteStreamRequest,
        options: Option<TcpOptions>,
    },
    Cobs {
        request: CobsStreamRequest,
        options: Option<TcpOptions>,
    },
    DropCheck,
}

impl TcpRequest {
    fn options(&self) -> Option<&TcpOptions> {
        match self {
            TcpRequest::SetOptions(x) => Some(x),
            TcpRequest::Bytes { options, .. } => options.as_ref(),
            TcpRequest::Cobs { options, .. } => options.as_ref(),
            _ => None,
        }
    }
}

pub enum TcpResponse {
    Bytes(ByteStreamResponse),
    Cobs(CobsStreamResponse),
    Nope,
}

impl Handler {
    fn set_options(&mut self, opts: &TcpOptions) {
        if let Some(drop_delay) = &opts.auto_drop {
            self.drop_delay = drop_delay.clone().into();
        }
        if let Some(connection_timeout) = &opts.connection_timeout {
            self.connection_timeout = connection_timeout.clone().into();
        }
    }

    fn close_stream(&mut self) {
        self.stream.take();
        if let Some(x) = self.cobs_stream.take() {
            x.cancel()
        }
    }

    fn check_close(&mut self, req: &TcpRequest) -> Option<crate::Result<TcpResponse>> {
        if matches!(
            req,
            TcpRequest::Bytes {
                request: ByteStreamRequest::Disconnect,
                ..
            }
        ) {
            self.close_stream();
            return Some(Ok(TcpResponse::Bytes(ByteStreamResponse::Done)));
        }
        if matches!(
            req,
            TcpRequest::Cobs {
                request: CobsStreamRequest::Stop,
                ..
            }
        ) {
            self.close_stream();
            return Some(Ok(TcpResponse::Cobs(CobsStreamResponse::Done)));
        }

        if let TcpRequest::DropCheck = &req {
            let now = Instant::now();
            if now - self.last_request > self.drop_delay {
                self.stream.take();
            }
            return Some(Ok(TcpResponse::Nope));
        }
        if let Some(x) = self.drop_delay_task.take() {
            x.abort();
        }
        None
    }

    fn create_byte_stream_instrument(&self) -> ByteStreamInstrument {
        ByteStreamInstrument::Tcp(TcpInstrument {
            address: TcpAddress {
                host: self.addr.ip().to_string(),
                port: self.addr.port(),
            },
            options: None,
        })
    }

    async fn handle_cobs_request(&mut self, req: CobsStreamRequest) -> crate::Result<TcpResponse> {
        if let CobsStreamRequest::Start { use_crc } = req {
            self.cobs_stream_use_crc = use_crc;
        }

        let cobs_stream = match self.cobs_stream.take() {
            Some(cobs_stream) if cobs_stream.is_alive() && cobs_stream.use_crc() == self.cobs_stream_use_crc => {
                cobs_stream
            }
            _ => {
                let stream = connect_tcp_stream(self.addr, self.connection_timeout).await?;
                let (read, write) = tokio::io::split(stream);
                CobsStream::start(
                    read,
                    write,
                    self.server.clone(),
                    self.create_byte_stream_instrument(),
                    self.cobs_stream_use_crc,
                )
            }
        };

        if let CobsStreamRequest::SendFrame { data } = req {
            // NOTE: this could cause a race condition is the stream drop between the .is_alive() call above
            // but that seems unlikely and not a big issue if it happens (an error is returned, just not an accurate one)
            cobs_stream.send(data)?;
        }

        self.cobs_stream.replace(cobs_stream);

        Ok(TcpResponse::Cobs(CobsStreamResponse::Done))
    }

    async fn handle_bytestream_request(
        &mut self,
        req: ByteStreamRequest,
        ctx: &mut IoContext<Self>,
    ) -> crate::Result<TcpResponse> {
        self.last_request = Instant::now();
        let mut tries = 0;
        let err = loop {
            tries += 1;
            let mut stream = if let Some(stream) = self.stream.take() {
                stream
            } else {
                let addr = self.addr;
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

            let ret = crate::protocol::bytestream::handle(&mut stream, req.clone())
                .await
                .map(TcpResponse::Bytes);
            match ret {
                Ok(ret) => {
                    self.stream.replace(stream);
                    let mut ctx = ctx.clone();
                    let drop_delay = self.drop_delay;
                    self.drop_delay_task = Some(task::spawn(async move {
                        sleep(drop_delay + Duration::from_millis(100)).await;
                        ctx.send(TcpRequest::DropCheck);
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

async fn connect_tcp_stream(addr: SocketAddr, connection_timeout: Duration) -> crate::Result<TcpStream> {
    let fut = async move { TcpStream::connect(&addr).await.map_err(Error::transport) };
    match timeout(connection_timeout, fut).await {
        Ok(Ok(x)) => Ok(x),
        Ok(Err(x)) => Err(x),
        Err(_) => Err(crate::Error::transport(io::Error::new(
            io::ErrorKind::TimedOut,
            "Connection timed out",
        ))),
    }
}

#[async_trait]
impl IoHandler for Handler {
    type Request = TcpRequest;
    type Response = TcpResponse;

    async fn handle(&mut self, ctx: &mut IoContext<Self>, req: Self::Request) -> crate::Result<Self::Response> {
        if let Some(opts) = req.options() {
            self.set_options(opts);
        }
        if let Some(ret) = self.check_close(&req) {
            return ret;
        }
        if let TcpRequest::Cobs {
            request: cobs_request, ..
        } = req
        {
            return self.handle_cobs_request(cobs_request).await;
        }
        if let Some(x) = self.cobs_stream.take() {
            x.cancel()
        }
        match req {
            TcpRequest::Bytes { request, .. } => self.handle_bytestream_request(request, ctx).await,
            _ => Err(crate::Error::internal(anyhow!("Unreachable code."))),
        }
    }
}

impl Instrument {
    pub fn new(addr: SocketAddr, server: Server) -> Self {
        let handler = Handler {
            stream: None,
            addr,
            last_request: Instant::now(),
            drop_delay: DEFAULT_DROP_DELAY,
            connection_timeout: DEFAULT_CONNECTION_TIMEOUT,
            drop_delay_task: None,
            cobs_stream: None,
            server,
            cobs_stream_use_crc: false,
        };
        Self {
            inner: IoTask::new(handler),
        }
    }

    pub async fn request(&mut self, req: TcpRequest) -> crate::Result<TcpResponse> {
        self.inner.request(req).await
    }
}

#[async_trait]
impl inventory::Instrument for Instrument {
    type Address = TcpAddress;

    fn connect(server: &crate::app::Server, addr: &Self::Address) -> crate::Result<Self> {
        let format_addr = (&addr.host as &str, addr.port).to_socket_addrs();
        let mut iter = format_addr.map_err(crate::Error::argument)?;
        if let Some(x) = iter.next() {
            Ok(Instrument::new(x, server.clone()))
        } else {
            Err(crate::Error::argument(anyhow!("Invalid tcp socket address: {:?}", addr)))
        }
    }

    async fn wait_for_closed(&self) {
        self.inner.wait_for_closed().await
    }
}
