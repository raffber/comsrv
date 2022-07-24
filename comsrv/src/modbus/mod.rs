mod tcp;
mod rtu;
mod registers;
mod ddp;

use comsrv_protocol::{ModBusProtocol, ModBusRequest, ModBusResponse};

use std::time::Duration;
use thiserror::Error;
use tokio::io::{AsyncRead, AsyncWrite};
use crate::modbus::ddp::Ddp;
use crate::modbus::function_codes::{READ_COILS, READ_DISCRETES, READ_HOLDINGS, READ_INPUTS};
use crate::modbus::registers::{ReadBoolRegisters, ReadU16Registers, WriteCoils, WriteRegisters};
use crate::modbus::rtu::RtuHandler;
use crate::modbus::tcp::TcpHandler;

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

pub trait FunctionCode {
    type Output;

    fn format_request(&self, data: &mut Vec<u8>);
    fn get_header_length(&self) -> usize;
    fn get_data_length_from_header(&self, data: &[u8]) -> crate::Result<usize>;
    fn parse_frame(&self, data: &[u8]) -> crate::Result<Self::Output>;

    fn function_code(&self) -> u8;
}

enum Handler<T: FunctionCode> {
    Tcp(TcpHandler<T>),
    Rtu(RtuHandler<T>),
}

impl<T: FunctionCode> Handler<T> {
    fn new(protocol: ModBusProtocol, function_code: T) -> Self {
        match protocol {
            ModBusProtocol::Tcp => Self::Tcp(TcpHandler::new(function_code)),
            ModBusProtocol::Rtu => Self::Rtu(RtuHandler::new(function_code)),
        }
    }
    async fn handle<S: AsyncRead + AsyncWrite + Unpin>(&self, stream: &mut S, timeout: Duration, transaction: &TransactionInfo) -> crate::Result<T::Output> {
        match tokio::time::timeout(timeout, self.handle_no_timeout(stream, transaction)).await {
            Ok(x) => x,
            Err(_) => Err(crate::Error::protocol_timeout())
        }
    }

    async fn handle_no_timeout<S: AsyncRead + AsyncWrite + Unpin>(&self, stream: &mut S, transaction: &TransactionInfo) -> crate::Result<T::Output> {
        match self {
            Handler::Tcp(x) => x.handle(&transaction, stream).await,
            Handler::Rtu(x) => x.handle(&transaction, stream).await,
        }
    }
}


pub struct TransactionInfo {
    transaction_id: u16,
    station_address: u8,
    protocol: ModBusProtocol,
    timeout: Duration,
}

impl TransactionInfo {
    pub fn new(station_address: u8, protocol: ModBusProtocol, timeout: Duration) -> Self {
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
    crate::bytestream::read_all(stream).await.map_err(crate::Error::transport)?;
    let transaction = TransactionInfo::new(station_address, protocol.clone(), timeout);
    let ret = match request {
        ModBusRequest::Ddp { sub_cmd, ddp_cmd, response, data } => {
            let fun_code = Ddp::new(ddp_cmd, sub_cmd, data, response)?;
            let ret = Handler::new(protocol, fun_code).handle(stream, timeout, &transaction).await?;
            ModBusResponse::Data(ret)
        }
        ModBusRequest::ReadCoil { addr, cnt } => {
            let fun_code = ReadBoolRegisters::new(READ_COILS, addr, cnt)?;
            let ret = Handler::new(protocol, fun_code).handle(stream, timeout, &transaction).await?;
            ModBusResponse::Bool(ret)
        }
        ModBusRequest::ReadDiscrete { addr, cnt } => {
            let fun_code = ReadBoolRegisters::new(READ_DISCRETES, addr, cnt)?;
            let ret = Handler::new(protocol, fun_code).handle(stream, timeout, &transaction).await?;
            ModBusResponse::Bool(ret)
        }
        ModBusRequest::ReadInput { addr, cnt } => {
            let fun_code = ReadU16Registers::new(READ_INPUTS, addr, cnt)?;
            let ret = Handler::new(protocol, fun_code).handle(stream, timeout, &transaction).await?;
            ModBusResponse::Number(ret)
        }
        ModBusRequest::ReadHolding { addr, cnt } => {
            let fun_code = ReadU16Registers::new(READ_HOLDINGS, addr, cnt)?;
            let ret = Handler::new(protocol, fun_code).handle(stream, timeout, &transaction).await?;
            ModBusResponse::Number(ret)
        }
        ModBusRequest::WriteCoils { addr, values } => {
            let fun_code = WriteCoils::new(addr, &values)?;
            Handler::new(protocol, fun_code).handle(stream, timeout, &transaction).await?;
            ModBusResponse::Done
        }
        ModBusRequest::WriteRegisters { addr, values } => {
            let fun_code = WriteRegisters::new(addr, &values)?;
            Handler::new(protocol, fun_code).handle(stream, timeout, &transaction).await?;
            ModBusResponse::Done
        }
    };
    Ok(ret)
}