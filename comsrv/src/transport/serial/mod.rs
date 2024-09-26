use std::time::{Duration, Instant};

use crate::app::Server;
use crate::protocol::cobs_stream::CobsStream;
use crate::rpc::FlowControl;
use async_trait::async_trait;
use comsrv_protocol::cobs_stream::{CobsStreamRequest, CobsStreamResponse};
use serde::{Deserialize, Serialize};
use tokio::io;
use tokio::task::{self, JoinHandle};
use tokio::time::sleep;
use tokio_serial::{SerialPort, SerialPortBuilderExt, SerialStream};

use anyhow::anyhow;
pub use params::SerialParams;

use crate::inventory;
use crate::iotask::{IoContext, IoHandler, IoTask};
use crate::protocol::bytestream;
use crate::protocol::prologix::{handle_prologix_request, init_prologix};
use crate::transport::serial::params::{DataBits, Parity, StopBits};
use comsrv_protocol::{
    ByteStreamInstrument, ByteStreamRequest, ByteStreamResponse, ScpiRequest, ScpiResponse, SerialAddress,
    SerialInstrument, SerialRequest, SerialResponse,
};

pub mod params;

#[cfg(target_os = "linux")]
mod linux_low_latency;

const DEFAULT_DROP_DELAY: Duration = Duration::from_secs(60);

pub enum Request {
    Prologix {
        gpib_addr: u8,
        req: ScpiRequest,
        timeout: Option<Duration>,
    },
    Bytes {
        params: SerialParams,
        req: ByteStreamRequest,
    },
    Serial {
        params: SerialParams,
        req: SerialRequest,
    },
    Cobs {
        params: SerialParams,
        req: CobsStreamRequest,
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
                hardware_flow_control: Default::default(),
            }),
            Request::Bytes { params, .. } => Some(params.clone()),
            Request::Serial { params, .. } => Some(params.clone()),
            Request::Cobs { params, .. } => Some(params.clone()),
            Request::DropCheck => None,
        }
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub enum Response {
    Bytes(ByteStreamResponse),
    Scpi(ScpiResponse),
    Serial(SerialResponse),
    Cobs(CobsStreamResponse),
    Done,
}

pub struct Handler {
    serial: Option<(SerialStream, SerialParams)>,
    cobs_stream: Option<(CobsStream, SerialParams)>,
    prologix_initialized: bool,
    path: String,
    drop_delay: Duration,
    last_request: Instant,
    drop_delay_task: Option<JoinHandle<()>>,
    server: Server,
    cobs_stream_use_crc: bool,
}

