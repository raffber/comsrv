use byteorder::{ByteOrder, LittleEndian};
use serde::{Deserialize, Serialize};
use std::iter::repeat;

#[derive(Clone, Serialize, Deserialize, Debug, Eq, Hash, PartialEq)]
pub enum CanAddress {
    PCan { address: String },
    SocketCan { interface: String },
    Loopback,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub enum CanInstrument {
    PCan { address: String, baudrate: u32 },
    SocketCan { interface: String },
    Loopback,
}

impl CanInstrument {
    pub fn bitrate(&self) -> Option<u32> {
        match self {
            CanInstrument::PCan { baudrate, .. } => Some(*baudrate),
            _ => None,
        }
    }
}

impl From<CanInstrument> for CanAddress {
    fn from(x: CanInstrument) -> Self {
        match x {
            CanInstrument::PCan { address, .. } => CanAddress::PCan { address },
            CanInstrument::SocketCan { interface } => CanAddress::SocketCan { interface },
            CanInstrument::Loopback => CanAddress::Loopback,
        }
    }
}

#[derive(Deserialize, Serialize, Clone, Debug)]
pub enum CanDriverType {
    SocketCAN,
    PCAN,
}

#[derive(Deserialize, Serialize, Clone, Debug)]
pub struct CanDeviceInfo {
    pub interface_name: String,
    pub driver_type: CanDriverType,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
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

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct DataFrame {
    pub id: u32,
    pub ext_id: bool,
    pub data: Vec<u8>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct RemoteFrame {
    pub id: u32,
    pub ext_id: bool,
    pub dlc: u8,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum CanRequest {
    ListenRaw(bool),
    ListenGct(bool),
    StopAll,
    EnableLoopback(bool),
    TxRaw(CanMessage),
    TxGct(GctMessage),
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum CanResponse {
    Started(CanAddress),
    Stopped(CanAddress),
    Ok,
    Raw(CanMessage),
    Gct(GctMessage),
}

#[derive(Clone, Serialize, Deserialize, Debug, PartialEq)]
pub enum SysCtrlType {
    Value,
    Query,
    None,
}

mod default {
    pub fn is_zero_or_one(x: &u32) -> bool {
        *x == 0 || *x == 1
    }
}

#[derive(Clone, Serialize, Deserialize, Debug)]
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
        #[serde(
            skip_serializing_if = "default::is_zero_or_one",
            default = "Default::default"
        )]
        version: u32,
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
pub const MAX_DDP_DATA_LEN_V1: usize = 61; // 8 message * 8bytes - crc - cmd
pub const MAX_DDP_DATA_LEN_V2: usize = 8 * 256 - 3; // 256 message * 8bytes - crc - cmd

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
            GctMessage::Ddp {
                src,
                dst,
                data,
                version,
            } => {
                let addr_ok = *src < BROADCAST_ADDR && *dst <= BROADCAST_ADDR;
                if *version == 0 || *version == 1 {
                    addr_ok && data.len() <= MAX_DDP_DATA_LEN_V1
                } else if *version == 2 {
                    addr_ok && data.len() <= MAX_DDP_DATA_LEN_V2
                } else {
                    false
                }
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
