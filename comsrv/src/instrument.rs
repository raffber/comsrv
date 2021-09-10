use serde::{Deserialize, Serialize};

use crate::address::Address;
use crate::app::Server;
use crate::can::Instrument as CanInstrument;
use crate::hid::Instrument as HidInstrument;
use crate::modbus::{ModBusAddress, ModBusTcpInstrument, ModBusTransport};
use crate::serial::Instrument as SerialInstrument;
use crate::tcp::Instrument as TcpInstrument;
use crate::visa::asynced::Instrument as VisaInstrument;
use crate::visa::VisaOptions;
use crate::vxi::Instrument as VxiInstrument;

#[derive(Clone)]
pub enum Instrument {
    Visa(VisaInstrument),
    ModBusTcp(ModBusTcpInstrument),
    Serial(SerialInstrument),
    Tcp(TcpInstrument),
    Vxi(VxiInstrument),
    Can(CanInstrument),
    Hid(HidInstrument),
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
            Address::Modbus {
                addr, transport, ..
            } => match addr {
                ModBusAddress::Serial { path, .. } => {
                    let instr = SerialInstrument::new(path.clone());
                    Instrument::Serial(instr)
                }
                ModBusAddress::Tcp { addr } => match transport {
                    ModBusTransport::Rtu => Instrument::Tcp(TcpInstrument::new(*addr)),
                    ModBusTransport::Tcp => Instrument::ModBusTcp(ModBusTcpInstrument::new(*addr)),
                },
            },
            Address::Tcp { addr } => Instrument::Tcp(TcpInstrument::new(*addr)),
            Address::Vxi { addr } => Instrument::Vxi(VxiInstrument::new(*addr)),
            Address::Can { addr } => Instrument::Can(CanInstrument::new(server, addr.clone())),
            Address::Sigrok { .. } => {
                return None;
            }
            Address::Hid { idn } => Instrument::Hid(HidInstrument::new(idn.clone())),
        };
        Some(ret)
    }

    pub fn disconnect(self) {
        match self {
            Instrument::Visa(x) => x.disconnect(),
            Instrument::ModBusTcp(x) => x.disconnect(),
            Instrument::Serial(x) => x.disconnect(),
            Instrument::Tcp(x) => x.disconnect(),
            Instrument::Vxi(x) => x.disconnect(),
            Instrument::Can(x) => x.disconnect(),
            Instrument::Hid(x) => x.disconnect(),
        }
    }

    pub fn into_visa(self) -> Option<VisaInstrument> {
        match self {
            Instrument::Visa(instr) => Some(instr),
            _ => None,
        }
    }

    pub fn into_modbus_tcp(self) -> Option<ModBusTcpInstrument> {
        match self {
            Instrument::ModBusTcp(instr) => Some(instr),
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
