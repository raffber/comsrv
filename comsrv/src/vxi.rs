use async_trait::async_trait;
use async_vxi11::CoreClient;
use std::net::{IpAddr, ToSocketAddrs};
use tokio::time::{sleep, Duration};

use crate::iotask::{IoContext, IoHandler, IoTask};
use crate::{scpi, Error};
use anyhow::anyhow;
use comsrv_protocol::{ScpiRequest, ScpiResponse};

const DEFAULT_TERMINATION: &str = "\n";

const DEFAULT_TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Clone)]
pub struct Instrument {
    inner: IoTask<Handler>,
}

#[derive(Clone)]
struct Request {
    req: ScpiRequest,
}

impl Instrument {
    pub fn new(addr: IpAddr) -> Self {
        Self {
            inner: IoTask::new(Handler { addr, client: None }),
        }
    }

    pub fn disconnect(mut self) {
        self.inner.disconnect();
    }

    pub async fn request(&mut self, req: ScpiRequest) -> crate::Result<ScpiResponse> {
        let req = Request { req };
        self.inner.request(req).await
    }
}

impl crate::inventory::Instrument for Instrument {
    type Address = String;

    fn connect(_server: &crate::app::Server, addr: &Self::Address) -> crate::Result<Self> {
        let format_addr = format!("{}:443", addr).to_socket_addrs();
        let mut iter = format_addr.map_err(crate::Error::argument)?;
        if let Some(x) = iter.next() {
            Ok(Instrument::new(x.ip()))
        } else {
            Err(crate::Error::argument(anyhow!(
                "Invalid tcp socket address: {:?}",
                addr
            )))
        }
    }
}

struct Handler {
    addr: IpAddr,
    client: Option<CoreClient>,
}

#[async_trait]
impl IoHandler for Handler {
    type Request = Request;
    type Response = ScpiResponse;

    async fn handle(
        &mut self,
        _ctx: &mut IoContext<Self>,
        req: Self::Request,
    ) -> crate::Result<Self::Response> {
        let mut client = if let Some(client) = self.client.take() {
            client
        } else {
            connect(self.addr).await?
        };
        let ret = handle_request_timeout(&mut client, req.clone()).await;
        match ret {
            Ok(ret) => {
                self.client.replace(client);
                Ok(ret)
            }
            Err(err) => {
                drop(client);
                if err.should_retry() {
                    sleep(Duration::from_millis(100)).await;
                    let mut client = connect(self.addr).await?;
                    let ret = handle_request_timeout(&mut client, req).await;
                    if ret.is_ok() {
                        self.client.replace(client);
                    }
                    ret
                } else {
                    Err(err)
                }
            }
        }
    }
}

async fn connect(addr: IpAddr) -> crate::Result<CoreClient> {
    let fut = CoreClient::connect(addr);
    let ret = tokio::time::timeout(DEFAULT_TIMEOUT, fut)
        .await
        .map_err(|_| crate::Error::protocol_timeout())?;
    ret.map_err(map_error)
}

async fn handle_request_timeout(
    client: &mut CoreClient,
    req: Request,
) -> crate::Result<ScpiResponse> {
    let fut = handle_request(client, req.req);
    tokio::time::timeout(DEFAULT_TIMEOUT, fut)
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
            client
                .device_write(data.as_bytes().to_vec())
                .await
                .map_err(map_error)?;
            let data = client.device_read().await.map_err(map_error)?;
            let ret = String::from_utf8(data)
                .map_err(|_| crate::Error::protocol(anyhow!("Data not terminated.")))?;
            if !ret.ends_with(DEFAULT_TERMINATION) {
                return Err(Error::protocol(anyhow!("Data not terminated.")));
            }
            let ret = ret[..ret.len() - DEFAULT_TERMINATION.len()].to_string();
            Ok(ScpiResponse::String(ret))
        }
        ScpiRequest::QueryBinary(data) => {
            client
                .device_write(data.as_bytes().to_vec())
                .await
                .map_err(map_error)?;
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

fn map_error(err: async_vxi11::Error) -> crate::Error {
    match err {
        async_vxi11::Error::Io(io) => crate::Error::transport(io),
        err => crate::Error::transport(anyhow!(err)),
    }
}
