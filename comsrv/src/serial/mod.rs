use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use tokio::task;
use tokio_serial::{SerialPortBuilderExt, SerialStream};

use anyhow::anyhow;
pub use params::SerialParams;

use crate::inventory;
use crate::iotask::{IoContext, IoHandler, IoTask};
use crate::prologix::{handle_prologix_request, init_prologix};
use crate::serial::params::{DataBits, Parity, StopBits};
use comsrv_protocol::{ByteStreamRequest, ByteStreamResponse, ScpiRequest, ScpiResponse, SerialAddress};

pub mod params;

#[cfg(target_os = "linux")]
mod linux_low_latency;

const DEFAULT_TIMEOUT_MS: u64 = 500;

// TODO: autodrop

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
            Request::Prologix { .. } => SerialParams {
                baud: 9600,
                data_bits: DataBits::Eight,
                stop_bits: StopBits::One,
                parity: Parity::None,
            },
            Request::Serial { params, .. } => params.clone(),
        }
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub enum Response {
    Bytes(ByteStreamResponse),
    Scpi(ScpiResponse),
}

pub struct Handler {
    serial: Option<(SerialStream, SerialParams)>,
    prologix_initialized: bool,
    path: String,
}

async fn open_serial_port(path: &str, params: &SerialParams) -> crate::Result<SerialStream> {
    let serial_stream = tokio_serial::new(path, params.baud)
        .parity(params.parity.into())
        .stop_bits(params.stop_bits.into())
        .data_bits(params.data_bits.into())
        .open_native_async()
        .map_err(|x| crate::Error::transport(anyhow!(x)))?;

    #[cfg(target_os = "linux")]
    {
        if let Err(x) = linux_low_latency::apply_low_latency(&serial_stream) {
            log::error!("Cannot set ASYNC_LOW_LATENCY on serial port: {}", x)
        }
        log::info!("Applied ASYNC_LOW_LATENCY to {}", path);
    }

    Ok(serial_stream)
}

#[async_trait]
impl IoHandler for Handler {
    type Request = Request;
    type Response = Response;

    async fn handle(&mut self, _ctx: &mut IoContext<Self>, req: Self::Request) -> crate::Result<Self::Response> {
        let new_params = req.params();
        let mut serial = match self.serial.take() {
            None => {
                log::debug!("Opening {}", self.path);
                self.prologix_initialized = false;
                open_serial_port(&self.path, &new_params).await?
            }
            Some((serial, old_params)) => {
                if old_params == new_params {
                    serial
                } else {
                    drop(serial);
                    self.prologix_initialized = false;
                    log::debug!("Reopening {}", self.path);
                    open_serial_port(&self.path, &new_params).await?
                }
            }
        };
        let ret = match req {
            Request::Prologix { gpib_addr, req } => {
                if !self.prologix_initialized {
                    init_prologix(&mut serial).await?;
                    self.prologix_initialized = true;
                }
                handle_prologix_request(&mut serial, gpib_addr, req).await.map(Response::Scpi)
            }
            Request::Serial { params: _, req } => {
                self.prologix_initialized = false;
                crate::bytestream::handle(&mut serial, req).await.map(Response::Bytes)
            }
        };
        match &ret {
            Err(crate::Error::Protocol(_)) | Ok(_) => {
                self.serial.replace((serial, new_params));
            }
            Err(_) => {}
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
        let handler = Handler {
            serial: None,
            path,
            prologix_initialized: false,
        };
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

impl inventory::Instrument for Instrument {
    type Address = SerialAddress;

    fn connect(_server: &crate::app::Server, addr: &Self::Address) -> crate::Result<Self> {
        Ok(Instrument::new(addr.port.clone()))
    }
}

pub async fn list_devices() -> crate::Result<Vec<String>> {
    task::spawn_blocking(move || match tokio_serial::available_ports() {
        Ok(x) => {
            let ports = x.iter().map(|x| x.port_name.clone()).collect();
            Ok(ports)
        }
        Err(err) => Err(crate::Error::transport(anyhow!(err.description)).into()),
    })
    .await
    .unwrap()
}
