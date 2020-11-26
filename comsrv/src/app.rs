use std::net::SocketAddr;

use serde::{Deserialize, Serialize};
use tokio::task;
use wsrpc::server::Server as WsrpcServer;

use crate::{Error, ScpiRequest, ScpiResponse};
use crate::bytestream::{ByteStreamRequest, ByteStreamResponse};
use crate::can::{CanRequest, CanResponse};
use crate::instrument::{Address, Instrument};
use crate::instrument::InstrumentOptions;
use crate::inventory::Inventory;
use crate::modbus::{ModBusRequest, ModBusResponse};
use crate::serial::{Request as SerialRequest, Response as SerialResponse, SerialParams};
use crate::visa::{VisaError, VisaOptions};

#[derive(Clone, Serialize, Deserialize)]
pub enum Request {
    Scpi {
        addr: String,
        task: ScpiRequest,
        #[serde(skip_serializing_if = "InstrumentOptions::is_default", default)]
        options: InstrumentOptions,
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
    ListInstruments,
    DropAll,
}

#[derive(Clone, Serialize, Deserialize)]
pub enum Response {
    Error(RpcError),
    Instruments(Vec<String>),
    Scpi(ScpiResponse),
    Bytes(ByteStreamResponse),
    ModBus(ModBusResponse),
    Can(CanResponse),
    Done,
}

#[derive(Clone, Serialize, Deserialize)]
pub enum RpcError {
    Io(String),
    Visa(VisaError),
    Disconnected,
    NotSupported,
    CannotConnect,
    DecodeError(String),
    InvalidBinaryHeader,
    NotTerminated,
    InvalidAddress,
    Timeout,
    Vxi(String),
    InvalidBitRate,
    PCanError(u32, String),
    CanBusError(async_can::BusError),
    CanSendQueueFull,
}

impl From<Error> for RpcError {
    fn from(x: Error) -> Self {
        match x {
            Error::Visa(x) => RpcError::Visa(x),
            Error::Io(x) => RpcError::Io(format!("{}", x)),
            Error::Disconnected => RpcError::Disconnected,
            Error::NotSupported => RpcError::NotSupported,
            Error::CannotConnect => RpcError::CannotConnect,
            Error::DecodeError(x) => RpcError::DecodeError(format!("{}", x)),
            Error::InvalidBinaryHeader => RpcError::InvalidBinaryHeader,
            Error::NotTerminated => RpcError::NotTerminated,
            Error::InvalidAddress => RpcError::InvalidAddress,
            Error::Timeout => RpcError::Timeout,
            Error::Vxi(x) => RpcError::Vxi(format!("{}", x)),
            Error::InvalidBitRate => RpcError::InvalidBitRate,
            Error::PCanError(code, desc) => RpcError::PCanError(code, desc),
            Error::CanBusError(err) => RpcError::CanBusError(err),
            Error::CanSendQueueFull => RpcError::CanSendQueueFull,
        }
    }
}

pub type Server = WsrpcServer<Request, Response>;

#[derive(Clone)]
pub struct App {
    server: Server,
    inventory: Inventory,
}

impl App {
    pub fn new() -> Self {
        App {
            server: Server::new(),
            inventory: Inventory::new(),
        }
    }

    pub async fn run(&self, port: u16) {
        let url = format!("0.0.0.0:{}", port);
        let http_addr: SocketAddr = format!("0.0.0.0:{}", port + 1).parse().unwrap();
        let mut stream = self.server.listen(url, http_addr).await;
        while let Some(msg) = stream.recv().await {
            let (req, rep) = msg.split();
            let app = self.clone();
            task::spawn(async move {
                let response = app.handle_request(req).await;
                rep.answer(response);
            });
        }
    }

