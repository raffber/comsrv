mod prologix;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use tokio_serial::Serial;

pub use params::SerialParams;

use crate::{Error, ScpiRequest, ScpiResponse};
use crate::bytestream::{ByteStreamRequest, ByteStreamResponse};
use crate::iotask::{IoHandler, IoTask};
use crate::serial::params::{DataBits, Parity, StopBits};
use crate::serial::prologix::{init_prologix, handle_prologix_request};

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
}

impl Request {
    pub fn params(&self) -> SerialParams {
        match self {
            Request::Prologix { .. } => {
                SerialParams {
                    baud: 9600,
                    data_bits: DataBits::Eight,
                    stop_bits: StopBits::One,
                    parity: Parity::None,
                }
            }
            Request::Serial { params, req: _ } => params.clone()
        }
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub enum Response {
    Bytes(ByteStreamResponse),
    Scpi(ScpiResponse),
}

pub struct Handler {
    serial: Option<(Serial, SerialParams)>,
    path: String,
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
                let settings = new_params.clone().into();
                (Serial::from_path(&self.path, &settings).map_err(Error::io)?, true)
            }
            Some((serial, old_params)) => {
                if old_params == new_params {
                    log::debug!("reusing already open handle to {}", self.path);
                    (serial, false)
                } else {
                    drop(serial);
                    log::debug!("Reopening {}", self.path);
                    let settings = new_params.clone().into();
                    (Serial::from_path(&self.path, &settings).map_err(Error::io)?, true)
                }
            }
        };
        if opened {
            match req {
                Request::Prologix { .. } => {
                    init_prologix(&mut serial).await?;
                }
                _ => {}
            }
        }
        let ret = handle_request(&mut serial, req).await;
        self.serial.replace((serial, new_params));
        ret
    }
}

#[derive(Clone)]
pub struct Instrument {
    inner: IoTask<Handler>,
}

impl Instrument {
    pub fn new(path: String) -> Self {
        let handler = Handler {
            serial: None,
            path,
        };
        Self {
            inner: IoTask::new(handler)
        }
    }

    pub async fn request(&mut self, req: Request) -> crate::Result<Response> {
        self.inner.request(req).await
    }

    pub fn disconnect(mut self) {
        self.inner.disconnect()
    }
}

async fn handle_request(serial: &mut Serial, req: Request) -> crate::Result<Response> {
    match req {
        Request::Prologix { gpib_addr, req } => {
            handle_prologix_request(serial, gpib_addr, req).await.map(Response::Scpi)
        }
        Request::Serial { params: _, req } => {
            crate::bytestream::handle(serial, req).await.map(Response::Bytes)
        }
    }
}
