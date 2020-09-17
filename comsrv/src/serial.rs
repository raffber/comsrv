use tokio_serial::{SerialPortSettings, FlowControl, Serial};
use tokio::io::{AsyncWriteExt, AsyncReadExt};
use serde::{Serialize, Deserialize};
use tokio::sync::{mpsc, oneshot};
use tokio::task;
use crate::Error;
use tokio::time::{Duration, timeout};

const DEFAULT_TIMEOUT_MS: u64 = 500;

#[derive(PartialEq, Clone, Copy, Serialize, Deserialize)]
pub enum StopBits {
    One,
    Two,
}

#[derive(PartialEq, Clone, Copy, Serialize, Deserialize)]
pub enum Parity {
    None,
    Odd,
    Even,
}

#[derive(PartialEq, Clone, Copy, Serialize, Deserialize)]
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

#[derive(PartialEq, Clone, Serialize, Deserialize)]
pub struct SerialParams {
    pub path: String,
    pub baud: u32,
    pub data_bits: DataBits,
    pub stop_bits: StopBits,
    pub parity: Parity,
}

impl SerialParams {
    pub fn from_string(addr: &str) -> Option<SerialParams> {
        let splits: Vec<_> = addr.split("::")
            .map(|x| x.to_string())
            .collect();
        if splits.len() != 4 {
            return None;
        }
        let path = splits[1].clone();
        let baud_rate: u32 = splits[2].parse().ok()?;
        let (bits, parity, stop) = parse_serial_settings(&splits[3])?;
        Some(SerialParams {
            path,
            baud: baud_rate,
            data_bits: bits,
            stop_bits: stop,
            parity,
        })
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub enum SerialRequest {
    Write(Vec<u8>),
    ReadExact {
        count: u32,
        timeout_ms: u32,
    },
    ReadUpTo(u32),
    ReadAll,
}

#[derive(Clone, Serialize, Deserialize)]
pub enum SerialResponse {
    Done,
    Data(Vec<u8>),
}

enum Msg {
    Request {
        request: SerialRequest,
        reply: oneshot::Sender<crate::Result<SerialResponse>>,
    },
    Drop,
}

#[derive(Clone)]
pub struct Instrument {
    params: SerialParams,
    tx: mpsc::UnboundedSender<Msg>,
}

impl Instrument {
    pub fn connect(params: SerialParams) -> Self {
        let path = params.path.clone();
        let params2 = params.clone();
        let settings: SerialPortSettings = params.into();
        let (tx, rx) = mpsc::unbounded_channel();
        task::spawn(run_serial(path, settings, rx));
        Self {
            params: params2,
            tx,
        }
    }

    pub fn path(&self) -> &str {
        &self.params.path
    }

    pub fn params(&self) -> &SerialParams {
        &self.params
    }

    pub async fn handle(&self, req: SerialRequest) -> crate::Result<SerialResponse> {
        let (tx, rx) = oneshot::channel();
        let msg = Msg::Request {
            request: req,
            reply: tx,
        };
        self.tx.send(msg).map_err(|_| Error::Disconnected)?;
        rx.await.map_err(|_| Error::Disconnected)?
    }

    pub fn disconnect(self) {
        let _ = self.tx.send(Msg::Drop);
    }
}

async fn run_serial(path: String, settings: SerialPortSettings, mut rx: mpsc::UnboundedReceiver<Msg>) -> crate::Result<()> {
    log::debug!("Connecting to serial port: {}", path);
    let mut serial = Serial::from_path(&path, &settings).map_err(Error::io)?;
    log::debug!("Successfully opened: {}", path);
    while let Some(msg) = rx.recv().await {
        match msg {
            Msg::Request { request, reply } => {
                let result = handle_request(&mut serial, request).await;
                let _ = reply.send(result);
            }
            Msg::Drop => break,
        }
    }
    Ok(())
}

async fn handle_request(serial: &mut Serial, req: SerialRequest) -> crate::Result<SerialResponse> {
    match req {
        SerialRequest::Write(data) => {
            AsyncWriteExt::write_all(serial, &data).await.map_err(Error::io)?;
            Ok(SerialResponse::Done)
        }
        SerialRequest::ReadExact { count, timeout_ms } => {
            let mut data = vec![0; count as usize];
            let fut = AsyncReadExt::read_exact(serial, data.as_mut_slice());
            let _ = match timeout(Duration::from_millis(timeout_ms as u64), fut).await {
                Ok(x) => x.map_err(Error::io),
                Err(_) => Err(Error::Timeout),
            }?;
            Ok(SerialResponse::Data(data))
        }
        SerialRequest::ReadUpTo(count) => {
            let mut data = vec![0; count as usize];
            let fut = AsyncReadExt::read(serial, &mut data);
            let num_read = match timeout(Duration::from_micros(100), fut).await {
                Ok(x) => x.map_err(Error::io)?,
                Err(_) => 0,
            };
            let data = data[..num_read].to_vec();
            Ok(SerialResponse::Data(data))
        }
        SerialRequest::ReadAll => {
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
