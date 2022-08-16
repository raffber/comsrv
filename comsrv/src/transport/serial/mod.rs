use std::time::{Duration, Instant};

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use tokio::task::{self, JoinHandle};
use tokio::time::sleep;
use tokio_serial::{SerialPortBuilderExt, SerialStream};

use anyhow::anyhow;
pub use params::SerialParams;

use crate::inventory;
use crate::iotask::{IoContext, IoHandler, IoTask};
use crate::protocol::bytestream;
use crate::protocol::prologix::{handle_prologix_request, init_prologix};
use crate::transport::serial::params::{DataBits, Parity, StopBits};
use comsrv_protocol::{ByteStreamRequest, ByteStreamResponse, ScpiRequest, ScpiResponse, SerialAddress};

pub mod params;

#[cfg(target_os = "linux")]
mod linux_low_latency;

const DEFAULT_DROP_DELAY: Duration = Duration::from_secs(60);

pub enum Request {
    Prologix {
        gpib_addr: u8,
        req: ScpiRequest,
    },
    Serial {
        params: SerialParams,
        req: ByteStreamRequest,
    },
    DropCheck,
}

impl Request {
    pub fn params(&self) -> Option<SerialParams> {
        match self {
            Request::Prologix { .. } => Some(SerialParams {
                baud: 9600,
                data_bits: DataBits::Eight,
                stop_bits: StopBits::One,
                parity: Parity::None,
            }),
            Request::Serial { params, .. } => Some(params.clone()),
            _ => None,
        }
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub enum Response {
    Bytes(ByteStreamResponse),
    Scpi(ScpiResponse),
    Done,
}

pub struct Handler {
    serial: Option<(SerialStream, SerialParams)>,
    prologix_initialized: bool,
    path: String,
    drop_delay: Duration,
    last_request: Instant,
    drop_delay_task: Option<JoinHandle<()>>,
}

impl Handler {
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
            log::info!("Applied ASYNC_LOW_LATENCY to {}", &path);
        }
        Ok(serial_stream)
    }

    fn drop_check(&mut self, req: &Request) -> Option<crate::Result<Response>> {
        if matches!(
            req,
            Request::Serial {
                req: ByteStreamRequest::Disconnect,
                ..
            }
        ) {
            self.serial.take();
            return Some(Ok(Response::Bytes(ByteStreamResponse::Done)));
        }
        if matches!(req, Request::DropCheck) {
            let now = Instant::now();
            if now - self.last_request > self.drop_delay {
                self.serial.take();
            }
            return Some(Ok(Response::Done));
        }
        if let Some(x) = self.drop_delay_task.take() {
            x.abort();
        }

        None
    }

    async fn open_serial(&mut self, new_params: &SerialParams) -> crate::Result<SerialStream> {
        let ret = match self.serial.take() {
            None => {
                log::debug!("Opening {}", self.path);
                self.prologix_initialized = false;
                Self::open_serial_port(&self.path, &new_params).await?
            }
            Some((serial, old_params)) => {
                if old_params == *new_params {
                    serial
                } else {
                    drop(serial);
                    self.prologix_initialized = false;
                    log::debug!("Reopening {}", self.path);
                    Self::open_serial_port(&self.path, &new_params).await?
                }
            }
        };
        Ok(ret)
    }

    async fn handle_request(&mut self, req: Request, serial: &mut SerialStream) -> crate::Result<Response> {
        match req {
            Request::Prologix { gpib_addr, req } => {
                if !self.prologix_initialized {
                    init_prologix(serial).await?;
                    self.prologix_initialized = true;
                }
                handle_prologix_request(serial, gpib_addr, req).await.map(Response::Scpi)
            }
            Request::Serial { params: _, req } => {
                self.prologix_initialized = false;
                bytestream::handle(serial, req).await.map(Response::Bytes)
            }
            Request::DropCheck => unreachable!(),
        }
    }
}

#[async_trait]
impl IoHandler for Handler {
    type Request = Request;
    type Response = Response;

    async fn handle(&mut self, ctx: &mut IoContext<Self>, req: Self::Request) -> crate::Result<Self::Response> {
        if let Some(reply) = self.drop_check(&req) {
            return reply;
        }
        // unwrap is ok because we handled DropCheck just above
        let new_params = req.params().unwrap();
        let mut serial = self.open_serial(&new_params).await?;

        let ret = self.handle_request(req, &mut serial).await;
        match &ret {
            Err(crate::Error::Protocol(_)) | Ok(_) => {
                self.serial.replace((serial, new_params));

                let mut ctx = ctx.clone();
                let drop_delay = self.drop_delay.clone();
                self.drop_delay_task = Some(task::spawn(async move {
                    sleep(drop_delay + Duration::from_millis(100)).await;
                    let _ = ctx.send(Request::DropCheck);
                }));
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
            drop_delay: DEFAULT_DROP_DELAY,
            last_request: Instant::now(),
            drop_delay_task: None,
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

#[async_trait]
impl inventory::Instrument for Instrument {
    type Address = SerialAddress;

    fn connect(_server: &crate::app::Server, addr: &Self::Address) -> crate::Result<Self> {
        Ok(Instrument::new(addr.port.clone()))
    }

    async fn wait_for_closed(&self) {
        self.inner.wait_for_closed().await
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
