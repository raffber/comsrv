use byteorder::{ByteOrder, LittleEndian};
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use std::collections::HashMap;
use std::iter::repeat;
use uuid::Uuid;

mod util;

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
pub enum CanMessage {
    Data(DataFrame),
    Remote(RemoteFrame),
}

impl CanMessage {
    pub fn id(&self) -> u32 {
        match self {
            CanMessage::Data(x) => x.id,
            CanMessage::Remote(x) => x.id,
        }
    }

    pub fn ext_id(&self) -> bool {
        match self {
            CanMessage::Data(x) => x.ext_id,
            CanMessage::Remote(x) => x.ext_id,
        }
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct DataFrame {
    pub id: u32,
    pub ext_id: bool,
    pub data: Vec<u8>,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct RemoteFrame {
    pub id: u32,
    pub ext_id: bool,
    pub dlc: u8,
}

#[derive(Serialize, Deserialize, Clone)]
pub enum CanRequest {
    ListenRaw(bool),
    ListenGct(bool),
    StopAll,
    EnableLoopback(bool),
    TxRaw(CanMessage),
    TxGct(GctMessage),
}

#[derive(Serialize, Deserialize, Clone)]
pub enum CanResponse {
    Started(String),
    Stopped(String),
    Ok,
    Raw(CanMessage),
    Gct(GctMessage),
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
    CobsRead(u32), // timeout
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

#[derive(Clone, Serialize, Deserialize)]
pub enum SysCtrlType {
    Value,
    Query,
    None,
}

#[derive(Clone, Serialize, Deserialize)]
pub enum GctMessage {
    SysCtrl {
        src: u8,
        dst: u8,
        cmd: u16,
        tp: SysCtrlType,
        data: Vec<u8>,
    },
    MonitoringData {
        src: u8,
        group_idx: u8,
        reading_idx: u8,
        data: Vec<u8>,
    },
    MonitoringRequest {
        src: u8,
        dst: u8,
        group_idx: u8,
        readings: u64,
    },
    Ddp {
        src: u8,
        dst: u8,
        data: Vec<u8>,
    },
    Heartbeat {
        src: u8,
        product_id: u16,
    },
}

pub const BROADCAST_ADDR: u8 = 0x7F;

pub const MSGTYPE_SYSCTRL: u8 = 1;
pub const MSGTYPE_MONITORING_DATA: u8 = 7;
pub const MSGTYPE_MONITORING_REQUEST: u8 = 8;
pub const MSGTYPE_DDP: u8 = 12;
pub const MSGTYPE_HEARTBEAT: u8 = 14;
pub const MAX_DDP_DATA_LEN: usize = 61; // 8 message * 8bytes - crc - cmd

impl GctMessage {
    pub fn validate(&self) -> Result<(), ()> {
        let ok = match self {
            GctMessage::SysCtrl {
                src,
                dst,
                data,
                cmd,
                ..
            } => {
                let cmd_ok = *cmd < 1024;
                let addr_ok = *src < BROADCAST_ADDR && *dst <= BROADCAST_ADDR;
                addr_ok && data.len() <= 8 && cmd_ok
            }
            GctMessage::MonitoringData {
                src,
                group_idx,
                reading_idx,
                data,
            } => *src < BROADCAST_ADDR && data.len() < 8 && *group_idx < 32 && *reading_idx < 64,
            GctMessage::MonitoringRequest {
                src,
                dst,
                group_idx,
                ..
            } => *src < BROADCAST_ADDR && *dst <= BROADCAST_ADDR && *group_idx < 32,
            GctMessage::Ddp { src, dst, data } => {
                let addr_ok = *src < BROADCAST_ADDR && *dst <= BROADCAST_ADDR;
                addr_ok && data.len() <= MAX_DDP_DATA_LEN
            }
            GctMessage::Heartbeat { src, product_id } => {
                let addr_ok = *src < BROADCAST_ADDR;
                let prod_id_ok = *product_id != 0 && *product_id != 0xFFFF;
                addr_ok && prod_id_ok
            }
        };
        if ok {
            Ok(())
        } else {
            Err(())
        }
    }

    pub fn try_decode_sysctrl(msg: DataFrame) -> Option<GctMessage> {
        let id = MessageId(msg.id);
        if id.src() == BROADCAST_ADDR {
            return None;
        }
        let value = (id.type_data() & 2) > 0;
        let query = (id.type_data() & 1) > 0;
        if value && query {
            return None;
        }
        let tp = if value {
            SysCtrlType::Value
        } else if query {
            SysCtrlType::Query
        } else {
            SysCtrlType::None
        };

        Some(GctMessage::SysCtrl {
            src: id.src(),
            dst: id.dst(),
            cmd: id.type_data() >> 2,
            data: msg.data.to_vec(),
            tp,
        })
    }

    pub fn try_decode_monitoring_data(msg: DataFrame) -> Option<GctMessage> {
        let id = MessageId(msg.id);
        if id.src() == BROADCAST_ADDR {
            return None;
        }
        let group_idx = (id.type_data() >> 6) as u8;
        let reading_idx = (id.type_data() & 0x3F) as u8;
        Some(GctMessage::MonitoringData {
            src: id.src(),
            group_idx,
            reading_idx,
            data: msg.data.to_vec(),
        })
    }

    pub fn try_decode_monitoring_request(msg: DataFrame) -> Option<GctMessage> {
        let id = MessageId(msg.id);
        if id.src() == BROADCAST_ADDR {
            return None;
        }
        let group_idx = (id.type_data() >> 6) as u8;
        let mut data = msg.data.to_vec();
        data.extend(repeat(0).take(8 - data.len()));
        let readings = LittleEndian::read_u64(&data);
        Some(GctMessage::MonitoringRequest {
            src: id.src(),
            dst: id.dst(),
            group_idx,
            readings,
        })
    }

    pub fn try_decode_heartbeat(msg: DataFrame) -> Option<GctMessage> {
        let id = MessageId(msg.id);
        if id.src() == BROADCAST_ADDR {
            return None;
        }
        if msg.data.len() < 2 {
            return None;
        }
        let product_id = LittleEndian::read_u16(&msg.data);
        Some(GctMessage::Heartbeat {
            src: id.src(),
            product_id,
        })
    }
}

pub struct MessageId(pub u32);

impl MessageId {
    pub fn new(msg_type: u8, src: u8, dst: u8, type_data: u16) -> Self {
        let ret = (type_data & 0x7FF) as u32
            | (dst as u32 & 0x7F) << 11
            | (src as u32 & 0x7F) << 18
            | (msg_type as u32 & 0xF) << 25;
        MessageId(ret)
    }

    pub fn msg_type(&self) -> u8 {
        ((self.0 >> 25) & 0xF) as u8
    }

    pub fn src(&self) -> u8 {
        ((self.0 >> 18) & 0x7F) as u8
    }

    pub fn dst(&self) -> u8 {
        ((self.0 >> 11) & 0x7F) as u8
    }

    pub fn type_data(&self) -> u16 {
        (self.0 & 0x7FF) as u16
    }
}
