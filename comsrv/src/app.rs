use wsrpc::server::Server;
use serde::{Serialize, Deserialize};
use crate::{ScpiRequest, ScpiResponse};
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

struct App {
    server: Server<Request, Response>,
}
