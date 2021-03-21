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
use crate::visa::VisaOptions;
use crate::{sigrok, Error};

#[derive(Clone, Serialize, Deserialize)]
pub enum Request {
    Scpi {
        addr: String,
        task: ScpiRequest,

        #[serde(skip_serializing_if = "InstrumentOptions::is_default", default)]
        options: InstrumentOptions, // XXX: currently unused, remove?
    },
    ModBus {
        addr: String,
        task: ModBusRequest,
    },
    Bytes {
        addr: String,
        task: ByteStreamRequest,
    },
    Can {
        addr: String,
        task: CanRequest,
    },
    Sigrok {
        addr: String,
        task: SigrokRequest,
    },
    ListSigrokDevices,
    ListInstruments,
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
            task::spawn(async move {
                let response = app.handle_request(req).await;
                rep.answer(response);
            });
            log::debug!("Leaving request handler.");
        }
    }

    async fn handle_scpi(
        &self,
        addr: String,
        task: ScpiRequest,
        options: &InstrumentOptions,
    ) -> Result<ScpiResponse, Error> {
        let addr = Address::parse(&addr)?;
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
    ) -> Result<ModBusResponse, Error> {
        let addr = Address::parse(&addr)?;
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
        match instr.request(task.clone()).await {
            Ok(x) => Ok(x),
            Err(x) => {
                self.inventory.disconnect(&addr);
                Err(x)
            }
        }
    }

    async fn handle_bytes(
        &self,
        addr: &str,
        task: ByteStreamRequest,
    ) -> Result<ByteStreamResponse, Error> {
        let addr = Address::parse(&addr)?;
        match &addr {
            Address::Serial { path: _, params } => self.handle_serial(&addr, params, task).await,
            Address::Tcp { .. } => self.handle_tcp(&addr, task).await,
            _ => Err(Error::NotSupported),
        }
    }

    async fn handle_can(&self, addr: &str, task: CanRequest) -> Result<CanResponse, Error> {
        let addr = Address::parse(&addr)?;
        let mut device = match self.inventory.connect(&self.server, &addr) {
            Instrument::Can(device) => device,
            _ => return Err(Error::NotSupported),
        };
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
                options,
            } => match self.handle_scpi(addr, task, &options).await {
                Ok(result) => Response::Scpi(result),
                Err(err) => Response::Error(err),
            },
            Request::ListInstruments => Response::Instruments(self.inventory.list()),
            Request::ModBus { addr, task } => match self.handle_modbus(addr, task).await {
                Ok(result) => Response::ModBus(result),
                Err(err) => Response::Error(err),
            },
            Request::Bytes { addr, task } => match self.handle_bytes(&addr, task).await {
                Ok(result) => Response::Bytes(result),
                Err(err) => Response::Error(err),
            },
            Request::Can { addr, task } => match self.handle_can(&addr, task).await {
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
        }
    }
}
