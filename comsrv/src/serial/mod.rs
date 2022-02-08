mod prologix;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use tokio_serial::{ErrorKind, SerialPortBuilderExt, SerialStream};

pub use params::SerialParams;

use crate::clonable_channel::ClonableChannel;
use crate::iotask::{IoHandler, IoTask};
use crate::modbus::handle_modbus_request_timeout;
use crate::serial::params::{DataBits, Parity, StopBits};
use crate::serial::prologix::{handle_prologix_request, init_prologix};
use comsrv_protocol::{
    ByteStreamRequest, ByteStreamResponse, ModBusRequest, ModBusResponse, ScpiRequest, ScpiResponse,
};
use std::time::Duration;
use tokio_modbus::prelude::Slave;

pub mod params;

const DEFAULT_TIMEOUT_MS: u64 = 500;

pub enum Request {
    Prologix {
        gpib_addr: u8,
        req: ScpiRequest,
    },
    Serial {
        params: SerialParams,
        req: ByteStreamRequest,
    },
    ModBus {
        params: SerialParams,
        req: ModBusRequest,
        slave_addr: u8,
    },
}

impl Request {
    pub fn params(&self) -> SerialParams {
        match self {
            Request::Prologix { .. } => SerialParams {
                baud: 9600,
                data_bits: DataBits::Eight,
                stop_bits: StopBits::One,
                parity: Parity::None,
            },
            Request::Serial { params, .. } => params.clone(),
            Request::ModBus { params, .. } => params.clone(),
        }
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub enum Response {
    Bytes(ByteStreamResponse),
    Scpi(ScpiResponse),
    ModBus(ModBusResponse),
}

pub struct Handler {
    serial: Option<(SerialStream, SerialParams)>,
    path: String,
}

impl From<tokio_serial::Error> for crate::Error {
    fn from(x: tokio_serial::Error) -> Self {
        match x.kind {
            ErrorKind::NoDevice => crate::Error::InvalidRequest,
            ErrorKind::InvalidInput => crate::Error::InvalidRequest,
            ErrorKind::Unknown => crate::Error::NotSupported,
            ErrorKind::Io(io) => {
                let desc = format!("{:?}", io);
                let io_err = std::io::Error::new(io, desc.as_str());
                crate::Error::io(io_err)
            }
        }
    }
}

async fn open_serial_port(path: &str, params: &SerialParams) -> crate::Result<SerialStream> {
    Ok(tokio_serial::new(path, params.baud)
        .parity(params.parity.into())
        .stop_bits(params.stop_bits.into())
        .data_bits(params.data_bits.into())
        .open_native_async()?)
}

#[async_trait]
impl IoHandler for Handler {
    type Request = Request;
    type Response = Response;

    async fn handle(&mut self, req: Self::Request) -> crate::Result<Self::Response> {
        let new_params = req.params();
        let (mut serial, opened) = match self.serial.take() {
            None => {
                log::debug!("Opening {}", self.path);
                let stream = open_serial_port(&self.path, &new_params).await?;
                (stream, true)
            }
            Some((serial, old_params)) => {
                if old_params == new_params {
                    log::debug!("reusing already open handle to {}", self.path);
                    (serial, false)
                } else {
                    drop(serial);
                    log::debug!("Reopening {}", self.path);
                    let stream = open_serial_port(&self.path, &new_params).await?;
                    (stream, true)
                }
            }
        };
        if opened {
            if let Request::Prologix { .. } = req {
                init_prologix(&mut serial).await?;
            }
        }
        let ret = match req {
            Request::Prologix { gpib_addr, req } => {
                handle_prologix_request(&mut serial, gpib_addr, req)
                    .await
                    .map(Response::Scpi)
            }
            Request::Serial { params: _, req } => crate::bytestream::handle(&mut serial, req)
                .await
                .map(Response::Bytes),
            Request::ModBus {
                params: _,
                req,
                slave_addr,
            } => {
                let channel = ClonableChannel::new(serial);
                let mut ctx =
                    tokio_modbus::client::rtu::connect_slave(channel.clone(), Slave(slave_addr))
                        .await?;
                let timeout = Duration::from_millis(1000);
                let ret = handle_modbus_request_timeout(&mut ctx, req, timeout)
                    .await
                    .map(Response::ModBus);
                serial = channel.take().unwrap();
                ret
            }
        };
        if ret.is_ok() {
            self.serial.replace((serial, new_params));
        }
        ret
    }
}

#[derive(Clone)]
pub struct Instrument {
    inner: IoTask<Handler>,
}

impl Instrument {
    pub fn new(path: String) -> Self {
        let handler = Handler { serial: None, path };
        Self {
            inner: IoTask::new(handler),
        }
    }

    pub async fn request(&mut self, req: Request) -> crate::Result<Response> {
        self.inner.request(req).await
    }

    pub fn disconnect(mut self) {
        self.inner.disconnect()
    }
}
