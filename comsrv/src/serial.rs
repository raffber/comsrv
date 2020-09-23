use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::time::{Duration, timeout};
use tokio_serial::{FlowControl, Serial, SerialPortSettings};

use crate::{Error, ScpiRequest, ScpiResponse};
use crate::iotask::{IoHandler, IoTask};
use std::time::Instant;
use std::fmt::Display;
use serde::export::Formatter;
use std::fmt;

const DEFAULT_TIMEOUT_MS: u64 = 500;
const PROLOGIX_TIMEOUT: f32 = 1.0;

#[derive(PartialEq, Clone, Copy, Serialize, Deserialize, Hash)]
pub enum StopBits {
    One,
    Two,
}

#[derive(PartialEq, Clone, Copy, Serialize, Deserialize, Hash)]
pub enum Parity {
    None,
    Odd,
    Even,
}

#[derive(PartialEq, Clone, Copy, Serialize, Deserialize, Hash)]
pub enum DataBits {
    Five,
    Six,
    Seven,
    Eight,
}

pub fn parse_serial_settings(settings: &str) -> Option<(DataBits, Parity, StopBits)> {
    let settings = settings.to_lowercase();
    let chars = settings.as_bytes();
    if chars.len() != 3 {
        return None;
    }
    let data_bits = match chars[0] as char {
        '8' => DataBits::Eight,
        '7' => DataBits::Seven,
        '6' => DataBits::Six,
        '5' => DataBits::Five,
        _ => return None
    };
    let parity = match chars[1] as char {
        'n' => Parity::None,
        'o' => Parity::Odd,
        'e' => Parity::Even,
        _ => return None
    };
    let stop_bits = match chars[2] as char {
        '1' => StopBits::One,
        '2' => StopBits::Two,
        _ => return None
    };
    Some((data_bits, parity, stop_bits))
}

#[derive(PartialEq, Clone, Serialize, Deserialize, Hash)]
pub struct SerialParams {
    pub baud: u32,
    pub data_bits: DataBits,
    pub stop_bits: StopBits,
    pub parity: Parity,
}

