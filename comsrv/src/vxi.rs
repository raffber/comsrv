use async_std::net::SocketAddr;
use async_trait::async_trait;
use async_vxi11::CoreClient;

use crate::{Error, ScpiRequest, ScpiResponse, util};
use crate::iotask::{IoHandler, IoTask};
use crate::visa::VisaOptions;

#[derive(Clone)]
pub struct Instrument {
    inner: IoTask<Handler>
}

struct Request {
    req: ScpiRequest,
    options: VisaOptions,
}

impl Instrument {
    pub fn new(addr: SocketAddr) -> Self {
        Self {
            inner: IoTask::new(Handler {
                addr,
                client: None,
            })
        }
    }

    pub fn disconnect(mut self) {
        self.inner.disconnect();
    }

    pub async fn request(&mut self, req: ScpiRequest, options: VisaOptions) -> crate::Result<ScpiResponse> {
        let req = Request {
            req,
            options,
        };
        self.inner.request(req).await
    }
}

struct Handler {
    addr: SocketAddr,
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
            CoreClient::connect(self.addr.ip()).await.map_err(Error::vxi)?
        };
        let ret = handle_request(&mut client, req.req, req.options).await;
        self.client.replace(client);
        ret
    }
}

async fn handle_request(client: &mut CoreClient, req: ScpiRequest, options: VisaOptions) -> crate::Result<ScpiResponse> {
    match req {
        ScpiRequest::Write(data) => {
            client.device_write(data.as_bytes().to_vec()).await
                .map(|_| ScpiResponse::Done)
                .map_err(Error::vxi)
        }
        ScpiRequest::QueryString(data) => {
            client.device_write(data.as_bytes().to_vec()).await.map_err(Error::vxi)?;
            let data = client.device_read().await.map_err(Error::vxi)?;
            let ret = String::from_utf8(data).map_err(Error::DecodeError)?;
            Ok(ScpiResponse::String(ret))
        }
        ScpiRequest::QueryBinary(data) => {
            client.device_write(data.as_bytes().to_vec()).await.map_err(Error::vxi)?;
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
