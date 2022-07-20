use comsrv_protocol::ByteStreamRequest;
use comsrv_protocol::CanAddress;
use comsrv_protocol::CanRequest;
use comsrv_protocol::FtdiInstrument;
use comsrv_protocol::PrologixInstrument;
use comsrv_protocol::PrologixRequest;
use comsrv_protocol::ScpiRequest;
use comsrv_protocol::SerialInstrument;
use comsrv_protocol::TcpInstrument;
use comsrv_protocol::VisaInstrument;
use comsrv_protocol::VxiInstrument;
use tokio::sync::mpsc::UnboundedReceiver;
use tokio::task;
use uuid::Uuid;
use wsrpc::server::{Requested, Server as WsrpcServer};

use crate::can;
use crate::ftdi;
use crate::ftdi::FtdiRequest;
use crate::hid;
use crate::serial;
use crate::sigrok;
use crate::tcp;
use crate::tcp::TcpRequest;
use crate::visa;
use crate::vxi;

use crate::inventory::Inventory;
use anyhow::anyhow;
use comsrv_protocol::{ByteStreamInstrument, CanInstrument, Request, Response, ScpiInstrument};
use std::sync::Arc;

pub type Server = WsrpcServer<Request, Response>;

macro_rules! crate_version {
    () => {
        env!("CARGO_PKG_VERSION")
    };
}

#[derive(Default)]
pub struct Inventories {
    serial: Inventory<serial::Instrument>,
    can: Inventory<can::Instrument>,
    ftdi: Inventory<ftdi::Instrument>,
    tcp: Inventory<tcp::Instrument>,
    visa: Inventory<visa::Instrument>,
    vxi: Inventory<vxi::Instrument>,
    hid: Inventory<hid::Instrument>,
}