impl SerialParams {
    pub fn from_string(addr: &str) -> Option<(String, SerialParams)> {
        let splits: Vec<_> = addr.split("::")
            .map(|x| x.to_string())
            .collect();
        if splits.len() != 4 {
            return None;
        }
        let path = splits[1].clone();
        let baud_rate: u32 = splits[2].parse().ok()?;
        let (bits, parity, stop) = parse_serial_settings(&splits[3])?;
        Some((path, SerialParams {
            baud: baud_rate,
            data_bits: bits,
            stop_bits: stop,
            parity,
        }))
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub enum SerialRequest {
    Prologix {
        gpib_addr: u8,
        req: ScpiRequest,
    },
    Write {
        params: SerialParams,
        data: Vec<u8>,
    },
    ReadExact {
        params: SerialParams,
        count: u32,
        timeout_ms: u32,
    },
    ReadUpTo {
        params: SerialParams,
        count: u32,
    },
    ReadAll {
        params: SerialParams,
    },
}

impl SerialRequest {
    pub fn params(&self) -> SerialParams {
        match self {
            SerialRequest::Prologix { .. } => {
                SerialParams {
                    baud: 9600,
                    data_bits: DataBits::Eight,
                    stop_bits: StopBits::One,
                    parity: Parity::None,
                }
            }
            SerialRequest::Write { params, data: _ } => params.clone(),
            SerialRequest::ReadExact { params, count: _, timeout_ms: _ } => params.clone(),
            SerialRequest::ReadUpTo { params, count: _ } => params.clone(),
            SerialRequest::ReadAll { params } => params.clone(),
        }
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub enum SerialResponse {
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
    type Request = SerialRequest;
    type Response = SerialResponse;

    async fn handle(&mut self, req: Self::Request) -> crate::Result<Self::Response> {
        let new_params = req.params();
        let mut serial = match self.serial.take() {
            None => {
                let settings = new_params.into();
                Serial::from_path(&self.path, &settings).map_err(Error::io)?
            }
            Some((serial, old_params)) => {
                if old_params == new_params {
                    serial
                } else {
                    drop(serial);
                    let settings = new_params.into();
                    Serial::from_path(&self.path, &settings).map_err(Error::io)?
                }
            }
        };
        handle_request(&mut serial, req).await
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

    pub async fn request(&mut self, req: SerialRequest) -> crate::Result<SerialResponse> {
        self.inner.request(req).await
    }

    pub fn disconnect(mut self) {
        self.inner.disconnect()
    }
}

async fn handle_request(serial: &mut Serial, req: SerialRequest) -> crate::Result<SerialResponse> {
    match req {
        SerialRequest::Write { params: _, data } => {
            AsyncWriteExt::write_all(serial, &data).await.map_err(Error::io)?;
            Ok(SerialResponse::Done)
        }
        SerialRequest::ReadExact { params: _, count, timeout_ms } => {
            let mut data = vec![0; count as usize];
            let fut = AsyncReadExt::read_exact(serial, data.as_mut_slice());
            let _ = match timeout(Duration::from_millis(timeout_ms as u64), fut).await {
                Ok(x) => x.map_err(Error::io),
                Err(_) => Err(Error::Timeout),
            }?;
            Ok(SerialResponse::Data(data))
        }
        SerialRequest::ReadUpTo { params: _, count } => {
            let mut data = vec![0; count as usize];
            let fut = AsyncReadExt::read(serial, &mut data);
            let num_read = match timeout(Duration::from_micros(100), fut).await {
                Ok(x) => x.map_err(Error::io)?,
                Err(_) => 0,
            };
            let data = data[..num_read].to_vec();
            Ok(SerialResponse::Data(data))
        }
        SerialRequest::ReadAll { params: _ } => {
            let mut ret = Vec::new();
            loop {
                let mut data = [0u8; 128];
                let fut = AsyncReadExt::read(serial, &mut data);
                let num_read = match timeout(Duration::from_micros(100), fut).await {
                    Ok(x) => x.map_err(Error::io)?,
                    Err(_) => break,
                };
                if num_read == 0 {
                    break;
                }
                ret.extend(&data[..num_read]);
            }
            Ok(SerialResponse::Data(ret))
        }
        SerialRequest::Prologix { gpib_addr: addr, req } => {
            let answer = handle_prologix_request(serial, addr, req).await?;
            Ok(SerialResponse::Scpi(answer))
        }
    }
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
                return Err(Error::io(x));
            }
            Err(_) => {
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
    let mut ret = Vec::with_capacity(128);
    serial.read_to_end(&mut ret).await.map_err(Error::io)?;
    ret.clear();
    let addr_set = format!("++addr {}", addr);
    let addr_set = addr_set.as_bytes();
    serial.write(addr_set).await.map_err(Error::io)?;
    match req {
        ScpiRequest::Write(x) => {
            write_prologix(serial, x).await?;
            Ok(ScpiResponse::Done)
        }
        ScpiRequest::QueryString(x) => {
            write_prologix(serial, x).await?;
            serial.write("++read eoi".as_bytes()).await.map_err(Error::io)?;
            let reply = read_prologix(serial).await?;
            Ok(ScpiResponse::String(reply))
        }
        ScpiRequest::QueryBinary(_) => {
            log::error!("ScpiRequest::QueryBinary not implemented for Prologix!!");
            Err(Error::NotSupported)
        }
    }
}

impl Into<SerialPortSettings> for SerialParams {
    fn into(self) -> SerialPortSettings {
        SerialPortSettings {
            baud_rate: self.baud,
            data_bits: tokio_serial::DataBits::Eight,
            flow_control: FlowControl::None,
            parity: self.parity.into(),
            stop_bits: self.stop_bits.into(),
            timeout: Duration::from_millis(DEFAULT_TIMEOUT_MS),
        }
    }
}

impl From<tokio_serial::StopBits> for StopBits {
    fn from(x: tokio_serial::StopBits) -> Self {
        match x {
            tokio_serial::StopBits::One => StopBits::One,
            tokio_serial::StopBits::Two => StopBits::Two,
        }
    }
}

impl Into<tokio_serial::StopBits> for StopBits {
    fn into(self) -> tokio_serial::StopBits {
        match self {
            StopBits::One => tokio_serial::StopBits::One,
            StopBits::Two => tokio_serial::StopBits::Two,
        }
    }
}


impl From<tokio_serial::Parity> for Parity {
    fn from(x: tokio_serial::Parity) -> Self {
        match x {
            tokio_serial::Parity::None => Parity::None,
            tokio_serial::Parity::Odd => Parity::Odd,
            tokio_serial::Parity::Even => Parity::Even,
        }
    }
}

impl Into<tokio_serial::Parity> for Parity {
    fn into(self) -> tokio_serial::Parity {
        match self {
            Parity::None => tokio_serial::Parity::None,
            Parity::Odd => tokio_serial::Parity::Odd,
            Parity::Even => tokio_serial::Parity::Even,
        }
    }
}

impl From<tokio_serial::DataBits> for DataBits {
    fn from(x: tokio_serial::DataBits) -> Self {
        match x {
            tokio_serial::DataBits::Five => DataBits::Five,
            tokio_serial::DataBits::Six => DataBits::Six,
            tokio_serial::DataBits::Seven => DataBits::Seven,
            tokio_serial::DataBits::Eight => DataBits::Eight,
        }
    }
}

impl Into<tokio_serial::DataBits> for DataBits {
    fn into(self) -> tokio_serial::DataBits {
        match self {
            DataBits::Five => tokio_serial::DataBits::Five,
            DataBits::Six => tokio_serial::DataBits::Six,
            DataBits::Seven => tokio_serial::DataBits::Seven,
            DataBits::Eight => tokio_serial::DataBits::Eight,
        }
    }
}

impl Display for DataBits {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let x = match self {
            DataBits::Five => "5",
            DataBits::Six => "6",
            DataBits::Seven => "7",
            DataBits::Eight => "8",
        };
        f.write_str(x)
    }
}

impl Display for Parity {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let x = match self {
            Parity::None => "N",
            Parity::Odd => "O",
            Parity::Even => "E",
        };
        f.write_str(x)
    }
}

impl Display for StopBits {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let x = match self {
            StopBits::One => "1",
            StopBits::Two => "2",
        };
        f.write_str(x)
    }
}
