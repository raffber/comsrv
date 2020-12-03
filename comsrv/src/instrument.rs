use std::fmt;
use std::fmt::{Display, Formatter};
use std::hash::Hash;
use std::net::SocketAddr;

use async_std::net::IpAddr;
use serde::{Deserialize, Serialize};

use crate::app::Server;
use crate::can::{CanAddress, Instrument as CanInstrument};
use crate::modbus::Instrument as ModBusInstrument;
use crate::serial::Instrument as SerialInstrument;
use crate::serial::SerialParams;
use crate::tcp::Instrument as TcpInstrument;
use crate::visa::asynced::Instrument as VisaInstrument;
use crate::visa::VisaOptions;
use crate::vxi::Instrument as VxiInstrument;
use crate::Error;

#[derive(Clone, Serialize, Deserialize)]
pub enum InstrumentOptions {
    Visa(VisaOptions),
    Default,
}

impl Default for InstrumentOptions {
    fn default() -> Self {
        InstrumentOptions::Default
    }
}

impl InstrumentOptions {
    pub fn is_default(&self) -> bool {
        matches!(self, InstrumentOptions::Default)
    }
}

#[derive(Clone)]
pub enum Instrument {
    Visa(VisaInstrument),
    Modbus(ModBusInstrument),
    Serial(SerialInstrument),
    Tcp(TcpInstrument),
    Vxi(VxiInstrument),
    Can(CanInstrument),
}

#[derive(Hash, PartialEq, Eq)]
pub struct HandleId {
    inner: String,
}

impl HandleId {
    fn new(inner: String) -> Self {
        Self { inner }
    }
}

impl ToString for HandleId {
    fn to_string(&self) -> String {
        self.inner.clone()
    }
}

#[derive(Clone, Hash)]
pub enum Address {
    Visa { splits: Vec<String> },
    Serial { path: String, params: SerialParams },
    Prologix { file: String, gpib: u8 },
    Modbus { addr: SocketAddr },
    Vxi { addr: IpAddr },
    Tcp { addr: SocketAddr },
    Can { addr: CanAddress },
}

impl Address {
    pub fn parse(addr: &str) -> crate::Result<Self> {
        let splits: Vec<_> = addr.split("::").map(|x| x.to_string()).collect();
        if splits.len() < 2 {
            return Err(Error::InvalidAddress);
        }

        if splits[0].to_lowercase() == "modbus" {
            // modbus::192.168.0.1:1234
            // TODO: move to URL?
            let addr = &splits[1].to_lowercase();
            let addr: SocketAddr = addr.parse().map_err(|_| Error::InvalidAddress)?;
            Ok(Address::Modbus { addr })
        } else if splits[0].to_lowercase() == "prologix" {
            // prologix::/dev/ttyUSB0::12
            if splits.len() != 3 {
                return Err(Error::InvalidAddress);
            }
            let serial_addr = &splits[1];
            let addr: u8 = splits[2].parse().map_err(|_| Error::InvalidAddress)?;
            Ok(Address::Prologix {
                file: serial_addr.to_string(),
                gpib: addr,
            })
        } else if splits[0].to_lowercase() == "serial" {
            // serial::/dev/ttyUSB0::9600::8N1
            let (path, params) = SerialParams::from_string(&addr).ok_or(Error::InvalidAddress)?;
            Ok(Address::Serial { path, params })
        } else if splits[0].to_lowercase() == "tcp" {
            // tcp::192.168.0.1:1234
            let addr = &splits[1].to_lowercase();
            let addr: SocketAddr = addr.parse().map_err(|_| Error::InvalidAddress)?;
            Ok(Address::Tcp { addr })
        } else if splits[0].to_lowercase() == "vxi" {
            // vxi::192.168.0.1:1234
            let addr = &splits[1].to_lowercase();
            let addr: IpAddr = addr.parse().map_err(|_| Error::InvalidAddress)?;
            Ok(Address::Vxi { addr })
        } else if splits[0].to_lowercase() == "can" {
            // can::socket::can0 or can::loopback or can::pcan::usb1
            let kind = &splits[1].to_lowercase();
            let can_addr = if kind == "socket" {
                if splits.len() < 3 {
                    return Err(Error::InvalidAddress);
                }
                CanAddress::Socket(splits[2].to_lowercase())
            } else if kind == "loopback" {
                CanAddress::Loopback
            } else if kind == "pcan" {
                if splits.len() < 4 {
                    return Err(Error::InvalidAddress);
                }
                let ifname = splits[2].to_lowercase();
                let bitrate: u32 = splits[3]
                    .to_lowercase()
                    .parse()
                    .map_err(|_| Error::InvalidAddress)?;
                CanAddress::PCan { ifname, bitrate }
            } else {
                return Err(Error::InvalidAddress);
            };
            Ok(Address::Can { addr: can_addr })
        } else {
            let splits: Vec<_> = splits
                .iter()
                .map(|x| x.to_lowercase().to_string())
                .collect();
            Ok(Address::Visa { splits })
        }
    }

