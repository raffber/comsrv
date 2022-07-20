use anyhow::anyhow;
use comsrv_protocol::{ModBusProtocol, ModBusRequest, ModBusResponse};
use rmodbus::{
    ModbusProto, MODBUS_GET_COILS, MODBUS_GET_DISCRETES, MODBUS_GET_INPUTS, MODBUS_SET_COIL, MODBUS_SET_HOLDING,
    MODBUS_SET_HOLDINGS_BULK,
};
use std::time::Duration;
use tokio::io::{AsyncRead, AsyncWrite};

fn into_rmodbus_proto(proto: ModBusProtocol) -> ModbusProto {
    match proto {
        ModBusProtocol::Tcp => ModbusProto::TcpUdp,
        ModBusProtocol::Rtu => ModbusProto::Rtu,
    }
}

pub async fn handle<T: AsyncRead + AsyncWrite + Unpin>(
    timeout: Duration,
    station_address: u8,
    protocol: ModBusProtocol,
    request: ModBusRequest,
) -> crate::Result<ModBusResponse> {
    let proto = into_rmodbus_proto(protocol);
    let mut wire_request = rmodbus::client::ModbusRequest {
        tr_id: rand::random(),
        unit_id: station_address,
        func: 0,
        reg: 0,
        count: 0,
        proto,
    };
    match request {
        ModBusRequest::Ddp {
            sub_cmd,
            ddp_cmd,
            response,
            data,
        } => {}
        ModBusRequest::ReadCoil { addr, cnt } => {
            wire_request.func = MODBUS_GET_COILS;
            wire_request.reg = addr;
            wire_request.count = cnt;
        }
        ModBusRequest::ReadDiscrete { addr, cnt } => {
            wire_request.func = MODBUS_GET_DISCRETES;
            wire_request.reg = addr;
            wire_request.count = cnt;
        }
        ModBusRequest::ReadInput { addr, cnt } => {
            wire_request.func = MODBUS_GET_INPUTS;
            wire_request.reg = addr;
            wire_request.count = cnt;
        }
        ModBusRequest::ReadHolding { addr, cnt } => {
            wire_request.func = MODBUS_SET_HOLDING;
            wire_request.reg = addr;
            wire_request.count = cnt;
        }
        ModBusRequest::WriteCoil { addr, values } => {
            wire_request.func = MODBUS_SET_COIL;
            wire_request.reg = addr;
            wire_request.count = values.len() as u16;
        }
        ModBusRequest::WriteRegister { addr, data } => {
            wire_request.func = MODBUS_SET_HOLDINGS_BULK;
            wire_request.reg = addr;
            wire_request.count = data.len() as u16;
        }
    }
    todo!()
}

pub fn crc(data: &[u8]) -> u16 {
    let mut crc = 0xFFFF_u16;
    for x in data {
        crc ^= *x as u16;
        for _ in 0..8 {
            if crc & 1 != 0 {
                crc = (crc >> 1) ^ 0xA001;
            } else {
                crc >>= 1;
            }
        }
    }
    crc
}

fn leading_request(req: rmodbus::client::ModbusRequest) -> Vec<u8> {
    let mut ret = Vec::new();
    if req.proto == ModbusProto::TcpUdp {
        ret.extend(&req.tr_id.to_be_bytes());
        ret.extend(&[0u8, 0, 0, 0]);
    }
    ret.extend(&[req.unit_id, req.func]);
    ret
}

fn finish_request(data: &mut Vec<u8>, req: rmodbus::client::ModbusRequest) -> crate::Result<()> {
    match req.proto {
        ModbusProto::TcpUdp => {
            assert!(data.len() >= 6);
            let mut l = data.len();
            l -= 6;
            if l > u16::MAX as usize {
                return Err(crate::Error::argument(anyhow!("ModBus frame over length.")));
            }
            let len_buf = (l as u16).to_be_bytes();
            data[4] = len_buf[0];
            data[5] = len_buf[0];
            Ok(())
        }
        ModbusProto::Rtu => {
            let l = data.len();
            if l > u8::MAX as usize {
                return Err(crate::Error::argument(anyhow!("ModBus frame over length.")));
            }
            let crc = crc(&data);
            data.extend(&crc.to_le_bytes());
            Ok(())
        }
        ModbusProto::Ascii => unreachable!(),
    }
}
