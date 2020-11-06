use std::time::Instant;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::time::{Duration, timeout};
use tokio_serial::Serial;

pub use params::SerialParams;

use crate::{Error, ScpiRequest, ScpiResponse};
use crate::app::WireSerialRequest;
use crate::cobs::{cobs_pack, cobs_unpack};
use crate::iotask::{IoHandler, IoTask};
use crate::serial::params::{DataBits, Parity, StopBits};

pub mod params;

const DEFAULT_TIMEOUT_MS: u64 = 500;
const PROLOGIX_TIMEOUT: f32 = 1.0;

pub enum Request {
    Prologix {
        gpib_addr: u8,
        req: ScpiRequest,
    },
    Serial {
        params: SerialParams,
        req: WireSerialRequest,
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
        let mut serial = match self.serial.take() {
            None => {
                log::debug!("Opening {}", self.path);
                let settings = new_params.clone().into();
                Serial::from_path(&self.path, &settings).map_err(Error::io)?
            }
            Some((serial, old_params)) => {
                if old_params == new_params {
                    log::debug!("reusing already open handle to {}", self.path);
                    serial
                } else {
                    drop(serial);
                    log::debug!("Reopening {}", self.path);
                    let settings = new_params.clone().into();
                    Serial::from_path(&self.path, &settings).map_err(Error::io)?
                }
            }
        };
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

async fn handle_serial_request(serial: &mut Serial, req: WireSerialRequest) -> crate::Result<Response> {
    match req {
        WireSerialRequest::Write(data) => {
            log::debug!("write: {:?}", data);
            AsyncWriteExt::write_all(serial, &data).await.map_err(Error::io)?;
            Ok(Response::Done)
        }
        WireSerialRequest::ReadExact { count, timeout_ms } => {
            log::debug!("read exactly {} bytes", count);
            let mut data = vec![0; count as usize];
            let fut = AsyncReadExt::read_exact(serial, data.as_mut_slice());
            let _ = match timeout(Duration::from_millis(timeout_ms as u64), fut).await {
                Ok(x) => x.map_err(Error::io),
                Err(_) => Err(Error::Timeout),
            }?;
            Ok(Response::Data(data))
        }
        WireSerialRequest::ReadUpTo(count) => {
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
        WireSerialRequest::ReadAll => {
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
        WireSerialRequest::CobsWrite(data) => {
            let data = cobs_pack(&data);
            AsyncWriteExt::write_all(serial, &data).await.map_err(Error::io)?;
            Ok(Response::Done)
        }
        WireSerialRequest::CobsQuery { data, timeout_ms } => {
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

async fn write_prologix(serial: &mut Serial, mut msg: String) -> crate::Result<()> {
    if !msg.ends_with("\n") {
        msg.push_str("\n");
    }
    serial.write(msg.as_bytes()).await.map(|_| ()).map_err(Error::io)
}

async fn read_prologix(serial: &mut Serial) -> crate::Result<String> {
    let start = Instant::now();
    let mut ret = Vec::new();
    loop {
        let mut x = [0; 1];
        match timeout(Duration::from_secs_f32(PROLOGIX_TIMEOUT), serial.read_exact(&mut x)).await {
            Ok(Ok(_)) => {
                let x = x[0];
                if x == b'\n' {
                    break;
                }
                ret.push(x);
            }
            Ok(Err(x)) => {
                log::debug!("read error");
                return Err(Error::io(x));
            }
            Err(_) => {
                log::debug!("instrument read timeout");
                return Err(Error::Timeout);
            }
        };
        let delta = start.elapsed().as_secs_f32();
        if delta > PROLOGIX_TIMEOUT {
            return Err(Error::Timeout);
        }
    }
    String::from_utf8(ret).map_err(Error::DecodeError)
}

async fn handle_prologix_request(serial: &mut Serial, addr: u8, req: ScpiRequest) -> crate::Result<ScpiResponse> {
    log::debug!("handling prologix request for address {}", addr);
    let mut ret = Vec::with_capacity(128);
    let fut = AsyncReadExt::read(serial, &mut ret);
    match timeout(Duration::from_micros(100), fut).await {
        Ok(x) => {
            x.map_err(Error::io)?;
        }
        Err(_) => {}
    };
    log::debug!("Read: {:?}", ret);
    ret.clear();
    let addr_set = format!("++addr {}\n", addr);
    serial.write(addr_set.as_bytes()).await.map_err(Error::io)?;
    match req {
        ScpiRequest::Write(x) => {
            write_prologix(serial, x).await?;
            Ok(ScpiResponse::Done)
        }
        ScpiRequest::QueryString(x) => {
            write_prologix(serial, x).await?;
            serial.write("++read eoi\n".as_bytes()).await.map_err(Error::io)?;
            let reply = read_prologix(serial).await?;
            Ok(ScpiResponse::String(reply))
        }
        ScpiRequest::QueryBinary(_) => {
            log::error!("ScpiRequest::QueryBinary not implemented for Prologix!!");
            Err(Error::NotSupported)
        }
    }
}
