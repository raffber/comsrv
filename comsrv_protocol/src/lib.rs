use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use std::collections::HashMap;
use uuid::Uuid;

mod can;
mod util;

pub use can::{
    CanMessage, CanRequest, CanResponse, DataFrame, GctMessage, MessageId, RemoteFrame,
    SysCtrlType, BROADCAST_ADDR, MSGTYPE_DDP, MSGTYPE_HEARTBEAT, MSGTYPE_MONITORING_DATA,
    MSGTYPE_MONITORING_REQUEST, MSGTYPE_SYSCTRL,
};

#[derive(Clone, Serialize, Deserialize)]
pub enum Request {
    Scpi {
        addr: String,
        task: ScpiRequest,
        #[serde(skip_serializing_if = "Option::is_none", default)]
        lock: Option<Uuid>,
    },
    ModBus {
        addr: String,
        task: ModBusRequest,
        #[serde(skip_serializing_if = "Option::is_none", default)]
        lock: Option<Uuid>,
    },
    Bytes {
        addr: String,
        task: ByteStreamRequest,
        #[serde(skip_serializing_if = "Option::is_none", default)]
        lock: Option<Uuid>,
    },
    Can {
        addr: String,
        task: CanRequest,
        #[serde(skip_serializing_if = "Option::is_none", default)]
        lock: Option<Uuid>,
    },
    Sigrok {
        addr: String,
        task: SigrokRequest,
    },
    Hid {
        addr: String,
        task: HidRequest,
        #[serde(skip_serializing_if = "Option::is_none", default)]
        lock: Option<Uuid>,
    },
    ListSigrokDevices,
    ListHidDevices,
    ListInstruments,
    Lock {
        addr: String,
        timeout_ms: u32,
    },
    Unlock(Uuid),
    DropAll,
    Drop(String),
    Shutdown,
}

#[derive(Clone, Serialize, Deserialize)]
pub enum Response {
    Error(JsonValue),
    Instruments(Vec<String>),
    Scpi(ScpiResponse),
    Bytes(ByteStreamResponse),
    ModBus(ModBusResponse),
    Can(CanResponse),
    Sigrok(SigrokResponse),
    Locked { addr: String, lock_id: Uuid },
    Hid(HidResponse),
    Done,
}

#[derive(Clone, Serialize, Deserialize)]
pub enum ByteStreamRequest {
    Write(Vec<u8>),
    ReadToTerm {
        term: u8,
        timeout_ms: u32,
    },
    ReadExact {
        count: u32,
        timeout_ms: u32,
    },
    ReadUpTo(u32),
    ReadAll,
    CobsWrite(Vec<u8>),
    CobsRead(u32),
    // timeout
    CobsQuery {
        data: Vec<u8>,
        timeout_ms: u32,
    },
    WriteLine {
        line: String,
        term: u8,
    },
    ReadLine {
        timeout_ms: u32,
        term: u8,
    },
    QueryLine {
        line: String,
        timeout_ms: u32,
        term: u8,
    },
}

#[derive(Clone, Serialize, Deserialize)]
pub enum ByteStreamResponse {
    Done,
    Data(Vec<u8>),
    String(String),
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum ScpiRequest {
    Write(String),
    QueryString(String),
    QueryBinary(String),
    ReadRaw,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum ScpiResponse {
    Done,
    String(String),
    Binary {
        #[serde(
            serialize_with = "util::to_base64",
            deserialize_with = "util::from_base64"
        )]
        data: Vec<u8>,
    },
}

#[derive(Clone, Serialize, Deserialize)]
pub struct SigrokRequest {
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub channels: Vec<String>,
    pub acquire: SigrokAcquire,
    pub sample_rate: u64,
}

#[derive(Clone, Serialize, Deserialize)]
pub enum SigrokAcquire {
    Time(f32),
    Samples(u64),
}

#[derive(Clone, Serialize, Deserialize)]
pub enum SigrokResponse {
    Data(SigrokData),
    Devices(Vec<SigrokDevice>),
}

#[derive(Clone, Serialize, Deserialize)]
pub struct SigrokDevice {
    pub addr: String,
    pub desc: String,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct SigrokData {
    pub tsample: f64,
    pub length: usize,
    pub channels: HashMap<String, Vec<u8>>,
}

#[derive(Clone, Serialize, Deserialize)]
pub enum ModBusRequest {
    ReadCoil { addr: u16, cnt: u16 },
    ReadDiscrete { addr: u16, cnt: u16 },
    ReadInput { addr: u16, cnt: u16 },
    ReadHolding { addr: u16, cnt: u16 },
    WriteCoil { addr: u16, values: Vec<bool> },
    WriteRegister { addr: u16, data: Vec<u16> },
    CustomCommand { code: u8, data: Vec<u8> },
}

#[derive(Clone, Serialize, Deserialize)]
pub enum ModBusResponse {
    Done,
    Number(Vec<u16>),
    Bool(Vec<bool>),
    Custom { code: u8, data: Vec<u8> },
}

#[derive(Clone, Serialize, Deserialize)]
pub enum HidRequest {
    Write { data: Vec<u8> },
    Read { timeout_ms: i32 },
    GetInfo,
}

#[derive(Clone, Serialize, Deserialize)]
pub enum HidResponse {
    Ok,
    Data(Vec<u8>),
    Info(HidDeviceInfo),
    List(Vec<HidDeviceInfo>),
}

#[derive(Clone, Serialize, Deserialize)]
pub struct HidDeviceInfo {
    pub idn: HidIdentifier,
    pub manufacturer: Option<String>,
    pub product: Option<String>,
    pub serial_number: Option<String>,
}

#[derive(Hash, Clone, Serialize, Deserialize)]
pub struct HidIdentifier {
    pub pid: u16,
    pub vid: u16,
}

impl HidIdentifier {
    pub fn new(vid: u16, pid: u16) -> Self {
        HidIdentifier { pid, vid }
    }

    pub fn pid(&self) -> u16 {
        self.pid
    }

    pub fn vid(&self) -> u16 {
        self.vid
    }
}