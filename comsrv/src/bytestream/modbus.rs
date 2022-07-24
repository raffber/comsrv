use anyhow::anyhow;
use comsrv_protocol::{ModBusProtocol, ModBusRequest, ModBusResponse};

use std::marker::PhantomData;
use std::time::Duration;
use thiserror::Error;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

mod function_codes {
    pub const READ_COILS: u8 = 1;
    pub const READ_DISCRETES: u8 = 2;
    pub const READ_HOLDINGS: u8 = 3;
    pub const READ_INPUTS: u8 = 4;
    pub const WRITE_COIL: u8 = 5;
    pub const WRITE_HOLDING: u8 = 6;
    pub const WRITE_MULTIPLE_COILS: u8 = 15;
    pub const WRITE_MULTIPLE_HOLDINGS: u8 = 16;
    pub const CUSTOM_DDP: u8 = 0x44;
}

use super::read_all;

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
            let function_code = function_codes::CUSTOM_DDP;
            let mut request = start_request(&transaction, function_code);
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
                    let ret = tcp_query(&transaction, function_code, stream).await?;
                    Ok(ModBusResponse::Data(ret))
                }
                ModBusProtocol::Rtu => {
                    let ret = ddp_rtu(stream, station_address, function_code, sub_cmd, response).await?;
                    Ok(ModBusResponse::Data(ret))
                }
            }
        }
        ModBusRequest::ReadCoil { addr, cnt } => {
            if cnt == 0 {
                return Err(crate::Error::argument(anyhow!("Number of read coils must be > 0")));
            }
            let function_code = function_codes::READ_COILS;
            let mut request = start_request(&transaction, function_code);
            request.extend(&addr.to_be_bytes());
            request.extend(&cnt.to_be_bytes());
            let reply = register_query(&transaction, function_code, &request, stream).await?;
            let ret = parse_bool_message(&reply, cnt as usize)?;
            Ok(ModBusResponse::Bool(ret))
        }
        ModBusRequest::ReadDiscrete { addr, cnt } => {
            if cnt == 0 {
                return Err(crate::Error::argument(anyhow!("Number of read coils must be > 0")));
            }
            let function_code = function_codes::READ_DISCRETES;
            let mut request = start_request(&transaction, function_code);
            request.extend(&addr.to_be_bytes());
            request.extend(&cnt.to_be_bytes());
            let reply = register_query(&transaction, function_code, &request, stream).await?;
            let ret = parse_bool_message(&reply, cnt as usize)?;
            Ok(ModBusResponse::Bool(ret))
        }
        ModBusRequest::ReadInput { addr, cnt } => {
            if cnt == 0 {
                return Err(crate::Error::argument(anyhow!("Number of read coils must be > 0")));
            }
            let function_code = function_codes::READ_INPUTS;
            let mut request = start_request(&transaction, function_code);
            request.extend(&addr.to_be_bytes());
            request.extend(&cnt.to_be_bytes());
            let reply = register_query(&transaction, function_code, &request, stream).await?;
            let ret = parse_u16_message(&reply, cnt as usize)?;
            Ok(ModBusResponse::Number(ret))
        }
        ModBusRequest::ReadHolding { addr, cnt } => {
            if cnt == 0 {
                return Err(crate::Error::argument(anyhow!("Number of read coils must be > 0")));
            }
            let function_code = function_codes::READ_INPUTS;
            let mut request = start_request(&transaction, function_code);
            request.extend(&addr.to_be_bytes());
            request.extend(&cnt.to_be_bytes());
            let reply = register_query(&transaction, function_code, &request, stream).await?;
            let ret = parse_u16_message(&reply, cnt as usize)?;
            Ok(ModBusResponse::Number(ret))
        }
        ModBusRequest::WriteCoils { addr, values } => {
            write_coils(stream, &transaction, addr, values).await?;
            Ok(ModBusResponse::Done)
        }
        ModBusRequest::WriteRegister { addr, data } => {
            write_registers(stream, &transaction, addr, values).await?;
            Ok(ModBusResponse::Done)
        }
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

fn parse_bool_message(reply: &[u8], cnt: usize) -> crate::Result<Vec<bool>> {
    assert!(cnt > 0);
    let expected_byte_count = ((cnt - 1) / 8) + 1;
    if reply.len() < expected_byte_count {
        return Err(crate::Error::protocol(anyhow!("Invalid receive frame length")));
    }
    let mut ret = Vec::new();
    'outer: for x in reply {
        let mut x = *x;
        for _ in 0..8 {
            ret.push((x & 1) == 1);
            if ret.len() == expected_byte_count {
                break 'outer;
            }
            x = x >> 1;
        }
    }
    Ok(ret)
}