impl Inventories {
    fn new() -> Self {
        Default::default()
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
            inventories: Arc::new(Inventories::new()),
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
                instrument: ByteStreamInstrument::Ftdi(instrument),
                request,
                lock,
            } => self.handle_bytestream_ftdi(instrument, request, lock).await,
            Request::ByteStream {
                instrument: ByteStreamInstrument::Serial(instr),
                request,
                lock,
            } => self.handle_bytestream_serial(instr, request, lock).await,
            Request::ByteStream {
                instrument: ByteStreamInstrument::Tcp(instr),
                request,
                lock,
            } => self.handle_bytestream_tcp(instr, request, lock).await,
            Request::Can {
                instrument,
                request,
                lock,
            } => self.handle_can(instrument, request, lock).await,
            Request::Scpi {
                instrument: ScpiInstrument::Visa(instr),
                request,
                lock,
            } => self.handle_visa(instr, request, lock).await,
            Request::Scpi {
                instrument: ScpiInstrument::Vxi(instr),
                request,
                lock,
            } => self.handle_vxi(instr, request, lock).await,
            Request::Prologix {
                instrument,
                request,
                lock,
            } => self.handle_prologix(instrument, request, lock).await,
            Request::Sigrok {
                instrument,
                request,
            } => sigrok::read(&instrument.address, request)
                .await
                .map(Response::Sigrok),
            Request::Hid {
                instrument,
                request,
                lock,
            } => self.handle_hid(instrument, request, lock).await,
            Request::Connect {
                instrument: _,
                timeout: _,
            } => todo!(),
            Request::ListSigrokDevices => todo!(),
            Request::ListSerialPorts => todo!(),
            Request::ListHidDevices => todo!(),
            Request::ListFtdiDevices => todo!(),
            Request::ListCanDevices => todo!(),
            Request::ListConnectedInstruments => todo!(),
            Request::Lock {
                addr: _,
                timeout_ms: _,
            } => todo!(),
            Request::Unlock(_) => todo!(),
            Request::DropAll => todo!(),
            Request::Version => todo!(),
            Request::Shutdown => todo!(),
        }
    }

    async fn handle_bytestream_ftdi(
        &self,
        instr: FtdiInstrument,
        req: ByteStreamRequest,
        lock: Option<Uuid>,
    ) -> crate::Result<Response> {
        self.inventories
            .ftdi
            .wait_connect(&self.server, &instr.address, lock.as_ref())
            .await?
            .request(FtdiRequest {
                request: req,
                port_config: instr.port_config,
                options: instr.options,
            })
            .await
            .map(Response::Bytes)
    }

    async fn handle_bytestream_tcp(
        &self,
        instr: TcpInstrument,
        req: ByteStreamRequest,
        lock: Option<Uuid>,
    ) -> crate::Result<Response> {
        let ret = self
            .inventories
            .tcp
            .wait_connect(&self.server, &instr.address, lock.as_ref())
            .await?
            .request(TcpRequest::Bytes {
                request: req,
                options: instr.options,
            })
            .await?;
        match ret {
            tcp::TcpResponse::Bytes(x) => Ok(Response::Bytes(x)),
            _ => Err(invalid_response_for_request()),
        }
    }

    async fn handle_bytestream_serial(
        &self,
        instr: SerialInstrument,
        req: ByteStreamRequest,
        lock: Option<Uuid>,
    ) -> crate::Result<Response> {
        let ret = self
            .inventories
            .serial
            .wait_connect(&self.server, &instr.address, lock.as_ref())
            .await?
            .request(serial::Request::Serial {
                params: instr.port_config.into(),
                req,
            })
            .await?;
        match ret {
            serial::Response::Bytes(x) => Ok(Response::Bytes(x)),
            serial::Response::Scpi(_) => Err(invalid_response_for_request()),
        }
    }

    async fn handle_can(
        &self,
        instr: CanInstrument,
        req: CanRequest,
        lock: Option<Uuid>,
    ) -> crate::Result<Response> {
        let bitrate = instr.bitrate();
        let addr: CanAddress = instr.into();
        self.inventories
            .can
            .wait_connect(&self.server, &addr, lock.as_ref())
            .await?
            .request(can::Request {
                inner: req,
                bitrate,
            })
            .await
            .map(Response::Can)
    }

    async fn handle_visa(
        &self,
        instr: VisaInstrument,
        req: ScpiRequest,
        lock: Option<Uuid>,
    ) -> crate::Result<Response> {
        self.inventories
            .visa
            .wait_connect(&self.server, &instr.address, lock.as_ref())
            .await?
            .request(req)
            .await
            .map(Response::Scpi)
    }

    async fn handle_vxi(
        &self,
        instr: VxiInstrument,
        req: ScpiRequest,
        lock: Option<Uuid>,
    ) -> crate::Result<Response> {
        self.inventories
            .vxi
            .wait_connect(&self.server, &instr.address, lock.as_ref())
            .await?
            .request(req)
            .await
            .map(Response::Scpi)
    }

    async fn handle_prologix(
        &self,
        instr: PrologixInstrument,
        req: PrologixRequest,
        lock: Option<Uuid>,
    ) -> crate::Result<Response> {
        let ret = self
            .inventories
            .serial
            .wait_connect(&self.server, &instr.address, lock.as_ref())
            .await?
            .request(serial::Request::Prologix {
                gpib_addr: req.addr,
                req: req.scpi,
            })
            .await?;
        match ret {
            serial::Response::Bytes(_) => Err(invalid_response_for_request()),
            serial::Response::Scpi(x) => Ok(Response::Scpi(x)),
        }
    }

    async fn handle_hid(
        &self,
        instrument: comsrv_protocol::HidInstrument,
        request: comsrv_protocol::HidRequest,
        lock: Option<Uuid>,
    ) -> crate::Result<Response> {
        self.inventories
            .hid
            .wait_connect(&self.server, &instrument.address, lock.as_ref())
            .await?
            .request(request)
            .await
            .map(Response::Hid)
    }
}

fn invalid_response_for_request() -> crate::Error {
    crate::Error::internal(anyhow!("Invalid resposne for request."))
}
