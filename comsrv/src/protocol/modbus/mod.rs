/// ModBus protocol implementation for ModBus TCP and RTU
///
/// Note that the ModBus protocol implementation all operate on bytestreams (i.e. `AsyncRead + AsyncWrite`). On a typical OS it is not
/// possible to implement ModBus RTU with timer-based framing.
mod ddp;
mod registers;
mod rtu;
mod tcp;

use comsrv_protocol::{ModBusProtocol, ModBusRequest, ModBusResponse};

use ddp::Ddp;
use function_codes::{READ_COILS, READ_DISCRETES, READ_HOLDINGS, READ_INPUTS};
use registers::{ReadBoolRegisters, ReadU16Registers, WriteCoils, WriteRegisters};
use rtu::RtuHandler;
use std::time::Duration;
use tcp::TcpHandler;
use thiserror::Error;
use tokio::io::{AsyncRead, AsyncWrite};

mod function_codes {
    #![allow(dead_code)]

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
pub enum ModBusException {
    #[error("InvalidFunction")]
    InvalidFunction,
    #[error("Invalid Data Address")]
    InvalidDataAddress,
    #[error("Invalid Data Value")]
    InvalidDataValue,
    #[error("Server Device Failure")]
    ServerDeviceFailure,
    #[error("Acknowledge")]
    Acknowledge,
    #[error("Server Device Busy")]
    ServerDeviceBusy,
    #[error("Negative Acknowledgement")]
    NegativeAcknowledgement,
    #[error("Memory Parity Error")]
    MemoryParityError,
    #[error("Gateway Path Unavailable")]
    GatewayPathUnavailable,
    #[error("Gateway Target Device Failed to Respond")]
    GatewayTargetFailedToRespond,
    #[error("Unknown Exception Code: {0}")]
    Unknown(u8),
}

impl ModBusException {
    pub fn from_code(code: u8) -> Self {
        match code {
            1 => ModBusException::InvalidFunction,
            2 => ModBusException::InvalidDataAddress,
            3 => ModBusException::InvalidDataValue,
            4 => ModBusException::ServerDeviceFailure,
            5 => ModBusException::Acknowledge,
            6 => ModBusException::ServerDeviceBusy,
            7 => ModBusException::NegativeAcknowledgement,
            8 => ModBusException::MemoryParityError,
            10 => ModBusException::GatewayPathUnavailable,
            11 => ModBusException::GatewayTargetFailedToRespond,
            x => ModBusException::Unknown(x),
        }
    }
}

/// Function code handler. Both the RTU and the TCP handler implementations
/// get the necessary information from these handlers to perform the framing
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
    async fn handle<S: AsyncRead + AsyncWrite + Unpin>(
        &self,
        stream: &mut S,
        timeout: Duration,
        transaction: &TransactionInfo,
    ) -> crate::Result<T::Output> {
        match tokio::time::timeout(timeout, self.handle_no_timeout(stream, transaction)).await {
            Ok(x) => x,
            Err(_) => Err(crate::Error::protocol_timeout()),
        }
    }

    async fn handle_no_timeout<S: AsyncRead + AsyncWrite + Unpin>(
        &self,
        stream: &mut S,
        transaction: &TransactionInfo,
    ) -> crate::Result<T::Output> {
        match self {
            Handler::Tcp(x) => x.handle(transaction, stream).await,
            Handler::Rtu(x) => x.handle(transaction, stream).await,
        }
    }
}

pub struct TransactionInfo {
    transaction_id: u16,
    station_address: u8,
}

impl TransactionInfo {
    pub fn new(station_address: u8) -> Self {
        Self {
            transaction_id: rand::random(),
            station_address,
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
    crate::protocol::bytestream::read_all(stream)
        .await
        .map_err(crate::Error::transport)?;
    let transaction = TransactionInfo::new(station_address);
    let ret = match request {
        ModBusRequest::Ddp {
            sub_cmd,
            ddp_cmd,
            response,
            data,
        } => {
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
