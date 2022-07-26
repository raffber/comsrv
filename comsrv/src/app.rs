use comsrv_protocol::Address;
use comsrv_protocol::ByteStreamRequest;
use comsrv_protocol::CanAddress;
use comsrv_protocol::CanDeviceInfo;
use comsrv_protocol::CanDriverType;
use comsrv_protocol::CanRequest;
use comsrv_protocol::FtdiInstrument;
use comsrv_protocol::HidResponse;
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

use crate::transport::can;
use crate::transport::ftdi;
use crate::transport::ftdi::FtdiRequest;
use crate::transport::hid;
use crate::transport::serial;
use crate::transport::sigrok;
use crate::transport::tcp;
use crate::transport::tcp::TcpRequest;
use crate::transport::visa;
use crate::transport::vxi;

use crate::inventory::Inventory;
use anyhow::anyhow;
use comsrv_protocol::{ByteStreamInstrument, CanInstrument, Request, Response, ScpiInstrument};
use std::convert::TryInto;
use std::sync::Arc;
use std::time::Duration;

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
            log::debug!("Incoming[{}]: {}", rep.request_id(), serde_json::to_string(&req).unwrap());
            task::spawn(async move {
                let response = app.handle(req).await.into();
                log::debug!("Answering: {}", serde_json::to_string(&response).unwrap());
                rep.answer(response);
            });
        }
    }

    async fn handle(&self, req: Request) -> crate::Result<Response> {
        match req {
            Request::Bytes {
                instrument: ByteStreamInstrument::Ftdi(instrument),
                request,
                lock,
            } => self.handle_bytestream_ftdi(instrument, request, lock).await,
            Request::Bytes {
                instrument: ByteStreamInstrument::Serial(instr),
                request,
                lock,
            } => self.handle_bytestream_serial(instr, request, lock).await,
            Request::Bytes {
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
            Request::Sigrok { instrument, request } => {
                sigrok::read(&instrument.address, request).await.map(Response::Sigrok)
            }
            Request::Hid {
                instrument,
                request,
                lock,
            } => self.handle_hid(instrument, request, lock).await,
            Request::ListSigrokDevices => sigrok::list().await.map(Response::Sigrok),
            Request::ListConnectedInstruments => self.list_connected_instruments(),
            Request::Lock { addr, timeout } => self.lock(addr, timeout).await,
            Request::Unlock { addr, id } => self.unlock(addr, id).await,
            Request::DropAll => self.drop_all(),
            Request::Shutdown => {
                let _ = self.drop_all();
                self.server.shutdown().await;
                Ok(Response::Done)
            }
            Request::ListHidDevices => hid::list_devices().await.map(|x| Response::Hid(HidResponse::List(x))),
            Request::Version => {
                let version = crate_version!();
                let version: Vec<_> = version.split(".").map(|x| x.parse::<u32>().unwrap()).collect();
                Ok(Response::Version {
                    major: version[0],
                    minor: version[1],
                    build: version[2],
                })
            }
            Request::ListSerialPorts => serial::list_devices().await.map(Response::SerialPorts),
            Request::ListFtdiDevices => ftdi::list_ftdi().await.map(Response::FtdiDevices),
            Request::ListCanDevices => match async_can::list_devices().await {
                Ok(x) => {
                    #[cfg(target_os = "linux")]
                    let driver_type = CanDriverType::SocketCAN;
                    #[cfg(target_os = "windows")]
                    let driver_type = CanDriverType::PCAN;
                    let ret = x
                        .iter()
                        .map(|y| CanDeviceInfo {
                            interface_name: y.interface_name.clone(),
                            driver_type: driver_type.clone(),
                        })
                        .collect();
                    Ok(Response::CanDevices(ret))
                }
                Err(x) => Err(crate::Error::transport(anyhow!(x))),
            },
            Request::Drop { addr, id } => self.drop(addr, id.as_ref()).await,
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
                params: instr.port_config.try_into()?,
                req,
            })
            .await?;
        match ret {
            serial::Response::Bytes(x) => Ok(Response::Bytes(x)),
            serial::Response::Scpi(_) => Err(invalid_response_for_request()),
        }
    }

    async fn handle_can(&self, instr: CanInstrument, req: CanRequest, lock: Option<Uuid>) -> crate::Result<Response> {
        let bitrate = instr.bitrate();
        let addr: CanAddress = instr.into();
        self.inventories
            .can
            .wait_connect(&self.server, &addr, lock.as_ref())
            .await?
            .request(can::Request { inner: req, bitrate })
            .await
            .map(|response| Response::Can { source: addr, response })
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

    async fn handle_vxi(&self, instr: VxiInstrument, req: ScpiRequest, lock: Option<Uuid>) -> crate::Result<Response> {
        self.inventories
            .vxi
            .wait_connect(&self.server, &instr.host, lock.as_ref())
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

    async fn lock(
        &self,
        addr: comsrv_protocol::Address,
        timeout: comsrv_protocol::Duration,
    ) -> crate::Result<Response> {
        let timeout: Duration = timeout.into();
        let lock_id = match addr {
            comsrv_protocol::Address::Tcp(x) => self.inventories.tcp.wait_and_lock(&self.server, &x, timeout).await,
            comsrv_protocol::Address::Ftdi(x) => self.inventories.ftdi.wait_and_lock(&self.server, &x, timeout).await,
            comsrv_protocol::Address::Hid(x) => self.inventories.hid.wait_and_lock(&self.server, &x, timeout).await,
            comsrv_protocol::Address::Serial(x) => {
                self.inventories.serial.wait_and_lock(&self.server, &x, timeout).await
            }
            comsrv_protocol::Address::Vxi(x) => self.inventories.vxi.wait_and_lock(&self.server, &x, timeout).await,
            comsrv_protocol::Address::Visa(x) => self.inventories.visa.wait_and_lock(&self.server, &x, timeout).await,
            comsrv_protocol::Address::Can(x) => self.inventories.can.wait_and_lock(&self.server, &x, timeout).await,
        }?;
        Ok(Response::Locked { lock_id })
    }

    async fn unlock(&self, addr: comsrv_protocol::Address, id: Uuid) -> crate::Result<Response> {
        match addr {
            comsrv_protocol::Address::Tcp(_) => self.inventories.tcp.unlock(id).await,
            comsrv_protocol::Address::Ftdi(_) => self.inventories.ftdi.unlock(id).await,
            comsrv_protocol::Address::Hid(_) => self.inventories.hid.unlock(id).await,
            comsrv_protocol::Address::Serial(_) => self.inventories.serial.unlock(id).await,
            comsrv_protocol::Address::Vxi(_) => self.inventories.vxi.unlock(id).await,
            comsrv_protocol::Address::Visa(_) => self.inventories.visa.unlock(id).await,
            comsrv_protocol::Address::Can(_) => self.inventories.can.unlock(id).await,
        }
        Ok(Response::Done)
    }

    fn drop_all(&self) -> crate::Result<Response> {
        self.inventories.tcp.disconnect_all();
        self.inventories.vxi.disconnect_all();
        self.inventories.hid.disconnect_all();
        self.inventories.serial.disconnect_all();
        self.inventories.visa.disconnect_all();
        self.inventories.can.disconnect_all();
        self.inventories.ftdi.disconnect_all();
        Ok(Response::Done)
    }

    fn list_connected_instruments(&self) -> crate::Result<Response> {
        let mut ret: Vec<Address> = self.inventories.tcp.list().drain(..).map(Address::Tcp).collect();
        ret.extend(self.inventories.can.list().drain(..).map(Address::Can));
        ret.extend(self.inventories.ftdi.list().drain(..).map(Address::Ftdi));
        ret.extend(self.inventories.vxi.list().drain(..).map(Address::Vxi));
        ret.extend(self.inventories.hid.list().drain(..).map(Address::Hid));
        ret.extend(self.inventories.serial.list().drain(..).map(Address::Serial));
        ret.extend(self.inventories.visa.list().drain(..).map(Address::Visa));
        Ok(Response::Instruments(ret))
    }

    async fn drop(&self, addr: Address, id: Option<&Uuid>) -> crate::Result<Response> {
        match addr {
            Address::Tcp(x) => self.inventories.tcp.wait_disconnect(&x, id).await,
            Address::Ftdi(x) => self.inventories.ftdi.wait_disconnect(&x, id).await,
            Address::Hid(x) => self.inventories.hid.wait_disconnect(&x, id).await,
            Address::Serial(x) => self.inventories.serial.wait_disconnect(&x, id).await,
            Address::Vxi(x) => self.inventories.vxi.wait_disconnect(&x, id).await,
            Address::Visa(x) => self.inventories.visa.wait_disconnect(&x, id).await,
            Address::Can(x) => self.inventories.can.wait_disconnect(&x, id).await,
        }
        Ok(Response::Done)
    }
}

fn invalid_response_for_request() -> crate::Error {
    crate::Error::internal(anyhow!("Invalid resposne for request."))
}
