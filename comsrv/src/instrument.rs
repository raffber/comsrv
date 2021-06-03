use std::fmt;
use std::fmt::{Display, Formatter};
use std::hash::Hash;
use std::net::SocketAddr;

use serde::{Deserialize, Serialize};
use std::net::IpAddr;

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

#[derive(Hash, Clone, PartialEq, Eq)]
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
    Modbus { addr: SocketAddr, slave_id: u8 },
    Vxi { addr: IpAddr },
    Tcp { addr: SocketAddr },
    Can { addr: CanAddress },
    Sigrok { device: String },
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
            let slave_id: u8 = if splits.len() == 3 {
                // slave id was also provided
                splits[2].parse().map_err(|_| Error::InvalidAddress)?
            } else {
                255
            };
            Ok(Address::Modbus { addr, slave_id })
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
            if splits.len() < 4 {
                return Err(Error::InvalidAddress);
            }
            let new_splits: Vec<&str> = splits.iter().map(|x| x.as_ref()).collect();
            let (path, params) = SerialParams::from_address(&new_splits[1..4])
                .ok_or(Error::InvalidAddress)?;
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
        } else if splits[0].to_lowercase() == "sigrok" {
            if splits.len() > 2 {
                return Err(Error::InvalidAddress);
            }
            Ok(Address::Sigrok {
                device: splits[1].clone(),
            })
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
            Address::Modbus { addr, .. } => HandleId::new(addr.to_string()),
            Address::Vxi { addr } => HandleId::new(addr.to_string()),
            Address::Tcp { addr } => HandleId::new(addr.to_string()),
            Address::Can { addr } => HandleId::new(addr.interface()),
            Address::Sigrok { device } => HandleId::new(device.to_string()),
        }
    }
}

impl Into<String> for Address {
    fn into(self) -> String {
        match self {
            Address::Visa { splits } => splits.join("::"),
            Address::Serial { path, params } =>
                format!("serial::{}::{}", path, params),
            Address::Prologix { file, gpib } => format!("prologix::{}::{}", file, gpib),
            Address::Modbus { addr, slave_id } => {
                if slave_id != 255 {
                    format!("modbus::{}::{}", addr, slave_id)
                } else {
                    format!("modbus::{}", addr)
                }
            },
            Address::Tcp { addr } => format!("tcp::{}", addr),
            Address::Vxi { addr } => format!("vxi::{}", addr),
            Address::Can { addr } => format!("can::{}", addr),
            Address::Sigrok { device } => format!("sigrok::{}", device),
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
    pub fn connect(server: &Server, addr: &Address) -> Option<Instrument> {
        let ret = match addr {
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
            Address::Modbus { addr, slave_id } => Instrument::Modbus(ModBusInstrument::new(addr.clone(), *slave_id)),
            Address::Tcp { addr } => Instrument::Tcp(TcpInstrument::new(addr.clone())),
            Address::Vxi { addr } => Instrument::Vxi(VxiInstrument::new(addr.clone())),
            Address::Can { addr } => Instrument::Can(CanInstrument::new(server, addr.clone())),
            Address::Sigrok { .. } => {
                return None;
            }
        };
        Some(ret)
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

    pub fn into_visa(self) -> Option<VisaInstrument> {
        match self {
            Instrument::Visa(instr) => Some(instr),
            _ => None,
        }
    }

    pub fn into_modbus(self) -> Option<ModBusInstrument> {
        match self {
            Instrument::Modbus(instr) => Some(instr),
            _ => None,
        }
    }

    pub fn into_serial(self) -> Option<SerialInstrument> {
        match self {
            Instrument::Serial(instr) => Some(instr),
            _ => None,
        }
    }

    pub fn into_tcp(self) -> Option<TcpInstrument> {
        match self {
            Instrument::Tcp(instr) => Some(instr),
            _ => None,
        }
    }

    pub fn into_vxi(self) -> Option<VxiInstrument> {
        match self {
            Instrument::Vxi(instr) => Some(instr),
            _ => None,
        }
    }

    pub fn into_can(self) -> Option<CanInstrument> {
        match self {
            Instrument::Can(instr) => Some(instr),
            _ => None,
        }
    }
}