impl Handler {
    async fn open_serial_port(path: &str, params: &SerialParams) -> crate::Result<SerialStream> {
        let flow_control = match params.hardware_flow_control {
            FlowControl::NoFlowControl => tokio_serial::FlowControl::None,
            FlowControl::Hardware => tokio_serial::FlowControl::Hardware,
            FlowControl::Software => tokio_serial::FlowControl::Software,
        };

        let serial_stream = tokio_serial::new(path, params.baud)
            .parity(params.parity.into())
            .stop_bits(params.stop_bits.into())
            .data_bits(params.data_bits.into())
            .flow_control(flow_control)
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
        match req {
            Request::Cobs {
                req: CobsStreamRequest::Stop,
                ..
            } => {
                self.cobs_stream.take();
                return Some(Ok(Response::Cobs(CobsStreamResponse::Done)));
            }
            Request::Bytes {
                req: ByteStreamRequest::Disconnect,
                ..
            } => {
                self.serial.take();
                return Some(Ok(Response::Bytes(ByteStreamResponse::Done)));
            }
            Request::DropCheck => {
                let now = Instant::now();
                if now - self.last_request > self.drop_delay {
                    self.serial.take();
                }
                return Some(Ok(Response::Done));
            }
            _ => {}
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
                Self::open_serial_port(&self.path, new_params).await?
            }
            Some((serial, old_params)) => {
                if old_params == *new_params {
                    serial
                } else {
                    drop(serial);
                    self.prologix_initialized = false;
                    log::debug!("Reopening {}", self.path);
                    Self::open_serial_port(&self.path, new_params).await?
                }
            }
        };
        Ok(ret)
    }

    async fn handle_request(&mut self, req: Request, serial: &mut SerialStream) -> crate::Result<Response> {
        match req {
            Request::Prologix {
                gpib_addr,
                req,
                timeout,
            } => {
                if !self.prologix_initialized {
                    init_prologix(serial).await?;
                    self.prologix_initialized = true;
                }
                handle_prologix_request(serial, gpib_addr, req, timeout)
                    .await
                    .map(Response::Scpi)
            }
            Request::Bytes { params: _, req } => {
                self.prologix_initialized = false;
                bytestream::handle(serial, req).await.map(Response::Bytes)
            }
            Request::Serial { params: _, req } => match req {
                SerialRequest::WriteDataTerminalReady(x) => {
                    serial.write_data_terminal_ready(x).map_err(map_tokio_serial_error)?;
                    Ok(Response::Serial(SerialResponse::Done))
                }
                SerialRequest::WriteRequestToSend(x) => {
                    serial.write_request_to_send(x).map_err(map_tokio_serial_error)?;
                    Ok(Response::Serial(SerialResponse::Done))
                }
                SerialRequest::ReadDataSetReady => Ok(Response::Serial(SerialResponse::PinLevel(
                    serial.read_data_set_ready().map_err(map_tokio_serial_error)?,
                ))),
                SerialRequest::ReadRingIndicator => Ok(Response::Serial(SerialResponse::PinLevel(
                    serial.read_ring_indicator().map_err(map_tokio_serial_error)?,
                ))),
                SerialRequest::ReadCarrierDetect => Ok(Response::Serial(SerialResponse::PinLevel(
                    serial.read_carrier_detect().map_err(map_tokio_serial_error)?,
                ))),
                SerialRequest::ReadClearToSend => Ok(Response::Serial(SerialResponse::PinLevel(
                    serial.read_clear_to_send().map_err(map_tokio_serial_error)?,
                ))),
            },
            Request::DropCheck => unreachable!(),
            Request::Cobs { .. } => unreachable!(),
        }
    }

    async fn handle_cobs_request(&mut self, params: SerialParams, req: CobsStreamRequest) -> crate::Result<Response> {
        drop(self.serial.take());

        if let CobsStreamRequest::Start { use_crc } = req {
            self.cobs_stream_use_crc = use_crc;
        }

        let cobs_stream = match self.cobs_stream.take() {
            Some((cobs_stream, old_params))
                if old_params == params
                    && cobs_stream.is_alive()
                    && cobs_stream.use_crc() == self.cobs_stream_use_crc =>
            {
                cobs_stream
            }
            _ => {
                let serial = self.open_serial(&params).await?;
                let (read, write) = io::split(serial);
                CobsStream::start(
                    read,
                    write,
                    self.server.clone(),
                    self.get_instrument(&params),
                    self.cobs_stream_use_crc,
                )
            }
        };

        if let CobsStreamRequest::SendFrame { data } = req {
            // NOTE: this could cause a race condition is the stream drop between the .is_alive() call above
            // but that seems unlikely and not a big issue if it happens (an error is returned, just not an accurate one)
            cobs_stream.send(data)?;
        }

        self.cobs_stream.replace((cobs_stream, params));

        Ok(Response::Cobs(CobsStreamResponse::Done))
    }

    fn get_instrument(&self, params: &SerialParams) -> ByteStreamInstrument {
        ByteStreamInstrument::Serial(SerialInstrument {
            address: SerialAddress {
                port: self.path.clone(),
            },
            port_config: params.clone().into(),
            options: None,
        })
    }
}

fn map_tokio_serial_error(err: tokio_serial::Error) -> crate::Error {
    crate::Error::transport(anyhow!(err))
}

#[async_trait]
impl IoHandler for Handler {
    type Request = Request;
    type Response = Response;

    async fn handle(&mut self, ctx: &mut IoContext<Self>, req: Self::Request) -> crate::Result<Self::Response> {
        if let Some(reply) = self.drop_check(&req) {
            return reply;
        }
        if let Request::Cobs { params, req } = req {
            return self.handle_cobs_request(params, req).await;
        }
        drop(self.cobs_stream.take());
        // unwrap is ok because we handled DropCheck just above
        let new_params = req.params().unwrap();
        let mut serial = self.open_serial(&new_params).await?;

        let ret = self.handle_request(req, &mut serial).await;
        match &ret {
            Err(crate::Error::Protocol(_)) | Ok(_) => {
                self.serial.replace((serial, new_params));

                let mut ctx = ctx.clone();
                let drop_delay = self.drop_delay;
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
    pub fn new(path: String, server: Server) -> Self {
        let handler = Handler {
            serial: None,
            path,
            prologix_initialized: false,
            drop_delay: DEFAULT_DROP_DELAY,
            last_request: Instant::now(),
            drop_delay_task: None,
            cobs_stream: None,
            server,
            cobs_stream_use_crc: true,
        };
        Self {
            inner: IoTask::new(handler),
        }
    }

    pub async fn request(&mut self, req: Request) -> crate::Result<Response> {
        self.inner.request(req).await
    }
}

#[async_trait]
impl inventory::Instrument for Instrument {
    type Address = SerialAddress;

    fn connect(server: &crate::app::Server, addr: &Self::Address) -> crate::Result<Self> {
        Ok(Instrument::new(addr.port.clone(), server.clone()))
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
        Err(err) => Err(crate::Error::transport(anyhow!(err.description))),
    })
    .await
    .unwrap()
}
