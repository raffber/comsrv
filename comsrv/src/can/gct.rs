use std::collections::HashMap;
use std::iter::repeat;

use async_can::{DataFrame, Message};
use byteorder::{ByteOrder, LittleEndian};
use serde::{Deserialize, Serialize};

use crate::can::CanError;

const BROADCAST_ADDR: u8 = 0x7F;

const MSGTYPE_SYSCTRL: u8 = 1;
const MSGTYPE_MONITORING_DATA: u8 = 7;
const MSGTYPE_MONITORING_REQUEST: u8 = 8;
const MSGTYPE_DDP: u8 = 12;
const MSGTYPE_HEARTBEAT: u8 = 14;

const MAX_DDP_DATA_LEN: usize = 62; // 8 message * 8bytes - crc - cmd

#[derive(Clone, Serialize, Deserialize)]
pub enum GctMessage {
    SysCtrl {
        src: u8,
        dst: u8,
        cmd: u16,
        value: bool,
        query: bool,
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

impl GctMessage {
    fn validate(&self) -> Result<(), CanError> {
        let ok = match self {
            GctMessage::SysCtrl { src, dst, data, cmd, .. } => {
                let cmd_ok = *cmd < 1024;
                let addr_ok = *src < BROADCAST_ADDR && *dst <= BROADCAST_ADDR;
                addr_ok && data.len() <= 8 && cmd_ok
            }
            GctMessage::MonitoringData { src, group_idx, reading_idx, data, } => {
                *src < BROADCAST_ADDR && data.len() < 8 && *group_idx < 32 && *reading_idx < 64
            }
            GctMessage::MonitoringRequest { src, dst, group_idx, .. } => {
                *src < BROADCAST_ADDR && *dst <= BROADCAST_ADDR && *group_idx < 32
            }
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
            Err(CanError::InvalidMessage)
        }
    }

    fn try_decode_sysctrl(msg: DataFrame) -> Option<GctMessage> {
        let id = MessageId(msg.id());
        if id.src() == BROADCAST_ADDR {
            return None;
        }
        Some(GctMessage::SysCtrl {
            src: id.src(),
            dst: id.dst(),
            cmd: id.type_data() >> 2,
            value: (id.type_data() & 2) > 0,
            query: (id.type_data() & 1) > 0,
            data: msg.data().to_vec(),
        })
    }

    fn try_decode_monitoring_data(msg: DataFrame) -> Option<GctMessage> {
        let id = MessageId(msg.id());
        if id.src() == BROADCAST_ADDR {
            return None;
        }
        let group_idx = (id.type_data() >> 6) as u8;
        let reading_idx = (id.type_data() & 0x3F) as u8;
        Some(GctMessage::MonitoringData {
            src: id.src(),
            group_idx,
            reading_idx,
            data: msg.data().to_vec(),
        })
    }

    fn try_decode_monitoring_request(msg: DataFrame) -> Option<GctMessage> {
        let id = MessageId(msg.id());
        if id.src() == BROADCAST_ADDR {
            return None;
        }
        let group_idx = (id.type_data() >> 6) as u8;
        let mut data = msg.data().to_vec();
        data.extend(repeat(0).take(8 - data.len()));
        let readings = LittleEndian::read_u64(&data);
        Some(GctMessage::MonitoringRequest {
            src: id.src(),
            dst: id.dst(),
            group_idx,
            readings,
        })
    }

    fn try_decode_heartbeat(msg: DataFrame) -> Option<GctMessage> {
        let id = MessageId(msg.id());
        if id.src() == BROADCAST_ADDR {
            return None;
        }
        if msg.data().len() < 2 {
            return None;
        }
        let product_id = LittleEndian::read_u16(msg.data());
        Some(GctMessage::Heartbeat {
            src: id.src(),
            product_id,
        })
    }
}

struct MessageId(u32);

impl MessageId {
    fn new(msg_type: u8, src: u8, dst: u8, type_data: u16) -> Self {
        let ret = (type_data & 0x7FF) as u32
            | (dst as u32 & 0x7F) << 11
            | (src as u32 & 0x7F) << 18
            | (msg_type as u32 & 0xF) << 25;
        MessageId(ret)
    }

    fn msg_type(&self) -> u8 {
        ((self.0 >> 25) & 0xF) as u8
    }

    fn src(&self) -> u8 {
        ((self.0 >> 18) & 0x7F) as u8
    }

    fn dst(&self) -> u8 {
        ((self.0 >> 11) & 0x7F) as u8
    }

    fn type_data(&self) -> u16 {
        (self.0 & 0x7FF) as u16
    }
}

struct DdpDecoder {
    dst_addr: u8,
    src_start_addr: u8,
    frames_received: u8,
    expected_frame_cnt: u8,
    started: bool,
    data: Vec<u8>,
}

impl DdpDecoder {
    fn new(dst: u8) -> Self {
        Self {
            dst_addr: dst,
            src_start_addr: 0,
            frames_received: 0,
            expected_frame_cnt: 0,
            started: false,
            data: vec![],
        }
    }

    fn reset(&mut self) {
        self.frames_received = 0;
        self.started = false;
        self.data.clear();
    }

    fn decode_completed(&mut self) -> Option<GctMessage> {
        if self.data.len() < 1 {
            return None;
        }
        if crc8(&self.data) != 0 {
            return None;
        }
        let data = self.data[0..self.data.len() - 1].to_vec();

        Some(GctMessage::Ddp {
            src: self.src_start_addr,
            dst: self.dst_addr,
            data,
        })
    }

    fn decode(&mut self, msg: DataFrame) -> Option<GctMessage> {
        let id = MessageId(msg.id());
        if id.dst() != self.dst_addr {
            return None;
        }
        let type_data = id.type_data();
        let frame_size = ((type_data >> 8) & 0x7) as u8;
        let frame_idx = ((type_data >> 5) & 0x7) as u8;

        if frame_idx == 0 {
            self.reset();
            self.expected_frame_cnt = frame_size;
            self.src_start_addr = id.src();
            self.started = true;
        } else if self.frames_received + 1 != frame_idx {
            // out of sequence
            self.reset();
            return None;
        } else if frame_size != self.expected_frame_cnt {
            // frame cnt changed during one transaction
            self.reset();
            return None;
        } else if !self.started {
            // first frame was missing
            return None;
        } else if self.src_start_addr != id.src() {
            // two nodes are interfering...
            return None;
        }
        self.frames_received = frame_idx;
        self.data.extend_from_slice(&msg.data());
        if frame_idx == frame_size {
            return self.decode_completed();
        }
        None
    }
}

pub struct Decoder {
    ddp: HashMap<u8, DdpDecoder>,
}

impl Decoder {
    pub fn new() -> Self {
        Self { ddp: Default::default() }
    }

    pub fn reset(&mut self) {
        self.ddp.clear()
    }

    pub fn decode(&mut self, msg: Message) -> Option<GctMessage> {
        let msg = match msg {
            Message::Data(msg) => msg,
            _ => return None,
        };
        if !msg.ext_id() {
            return None;
        }
        let id = MessageId(msg.id());
        match id.msg_type() {
            MSGTYPE_SYSCTRL => GctMessage::try_decode_sysctrl(msg),
            MSGTYPE_MONITORING_DATA => GctMessage::try_decode_monitoring_data(msg),
            MSGTYPE_MONITORING_REQUEST => GctMessage::try_decode_monitoring_request(msg),
            MSGTYPE_DDP => {
                let dst = id.dst();
                let decoder = self.ddp.entry(dst).or_insert_with(|| DdpDecoder::new(dst));
                decoder.decode(msg)
            }
            MSGTYPE_HEARTBEAT => GctMessage::try_decode_heartbeat(msg),
            _ => None
        }
    }
}

pub fn encode(msg: GctMessage) -> Result<Vec<Message>, CanError> {
    msg.validate()?;
    let ret = match msg {
        GctMessage::SysCtrl { src, dst, cmd, value, query, data } => {
            let type_data = (cmd << 2) | (value as u16) << 1 | query as u16;
            let id = MessageId::new(MSGTYPE_SYSCTRL, src, dst, type_data);
            vec![Message::new_data(id.0, true, &data).unwrap()]
        }
        GctMessage::MonitoringData { src, group_idx, reading_idx, data } => {
            let type_data = ((group_idx as u16) << 6) | reading_idx as u16;
            let id = MessageId::new(MSGTYPE_MONITORING_DATA, src, BROADCAST_ADDR, type_data);
            vec![Message::new_data(id.0, true, &data).unwrap()]
        }
        GctMessage::MonitoringRequest { src, dst, group_idx, readings } => {
            let id = MessageId::new(MSGTYPE_MONITORING_REQUEST, src, dst, (group_idx as u16) << 6);
            let mut data = [0_u8; 8];
            LittleEndian::write_u64(&mut data, readings);
            vec![Message::new_data(id.0, true, &data).unwrap()]
        }
        GctMessage::Ddp { src, dst, mut data } => {
            data.push(crc8(&data));
            let chunks: Vec<_> = data.chunks(8).collect();
            let num_chunks = chunks.len();
            let mut ret = Vec::with_capacity(num_chunks);
            let part_count = num_chunks - 1;
            let mut idx = 0;
            for chunk in chunks {
                let type_data = (part_count as u16) << 8 | idx << 5;
                let id = MessageId::new(MSGTYPE_DDP, src, dst, type_data);
                ret.push(Message::new_data(id.0, true, &chunk).unwrap());
                idx += 1;
            }
            ret
        }
        GctMessage::Heartbeat { src, product_id } => {
            let id = MessageId::new(MSGTYPE_HEARTBEAT, src, BROADCAST_ADDR, 0);
            let mut data = [0_u8; 2];
            LittleEndian::write_u16(&mut data, product_id);
            vec![Message::new_data(id.0, true, &data).unwrap()]
        }
    };
    Ok(ret)
}

pub fn crc8(data: &[u8]) -> u8 {
    let poly = 0x07_u8;
    let mut crc = 0xFF_u8;
    for b in data {
        crc ^= *b;
        for _ in 0..8 {
            if (crc & 0x80) != 0 {
                crc = (crc << 1) ^ poly;
            } else {
                crc <<= 1;
            }
        }
        crc &= 0xFF;
    }
    return crc;
}
