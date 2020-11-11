use async_std::net::SocketAddr;
use async_vxi11::CoreClient;

use crate::{Error, ScpiRequest, ScpiResponse};
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
        let mut ctx = if let Some(ctx) = self.ctx.take() {
            ctx
        } else {
            CoreClient::connect(self.addr.clone()).await.map_err(Error::vxi)?
        };
        let ret = handle_request(&mut ctx, req.req, req.options).await;
        self.ctx.replace(ctx);
        ret
    }
}

async fn handle_request(client: &mut CoreClient, req: ScpiRequest, options: VisaOptions) -> crate::Result<ScpiResponse> {
    match req {
        ScpiRequest::Write(data) => {}
        ScpiRequest::QueryString(_) => {}
        ScpiRequest::QueryBinary(_) => {}
        ScpiRequest::ReadRaw => {}
    }
    todo!()
}
