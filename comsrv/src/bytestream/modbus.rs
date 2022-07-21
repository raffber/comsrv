use anyhow::anyhow;
use comsrv_protocol::{ModBusProtocol, ModBusRequest, ModBusResponse};
use rmodbus::{
    ModbusProto, MODBUS_GET_COILS, MODBUS_GET_DISCRETES, MODBUS_GET_INPUTS, MODBUS_SET_COIL, MODBUS_SET_HOLDING,
    MODBUS_SET_HOLDINGS_BULK,
};
use std::time::Duration;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

const CUSTOM_DDP_FUNCTION: u8 = 0x44;

use super::read_all;

fn into_rmodbus_proto(proto: ModBusProtocol) -> ModbusProto {
    match proto {
        ModBusProtocol::Tcp => ModbusProto::TcpUdp,
        ModBusProtocol::Rtu => ModbusProto::Rtu,
    }
}

struct TransactionInfo {
    transaction_id: u16,
    station_address: u8,
    protocol: ModBusProtocol,
    timeout: Duration,
}

impl TransactionInfo {
    fn new(station_address: u8, protocol: ModBusProtocol, timeout: Duration) -> Self {
        Self {
            transaction_id: rand::random(),
            station_address,
            protocol,
            timeout,
        }
    }
}

pub async fn handle<T: AsyncRead + AsyncWrite + Unpin>(
    timeout: Duration,
    station_address: u8,
    protocol: ModBusProtocol,
    request: ModBusRequest,
    stream: &mut T,
) -> crate::Result<ModBusResponse> {
    read_all(stream).await.map_err(crate::Error::transport)?;
    let transaction = TransactionInfo::new(station_address, protocol.clone(), timeout);
    // TODO: timeout the whole thing

    match request {
        ModBusRequest::Ddp {
            sub_cmd,
            mut ddp_cmd,
            response,
            data,
        } => {
            let mut request = start_request(&transaction, CUSTOM_DDP_FUNCTION);
            if response {
                ddp_cmd |= 0x80;
            }
            request.push(sub_cmd);
            request.push((data.len() + 1) as u8);
            request.push(ddp_cmd);
            request.extend(data);
            finish_request(transaction.protocol, &mut request)?;
            stream.write_all(&request).await.map_err(crate::Error::transport)?;
            match protocol {
                ModBusProtocol::Tcp => {
                    let ret = tcp_query(&transaction, CUSTOM_DDP_FUNCTION, stream).await?;
                    Ok(ModBusResponse::Data(ret))
                }
                ModBusProtocol::Rtu => {
                    let ret = ddp_rtu(stream, station_address, CUSTOM_DDP_FUNCTION, sub_cmd, response).await?;
                    Ok(ModBusResponse::Data(ret))
                }
            }
        }
        ModBusRequest::ReadCoil { addr, cnt } => todo!(),
        ModBusRequest::ReadDiscrete { addr, cnt } => todo!(),
        ModBusRequest::ReadInput { addr, cnt } => todo!(),
        ModBusRequest::ReadHolding { addr, cnt } => todo!(),
        ModBusRequest::WriteCoil { addr, values } => todo!(),
        ModBusRequest::WriteRegister { addr, data } => todo!(),
    }
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

fn start_request(info: &TransactionInfo, function_code: u8) -> Vec<u8> {
    let mut ret = Vec::new();
    if info.protocol == ModBusProtocol::Tcp {
        ret.extend(&info.transaction_id.to_be_bytes());
        ret.extend(&[0u8, 0, 0, 0]);
    }
    ret.extend(&[info.station_address, function_code]);
    ret
}

fn finish_request(protocol: ModBusProtocol, data: &mut Vec<u8>) -> crate::Result<()> {
    match protocol {
        ModBusProtocol::Tcp => {
            assert!(data.len() >= 6);
            let mut l = data.len();
            l -= 6;
            if l > u16::MAX as usize {
                return Err(crate::Error::argument(anyhow!("ModBus frame over length.")));
            }
            let len_buf = (l as u16).to_be_bytes();
            data[4] = len_buf[0];
            data[5] = len_buf[1];
            Ok(())
        }
        ModBusProtocol::Rtu => {
            let l = data.len();
            if l > u8::MAX as usize {
                return Err(crate::Error::argument(anyhow!("ModBus frame over length.")));
            }
            let crc = crc(&data);
            data.extend(&crc.to_le_bytes());
            Ok(())
        }
    }
}

async fn tcp_query<T: AsyncRead + AsyncWrite + Unpin>(
    transaction: &TransactionInfo,
    function_code: u8,
    stream: &mut T,
) -> crate::Result<Vec<u8>> {
    let mut header = [0_u8; 8];
    stream.read_exact(&mut header).await.map_err(crate::Error::transport)?;
    let transaction_id = u16::from_be_bytes([header[0], header[1]]);
    let proto = u16::from_be_bytes([header[2], header[3]]);
    let len = u16::from_be_bytes([header[4], header[5]]);
    let station_address = header[6];
    let parsed_function_code = header[7];
    if station_address != transaction.station_address
        || function_code != parsed_function_code
        || transaction_id != transaction.transaction_id
        || proto != 0
        || len < 2
    {
        return Err(crate::Error::protocol(anyhow!("Invalid answer received.")));
    }
    let mut buf = vec![0_u8; (len - 2) as usize];
    stream.read_exact(&mut buf).await.map_err(crate::Error::transport)?;
    Ok(buf)
}

async fn register_query<T: AsyncRead + AsyncWrite + Unpin>(
    transaction: &TransactionInfo,
    function_code: u8,
    request: &[u8],
    stream: &mut T,
) -> crate::Result<Vec<u8>> {
    stream.write_all(&request).await.map_err(crate::Error::transport)?;
    match transaction.protocol {
        ModBusProtocol::Tcp => tcp_query(transaction, function_code, stream).await,
        ModBusProtocol::Rtu => {
            let mut header = [0_u8; 3];
            stream.read_exact(&mut header).await.map_err(crate::Error::transport)?;
            let station_address = header[0];
            let parsed_function_code = header[1];
            if function_code != parsed_function_code || station_address != transaction.station_address {
                return Err(crate::Error::protocol(anyhow!("Invalid answer received.")));
            }
            let len = header[2];
            let mut buf = vec![0_u8; (len + 5) as usize];
            stream.read_exact(&mut buf[3..]).await.map_err(crate::Error::transport)?;
            buf[0..3].copy_from_slice(&header);
            if crc(&buf) != 0 {
                return Err(crate::Error::protocol(anyhow!("Invalid CRC in answer")));
            }
            Ok(buf[3..buf.len() - 2].to_vec())
        }
    }
}

async fn ddp_rtu<T: AsyncRead + AsyncWrite + Unpin>(
    stream: &mut T,
    station_address: u8,
    function_code: u8,
    sub_cmd: u8,
    response: bool,
) -> crate::Result<Vec<u8>> {
    let mut data = vec![0_u8; 300];
    stream.read_exact(&mut data[0..4]).await?;
    if data[0] != station_address || data[1] != function_code || data[2] != sub_cmd {
        return Err(crate::Error::protocol(anyhow!("Invalid Response")));
    }
    if !response {
        return Ok(vec![]);
    }
    let len = data[3];
    if len == 0 {
        return Err(crate::Error::protocol(anyhow!("Invalid Response")));
    }
    stream.read_exact(&mut data[4..6 + len as usize]).await?;

    if crc(&data[0..6 + len as usize]) != 0 {
        return Err(crate::Error::protocol(anyhow!("Invalid Response")));
    }
    let reply = &data[4..4 + len as usize];
    Ok(reply.to_vec())
}
