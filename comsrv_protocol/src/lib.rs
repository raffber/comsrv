use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use std::{collections::HashMap, time::Duration};
use uuid::Uuid;
use std::fmt::{Debug, Formatter};
pub use can::{
    CanMessage, CanRequest, CanResponse, DataFrame, GctMessage, MessageId, RemoteFrame,
    SysCtrlType, BROADCAST_ADDR, MSGTYPE_DDP, MSGTYPE_HEARTBEAT, MSGTYPE_MONITORING_DATA,
    MSGTYPE_MONITORING_REQUEST, MSGTYPE_SYSCTRL,
};

mod can;
mod util;
mod error;



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
    ListSerialPorts,
    ListHidDevices,
    ListFtdiDevices,
    ListCanDevices,
    ListInstruments,
    Lock {
        addr: String,
        timeout_ms: u32,
    },
    Unlock(Uuid),
    DropAll,
    Drop(String),
    Version,
    Shutdown,
}

impl Debug for Request {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(&serde_json::to_string(self).unwrap())
    }
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

#[derive(Deserialize, Serialize, Clone)]
pub enum CanDriverType {
    SocketCAN,
    PCAN,
}

#[derive(Deserialize, Serialize, Clone)]
pub struct CanDeviceInfo {
    pub interface_name: String,
    pub driver_type: CanDriverType,
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
    ModBusRtuDdp {
        timeout_ms: u32,
        station_address: u8,
        custom_command: u8,
        sub_cmd: u8,
        ddp_cmd: u8,
        response: bool,
        data: Vec<u8>,
    },
}

impl ByteStreamRequest {
    pub fn timeout(&self) -> Option<Duration> {
        match self {
            ByteStreamRequest::ReadToTerm { timeout_ms, .. } => Some(Duration::from_millis(*timeout_ms as u64)),
            ByteStreamRequest::ReadExact { timeout_ms , ..} => Some(Duration::from_millis(*timeout_ms as u64)),
            ByteStreamRequest::CobsQuery { timeout_ms, ..} => Some(Duration::from_millis(*timeout_ms as u64)),
            ByteStreamRequest::ReadLine { timeout_ms, .. } => Some(Duration::from_millis(*timeout_ms as u64)),
            ByteStreamRequest::QueryLine { timeout_ms, .. } => Some(Duration::from_millis(*timeout_ms as u64)),
            ByteStreamRequest::ModBusRtuDdp { timeout_ms, ..} => Some(Duration::from_millis(*timeout_ms as u64)),
            _ => None
        }
    }
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

fn is_zero(x: &u8) -> bool {
    *x == 0
}

#[derive(Clone, Serialize, Deserialize)]
pub enum ModBusRequest {
    ReadCoil {
        addr: u16,
        cnt: u16,
        #[serde(skip_serializing_if = "is_zero", default)]
        slave_id: u8,
    },
    ReadDiscrete {
        addr: u16,
        cnt: u16,
        #[serde(skip_serializing_if = "is_zero", default)]
        slave_id: u8,
    },
    ReadInput {
        addr: u16,
        cnt: u16,
        #[serde(skip_serializing_if = "is_zero", default)]
        slave_id: u8,
    },
    ReadHolding {
        addr: u16,
        cnt: u16,
        #[serde(skip_serializing_if = "is_zero", default)]
        slave_id: u8,
    },
    WriteCoil {
        addr: u16,
        values: Vec<bool>,
        #[serde(skip_serializing_if = "is_zero", default)]
        slave_id: u8,
    },
    WriteRegister {
        addr: u16,
        data: Vec<u16>,
        #[serde(skip_serializing_if = "is_zero", default)]
        slave_id: u8,
    },
    CustomCommand {
        code: u8,
        data: Vec<u8>,
        #[serde(skip_serializing_if = "is_zero", default)]
        slave_id: u8,
    },
}

impl ModBusRequest {
    pub fn slave_id(&self) -> u8 {
        match self {
            ModBusRequest::ReadCoil { slave_id, .. } => *slave_id,
            ModBusRequest::ReadDiscrete {  slave_id, ..  } => *slave_id,
            ModBusRequest::ReadInput {  slave_id, ..  } => *slave_id,
            ModBusRequest::ReadHolding {  slave_id, ..  } => *slave_id,
            ModBusRequest::WriteCoil {  slave_id, ..  } => *slave_id,
            ModBusRequest::WriteRegister {  slave_id, ..  } => *slave_id,
            ModBusRequest::CustomCommand {  slave_id, ..  } => *slave_id,
        }
    }
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

#[derive(Clone, Serialize, Deserialize)]
pub struct FtdiDeviceInfo {
    pub port_open: bool,
    pub vendor_id: u16,
    pub product_id: u16,
    pub serial_number: String,
    pub description: String,
}