fn parse_u16_message(reply: &[u8], cnt: usize) -> crate::Result<Vec<u16>> {
    assert!(cnt > 0);
    let expected_byte_count = 2 * cnt;
    if reply.len() < expected_byte_count {
        return Err(crate::Error::protocol(anyhow!("Invalid receive frame length")));
    }
    let mut ret = Vec::with_capacity(cnt);
    for x in reply.chunks(2).take(cnt) {
        let value = u16::from_be_bytes([x[0], x[1]]);
        ret.push(value);
    }
    Ok(ret)
}

async fn write_coils<T: AsyncRead + AsyncWrite + Unpin>(
    stream: &mut T,
    transaction: &TransactionInfo,
    addr: u16,
    values: Vec<bool>,
) -> crate::Result<()> {
    if values.len() == 0 {
        return Err(crate::Error::argument(anyhow!("Number of write coils must be > 0")));
    }
    if values.len() > 0x7B0 {
        return Err(crate::Error::argument(anyhow!("Number of write coils must be <= 1968")));
    }
    let function_code = function_codes::WRITE_MULTIPLE_COILS;
    let mut request = start_request(&transaction, function_code);
    request.extend(addr.to_be_bytes());
    request.extend((values.len() as u16).to_be_bytes());
    for chunk in values.chunks(8) {
        let mut byte: u8 = 0;
        let mut k = 0;
        for x in chunk {
            if *x {
                byte |= 1 << k;
            }
            k += 1;
        }
        request.push(byte);
    }
    finish_request(transaction.protocol, &mut request)?;
    stream.write_all(&request).await.map_err(crate::Error::transport)?;
    read_write_register_reponse(stream, transaction, addr, values.len(), function_code).await?;
    Ok(())
}

async fn read_write_register_reponse<T: AsyncRead + AsyncWrite + Unpin>(
    stream: &mut T,
    transaction: &TransactionInfo,
    addr: u16,
    num_values: usize,
    function_code: u8,
) -> crate::Result<()> {
    let reply = match transaction.protocol {
        ModBusProtocol::Tcp => {
            let reply = tcp_query(&transaction, function_code, stream).await?;
            // TODO: error check first
            if reply.len() != 4 {
                return Err(crate::Error::protocol(anyhow!("Invalid Frame.")));
            }
            reply
        }
        ModBusProtocol::Rtu => {
            // TODO: error check first
            let mut header = [0_u8; 6];
            stream.read_exact(&mut header).await.map_err(crate::Error::transport)?;
            let station_address = header[0];
            let parsed_function_code = header[1];
            if function_code != parsed_function_code || station_address != transaction.station_address {
                return Err(crate::Error::protocol(anyhow!("Invalid answer received.")));
            }
            header[2..].to_vec()
        }
    };
    let starting_address = u16::from_be_bytes([reply[0], reply[1]]);
    let num_outputs = u16::from_be_bytes([reply[2], reply[3]]);
    if num_outputs as usize != num_values || starting_address != addr {
        return Err(crate::Error::protocol(anyhow!("Invalid answer received.")));
    }
    Ok(())
}

async fn write_registers<T: AsyncRead + AsyncWrite + Unpin>(
    stream: &mut T,
    transaction: &TransactionInfo,
    addr: u16,
    values: Vec<u16>,
) -> crate::Result<()> {
    if values.len() == 0 {
        return Err(crate::Error::argument(anyhow!("Number of write coils must be > 0")));
    }
    if values.len() > 0x7B0 {
        return Err(crate::Error::argument(anyhow!("Number of write coils must be <= 1968")));
    }
    let function_code = function_codes::WRITE_MULTIPLE_COILS;
    let mut request = start_request(&transaction, function_code);
    request.extend(addr.to_be_bytes());
    request.extend((values.len() as u16).to_be_bytes());
    for value in &values {
        request.extend(value.to_be_bytes());
    }
    finish_request(transaction.protocol, &mut request)?;
    stream.write_all(&request).await.map_err(crate::Error::transport)?;
    read_write_register_reponse(stream, transaction, addr, values.len(), function_code).await?;
    Ok(())
}

#[derive(Error, Debug)]
enum ModBusException {
    #[error("InvalidData")]
    InvalidData,
}

#[derive(Error, Debug)]
enum ModBusFrameError {
    #[error("Not enough data")]
    NotEnoughData,
}

trait FunctionCode {
    type Output;

    fn get_header_length(data: &[u8]) -> crate::Result<usize>;
    fn get_data_length_from_header(data: &[u8]) -> crate::Result<usize>;
    fn parse_frame(data: &[u8]) -> crate::Result<Self::Output>;
}

struct TcpHandler<T: FunctionCode> {
    marker: PhantomData<T>,
}
struct RtuHandler<T: FunctionCode> {
    marker: PhantomData<T>,
}
