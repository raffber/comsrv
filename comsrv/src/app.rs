use tokio::sync::mpsc::UnboundedReceiver;
use tokio::task;
use wsrpc::server::{Requested, Server as WsrpcServer};

use crate::address::Address;
use crate::can::Request as InternalCanRequest;
use crate::ftdi::{FtdiRequest, FtdiResponse};
use crate::instrument::Instrument;
use crate::inventory::Inventory;
use crate::modbus::{ModBusAddress, ModBusTransport};
use crate::serial::Response as SerialResponse;
use crate::serial::{Request as SerialRequest, SerialParams};
use crate::tcp::{TcpRequest, TcpResponse};
use crate::{sigrok, Error};
use comsrv_protocol::{
    ByteStreamRequest, ByteStreamResponse, CanRequest, CanResponse, ModBusRequest, ModBusResponse,
    Request, Response, ScpiRequest, ScpiResponse,
};
use comsrv_protocol::{HidRequest, HidResponse};

use std::time::Duration;
use uuid::Uuid;

pub type Server = WsrpcServer<Request, Response>;

macro_rules! crate_version {
    () => {
        env!("CARGO_PKG_VERSION")
    };
}

#[derive(Clone)]
pub struct App {
    pub server: Server,
    pub inventory: Inventory,
}

