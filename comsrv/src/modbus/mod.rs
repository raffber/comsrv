use std::net::SocketAddr;

use serde::{Deserialize, Serialize};
use tokio::stream::StreamExt;
use tokio::sync::{mpsc, oneshot};
use tokio::sync::mpsc::UnboundedReceiver;
use tokio::task;
use tokio_modbus::client::{Context, Reader, tcp, Writer};

use crate::Error;

fn is_one(x: &u16) -> bool {
    *x == 1
}

#[derive(Clone, Serialize, Deserialize)]
pub enum ModBusRequest {
    ReadCoil {
        addr: u16,
        #[serde(skip_serializing_if = "is_one")]
        cnt: u16,
    },
    ReadDiscrete {
        addr: u16,
        #[serde(skip_serializing_if = "is_one")]
        cnt: u16,
    },
    ReadInput {
        addr: u16,
        #[serde(skip_serializing_if = "is_one")]
        cnt: u16,
    },
    ReadHolding {
        addr: u16,
        #[serde(skip_serializing_if = "is_one")]
        cnt: u16,
    },
    WriteCoil {
        addr: u16,
        values: Vec<bool>,
    },
    WriteRegister {
        addr: u16,
        data: Vec<u16>,
    },
}

#[derive(Clone, Serialize, Deserialize)]
pub enum ModBusResponse {
    Done,
    Number(Vec<u16>),
    Bool(Vec<bool>),
}

struct Msg {
    req: ModBusRequest,
    tx: oneshot::Sender<crate::Result<ModBusResponse>>,
}

#[derive(Clone)]
struct Instrument {
    tx: mpsc::UnboundedSender<Msg>,
}

impl Instrument {
    async fn connect(addr: SocketAddr) -> crate::Result<Self> {
        let (tx, rx) = mpsc::unbounded_channel();
        let ctx = tcp::connect(addr).await.map_err(Error::io)?;
        task::spawn(thread(ctx, rx));
        Ok(Instrument {
            tx
        })
    }

    async fn handle(&mut self, req: ModBusRequest) -> crate::Result<ModBusResponse> {
        let (tx, rx) = oneshot::channel();
        let req = Msg {
            req,
            tx,
        };
        self.tx.send(req).map_err(|_| Error::Disconnected)?;
        rx.await.map_err(|_| Error::Disconnected)?
    }
}

async fn thread(mut ctx: Context, mut rx: UnboundedReceiver<Msg>) {
    while let Some(msg) = rx.next().await {
        match msg.req {
            ModBusRequest::ReadCoil { addr, cnt } => {
                let ret = ctx.read_coils(addr, cnt).await
                    .map_err(Error::io)
                    .map(ModBusResponse::Bool);
                if msg.tx.send(ret).is_err() {
                    break;
                }
            }
            ModBusRequest::ReadDiscrete { addr, cnt } => {
                let ret = ctx.read_discrete_inputs(addr, cnt).await
                    .map_err(Error::io)
                    .map(ModBusResponse::Bool);
                if msg.tx.send(ret).is_err() {
                    break;
                }
            }
            ModBusRequest::ReadInput { addr, cnt } => {
                let ret = ctx.read_input_registers(addr, cnt).await
                    .map_err(Error::io)
                    .map(ModBusResponse::Number);
                if msg.tx.send(ret).is_err() {
                    break;
                }
            }
            ModBusRequest::ReadHolding { addr, cnt } => {
                let ret = ctx.read_holding_registers(addr, cnt).await
                    .map_err(Error::io)
                    .map(ModBusResponse::Number);
                if msg.tx.send(ret).is_err() {
                    break;
                }
            }
            ModBusRequest::WriteCoil { addr, values } => {
                let ret = ctx.write_multiple_coils(addr, &values).await
                    .map_err(Error::io)
                    .map(|_| ModBusResponse::Done);
                if msg.tx.send(ret).is_err() {
                    break;
                }
            }
            ModBusRequest::WriteRegister { addr, data } => {
                let ret = ctx.write_multiple_registers(addr, &data).await
                    .map_err(Error::io)
                    .map(|_| ModBusResponse::Done);
                if msg.tx.send(ret).is_err() {
                    break;
                }
            }
        }
    }
}
