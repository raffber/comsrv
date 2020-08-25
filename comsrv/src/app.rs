use serde::{Deserialize, Serialize};
use tokio::task;

use wsrpc::server::Server;

use crate::{ScpiRequest, ScpiResponse, Error};
use crate::inventory::Inventory;
use crate::visa::{VisaError, VisaOptions};

#[derive(Clone, Serialize, Deserialize)]
pub enum InstrumentOptions {
    Visa(VisaOptions),
    Default,
}

#[derive(Clone, Serialize, Deserialize)]
pub enum Request {
    Scpi {
        addr: String,
        task: ScpiRequest,
        options: InstrumentOptions,
    },
    ListInstruments,
}

#[derive(Clone, Serialize, Deserialize)]
pub enum RpcError {
    Io(String),
    Visa(VisaError),
    Disconnected,
    NotSupported,
}

#[derive(Clone, Serialize, Deserialize)]
pub enum Response {
    Error(RpcError),
    Instruments(Vec<String>),
    Scpi(ScpiResponse),
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
        let mut stream = self.server.listen(url).await;
        while let Some(msg) = stream.recv().await {
            let (req, rep) = msg.take();
            let app = self.clone();
            task::spawn(async move {
                let response = app.handle_request(req).await;
                rep.answer(response);
            });
        }
    }

    fn map_error(err: Error) -> RpcError {
        todo!()

    }

    async fn handle_scpi(&self, addr: String, task: ScpiRequest, options: InstrumentOptions) -> Result<ScpiResponse, RpcError> {
        let inventory = self.inventory.clone();
        let instr = inventory.connect(addr, options).await.map_err(Self::map_error)?;
        todo!()
    }

    async fn handle_request(&self, req: Request) -> Response {
        match req {
            Request::Scpi { addr, task, options } => {
                match self.handle_scpi(addr, task, options).await {
                    Ok(result) => Response::Scpi(result),
                    Err(err) => Response::Error(err)
                }
            }
            Request::ListInstruments => {
                Response::Instruments(self.inventory.list().await)
            }
        }
    }
}

