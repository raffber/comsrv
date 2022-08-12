use serde::{Deserialize, Serialize};
use std::fmt::Debug;
use uuid::Uuid;

pub use bytestream::*;
pub use can::*;
pub use error::*;
pub use hid::*;
pub use scpi::*;
pub use sigrok::*;

pub mod bytestream;
pub mod can;
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

impl Into<std::time::Duration> for Duration {
    fn into(self) -> std::time::Duration {
        std::time::Duration::from_micros((self.seconds as u64 * 1000000_u64) + (self.micros as u64))
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
    },
    Prologix {
        instrument: PrologixInstrument,
        request: PrologixRequest,
        #[serde(skip_serializing_if = "Option::is_none", default)]
        lock: Option<Uuid>,
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
