use std::net::SocketAddr;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use tokio_modbus::client::{tcp, Client, Context, Reader, Writer};

use crate::iotask::{IoHandler, IoTask};
use crate::serial::SerialParams;
use crate::Error;
use std::fmt::{Display, Formatter, Result as FmtResult};
use tokio::time::{sleep, timeout, Duration};
use tokio_modbus::prelude::{Response, Slave, SlaveContext};

fn is_one(x: &u16) -> bool {
    *x == 1
}

#[derive(Clone, Hash, Copy)]
pub enum ModBusTransport {
    Rtu,
    Tcp,
}

impl Display for ModBusTransport {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        match self {
            ModBusTransport::Rtu => f.write_str("rtu"),
            ModBusTransport::Tcp => f.write_str("tcp"),
        }
    }
}

#[derive(Clone, Hash)]
pub enum ModBusAddress {
    Serial { path: String, params: SerialParams },
    Tcp { addr: SocketAddr },
}

impl Display for ModBusAddress {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        match self {
            ModBusAddress::Serial { path, params } => {
                f.write_fmt(format_args!("{}::{}", path, params))
            }
            ModBusAddress::Tcp { addr } => f.write_fmt(format_args!("{}", addr)),
        }
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub enum ModBusRequest {
    ReadCoil { addr: u16, cnt: u16 },
    ReadDiscrete { addr: u16, cnt: u16 },
    ReadInput { addr: u16, cnt: u16 },
    ReadHolding { addr: u16, cnt: u16 },
    WriteCoil { addr: u16, values: Vec<bool> },
    WriteRegister { addr: u16, data: Vec<u16> },
    CustomCommand { code: u8, data: Vec<u8> },
}

#[derive(Clone, Serialize, Deserialize)]
pub enum ModBusResponse {
    Done,
    Number(Vec<u16>),
    Bool(Vec<bool>),
    Custom { code: u8, data: Vec<u8> },
}

#[derive(Clone)]
struct HandlerRequest {
    inner: ModBusRequest,
    slave_id: u8,
}

#[derive(Clone)]
pub struct ModBusTcpInstrument {
    inner: IoTask<Handler>,
}

impl ModBusTcpInstrument {
    pub fn new(addr: SocketAddr) -> Self {
        Self {
            inner: IoTask::new(Handler { addr, ctx: None }),
        }
    }
    pub async fn request(
        &mut self,
        req: ModBusRequest,
        slave_id: u8,
    ) -> crate::Result<ModBusResponse> {
        let req = HandlerRequest {
            inner: req,
            slave_id,
        };
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
    type Request = HandlerRequest;
    type Response = ModBusResponse;

    async fn handle(&mut self, req: Self::Request) -> crate::Result<Self::Response> {
        let mut ctx = if let Some(ctx) = self.ctx.take() {
            ctx
        } else {
            tcp::connect(self.addr).await.map_err(Error::io)?
        };
        ctx.set_slave(Slave(req.slave_id));
        let timeout = Duration::from_millis(1000);
        let ret = handle_modbus_request_timeout(&mut ctx, req.inner.clone(), timeout).await;
        match ret {
            Ok(ret) => {
                self.ctx.replace(ctx);
                Ok(ret)
            }
            Err(err) => {
                drop(ctx);
                if err.should_retry() {
                    sleep(Duration::from_millis(1000)).await;
                    let mut ctx = tcp::connect(self.addr).await.map_err(Error::io)?;
                    ctx.set_slave(Slave(req.slave_id));
                    let ret =
                        handle_modbus_request_timeout(&mut ctx, req.inner.clone(), timeout).await;
                    if ret.is_ok() {
                        // this time we succeeded, reinsert ctx
                        self.ctx.replace(ctx);
                    }
                    ret
                } else {
                    Err(err)
                }
            }
        }
    }
}

pub async fn handle_modbus_request_timeout(
    ctx: &mut Context,
    req: ModBusRequest,
    duration: Duration,
) -> crate::Result<ModBusResponse> {
    let fut = handle_modbus_request(ctx, req);
    match timeout(duration, fut).await {
        Ok(x) => x,
        Err(_) => Err(crate::Error::Timeout),
    }
}

pub async fn handle_modbus_request(
    ctx: &mut Context,
    req: ModBusRequest,
) -> crate::Result<ModBusResponse> {
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
        ModBusRequest::CustomCommand { code, data } => {
            use tokio_modbus::prelude::Request;
            let resp = ctx
                .call(Request::Custom(code, data))
                .await
                .map_err(Error::io)?;
            match resp {
                Response::Custom(code, data) => Ok(ModBusResponse::Custom { code, data }),
                _ => Err(Error::InvalidResponse),
            }
        }
    }
}