    async fn handle_scpi(&self, addr: String, task: ScpiRequest, options: &InstrumentOptions) -> Result<ScpiResponse, RpcError> {
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
            Instrument::Serial(mut instr) => {
                match addr {
                    Address::Prologix { file: _, gpib } => {
                        let response = instr.request(SerialRequest::Prologix {
                            gpib_addr: gpib,
                            req: task,
                        }).await;
                        match response {
                            Ok(SerialResponse::Scpi(resp)) => {
                                Ok(resp)
                            }
                            Ok(_) => {
                                self.inventory.disconnect(&addr);
                                Err(RpcError::NotSupported)
                            }
                            Err(x) => {
                                self.inventory.disconnect(&addr);
                                Err(x.into())
                            }
                        }
                    }
                    _ => Err(RpcError::NotSupported)
                }
            }
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
            _ => Err(RpcError::NotSupported)
        }
    }

    async fn handle_modbus(&self, addr: String, task: ModBusRequest) -> Result<ModBusResponse, RpcError> {
        let addr = Address::parse(&addr)?;
        match self.inventory.connect(&self.server, &addr) {
            Instrument::Modbus(mut instr) => {
                match instr.request(task).await {
                    Ok(x) => Ok(x),
                    Err(x) => {
                        self.inventory.disconnect(&addr);
                        Err(x.into())
                    }
                }
            }
            _ => {
                Err(RpcError::NotSupported)
            }
        }
    }

    async fn handle_serial(&self, addr: Address, params: &SerialParams, task: ByteStreamRequest) -> Result<ByteStreamResponse, RpcError> {
        let params = params.clone();
        let req = SerialRequest::Serial {
            params,
            req: task,
        };
        match self.inventory.connect(&self.server, &addr) {
            Instrument::Serial(mut instr) => {
                match instr.request(req).await {
                    Ok(x) => {
                        match x {
                            SerialResponse::Bytes(x) => Ok(x),
                            _ => panic!("Invalid answer. This is a bug"),
                        }
                    }
                    Err(x) => {
                        self.inventory.disconnect(&addr);
                        Err(x.into())
                    }
                }
            }
            _ => {
                Err(RpcError::NotSupported)
            }
        }
    }

    async fn handle_bytes(&self, addr: &str, task: ByteStreamRequest) -> Result<ByteStreamResponse, RpcError> {
        let addr = Address::parse(&addr)?;
        let addr2 = addr.clone();
        match &addr {
            Address::Serial { path: _, params } => {
                self.handle_serial(addr2, params, task).await
            }
            _ => {
                Err(RpcError::NotSupported)
            }
        }
    }

    async fn handle_can(&self, addr: &str, task: CanRequest) -> Result<CanResponse, RpcError> {
        let addr = Address::parse(&addr)?;
        let mut device = match self.inventory.connect(&self.server, &addr) {
            Instrument::Can(device) => device,
            _ => return Err(RpcError::NotSupported)
        };
        match device.request(task).await {
            Ok(x) => Ok(x),
            Err(x) => {
                self.inventory.disconnect(&addr);
                Err(x.into())
            },
        }
    }

    async fn handle_request(&self, req: Request) -> Response {
        match req {
            Request::Scpi { addr, task, options } => {
                match self.handle_scpi(addr, task, &options).await {
                    Ok(result) => Response::Scpi(result),
                    Err(err) => Response::Error(err)
                }
            }
            Request::ListInstruments => {
                Response::Instruments(self.inventory.list())
            }
            Request::ModBus { addr, task } => {
                match self.handle_modbus(addr, task).await {
                    Ok(result) => Response::ModBus(result),
                    Err(err) => Response::Error(err)
                }
            }
            Request::Bytes { addr, task } => {
                match self.handle_bytes(&addr, task).await {
                    Ok(result) => Response::Bytes(result),
                    Err(err) => Response::Error(err),
                }
            }
            Request::Can { addr, task } => {
                match self.handle_can(&addr, task).await {
                    Ok(result) => Response::Can(result),
                    Err(err) => Response::Error(err),
                }
            }
            Request::DropAll => {
                self.inventory.disconnect_all();
                Response::Done
            }
        }
    }
}

