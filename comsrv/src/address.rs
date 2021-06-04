use crate::can::CanAddress;
use crate::instrument::HandleId;
use crate::modbus::{ModBusAddress, ModBusTransport};
use crate::serial::SerialParams;
use crate::Error;
use std::fmt;
use std::fmt::{Display, Formatter};
use std::net::{IpAddr, SocketAddr};

#[derive(Clone, Hash)]
pub enum Address {
    Visa {
        splits: Vec<String>,
    },
    Serial {
        path: String,
        params: SerialParams,
    },
    Prologix {
        file: String,
        gpib: u8,
    },
    Modbus {
        addr: ModBusAddress,
        transport: ModBusTransport,
        slave_id: u8,
    },
    Vxi {
        addr: IpAddr,
    },
    Tcp {
        addr: SocketAddr,
    },
    Can {
        addr: CanAddress,
    },
    Sigrok {
        device: String,
    },
}

impl Address {
    fn parse_modbus(splits: &[&str]) -> crate::Result<Self> {
        if splits.len() < 3 {
            return Err(Error::InvalidAddress);
        }
        let kind = &splits[1].to_lowercase();
        let addr = &splits[2].to_lowercase();
        if kind == "tcp" {
            // modbus::tcp::192.168.0.1:1234
            if splits.len() != 3 || splits.len() != 4 {
                return Err(Error::InvalidAddress);
            }
            let addr: SocketAddr = addr.parse().map_err(|_| Error::InvalidAddress)?;
            let slave_id: u8 = if splits.len() == 4 {
                splits[3].parse().map_err(|_| Error::InvalidAddress)?
            } else {
                255
            };
            Ok(Address::Modbus {
                addr: ModBusAddress::Tcp { addr },
                transport: ModBusTransport::Tcp,
                slave_id,
            })
        } else if kind == "rtu" {
            if let Ok(addr) = addr.parse() {
                // rtu over tcp
                // modbus::rtu::192.168.1.123{::56}
                if splits.len() != 3 || splits.len() != 4 {
                    return Err(Error::InvalidAddress);
                }
                let slave_id: u8 = if splits.len() == 4 {
                    splits[3].parse().map_err(|_| Error::InvalidAddress)?
                } else {
                    255
                };
                Ok(Address::Modbus {
                    addr: ModBusAddress::Tcp { addr },
                    transport: ModBusTransport::Rtu,
                    slave_id,
                })
            } else {
                // rtu over serial
                // modbus::rtu::/dev/ttyUSB0::115200::8N1{::123}
                if splits.len() != 5 || splits.len() != 6 {
                    return Err(Error::InvalidAddress);
                }
                let (path, params) = SerialParams::from_address(&splits[2..5])?;
                let slave_id: u8 = if splits.len() == 6 {
                    splits[6].parse().map_err(|_| Error::InvalidAddress)?
                } else {
                    255
                };
                Ok(Address::Modbus {
                    addr: ModBusAddress::Serial { path, params },
                    transport: ModBusTransport::Rtu,
                    slave_id,
                })
            }
        } else {
            Err(Error::InvalidAddress)
        }
    }

    pub fn parse(addr: &str) -> crate::Result<Self> {
        let splits: Vec<_> = addr.split("::").map(|x| x.to_string()).collect();
        if splits.len() < 2 {
            return Err(Error::InvalidAddress);
        }

        if splits[0].to_lowercase() == "modbus" {
            let new_splits: Vec<&str> = splits.iter().map(|x| x.as_ref()).collect();
            Self::parse_modbus(&new_splits)
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
            let (path, params) = SerialParams::from_address(&new_splits[1..4])?;
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
            Address::Modbus { addr, .. } => match addr {
                ModBusAddress::Serial { path, .. } => HandleId::new(path.to_string()),
                ModBusAddress::Tcp { addr } => HandleId::new(addr.to_string()),
            },
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
            Address::Serial { path, params } => format!("serial::{}::{}", path, params),
            Address::Prologix { file, gpib } => format!("prologix::{}::{}", file, gpib),
            Address::Modbus {
                addr,
                transport,
                slave_id,
            } => {
                if slave_id != 255 {
                    format!("modbus::{}::{}::{}", transport, addr, slave_id)
                } else {
                    format!("modbus::{}::{}", transport, addr)
                }
            }
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
