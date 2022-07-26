use async_trait::async_trait;
use async_vxi11::CoreClient;
use std::net::{IpAddr, ToSocketAddrs};
use std::time::Instant;
use tokio::task::{self, JoinHandle};
use tokio::time::{sleep, Duration};

use crate::iotask::{IoContext, IoHandler, IoTask};
use crate::{protocol::scpi, Error};
use anyhow::anyhow;
use comsrv_protocol::{ScpiRequest, ScpiResponse};

const DEFAULT_TERMINATION: &str = "\n";

const DEFAULT_CONNECTION_TIMEOUT: Duration = Duration::from_secs(10);
const DEFAULT_DROP_DELAY: Duration = Duration::from_secs(60);

#[derive(Clone)]
pub struct Instrument {
    inner: IoTask<Handler>,
}

#[derive(Clone)]
enum Request {
    Scpi(ScpiRequest),
    DropCheck,
}

impl Request {
    fn into_scpi(self) -> Option<ScpiRequest> {
        match self {
            Request::Scpi(x) => Some(x),
            _ => None,
        }
    }
}

enum Response {
    Scpi(ScpiResponse),
    Done,
}

impl Instrument {
    pub fn new(addr: IpAddr) -> Self {
        Self {
            inner: IoTask::new(Handler {
                addr,
                client: None,
                drop_delay: DEFAULT_DROP_DELAY,
                last_request: Instant::now(),
                drop_delay_task: None,
            }),
        }
    }

    pub fn disconnect(mut self) {
        self.inner.disconnect();
    }

    pub async fn request(&mut self, req: ScpiRequest) -> crate::Result<ScpiResponse> {
        let req = Request::Scpi(req);
        match self.inner.request(req).await? {
            Response::Scpi(x) => Ok(x),
            Response::Done => Err(crate::Error::internal(anyhow!("Invalid response for request."))),
        }
    }
}

#[async_trait]
impl crate::inventory::Instrument for Instrument {
    type Address = String;

    fn connect(_server: &crate::app::Server, addr: &Self::Address) -> crate::Result<Self> {
        let format_addr = format!("{}:443", addr).to_socket_addrs();
        let mut iter = format_addr.map_err(crate::Error::argument)?;
        if let Some(x) = iter.next() {
            Ok(Instrument::new(x.ip()))
        } else {
            Err(crate::Error::argument(anyhow!("Invalid tcp socket address: {:?}", addr)))
        }
    }

    async fn wait_for_closed(&self) {
        self.inner.wait_for_closed().await
    }
}

struct Handler {
    addr: IpAddr,
    client: Option<CoreClient>,
    drop_delay: Duration,
    last_request: Instant,
    drop_delay_task: Option<JoinHandle<()>>,
}

impl Handler {
    fn drop_check(&mut self, req: &Request) -> Option<crate::Result<Response>> {
        if matches!(req, Request::DropCheck) {
            let now = Instant::now();
            if now - self.last_request > self.drop_delay {
                self.client.take();
            }
            return Some(Ok(Response::Done));
        }
        if let Some(x) = self.drop_delay_task.take() {
            x.abort();
        }

        None
    }

    async fn connect(&self) -> crate::Result<CoreClient> {
        let fut = CoreClient::connect(self.addr.clone());
        let ret = tokio::time::timeout(DEFAULT_CONNECTION_TIMEOUT, fut)
            .await
            .map_err(|_| crate::Error::protocol_timeout())?;
        ret.map_err(map_error)
    }

    async fn handle_request_timeout(client: &mut CoreClient, req: ScpiRequest) -> crate::Result<ScpiResponse> {
        let fut = Self::handle_request(client, req);
        tokio::time::timeout(DEFAULT_CONNECTION_TIMEOUT, fut)
            .await
            .map_err(|_| crate::Error::protocol_timeout())?
    }

    async fn handle_request(client: &mut CoreClient, req: ScpiRequest) -> crate::Result<ScpiResponse> {
        match req {
            ScpiRequest::Write(mut msg) => {
                if !msg.ends_with(DEFAULT_TERMINATION) {
                    msg.push_str(DEFAULT_TERMINATION);
                }
                client
                    .device_write(msg.as_bytes().to_vec())
                    .await
                    .map(|_| ScpiResponse::Done)
                    .map_err(map_error)
            }
            ScpiRequest::QueryString(data) => {
                client.device_write(data.as_bytes().to_vec()).await.map_err(map_error)?;
                let data = client.device_read().await.map_err(map_error)?;
                let ret =
                    String::from_utf8(data).map_err(|_| crate::Error::protocol(anyhow!("Data not terminated.")))?;
                if !ret.ends_with(DEFAULT_TERMINATION) {
                    return Err(Error::protocol(anyhow!("Data not terminated.")));
                }
                let ret = ret[..ret.len() - DEFAULT_TERMINATION.len()].to_string();
                Ok(ScpiResponse::String(ret))
            }
            ScpiRequest::QueryBinary(data) => {
                client.device_write(data.as_bytes().to_vec()).await.map_err(map_error)?;
                let rx = client.device_read().await.map_err(map_error)?;
                let (offset, length) = scpi::parse_binary_header(&rx)?;
                let ret = rx[offset..offset + length].to_vec();
                Ok(ScpiResponse::Binary { data: ret })
            }
            ScpiRequest::ReadRaw => {
                let data = client.device_read().await.map_err(map_error)?;
                Ok(ScpiResponse::Binary { data })
            }
        }
    }

    fn spawn_drop_check(&mut self, ctx: &mut IoContext<Self>) {
        let mut ctx = ctx.clone();
        let drop_delay = self.drop_delay.clone();
        self.drop_delay_task = Some(task::spawn(async move {
            sleep(drop_delay + Duration::from_millis(100)).await;
            let _ = ctx.send(Request::DropCheck);
        }));
    }
}

#[async_trait]
impl IoHandler for Handler {
    type Request = Request;
    type Response = Response;

    async fn handle(&mut self, ctx: &mut IoContext<Self>, req: Self::Request) -> crate::Result<Self::Response> {
        if let Some(x) = self.drop_check(&req) {
            return x;
        }
        let mut client = if let Some(client) = self.client.take() {
            client
        } else {
            self.connect().await?
        };
        // save because drop check_check handled other
        let req = req.into_scpi().unwrap();
        let ret = Self::handle_request_timeout(&mut client, req.clone()).await;
        match ret {
            Ok(ret) => {
                self.client.replace(client);
                self.spawn_drop_check(ctx);
                Ok(Response::Scpi(ret))
            }
            Err(err) => {
                drop(client);
                if err.should_retry() {
                    sleep(Duration::from_millis(100)).await;
                    let mut client = self.connect().await?;
                    let ret = Self::handle_request_timeout(&mut client, req).await;
                    if ret.is_ok() {
                        self.client.replace(client);
                        self.spawn_drop_check(ctx);
                    }
                    Ok(Response::Scpi(ret?))
                } else {
                    Err(err)
                }
            }
        }
    }
}

fn map_error(err: async_vxi11::Error) -> crate::Error {
    match err {
        async_vxi11::Error::Io(io) => crate::Error::transport(io),
        err => crate::Error::transport(anyhow!(err)),
    }
}
