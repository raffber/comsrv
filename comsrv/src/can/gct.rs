use std::collections::HashMap;

use crate::can::crc::crc16;
use crate::can::CanError;
use byteorder::{ByteOrder, LittleEndian};
use comsrv_protocol::{
    CanMessage, DataFrame, GctMessage, MessageId, SysCtrlType, BROADCAST_ADDR, MSGTYPE_DDP,
    MSGTYPE_HEARTBEAT, MSGTYPE_MONITORING_DATA, MSGTYPE_MONITORING_REQUEST, MSGTYPE_SYSCTRL,
};

struct DdpDecoderV1 {
    dst_addr: u8,
    src_start_addr: u8,
    frames_received: u8,
    expected_frame_cnt: u8,
    started: bool,
    data: Vec<u8>,
}

impl DdpDecoderV1 {
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
        if self.data.len() < 2 {
            return None;
        }
        if crc16(&self.data) != 0 {
            return None;
        }
        let data = self.data[0..self.data.len() - 2].to_vec();

        Some(GctMessage::Ddp {
            src: self.src_start_addr,
            dst: self.dst_addr,
            data,
            version: 1,
        })
    }

    fn decode(&mut self, msg: &DataFrame) -> Option<GctMessage> {
        let id = MessageId(msg.id);
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
        } else if self.frames_received + 1 != frame_idx || frame_size != self.expected_frame_cnt {
            // out of sequence
            // or frame cnt changed during one transaction
            self.reset();
            return None;
        } else if self.src_start_addr != id.src() || !self.started {
            // first frame was missing
            // or two nodes are interfering...
            return None;
        }
        self.frames_received = frame_idx;
        self.data.extend_from_slice(&msg.data);
        if frame_idx == frame_size {
            return self.decode_completed();
        }
        None
    }
}

struct DdpDecoderV2 {
    dst_addr: u8,
    src_start_addr: u8,
    parts: HashMap<u32, Vec<u8>>,
    eof_received: bool,
    max_part_idx: u32,
}

impl DdpDecoderV2 {
    fn new(dst: u8) -> Self {
        Self {
            dst_addr: dst,
            src_start_addr: 0,
            parts: Default::default(),
            eof_received: false,
            max_part_idx: 0,
        }
    }

    fn reset(&mut self) {
        self.parts.clear();
        self.src_start_addr = 0;
        self.eof_received = false;
    }

    pub fn decode(&mut self, msg: DataFrame) -> Option<GctMessage> {
        let id = MessageId(msg.id);
        if id.dst() != self.dst_addr {
            return None;
        }
        let type_data = id.type_data();
        let idx = (type_data & 0xFF) as u32;
        let eof = ((type_data >> 10) & 1) > 1;
        if idx == 0 {
            self.reset();
            self.src_start_addr = id.src();
        }
        if id.src() != self.src_start_addr {
            self.reset();
            return None;
        }
        self.parts.insert(idx, msg.data);
        if eof {
            self.eof_received = true;
            self.max_part_idx = idx;
        }

        if !self.eof_received {
            return None;
        }
        // check if we got all parts
        for k in 0..(self.max_part_idx + 1) {
            if self.parts.get(&k).is_none() {
                return None;
            }
        }
        // yes complete..., try decoding
        let mut data = Vec::with_capacity((self.max_part_idx + 1) as usize * 8);
        for k in 0..(self.max_part_idx + 1) {
            data.extend(self.parts.get(&k).unwrap());
        }
        self.reset();

        if data.len() < 2 || crc16(&data) != 0 {
            return None;
        }
        Some(GctMessage::Ddp {
            src: self.src_start_addr,
            dst: self.dst_addr,
            data: data[0..data.len() - 2].to_vec(),
            version: 2,
        })
    }
}

pub struct Decoder {
    ddp_v1: HashMap<u8, DdpDecoderV1>,
    ddp_v2: HashMap<u8, DdpDecoderV2>,
}

impl Decoder {
    pub fn new() -> Self {
        Self {
            ddp_v1: Default::default(),
            ddp_v2: Default::default(),
        }
    }

    pub fn reset(&mut self) {
        self.ddp_v1.clear()
    }

