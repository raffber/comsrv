use bytestream::{ByteStreamRequest, ByteStreamResponse};
use can::{CanInstrument, CanDeviceInfo};
use hid::{HidInstrument, HidRequest, HidResponse};
use scpi::{ScpiInstrument, ScpiRequest, ScpiResponse};
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use sigrok::{SigrokResponse, SigrokRequest, SigrokInstrument};
use uuid::Uuid;
use std::fmt::{Debug, Formatter};
pub use can::{
    CanMessage, CanRequest, CanResponse, DataFrame, GctMessage, MessageId, RemoteFrame,
    SysCtrlType, BROADCAST_ADDR, MSGTYPE_DDP, MSGTYPE_HEARTBEAT, MSGTYPE_MONITORING_DATA,
    MSGTYPE_MONITORING_REQUEST, MSGTYPE_SYSCTRL,
};

pub use bytestream::ByteStreamInstrument;

pub mod can;
pub mod error;
pub mod bytestream;
pub mod scpi;
pub mod hid;
pub mod sigrok;
mod util;

pub use crate::error::{Error, TransportError, ProtocolError};


#[derive(Clone, Serialize, Deserialize)]
pub struct Duration {
    pub micros: u32,
    pub seconds: u32,
}

#[derive(Clone, Serialize, Deserialize)]
pub enum Instrument {
    ByteStream(ByteStreamInstrument),
    Scpi(ScpiInstrument),
    Can(CanInstrument),
    Hid(HidInstrument),
}

#[derive(Clone, Serialize, Deserialize)]
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

#[derive(Clone, Serialize, Deserialize)]
pub enum Response {
    Error(JsonValue),
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

impl Debug for Response {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(&serde_json::to_string(self).unwrap())
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct FtdiDeviceInfo {
    pub port_open: bool,
    pub vendor_id: u16,
    pub product_id: u16,
    pub serial_number: String,
    pub description: String,
}
