use serde::{Deserialize, Serialize};
use uuid::Uuid;
use std::fmt::{Debug, Formatter};

pub use can::*;
pub use error::*;
pub use bytestream::*;
pub use scpi::*;
pub use hid::*;
pub use sigrok::*;


pub mod can;
pub mod error;
pub mod bytestream;
pub mod scpi;
pub mod hid;
pub mod sigrok;
mod util;

pub use crate::error::{Error, TransportError, ProtocolError};


#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct Duration {
    pub micros: u32,
    pub seconds: u32,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub enum Instrument {
    ByteStream(ByteStreamInstrument),
    Scpi(ScpiInstrument),
    Can(CanInstrument),
    Hid(HidInstrument),
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub enum Request {
    ByteStream {
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
    Connect {
        instrument: Instrument,
        #[serde(skip_serializing_if = "Option::is_none", default)]
        timeout: Option<Duration>,
    },
    ListSigrokDevices,
    ListSerialPorts,
    ListHidDevices,
    ListFtdiDevices,
    ListCanDevices,
    ListConnectedInstruments,
    Lock {
        addr: String,
        timeout_ms: u32,
    },
    Unlock(Uuid),
    DropAll,
    Version,
    Shutdown,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub enum Response {
    Error(Error),
    Instruments(Vec<Instrument>),
    Scpi(ScpiResponse),
    Bytes(ByteStreamResponse),
    Can(CanResponse),
    Sigrok(SigrokResponse),
    Locked { instrument: Instrument, lock_id: Uuid },
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