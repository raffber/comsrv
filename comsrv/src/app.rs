use tokio::sync::mpsc::UnboundedReceiver;
use tokio::task;
use wsrpc::server::{Requested, Server as WsrpcServer};

use crate::can;
use crate::ftdi;
use crate::serial;
use crate::tcp;
use crate::visa;
use crate::vxi;

use crate::inventory::Inventory;
use comsrv_protocol::{
    ByteStreamInstrument, CanInstrument, FtdiAddress, Request, Response, ScpiInstrument,
    SerialAddress, TcpAddress,
};
use std::sync::Arc;

pub type Server = WsrpcServer<Request, Response>;

macro_rules! crate_version {
    () => {
        env!("CARGO_PKG_VERSION")
    };
}

pub struct Inventories {
    serial: Inventory<serial::Instrument>,
    pcan: Inventory<can::Instrument>,
    socket_can: Inventory<can::Instrument>,
    ftdi: Inventory<ftdi::Instrument>,
    tcp: Inventory<tcp::Instrument>,
    visa: Inventory<visa::Instrument>,
    vxi: Inventory<vxi::Instrument>,
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
                let response = app.handle(req).await.into();
                log::debug!("Answering: {}", serde_json::to_string(&response).unwrap());
                rep.answer(response);
            });
        }
    }

    async fn handle(&self, req: Request) -> crate::Result<Response> {
        match req {
            Request::ByteStream {
                instrument: ByteStreamInstrument::Ftdi(instr),
                request,
                lock,
            } => {
                let instr = self
                    .inventories
                    .ftdi
                    .connect(&self.server, &instr.address)?;
                instr.request(req)
            }
            Request::ByteStream {
                instrument: ByteStreamInstrument::Serial(instr),
                request,
                lock,
            } => todo!(),
            Request::ByteStream {
                instrument: ByteStreamInstrument::Tcp(instr),
                request,
                lock,
            } => todo!(),
            Request::Can {
                instrument: CanInstrument::PCan { address, baudrate },
                request,
                lock,
            } => todo!(),
            Request::Can {
                instrument: CanInstrument::SocketCan { interface },
                request,
                lock,
            } => todo!(),
            Request::Scpi {
                instrument: ScpiInstrument::PrologixSerial(instr),
                request,
            } => todo!(),
            Request::Scpi {
                instrument: ScpiInstrument::Visa(instr),
                request,
            } => todo!(),
            Request::Scpi {
                instrument: ScpiInstrument::Vxi(instr),
                request,
            } => todo!(),
            Request::Sigrok {
                instrument,
                request,
            } => todo!(),
            Request::Hid {
                instrument,
                request,
                lock,
            } => todo!(),
            Request::Connect {
                instrument,
                timeout,
            } => todo!(),
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
            Request::Prologix {
                instrument,
                request,
            } => todo!(),
        }
    }
}
