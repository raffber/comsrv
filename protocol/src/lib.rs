use cobs_stream::{CobsStreamRequest, CobsStreamResponse};
use serde::{Deserialize, Serialize};
use std::fmt::Debug;
use uuid::Uuid;

pub use bytestream::*;
pub use can::*;

#[allow(unused_imports)]
pub use error::*;

pub use hid::*;
pub use scpi::*;
pub use sigrok::*;

pub mod bytestream;
pub mod can;
pub mod cobs_stream;
pub mod error;
pub mod hid;
pub mod scpi;
pub mod sigrok;
mod util;

pub use crate::error::{Error, ProtocolError, TransportError};

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct Duration {
    pub micros: u32,
    pub seconds: u32,
}

impl From<Duration> for std::time::Duration {
    fn from(val: Duration) -> Self {
        std::time::Duration::from_micros((val.seconds as u64 * 1000000_u64) + (val.micros as u64))
    }
}

impl From<std::time::Duration> for Duration {
    fn from(x: std::time::Duration) -> Self {
        Self {
            micros: x.subsec_micros(),
            seconds: x.as_secs() as u32,
        }
    }
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub enum Address {
    Tcp(TcpAddress),
    Ftdi(FtdiAddress),
    Hid(HidIdentifier),
    Serial(SerialAddress),
    Vxi(String),
    Visa(String),
    Can(CanAddress),
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub enum Request {
    Bytes {
        instrument: ByteStreamInstrument,
        request: ByteStreamRequest,
        #[serde(skip_serializing_if = "Option::is_none", default)]
        lock: Option<Uuid>,
    },
    CobsStream {
        instrument: ByteStreamInstrument,
        request: CobsStreamRequest,
        #[serde(skip_serializing_if = "Option::is_none", default)]
        lock: Option<Uuid>,
    },
    Can {
        instrument: CanInstrument,
        request: CanRequest,
        #[serde(skip_serializing_if = "Option::is_none", default)]
        lock: Option<Uuid>,
    },
    Scpi {
        instrument: ScpiInstrument,
        request: ScpiRequest,
        #[serde(skip_serializing_if = "Option::is_none", default)]
        lock: Option<Uuid>,
        #[serde(default = "Option::default")]
        timeout: Option<Duration>,
    },
    Prologix {
        instrument: PrologixInstrument,
        request: PrologixRequest,
        #[serde(skip_serializing_if = "Option::is_none", default)]
        lock: Option<Uuid>,
        #[serde(default = "Option::default")]
        timeout: Option<Duration>,
    },
    Sigrok {
        instrument: SigrokInstrument,
        request: SigrokRequest,
    },
    Hid {
        instrument: HidInstrument,
        request: HidRequest,
        #[serde(skip_serializing_if = "Option::is_none", default)]
        lock: Option<Uuid>,
    },
    Serial {
        instrument: SerialInstrument,
        request: SerialRequest,
        #[serde(skip_serializing_if = "Option::is_none", default)]
        lock: Option<Uuid>,
    },
    ListSigrokDevices,
    ListSerialPorts,
    ListHidDevices,
    ListFtdiDevices,
    ListCanDevices,
    ListConnectedInstruments,
    Lock {
        addr: Address,
        timeout: Duration,
    },
    Unlock {
        addr: Address,
        id: Uuid,
    },
    Drop {
        addr: Address,
        id: Option<Uuid>,
    },
    DropAll,
    Version,
    Shutdown,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub enum Response {
    Error(Error),
    Instruments(Vec<Address>),
    Scpi(ScpiResponse),
    Bytes(ByteStreamResponse),
    CobsStream(CobsStreamResponse),
    Can {
        source: CanAddress,
        response: CanResponse,
    },
    Sigrok(SigrokResponse),
    Locked {
        lock_id: Uuid,
    },
    Hid(HidResponse),
    Version {
        major: u32,
        minor: u32,
        build: u32,
    },
    Serial(SerialResponse),
    SerialPorts(Vec<String>),
    FtdiDevices(Vec<FtdiDeviceInfo>),
    CanDevices(Vec<CanDeviceInfo>),
    Done,
}

impl From<std::result::Result<Response, Error>> for Response {
    fn from(x: std::result::Result<Response, Error>) -> Self {
        match x {
            Ok(x) => x,
            Err(e) => Response::Error(e),
        }
    }
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub enum SerialRequest {
    WriteDataTerminalReady(bool),
    WriteRequestToSend(bool),
    ReadDataSetReady,
    ReadRingIndicator,
    ReadCarrierDetect,
    ReadClearToSend,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub enum SerialResponse {
    PinLevel(bool),
    Done,
}
