use serde::{Deserialize, Serialize};
use tokio::task;

use wsrpc::server::Server;

use crate::{Error, ScpiRequest, ScpiResponse};
use crate::instrument::Instrument;
use crate::instrument::InstrumentOptions;
use crate::inventory::Inventory;
use crate::modbus::{ModBusRequest, ModBusResponse};
use crate::visa::VisaError;
use std::net::SocketAddr;

#[derive(Clone, Serialize, Deserialize)]
pub enum Request {
    Scpi {
        addr: String,
        task: ScpiRequest,

        #[serde(skip_serializing_if = "InstrumentOptions::is_default")]
        options: InstrumentOptions,
    },
    ModBus {
        addr: String,
        task: ModBusRequest,
    },
    ListInstruments,
}

#[derive(Clone, Serialize, Deserialize)]
pub enum Response {
    Error(RpcError),
    Instruments(Vec<String>),
    Scpi(ScpiResponse),
    ModBus(ModBusResponse),
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
        }
    }
}

#[derive(Clone)]
pub struct App {
    server: Server<Request, Response>,
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
        let http_addr: SocketAddr = format!("0.0.0.0:{}", port+1).parse().unwrap();
        let mut stream = self.server.listen(url, http_addr).await;
        while let Some(msg) = stream.recv().await {
            let (req, rep) = msg.take();
            let app = self.clone();
            task::spawn(async move {
                let response = app.handle_request(req).await;
                rep.answer(response);
            });
        }
    }

    async fn handle_scpi(&self, addr: String, task: ScpiRequest, options: &InstrumentOptions) -> Result<ScpiResponse, RpcError> {
        let instr = self.get_instrument(&addr, options).await?;
        match instr {
            Instrument::Visa(instr) => {
                let ret = instr.handle_scpi(task).await;
                if ret.is_err() {
                    self.inventory.close(&addr).await;
                }
                Ok(ret?)
            }
            Instrument::Modbus(_) => {
                Err(RpcError::NotSupported)
            }
            Instrument::Prologix(mut instr) => {
                let ret = instr.handle(task).await;
                if ret.is_err() {
                    self.inventory.close(&addr).await;
                }
                Ok(ret?)
            }
        }
    }

    async fn handle_modbus(&self, addr: String, task: ModBusRequest) -> Result<ModBusResponse, RpcError> {
        let instr = self.get_instrument(&addr, &InstrumentOptions::Default).await?;
        match instr {
            Instrument::Visa(_) => Err(RpcError::NotSupported),
            Instrument::Modbus(mut instr) => {
                let ret = instr.handle(task).await;
                if ret.is_err() {
                    self.inventory.close(&addr).await;
                }
                Ok(ret?)
            }
            Instrument::Prologix(_) => Err(RpcError::NotSupported),
        }
    }

    async fn get_instrument(&self, addr: &str, options: &InstrumentOptions) -> Result<Instrument, RpcError> {
        let inventory = self.inventory.clone();
        let instr = inventory.connect(addr, options).await;
        if instr.is_err() {
            inventory.close(&addr).await;
        }
        Ok(instr?)
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
                Response::Instruments(self.inventory.list().await)
            }
            Request::ModBus { addr, task } => {
                match self.handle_modbus(addr, task).await {
                    Ok(result) => Response::ModBus(result),
                    Err(err) => Response::Error(err)
                }
            }
        }
    }
}

