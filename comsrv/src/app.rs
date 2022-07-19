use tokio::sync::mpsc::UnboundedReceiver;
use tokio::task;
use wsrpc::server::{Requested, Server as WsrpcServer};

use crate::address::Address;
use crate::can::Request as InternalCanRequest;
use crate::ftdi::{self, FtdiRequest, FtdiResponse};
use crate::instrument::Instrument;
use crate::inventory::Inventory;
use crate::modbus::{ModBusAddress, ModBusTransport};
use crate::serial::Response as SerialResponse;
use crate::serial::{Request as SerialRequest, SerialParams};
use crate::tcp::{TcpRequest, TcpResponse};
use crate::{sigrok, Error};
use comsrv_protocol::{
    ByteStreamRequest, ByteStreamResponse, CanDeviceInfo, CanRequest, CanResponse, ModBusRequest,
    ModBusResponse, OldRequest, Response, ScpiRequest, ScpiResponse, CanDriverType, Request,
};
use std::sync::Arc;

pub type Server = WsrpcServer<OldRequest, Response>;

macro_rules! crate_version {
    () => {
        env!("CARGO_PKG_VERSION")
    };
}

pub struct Inventories {

}

impl Inventories {
    fn new() -> Self {
        todo!()
    }
}

#[derive(Clone)]
pub struct App {
    pub server: Server,
    pub inventories: Arc<Inventories>,
}

impl App {
    pub fn new() -> (Self, UnboundedReceiver<Requested<Request, Response>>) {
        let (server, rx) = Server::new();
        let app = App {
            server,
            inventories: Inventories::new(),
        };
        (app, rx)
    }

    pub async fn run(&self, mut rx: UnboundedReceiver<Requested<Request, Response>>) {
        while let Some(msg) = rx.recv().await {
            let (req, rep) = msg.split();
            let app = self.clone();
            log::debug!(
                "Incoming[{}]: {}",
                rep.request_id(),
                serde_json::to_string(&req).unwrap()
            );
            task::spawn(async move {
                let response = app.handle(req).await;
                log::debug!("Answering: {}", serde_json::to_string(&response).unwrap());
                rep.answer(response);
            });
        }
    }

    async fn handle(&self, req: Request) -> Response {

        todo!()
    }
}