impl App {
    pub fn new() -> (Self, UnboundedReceiver<Requested<Request, Response>>) {
        let (server, rx) = Server::new();
        let app = App {
            server,
            inventory: Inventory::new(),
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
                let response = app.handle_request(req).await;
                log::debug!("Answering: {}", serde_json::to_string(&response).unwrap());
                rep.answer(response);
            });
        }
    }

    async fn handle_scpi(
        &self,
        addr: String,
        task: ScpiRequest,
        lock: Option<Uuid>,
    ) -> Result<ScpiResponse, Error> {
        let addr = Address::parse(&addr)?;
        self.inventory.wait_for_lock(&addr, lock.as_ref()).await;
        match self.inventory.connect(&self.server, &addr) {
            Instrument::Visa(instr) => instr.request(task).await,
            Instrument::Serial(mut instr) => match addr {
                Address::Prologix {
                    file: _,
                    gpib_addr: gpib,
                } => {
                    let response = instr
                        .request(SerialRequest::Prologix {
                            gpib_addr: gpib,
                            req: task,
                        })
                        .await;
                    match response {
                        Ok(SerialResponse::Scpi(resp)) => Ok(resp),
                        Ok(_) => Err(Error::NotSupported),
                        Err(x) => Err(x),
                    }
                }
                _ => Err(Error::NotSupported),
            },
            Instrument::Vxi(mut instr) => instr.request(task).await,
            _ => Err(Error::NotSupported),
        }
    }

    async fn handle_modbus(
        &self,
        addr: String,
        task: ModBusRequest,
        lock: Option<Uuid>,
    ) -> Result<ModBusResponse, Error> {
        let addr = Address::parse(&addr)?;
        self.inventory.wait_for_lock(&addr, lock.as_ref()).await;
        let instr = self.inventory.connect(&self.server, &addr);
        let (modbus_addr, transport, slave_id) = match addr.clone() {
            Address::Modbus {
                addr,
                transport,
                slave_id,
            } => (addr, transport, slave_id),
            Address::Serial { path, params } => {
                let addr = ModBusAddress::Serial { path, params };
                let transport = ModBusTransport::Rtu;
                let slave_id = task.slave_id();
                (addr, transport, slave_id)
            }
            Address::Tcp { addr } => {
                let addr = ModBusAddress::Tcp { addr };
                let transport = ModBusTransport::Rtu;
                let slave_id = task.slave_id();
                (addr, transport, slave_id)
            }
            Address::Ftdi { addr } => {
                let addr = ModBusAddress::Ftdi { addr };
                let transport = ModBusTransport::Rtu;
                let slave_id = task.slave_id();
                (addr, transport, slave_id)
            }
            _ => return Err(Error::InvalidAddress),
        };

        match modbus_addr {
            ModBusAddress::Serial { path: _, params } => match transport {
                ModBusTransport::Tcp => return Err(Error::NotSupported),
                ModBusTransport::Rtu => {
                    let req = SerialRequest::ModBus {
                        params,
                        req: task,
                        slave_addr: slave_id,
                    };
                    let mut instr = instr.into_serial().ok_or(Error::NotSupported)?;
                    let ret = instr.request(req).await;
                    match ret {
                        Ok(SerialResponse::ModBus(ret)) => Ok(ret),
                        Err(x) => Err(x),
                        _ => {
                            log::error!("SerialResponse was not ModBus but request was");
                            return Err(Error::NotSupported);
                        }
                    }
                }
            },
            ModBusAddress::Tcp { .. } => match transport {
                ModBusTransport::Rtu => {
                    let mut instr = instr.into_tcp().ok_or(Error::NotSupported)?;
                    let ret = instr
                        .request(TcpRequest::ModBus {
                            slave_id,
                            req: task,
                        })
                        .await;
                    match ret {
                        Ok(TcpResponse::ModBus(ret)) => Ok(ret),
                        Err(x) => Err(x),
                        _ => {
                            log::error!("TcpResponse was not ModBus but request was");
                            return Err(Error::NotSupported);
                        }
                    }
                }
                ModBusTransport::Tcp => {
                    let mut instr = instr.into_modbus_tcp().ok_or(Error::NotSupported)?;
                    instr.request(task, slave_id).await
                }
            },
            ModBusAddress::Ftdi { addr } => match transport {
                ModBusTransport::Rtu => {
                    let req = FtdiRequest::ModBus {
                        params: addr.params,
                        req: task,
                        slave_addr: slave_id,
                    };
                    let mut instr = instr.into_ftdi().ok_or(Error::NotSupported)?;
                    let ret = instr.request(req).await;
                    match ret {
                        Ok(FtdiResponse::ModBus(ret)) => Ok(ret),
                        Err(x) => Err(x),
                        _ => {
                            log::error!("FtdiResponse was not ModBus but request was");
                            return Err(Error::NotSupported);
                        }
                    }
                }
                ModBusTransport::Tcp => return Err(Error::NotSupported),
            },
        }
    }

    async fn handle_serial(
        &self,
        addr: &Address,
        params: &SerialParams,
        task: ByteStreamRequest,
    ) -> Result<ByteStreamResponse, Error> {
        let params = params.clone();
        let req = SerialRequest::Serial { params, req: task };
        let mut instr = self
            .inventory
            .connect(&self.server, &addr)
            .into_serial()
            .ok_or(Error::NotSupported)?;
        match instr.request(req).await {
            Ok(x) => match x {
                SerialResponse::Bytes(x) => Ok(x),
                _ => panic!("Invalid answer. This is a bug"),
            },
            Err(x) => Err(x),
        }
    }

    async fn handle_tcp(
        &self,
        addr: &Address,
        task: ByteStreamRequest,
    ) -> Result<ByteStreamResponse, Error> {
        let mut instr = self
            .inventory
            .connect(&self.server, &addr)
            .into_tcp()
            .ok_or(Error::NotSupported)?;
        match instr.request(TcpRequest::Bytes(task.clone())).await {
            Ok(TcpResponse::Bytes(x)) => Ok(x),
            Err(x) => Err(x),
            _ => panic!(),
        }
    }

    async fn handle_bytes(
        &self,
        addr: &str,
        task: ByteStreamRequest,
        lock: Option<Uuid>,
    ) -> Result<ByteStreamResponse, Error> {
        let addr = Address::parse(&addr)?;
        self.inventory.wait_for_lock(&addr, lock.as_ref()).await;
        match &addr {
            Address::Serial { path: _, params } => self.handle_serial(&addr, params, task).await,
            Address::Tcp { .. } => self.handle_tcp(&addr, task).await,
            Address::Ftdi { addr: ftdi_address } => {
                let mut instr = self
                    .inventory
                    .connect(&self.server, &addr)
                    .into_ftdi()
                    .ok_or(Error::NotSupported)?;
                match instr.request(FtdiRequest::Bytes {
                    req: task,
                    params: ftdi_address.params.clone()
                }).await {
                    Ok(FtdiResponse::Bytes(x)) => Ok(x),
                    Err(x) => Err(x),
                    _ => panic!(),
                }
            }
            _ => Err(Error::NotSupported),
        }
    }

    async fn handle_can(
        &self,
        addr: &str,
        task: CanRequest,
        lock: Option<Uuid>,
    ) -> Result<CanResponse, Error> {
        let addr = Address::parse(&addr)?;
        let mut device = match self.inventory.connect(&self.server, &addr) {
            Instrument::Can(device) => device,
            _ => return Err(Error::NotSupported),
        };
        self.inventory.wait_for_lock(&addr, lock.as_ref()).await;
        let request = InternalCanRequest::from_request_and_address(task, addr)?;
        device.request(request).await
    }

    async fn handle_hid(
        &self,
        addr: &str,
        task: HidRequest,
        lock: Option<Uuid>,
    ) -> Result<HidResponse, Error> {
        let addr = Address::parse(&addr)?;
        let mut device = match self.inventory.connect(&self.server, &addr) {
            Instrument::Hid(device) => device,
            _ => return Err(Error::NotSupported),
        };
        self.inventory.wait_for_lock(&addr, lock.as_ref()).await;
        device.request(task).await
    }

    pub async fn shutdown(&self) {
        self.inventory.disconnect_all();
        self.server.shutdown().await;
    }

    async fn handle_request(&self, req: Request) -> Response {
        match req {
            Request::Scpi { addr, task, lock } => match self.handle_scpi(addr, task, lock).await {
                Ok(result) => Response::Scpi(result),
                Err(err) => err.into(),
            },
            Request::ListInstruments => Response::Instruments(self.inventory.list()),
            Request::ModBus { addr, task, lock } => {
                match self.handle_modbus(addr, task, lock).await {
                    Ok(result) => Response::ModBus(result),
                    Err(err) => err.into(),
                }
            }
            Request::Bytes { addr, task, lock } => match self.handle_bytes(&addr, task, lock).await
            {
                Ok(result) => Response::Bytes(result),
                Err(err) => err.into(),
            },
            Request::Can { addr, task, lock } => match self.handle_can(&addr, task, lock).await {
                Ok(result) => Response::Can(result),
                Err(err) => err.into(),
            },
            Request::DropAll => {
                self.inventory.disconnect_all();
                Response::Done
            }
            Request::Shutdown => {
                self.shutdown().await;
                Response::Done
            }
            Request::Drop(addr) => match Address::parse(&addr) {
                Ok(addr) => {
                    self.inventory.disconnect(&addr);
                    Response::Done
                }
                Err(err) => err.into(),
            },
            Request::Sigrok { addr, task } => {
                let addr = match Address::parse(&addr) {
                    Ok(addr) => addr,
                    Err(err) => return err.into(),
                };
                let device = match addr {
                    Address::Sigrok { device } => device,
                    _ => return Error::NotSupported.into(),
                };
                match sigrok::read(device, task).await {
                    Ok(resp) => Response::Sigrok(resp),
                    Err(err) => err.into(),
                }
            }
            Request::ListSigrokDevices => match sigrok::list().await {
                Ok(resp) => Response::Sigrok(resp),
                Err(err) => err.into(),
            },
            Request::Lock { addr, timeout_ms } => {
                let addr = match Address::parse(&addr) {
                    Ok(addr) => addr,
                    Err(err) => return err.into(),
                };
                let timeout = Duration::from_millis(timeout_ms as u64);
                self.inventory.wait_for_lock(&addr, None).await;
                let ret = self.inventory.lock(&self.server, &addr, timeout).await;
                Response::Locked {
                    addr: addr.to_string(),
                    lock_id: ret,
                }
            }
            Request::Unlock(id) => {
                self.inventory.unlock(id).await;
                Response::Done
            }
            Request::Hid { addr, task, lock } => match self.handle_hid(&addr, task, lock).await {
                Ok(x) => Response::Hid(x),
                Err(x) => x.into(),
            },
            Request::ListHidDevices => match crate::hid::list_devices().await {
                Ok(result) => Response::Hid(HidResponse::List(result)),
                Err(x) => x.into(),
            },
            Request::Version => {
                let version = crate_version!();
                let version: Vec<_> = version
                    .split(".")
                    .map(|x| x.parse::<u32>().unwrap())
                    .collect();
                Response::Version {
                    major: version[0],
                    minor: version[1],
                    build: version[2],
                }
            }
            Request::ListSerialPorts => match crate::serial::list_devices().await {
                Ok(x) => Response::SerialPorts(x),
                Err(x) => x.into(),
            },
            Request::ListFtdiDevices => todo!(),
        }
    }
}
