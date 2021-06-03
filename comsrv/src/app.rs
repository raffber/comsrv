use serde::{Deserialize, Serialize};
use tokio::sync::mpsc::UnboundedReceiver;
use tokio::task;
use wsrpc::server::{Requested, Server as WsrpcServer};

use crate::bytestream::{ByteStreamRequest, ByteStreamResponse};
use crate::can::{CanRequest, CanResponse};
use crate::instrument::InstrumentOptions;
use crate::instrument::{Address, Instrument};
use crate::inventory::Inventory;
use crate::modbus::{ModBusRequest, ModBusResponse};
use crate::scpi::{ScpiRequest, ScpiResponse};
use crate::serial::{Request as SerialRequest, Response as SerialResponse, SerialParams};
use crate::sigrok::{SigrokRequest, SigrokResponse};
use crate::tcp::{TcpRequest, TcpResponse};
use crate::visa::VisaOptions;
use crate::{sigrok, Error};
use std::time::Duration;
use uuid::Uuid;

#[derive(Clone, Serialize, Deserialize)]
pub enum Request {
    Scpi {
        addr: String,
        task: ScpiRequest,
        #[serde(skip_serializing_if = "Option::is_none", default)]
        lock: Option<Uuid>,
        #[serde(skip_serializing_if = "InstrumentOptions::is_default", default)]
        options: InstrumentOptions, // XXX: currently unused, remove?
    },
    ModBus {
        addr: String,
        task: ModBusRequest,
        #[serde(skip_serializing_if = "Option::is_none", default)]
        lock: Option<Uuid>,
    },
    Bytes {
        addr: String,
        task: ByteStreamRequest,
        #[serde(skip_serializing_if = "Option::is_none", default)]
        lock: Option<Uuid>,
    },
    Can {
        addr: String,
        task: CanRequest,
        #[serde(skip_serializing_if = "Option::is_none", default)]
        lock: Option<Uuid>,
    },
    Sigrok {
        addr: String,
        task: SigrokRequest,
    },
    ListSigrokDevices,
    ListInstruments,
    Lock {
        addr: String,
        timeout_ms: u32,
    },
    Unlock(Uuid),
    DropAll,
    Drop(String),
    Shutdown,
}

#[derive(Clone, Serialize, Deserialize)]
pub enum Response {
    Error(crate::Error),
    Instruments(Vec<String>),
    Scpi(ScpiResponse),
    Bytes(ByteStreamResponse),
    ModBus(ModBusResponse),
    Can(CanResponse),
    Sigrok(SigrokResponse),
    Locked { addr: String, lock_id: Uuid },
    Done,
}

pub type Server = WsrpcServer<Request, Response>;

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
            log::debug!("Incoming Request: {}", serde_json::to_string(&req).unwrap());
            task::spawn(async move {
                let response = app.handle_request(req).await;
                log::debug!("Answering: {}", serde_json::to_string(&response).unwrap());
                rep.answer(response);
            });
            log::debug!("Leaving request handler.");
        }
    }

    async fn handle_scpi(
        &self,
        addr: String,
        task: ScpiRequest,
        lock: Option<Uuid>,
        options: InstrumentOptions,
    ) -> Result<ScpiResponse, Error> {
        let addr = Address::parse(&addr)?;
        self.inventory.wait_for_lock(&addr, lock.as_ref()).await;
        match self.inventory.connect(&self.server, &addr) {
            Instrument::Visa(instr) => {
                let opt = match options {
                    InstrumentOptions::Visa(x) => x.clone(),
                    InstrumentOptions::Default => VisaOptions::default(),
                };
                match instr.request(task, opt).await {
                    Ok(x) => Ok(x),
                    Err(x) => {
                        self.inventory.disconnect(&addr);
                        Err(x.into())
                    }
                }
            }
            Instrument::Serial(mut instr) => match addr {
                Address::Prologix { file: _, gpib } => {
                    let response = instr
                        .request(SerialRequest::Prologix {
                            gpib_addr: gpib,
                            req: task,
                        })
                        .await;
                    match response {
                        Ok(SerialResponse::Scpi(resp)) => Ok(resp),
                        Ok(_) => {
                            self.inventory.disconnect(&addr);
                            Err(Error::NotSupported)
                        }
                        Err(x) => {
                            self.inventory.disconnect(&addr);
                            Err(x.into())
                        }
                    }
                }
                _ => Err(Error::NotSupported),
            },
            Instrument::Vxi(mut instr) => {
                let opt = match options {
                    InstrumentOptions::Visa(x) => x.clone(),
                    InstrumentOptions::Default => VisaOptions::default(),
                };
                match instr.request(task, opt).await {
                    Ok(x) => Ok(x),
                    Err(x) => {
                        self.inventory.disconnect(&addr);
                        Err(x.into())
                    }
                }
            }
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
        let mut instr = self
            .inventory
            .connect(&self.server, &addr)
            .into_modbus()
            .ok_or(Error::NotSupported)?;
        match instr.request(task).await {
            Ok(x) => Ok(x),
            Err(x) => {
                self.inventory.disconnect(&addr);
                Err(x)
            }
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
            Err(x) => {
                self.inventory.disconnect(&addr);
                Err(x)
            }
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
            Err(x) => {
                self.inventory.disconnect(&addr);
                Err(x)
            }
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
        match device.request(task).await {
            Ok(x) => Ok(x),
            Err(x) => {
                if device.check_disconnect(&x) {
                    self.inventory.disconnect(&addr);
                }
                Err(x.into())
            }
        }
    }

    pub async fn shutdown(&self) {
        self.inventory.disconnect_all();
        self.server.shutdown().await;
    }

    async fn handle_request(&self, req: Request) -> Response {
        match req {
            Request::Scpi {
                addr,
                task,
                lock,
                options,
            } => match self.handle_scpi(addr, task, lock, options).await {
                Ok(result) => Response::Scpi(result),
                Err(err) => Response::Error(err),
            },
            Request::ListInstruments => Response::Instruments(self.inventory.list()),
            Request::ModBus { addr, task, lock } => {
                match self.handle_modbus(addr, task, lock).await {
                    Ok(result) => Response::ModBus(result),
                    Err(err) => Response::Error(err),
                }
            }
            Request::Bytes { addr, task, lock } => match self.handle_bytes(&addr, task, lock).await
            {
                Ok(result) => Response::Bytes(result),
                Err(err) => Response::Error(err),
            },
            Request::Can { addr, task, lock } => match self.handle_can(&addr, task, lock).await {
                Ok(result) => Response::Can(result),
                Err(err) => Response::Error(err),
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
                Err(err) => Response::Error(err),
            },
            Request::Sigrok { addr, task } => {
                let addr = match Address::parse(&addr) {
                    Ok(addr) => addr,
                    Err(err) => return Response::Error(err.into()),
                };
                let device = match addr {
                    Address::Sigrok { device } => device,
                    _ => return Response::Error(Error::NotSupported),
                };
                match sigrok::read(device, task).await {
                    Ok(resp) => Response::Sigrok(resp),
                    Err(err) => Response::Error(err.into()),
                }
            }
            Request::ListSigrokDevices => match sigrok::list().await {
                Ok(resp) => Response::Sigrok(resp),
                Err(err) => Response::Error(err.into()),
            },
            Request::Lock { addr, timeout_ms } => {
                let addr = match Address::parse(&addr) {
                    Ok(addr) => addr,
                    Err(err) => return Response::Error(err.into()),
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
        }
    }
}
