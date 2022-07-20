use crate::Duration;
use serde::{Deserialize, Serialize};

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct SerialOptions {
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub auto_drop: Option<Duration>,
}

#[derive(Clone, Serialize, Deserialize, Debug, Hash, PartialEq, Eq)]
pub struct FtdiAddress {
    pub port: String,
}

#[derive(Clone, Serialize, Deserialize, Debug, Hash, PartialEq, Eq)]
pub struct SerialAddress {
    pub port: String,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct SerialPortConfig {
    pub config: String,
    pub baudrate: u32,
}

#[derive(Clone, Serialize, Deserialize, Debug, Hash, PartialEq, Eq)]
pub struct TcpAddress {
    pub host: String,
    pub port: u16,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct SerialInstrument {
    pub address: SerialAddress,
    pub port_config: SerialPortConfig,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub options: Option<SerialOptions>,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct FtdiInstrument {
    pub address: FtdiAddress,
    pub port_config: SerialPortConfig,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub options: Option<SerialOptions>,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct TcpOptions {
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub auto_drop: Option<Duration>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub connection_timeout: Option<Duration>,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct TcpInstrument {
    pub address: TcpAddress,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub options: Option<TcpOptions>,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub enum ByteStreamInstrument {
    Serial(SerialInstrument),
    Ftdi(FtdiInstrument),
    Tcp(TcpInstrument),
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub enum ModBusProtocol {
    Tcp,
    Rtu,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub enum ByteStreamRequest {
    Write(Vec<u8>),
    ReadToTerm {
        term: u8,
        timeout: Duration,
    },
    ReadExact {
        count: u32,
        timeout: Duration,
    },
    ReadUpTo(u32),
    ReadAll,
    CobsWrite(Vec<u8>),
    CobsRead(Duration),
    CobsQuery {
        data: Vec<u8>,
        timeout: Duration,
    },
    WriteLine {
        line: String,
        term: u8,
    },
    ReadLine {
        timeout: Duration,
        term: u8,
    },
    QueryLine {
        line: String,
        timeout: Duration,
        term: u8,
    },
    ModBus {
        timeout: Duration,
        station_address: u8,
        protocol: ModBusProtocol,
        request: ModBusRequest,
    },
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub enum ModBusRequest {
    Ddp {
        sub_cmd: u8,
        ddp_cmd: u8,
        response: bool,
        data: Vec<u8>,
    },
    ReadCoil {
        addr: u16,
        cnt: u16,
    },
    ReadDiscrete {
        addr: u16,
        cnt: u16,
    },
    ReadInput {
        addr: u16,
        cnt: u16,
    },
    ReadHolding {
        addr: u16,
        cnt: u16,
    },
    WriteCoil {
        addr: u16,
        values: Vec<bool>,
    },
    WriteRegister {
        addr: u16,
        data: Vec<u16>,
    },
    CustomCommand {
        code: u8,
        data: Vec<u8>,
    },
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub enum ByteStreamResponse {
    Done,
    Data(Vec<u8>),
    String(String),
    ModBus(ModBusResponse),
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub enum ModBusResponse {
    Done,
    Number(Vec<u16>),
    Bool(Vec<bool>),
    Data(Vec<u8>),
    Custom { code: u8, data: Vec<u8> },
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct FtdiDeviceInfo {
    pub port_open: bool,
    pub vendor_id: u16,
    pub product_id: u16,
    pub serial_number: String,
    pub description: String,
}