    pub fn handle_id(&self) -> HandleId {
        match self {
            Address::Visa { splits } => {
                let id = format!("{}::{}", splits[0], splits[1]);
                HandleId::new(id)
            }
            Address::Serial { path, .. } => HandleId::new(path.clone()),
            Address::Prologix { file, .. } => HandleId::new(file.clone()),
            Address::Modbus { addr } => HandleId::new(addr.to_string()),
            Address::Vxi { addr } => HandleId::new(addr.to_string()),
            Address::Tcp { addr } => HandleId::new(addr.to_string()),
            Address::Can { addr } => HandleId::new(addr.interface()),
        }
    }
}

impl Into<String> for Address {
    fn into(self) -> String {
        match self {
            Address::Visa { splits } => splits.join("::"),
            Address::Serial { path, params } => format!(
                "serial::{}::{}::{}{}{}",
                path, params.baud, params.data_bits, params.parity, params.stop_bits
            ),
            Address::Prologix { file, gpib } => format!("prologix::{}::{}", file, gpib),
            Address::Modbus { addr } => format!("modbus::{}", addr),
            Address::Tcp { addr } => format!("tcp::{}", addr),
            Address::Vxi { addr } => format!("vxi::{}", addr),
            Address::Can { addr } => format!("can::{}", addr),
        }
    }
}

impl Display for Address {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let x: String = self.clone().into();
        f.write_str(&x)
    }
}

impl Instrument {
    pub fn connect(server: &Server, addr: &Address) -> Instrument {
        match addr {
            Address::Visa { splits } => {
                let addr = splits.join("::");
                let instr = VisaInstrument::connect(addr);
                Instrument::Visa(instr)
            }
            Address::Serial { path, .. } => {
                let instr = SerialInstrument::new(path.clone());
                Instrument::Serial(instr)
            }
            Address::Prologix { file, .. } => {
                let instr = SerialInstrument::new(file.clone());
                Instrument::Serial(instr)
            }
            Address::Modbus { addr } => Instrument::Modbus(ModBusInstrument::new(addr.clone())),
            Address::Tcp { addr } => Instrument::Tcp(TcpInstrument::new(addr.clone())),
            Address::Vxi { addr } => Instrument::Vxi(VxiInstrument::new(addr.clone())),
            Address::Can { addr } => Instrument::Can(CanInstrument::new(server, addr.clone())),
        }
    }

    pub fn disconnect(self) {
        match self {
            Instrument::Visa(x) => x.disconnect(),
            Instrument::Modbus(x) => x.disconnect(),
            Instrument::Serial(x) => x.disconnect(),
            Instrument::Tcp(x) => x.disconnect(),
            Instrument::Vxi(x) => x.disconnect(),
            Instrument::Can(x) => x.disconnect(),
        }
    }
}
