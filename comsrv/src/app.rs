use tokio::sync::mpsc::UnboundedReceiver;
use tokio::task;
use wsrpc::server::{Requested, Server as WsrpcServer};

use crate::address::Address;
use crate::can::{Request as InternalCanRequest, CanAddress};
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
    ModBusResponse, OldRequest, Response, ScpiRequest, ScpiResponse, CanDriverType, Request, SerialAddress, FtdiAddress, TcpAddress, ByteStreamInstrument,
    CanInstrument, ScpiInstrument, PrologixInstrument
};
use std::sync::Arc;

pub type Server = WsrpcServer<OldRequest, Response>;

macro_rules! crate_version {
    () => {
        env!("CARGO_PKG_VERSION")
    };
}

pub struct Inventories {
    serial: Inventory<serial::Instrument, SerialAddress>,
    can: Inventory<can::Instrument, CanAddress>,
    ftdi: Inventory<ftdi::Instrument, FtdiAddress>,
    tcp: Inventory<tcp::Instrument, TcpAddress>,
    visa: Inventory<visa::Instrument, String>,
    vxi: Inventory<vxi::Instrument, String>,
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
        match req {
            Request::ByteStream { instrument: ByteStreamInstrument::Ftdi(instr), request, lock } => {
                let instr = self.inventories.ftdi.connect(&instr.address, || ftdi::Instrument::new(&instr.address.port));
            },
            Request::ByteStream { instrument: ByteStreamInstrument::Serial(instr), request, lock } => todo!(),
            Request::ByteStream { instrument: ByteStreamInstrument::Tcp(instr), request, lock } => todo!(),
            Request::Can { instrument: CanInstrument::PCan { address, baudrate },  request, lock } => todo!(),
            Request::Can { instrument: CanInstrument::SocketCan { interface },  request, lock } => todo!(),
            Request::Scpi { instrument: ScpiInstrument::PrologixSerial(instr), request } => todo!(),
            Request::Scpi { instrument: ScpiInstrument::Visa(instr), request } => todo!(),
            Request::Scpi { instrument: ScpiInstrument::Vxi(instr), request } => todo!(),
            Request::Sigrok { instrument, request } => todo!(),
            Request::Hid { instrument, request, lock } => todo!(),
            Request::Connect { instrument, timeout } => todo!(),
            Request::ListSigrokDevices => todo!(),
            Request::ListSerialPorts => todo!(),
            Request::ListHidDevices => todo!(),
            Request::ListFtdiDevices => todo!(),
            Request::ListCanDevices => todo!(),
            Request::ListConnectedInstruments => todo!(),
            Request::Lock { addr, timeout_ms } => todo!(),
            Request::Unlock(_) => todo!(),
            Request::DropAll => todo!(),
            Request::Version => todo!(),
            Request::Shutdown => todo!(),
        }
    }
}
