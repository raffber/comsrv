mod prologix;

use std::time::Instant;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::time::{Duration, timeout};
use tokio_serial::Serial;

pub use params::SerialParams;

use crate::{Error, ScpiRequest, ScpiResponse};
use crate::app::ByteStreamRequest;
use crate::cobs::{cobs_pack, cobs_unpack};
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
    Done,
    Data(Vec<u8>),
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
            handle_serial_request(serial, req).await
        }
    }
}

async fn handle_serial_request(serial: &mut Serial, req: ByteStreamRequest) -> crate::Result<Response> {
    match req {
        ByteStreamRequest::Write(data) => {
            log::debug!("write: {:?}", data);
            AsyncWriteExt::write_all(serial, &data).await.map_err(Error::io)?;
            Ok(Response::Done)
        }
        ByteStreamRequest::ReadExact { count, timeout_ms } => {
            log::debug!("read exactly {} bytes", count);
            let mut data = vec![0; count as usize];
            let fut = AsyncReadExt::read_exact(serial, data.as_mut_slice());
            let _ = match timeout(Duration::from_millis(timeout_ms as u64), fut).await {
                Ok(x) => x.map_err(Error::io),
                Err(_) => Err(Error::Timeout),
            }?;
            Ok(Response::Data(data))
        }
        ByteStreamRequest::ReadUpTo(count) => {
            log::debug!("read up to {} bytes", count);
            let mut data = vec![0; count as usize];
            let fut = AsyncReadExt::read(serial, &mut data);
            let num_read = match timeout(Duration::from_micros(100), fut).await {
                Ok(x) => x.map_err(Error::io)?,
                Err(_) => 0,
            };
            let data = data[..num_read].to_vec();
            Ok(Response::Data(data))
        }
        ByteStreamRequest::ReadAll => {
            log::debug!("read all bytes");
            let mut ret = Vec::new();
            let fut = AsyncReadExt::read_buf(serial, &mut ret);
            match timeout(Duration::from_micros(100), fut).await {
                Ok(x) => {
                    x.map_err(Error::io)?;
                }
                Err(_) => {}
            };
            Ok(Response::Data(ret))
        }
        ByteStreamRequest::CobsWrite(data) => {
            let data = cobs_pack(&data);
            AsyncWriteExt::write_all(serial, &data).await.map_err(Error::io)?;
            Ok(Response::Done)
        }
        ByteStreamRequest::CobsQuery { data, timeout_ms } => {
            cobs_query(serial, data, timeout_ms).await
        }
    }
}

async fn pop(serial: &mut Serial, timeout_ms: u32) -> crate::Result<u8> {
    let fut = AsyncReadExt::read_u8(serial);
    match timeout(Duration::from_millis(timeout_ms as u64), fut).await {
        Ok(x) => x.map_err(Error::io),
        Err(_) => Err(Error::Timeout),
    }
}

async fn cobs_query(serial: &mut Serial, data: Vec<u8>, timeout_ms: u32) -> crate::Result<Response> {
    let mut garbage = Vec::new();
    let fut = serial.read_buf(&mut garbage);
    match timeout(Duration::from_micros(100), fut).await {
        Ok(x) => {
            x.map_err(Error::io)?;
        }
        Err(_) => {}
    };
    let data = cobs_pack(&data);
    AsyncWriteExt::write_all(serial, &data).await.map_err(Error::io)?;
    let mut ret = Vec::new();
    let start = Instant::now();
    while ret.len() == 0 {
        let x = pop(serial, timeout_ms).await?;
        if (Instant::now() - start).as_millis() > timeout_ms as u128 {
            return Err(Error::Timeout);
        }
        if x == 0 {
            continue;
        }
        ret.push(x);
    }
    loop {
        let x = pop(serial, timeout_ms).await?;
        if (Instant::now() - start).as_millis() > timeout_ms as u128 {
            return Err(Error::Timeout);
        }
        ret.push(x);
        if x == 0 {
            break;
        }
    }
    // unwrap is save because we cancel above loop only in case we pushed x == 0
    let ret = cobs_unpack(&ret).unwrap();
    Ok(Response::Data(ret))
}