    pub fn decode(&mut self, msg: CanMessage) -> Option<GctMessage> {
        let msg = match msg {
            CanMessage::Data(msg) => msg,
            _ => return None,
        };
        if !msg.ext_id {
            return None;
        }
        let id = MessageId(msg.id);
        match id.msg_type() {
            MSGTYPE_SYSCTRL => GctMessage::try_decode_sysctrl(msg),
            MSGTYPE_MONITORING_DATA => GctMessage::try_decode_monitoring_data(msg),
            MSGTYPE_MONITORING_REQUEST => GctMessage::try_decode_monitoring_request(msg),
            MSGTYPE_DDP => {
                let dst = id.dst();
                let decoder_v1 = self
                    .ddp_v1
                    .entry(dst)
                    .or_insert_with(|| DdpDecoderV1::new(dst));
                let decoder_v2 = self
                    .ddp_v2
                    .entry(dst)
                    .or_insert_with(|| DdpDecoderV2::new(dst));
                if let Some(x) = decoder_v1.decode(&msg) {
                    Some(x)
                } else if let Some(x) = decoder_v2.decode(msg) {
                    Some(x)
                } else {
                    None
                }
            }
            MSGTYPE_HEARTBEAT => GctMessage::try_decode_heartbeat(msg),
            _ => None,
        }
    }
}

fn encode_ddp_v1(src: u8, dst: u8, mut data: Vec<u8>) -> Vec<CanMessage> {
    let crc = crc16(&data);
    data.push((crc >> 8) as u8);
    data.push((crc & 0xFF_u16) as u8);
    let chunks: Vec<_> = data.chunks(8).collect();
    let num_chunks = chunks.len();
    let mut ret = Vec::with_capacity(num_chunks);
    let part_count = num_chunks - 1;
    for (idx, chunk) in chunks.into_iter().enumerate() {
        let type_data = (part_count as u16) << 8 | (idx as u16) << 5;
        let id = MessageId::new(MSGTYPE_DDP, src, dst, type_data);
        let msg = CanMessage::Data(DataFrame {
            id: id.0,
            ext_id: true,
            data: chunk.to_vec(),
        });
        ret.push(msg);
    }
    ret
}

fn encode_ddp_v2(src: u8, dst: u8, mut data: Vec<u8>) -> Vec<CanMessage> {
    let crc = crc16(&data);
    data.push((crc >> 8) as u8);
    data.push((crc & 0xFF_u16) as u8);
    let max_idx = data.len() / 8;
    let mut ret = Vec::with_capacity(max_idx + 1);
    for (idx, chunk) in data.chunks(8).enumerate() {
        let mut type_data = idx & 0xFF;
        if idx == max_idx {
            // set EOF
            type_data |= 1 << 10;
        }
        let id = MessageId::new(MSGTYPE_DDP, src, dst, type_data as u16);
        let msg = CanMessage::Data(DataFrame {
            id: id.0,
            ext_id: true,
            data: chunk.to_vec(),
        });
        ret.push(msg);
    }
    ret
}

