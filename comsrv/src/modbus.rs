use std::net::SocketAddr;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use tokio_modbus::client::{tcp, Context, Reader, Writer};

use crate::iotask::{IoHandler, IoTask};
use crate::Error;

fn is_one(x: &u16) -> bool {
    *x == 1
}

#[derive(Clone, Serialize, Deserialize)]
pub enum ModBusRequest {
    ReadCoil { addr: u16, cnt: u16 },
    ReadDiscrete { addr: u16, cnt: u16 },
    ReadInput { addr: u16, cnt: u16 },
    ReadHolding { addr: u16, cnt: u16 },
    WriteCoil { addr: u16, values: Vec<bool> },
    WriteRegister { addr: u16, data: Vec<u16> },
}

#[derive(Clone, Serialize, Deserialize)]
pub enum ModBusResponse {
    Done,
    Number(Vec<u16>),
    Bool(Vec<bool>),
}

#[derive(Clone)]
pub struct Instrument {
    inner: IoTask<Handler>,
}

impl Instrument {
    pub fn new(addr: SocketAddr) -> Self {
        Self {
            inner: IoTask::new(Handler { addr, ctx: None }),
        }
    }
    pub async fn request(&mut self, req: ModBusRequest) -> crate::Result<ModBusResponse> {
        self.inner.request(req).await
    }

    pub fn disconnect(mut self) {
        self.inner.disconnect()
    }
}

struct Handler {
    addr: SocketAddr,
    ctx: Option<Context>,
}

#[async_trait]
impl IoHandler for Handler {
    type Request = ModBusRequest;
    type Response = ModBusResponse;

    async fn handle(&mut self, req: Self::Request) -> crate::Result<Self::Response> {
        let mut ctx = if let Some(ctx) = self.ctx.take() {
            ctx
        } else {
            tcp::connect(self.addr.clone()).await.map_err(Error::io)?
        };
        let ret = handle_request(&mut ctx, req).await;
        self.ctx.replace(ctx);
        ret
    }
}

async fn handle_request(ctx: &mut Context, req: ModBusRequest) -> crate::Result<ModBusResponse> {
    match req {
        ModBusRequest::ReadCoil { addr, cnt } => ctx
            .read_coils(addr, cnt)
            .await
            .map_err(Error::io)
            .map(ModBusResponse::Bool),
        ModBusRequest::ReadDiscrete { addr, cnt } => ctx
            .read_discrete_inputs(addr, cnt)
            .await
            .map_err(Error::io)
            .map(ModBusResponse::Bool),
        ModBusRequest::ReadInput { addr, cnt } => ctx
            .read_input_registers(addr, cnt)
            .await
            .map_err(Error::io)
            .map(ModBusResponse::Number),
        ModBusRequest::ReadHolding { addr, cnt } => ctx
            .read_holding_registers(addr, cnt)
            .await
            .map_err(Error::io)
            .map(ModBusResponse::Number),
        ModBusRequest::WriteCoil { addr, values } => ctx
            .write_multiple_coils(addr, &values)
            .await
            .map_err(Error::io)
            .map(|_| ModBusResponse::Done),
        ModBusRequest::WriteRegister { addr, data } => ctx
            .write_multiple_registers(addr, &data)
            .await
            .map_err(Error::io)
            .map(|_| ModBusResponse::Done),
    }
}
