use crate::{Address, Duration};
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

#[derive(Copy, Clone, Serialize, Deserialize, Debug, Hash, PartialEq, Eq)]
pub enum FlowControl {
    NoFlowControl,
    Hardware,
    Software,
}

impl Default for FlowControl {
    fn default() -> Self {
        FlowControl::NoFlowControl
    }
}

impl FlowControl {
    fn has_no_flow_control(&self) -> bool {
        *self == FlowControl::NoFlowControl
    }
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct SerialPortConfig {
    pub config: String,
    pub baudrate: u32,
    #[serde(skip_serializing_if = "FlowControl::has_no_flow_control", default)]
    pub hardware_flow_control: FlowControl,
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

impl From<SerialInstrument> for SerialAddress {
    fn from(val: SerialInstrument) -> Self {
        val.address
    }
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct FtdiInstrument {
    pub address: FtdiAddress,
    pub port_config: SerialPortConfig,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub options: Option<SerialOptions>,
}

impl From<FtdiInstrument> for FtdiAddress {
    fn from(val: FtdiInstrument) -> Self {
        val.address
    }
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

impl From<TcpInstrument> for TcpAddress {
    fn from(val: TcpInstrument) -> Self {
        val.address
    }
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub enum ByteStreamInstrument {
    Serial(SerialInstrument),
    Ftdi(FtdiInstrument),
    Tcp(TcpInstrument),
}

impl From<ByteStreamInstrument> for Address {
    fn from(val: ByteStreamInstrument) -> Self {
        match val {
            ByteStreamInstrument::Serial(x) => Address::Serial(x.into()),
            ByteStreamInstrument::Ftdi(x) => Address::Ftdi(x.into()),
            ByteStreamInstrument::Tcp(x) => Address::Tcp(x.into()),
        }
    }
}

impl ByteStreamInstrument {
    pub fn address(&self) -> Address {
        match self {
            ByteStreamInstrument::Serial(x) => Address::Serial(x.address.clone()),
            ByteStreamInstrument::Ftdi(x) => Address::Ftdi(x.address.clone()),
            ByteStreamInstrument::Tcp(x) => Address::Tcp(x.address.clone()),
        }
    }
}

#[derive(Copy, Clone, Serialize, Deserialize, Debug, Eq, PartialEq)]
pub enum ModBusProtocol {
    Tcp,
    Rtu,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub enum ByteStreamRequest {
    Connect,
    Disconnect,
    Write(Vec<u8>),
    ReadToTerm {
        term: u8,
        timeout: Duration,
    },
    ReadExact {
        count: u32,
        timeout: Duration,
    },
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
        cnt: u8,
    },
    ReadHolding {
        addr: u16,
        cnt: u8,
    },
    WriteCoils {
        addr: u16,
        values: Vec<bool>,
    },
    WriteRegisters {
        addr: u16,
        values: Vec<u16>,
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
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct FtdiDeviceInfo {
    pub port_open: bool,
    pub vendor_id: u16,
    pub product_id: u16,
    pub serial_number: String,
    pub description: String,
}
