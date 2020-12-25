use std::net::IpAddr;
use async_trait::async_trait;
use async_vxi11::CoreClient;
use tokio::time::{Duration, delay_for};

use crate::iotask::{IoHandler, IoTask};
use crate::visa::VisaOptions;
use crate::{util, Error, ScpiRequest, ScpiResponse};

const DEFAULT_TERMINATION: &'static str = "\n";

const DEFAULT_TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Clone)]
pub struct Instrument {
    inner: IoTask<Handler>,
}

#[derive(Clone)]
struct Request {
    req: ScpiRequest,
    options: VisaOptions,
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

    pub async fn request(
        &mut self,
        req: ScpiRequest,
        options: VisaOptions,
    ) -> crate::Result<ScpiResponse> {
        let req = Request { req, options };
        self.inner.request(req).await
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

    async fn handle(&mut self, req: Self::Request) -> crate::Result<Self::Response> {
        let mut client = if let Some(client) = self.client.take() {
            client
        } else {
            connect(self.addr.clone()).await?
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
                    delay_for(Duration::from_millis(100)).await;
                    let mut client = connect(self.addr.clone()).await?;
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
    let ret = tokio::time::timeout(DEFAULT_TIMEOUT, fut).await.map_err(|_| crate::Error::Timeout)?;
    ret.map_err(Error::vxi)
}

async fn handle_request_timeout(
    client: &mut CoreClient,
    req: Request,
) -> crate::Result<ScpiResponse> {
    let fut = handle_request(client, req.req, req.options);
    tokio::time::timeout(DEFAULT_TIMEOUT, fut)
        .await
        .map_err(|_| crate::Error::Timeout)?
}

async fn handle_request(
    client: &mut CoreClient,
    req: ScpiRequest,
    _options: VisaOptions,
) -> crate::Result<ScpiResponse> {
    match req {
        ScpiRequest::Write(mut msg) => {
            if !msg.ends_with(DEFAULT_TERMINATION) {
                msg.push_str(DEFAULT_TERMINATION);
            }
            client
                .device_write(msg.as_bytes().to_vec())
                .await
                .map(|_| ScpiResponse::Done)
                .map_err(Error::vxi)
        }
        ScpiRequest::QueryString(data) => {
            client
                .device_write(data.as_bytes().to_vec())
                .await
                .map_err(Error::vxi)?;
            let data = client.device_read().await.map_err(Error::vxi)?;
            let ret = String::from_utf8(data).map_err(Error::DecodeError)?;
            if !ret.ends_with(DEFAULT_TERMINATION) {
                return Err(Error::NotTerminated);
            }
            let ret = ret[..ret.len() - DEFAULT_TERMINATION.len()].to_string();
            Ok(ScpiResponse::String(ret))
        }
        ScpiRequest::QueryBinary(data) => {
            client
                .device_write(data.as_bytes().to_vec())
                .await
                .map_err(Error::vxi)?;
            let rx = client.device_read().await.map_err(Error::vxi)?;
            let (offset, length) = util::parse_binary_header(&rx)?;
            let ret = rx[offset..offset + length].iter().cloned().collect();
            Ok(ScpiResponse::Binary { data: ret })
        }
        ScpiRequest::ReadRaw => {
            let data = client.device_read().await.map_err(Error::vxi)?;
            Ok(ScpiResponse::Binary { data })
        }
    }
}
