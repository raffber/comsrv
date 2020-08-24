use serde::{Deserialize, Serialize};
use tokio::task;

use wsrpc::server::Server;

use crate::{ScpiRequest, ScpiResponse};
use crate::inventory::Inventory;
use crate::visa::VisaError;

#[derive(Clone, Serialize, Deserialize)]
pub enum Request {
    Scpi {
        addr: String,
        task: ScpiRequest,
    },
    SetTimeout {
        addr: String,
        timeout: f32,
    },
    GetTimeout(String),
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

    async fn handle_request(&self, _req: Request) -> Response {
        todo!()
    }
}