pub fn encode(msg: GctMessage) -> Result<Vec<CanMessage>, CanError> {
    if let Err(_) = msg.validate() {
        return Err(CanError::InvalidMessage);
    }
    let ret = match msg {
        GctMessage::SysCtrl {
            src,
            dst,
            cmd,
            tp,
            data,
        } => {
            let (value, query) = match tp {
                SysCtrlType::Value => (true, false),
                SysCtrlType::Query => (false, true),
                SysCtrlType::None => (false, false),
            };
            let type_data = (cmd << 2) | (value as u16) << 1 | query as u16;
            let id = MessageId::new(MSGTYPE_SYSCTRL, src, dst, type_data);
            let msg = CanMessage::Data(DataFrame {
                id: id.0,
                ext_id: true,
                data,
            });
            vec![msg]
        }
        GctMessage::MonitoringData {
            src,
            group_idx,
            reading_idx,
            data,
        } => {
            let type_data = ((group_idx as u16) << 6) | reading_idx as u16;
            let id = MessageId::new(MSGTYPE_MONITORING_DATA, src, BROADCAST_ADDR, type_data);
            let msg = CanMessage::Data(DataFrame {
                id: id.0,
                ext_id: true,
                data,
            });
            vec![msg]
        }
        GctMessage::MonitoringRequest {
            src,
            dst,
            group_idx,
            readings,
        } => {
            let id = MessageId::new(
                MSGTYPE_MONITORING_REQUEST,
                src,
                dst,
                (group_idx as u16) << 6,
            );
            let mut data = [0_u8; 8];
            LittleEndian::write_u64(&mut data, readings);
            let msg = CanMessage::Data(DataFrame {
                id: id.0,
                ext_id: true,
                data: data.to_vec(),
            });
            vec![msg]
        }
        GctMessage::Ddp {
            src,
            dst,
            data,
            version,
        } => {
            if version == 0 || version == 1 {
                encode_ddp_v1(src, dst, data)
            } else if version == 2 {
                encode_ddp_v2(src, dst, data)
            } else {
                vec![]
            }
        }
        GctMessage::Heartbeat { src, product_id } => {
            let id = MessageId::new(MSGTYPE_HEARTBEAT, src, BROADCAST_ADDR, 0);
            let mut data = [0_u8; 2];
            LittleEndian::write_u16(&mut data, product_id);
            let msg = CanMessage::Data(DataFrame {
                id: id.0,
                ext_id: true,
                data: data.to_vec(),
            });
            vec![msg]
        }
    };
    Ok(ret)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ddp() {
        let data = vec![0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12];
        let msg = GctMessage::Ddp {
            src: 12,
            dst: 34,
            data: data.clone(),
            version: 0,
        };
        let raw = encode(msg).unwrap();
        let mut decoder = DdpDecoderV1::new(34);
        let mut result = None;
        for x in raw {
            match x {
                CanMessage::Data(x) => {
                    result = decoder.decode(&x);
                }
                CanMessage::Remote(_) => {
                    panic!()
                }
            }
        }
        let msg = result.unwrap();
        match msg {
            GctMessage::Ddp {
                src,
                dst,
                data: rx_data,
                version: _,
            } => {
                assert_eq!(data, rx_data);
                assert_eq!(src, 12);
                assert_eq!(dst, 34);
            }
            _ => {
                panic!()
            }
        }
    }

    fn encode_decode_one(msg: GctMessage) -> GctMessage {
        let mut decoder = Decoder::new();
        let mut msgs = encode(msg).unwrap();
        assert_eq!(msgs.len(), 1);
        let msg = msgs.drain(..).next().unwrap();
        decoder.decode(msg).unwrap()
    }

    #[test]
    fn heartbeat() {
        let hb = GctMessage::Heartbeat {
            src: 12,
            product_id: 0xABCD,
        };
        let result = encode_decode_one(hb);
        match result {
            GctMessage::Heartbeat { src, product_id } => {
                assert_eq!(src, 12);
                assert_eq!(0xABCD, product_id);
            }
            _ => {
                panic!()
            }
        }
    }

    #[test]
    fn reading_request() {
        let request = GctMessage::MonitoringRequest {
            src: 12,
            dst: 34,
            group_idx: 3,
            readings: 43,
        };
        let result = encode_decode_one(request);

        match result {
            GctMessage::MonitoringRequest {
                src,
                dst,
                group_idx,
                readings,
            } => {
                assert_eq!(src, 12);
                assert_eq!(dst, 34);
                assert_eq!(group_idx, 3);
                assert_eq!(readings, 43);
            }
            _ => {
                panic!()
            }
        }
    }

    #[test]
    fn reading() {
        let msg = GctMessage::MonitoringData {
            src: 12,
            group_idx: 23,
            reading_idx: 53,
            data: vec![1, 2, 3, 4],
        };
        let result = encode_decode_one(msg);
        match result {
            GctMessage::MonitoringData {
                src,
                group_idx,
                reading_idx,
                data,
            } => {
                assert_eq!(src, 12);
                assert_eq!(group_idx, 23);
                assert_eq!(reading_idx, 53);
                assert_eq!(data, vec![1, 2, 3, 4]);
            }
            _ => panic!(),
        }
    }

    #[test]
    fn sysctrl() {
        let msg = GctMessage::SysCtrl {
            src: 12,
            dst: 34,
            cmd: 452,
            tp: SysCtrlType::Value,
            data: vec![1, 2, 3, 4],
        };
        let result = encode_decode_one(msg);
        match result {
            GctMessage::SysCtrl {
                src,
                dst,
                cmd,
                tp,
                data,
            } => {
                assert_eq!(src, 12);
                assert_eq!(dst, 34);
                assert_eq!(cmd, 452);
                assert!(matches!(tp, SysCtrlType::Value));
                assert_eq!(data, vec![1, 2, 3, 4]);
            }
            _ => panic!(),
        }
    }
}
