/// This module implements `Address` which is used for parsing
/// address strings of the form "serial::COM3::115200::8N1"
use crate::can::CanAddress;
use crate::modbus::{ModBusAddress, ModBusTransport};
use crate::serial::SerialParams;
use crate::Error;
use comsrv_protocol::HidIdentifier;
use std::fmt;
use std::fmt::{Display, Formatter};
use std::net::{IpAddr, SocketAddr};

/// Represents a parsed address string.
/// An address maps to a unique hardware resource (as given by `HandleId`) but
/// may carry some additional settings for the communication link.
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
        gpib_addr: u8,
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
    Hid {
        idn: HidIdentifier,
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
            if splits.len() != 3 && splits.len() != 4 {
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
                if splits.len() != 3 && splits.len() != 4 {
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
                if splits.len() != 5 && splits.len() != 6 {
                    return Err(Error::InvalidAddress);
                }
                let (path, params) = SerialParams::from_address(&splits[2..5])?;
                let slave_id: u8 = if splits.len() == 6 {
                    splits[5].parse().map_err(|_| Error::InvalidAddress)?
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

    /// Create a new `Address` by parsing the given address string.
    /// If the address uses an incorrect format, it will return `Err(Error::InvalidAddress)`.
    /// Addresses not matching any of the predefined prefixes will be treated as a VISA instrument.
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
                gpib_addr: addr,
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
        } else if splits[0].to_lowercase() == "hid" {
            if splits.len() != 3 {
                return Err(Error::InvalidAddress);
            }
            let vid = u16::from_str_radix(&splits[1], 16).map_err(|_| Error::InvalidAddress)?;
            let pid = u16::from_str_radix(&splits[2], 16).map_err(|_| Error::InvalidAddress)?;
            Ok(Address::Hid {
                idn: HidIdentifier::new(vid, pid),
            })
        } else {
            let splits: Vec<_> = splits.iter().map(|x| x.to_lowercase()).collect();
            Ok(Address::Visa { splits })
        }
    }

    /// Get a `HandleId` based on the address. A `HandleId` maps directly to some underlying
    /// hardware resource.
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
            Address::Hid { .. } => HandleId::new(self.clone().into()),
        }
    }
}

impl From<Address> for String {
    fn from(addr: Address) -> Self {
        match addr {
            Address::Visa { splits } => splits.join("::"),
            Address::Serial { path, params } => format!("serial::{}::{}", path, params),
            Address::Prologix {
                file,
                gpib_addr: gpib,
            } => format!("prologix::{}::{}", file, gpib),
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
            Address::Hid { idn } => format!("hid::{:#x}::{:#x}", idn.vid(), idn.pid()),
        }
    }
}

impl Display for Address {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let x: String = self.clone().into();
        f.write_str(&x)
    }
}

/// Represents an identifier for an exclusive hardware resource, such
/// as a serial port, a TCP connection or similar, as such there can
/// be only one open instrument per handle
#[derive(Hash, Clone, PartialEq, Eq, Debug)]
pub struct HandleId {
    inner: String,
}

impl HandleId {
    pub fn new(inner: String) -> Self {
        Self { inner }
    }
}

impl ToString for HandleId {
    fn to_string(&self) -> String {
        self.inner.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::serial::params::{DataBits, Parity, StopBits};
    use std::net::SocketAddr;

    #[test]
    fn test_modbus_address() {
        let addr = Address::parse("modbus::tcp::192.168.1.1:509").unwrap();
        let ref_sock_addr: SocketAddr = "192.168.1.1:509".parse().unwrap();
        match addr {
            Address::Modbus {
                addr,
                transport,
                slave_id,
            } => {
                match addr {
                    ModBusAddress::Tcp { addr } => assert_eq!(addr, ref_sock_addr),
                    _ => {}
                }
                assert!(matches!(transport, ModBusTransport::Tcp));
                assert_eq!(slave_id, 255);
            }
            _ => panic!(),
        }
        let addr = Address::parse("modbus::tcp::192.168.1.1:509::123").unwrap();
        match addr {
            Address::Modbus {
                addr,
                transport,
                slave_id,
            } => {
                match addr {
                    ModBusAddress::Tcp { addr } => assert_eq!(addr, ref_sock_addr),
                    _ => {}
                }
                assert!(matches!(transport, ModBusTransport::Tcp));
                assert_eq!(slave_id, 123);
            }
            _ => panic!(),
        }

        let addr = Address::parse("modbus::rtu::192.168.1.1:509::123").unwrap();
        match addr {
            Address::Modbus {
                addr,
                transport,
                slave_id,
            } => {
                match addr {
                    ModBusAddress::Tcp { addr } => assert_eq!(addr, ref_sock_addr),
                    _ => {}
                }
                assert!(matches!(transport, ModBusTransport::Rtu));
                assert_eq!(slave_id, 123);
            }
            _ => panic!(),
        }

        let addr = Address::parse("modbus::rtu::/dev/ttyUSB0::115200::8N1::123").unwrap();
        match addr {
            Address::Modbus {
                addr,
                transport,
                slave_id,
            } => {
                match addr {
                    ModBusAddress::Serial { path, params } => {
                        assert_eq!(path, "/dev/ttyUSB0");
                        assert_eq!(
                            params,
                            SerialParams {
                                baud: 115200,
                                data_bits: DataBits::Eight,
                                stop_bits: StopBits::One,
                                parity: Parity::None,
                            }
                        );
                    }
                    _ => {}
                }
                assert!(matches!(transport, ModBusTransport::Rtu));
                assert_eq!(slave_id, 123);
            }
            _ => panic!(),
        }
    }

    #[test]
    fn parse_tcp() {
        let addr = Address::parse("tcp::192.168.1.1:123").unwrap();
        match addr {
            Address::Tcp { addr } => {
                assert_eq!(addr, "192.168.1.1:123".parse().unwrap())
            }
            _ => panic!(),
        }
    }

    #[test]
    fn parse_serial() {
        let addr = Address::parse("serial::COM1::115200::8N1").unwrap();
        match addr {
            Address::Serial { path, params } => {
                assert_eq!(path, "COM1");
                assert_eq!(
                    params,
                    SerialParams {
                        baud: 115200,
                        data_bits: DataBits::Eight,
                        stop_bits: StopBits::One,
                        parity: Parity::None,
                    }
                );
            }
            _ => panic!(),
        }

        let addr = Address::parse("serial::COM1::9600::5E2").unwrap();
        match addr {
            Address::Serial { path, params } => {
                assert_eq!(path, "COM1");
                assert_eq!(
                    params,
                    SerialParams {
                        baud: 9600,
                        data_bits: DataBits::Five,
                        stop_bits: StopBits::Two,
                        parity: Parity::Even,
                    }
                );
            }
            _ => panic!(),
        }
    }

    #[test]
    fn parse_prologix() {
        let addr = Address::parse("prologix::/dev/ttyUSB0::10").unwrap();
        match addr {
            Address::Prologix { file, gpib_addr } => {
                assert_eq!(gpib_addr, 10);
                assert_eq!(file, "/dev/ttyUSB0");
            }
            _ => panic!(),
        }
    }

    #[test]
    fn parse_vxi() {
        let addr = Address::parse("vxi::192.168.1.1").unwrap();
        match addr {
            Address::Vxi { addr } => {
                assert_eq!(addr, "192.168.1.1".parse::<IpAddr>().unwrap())
            }
            _ => panic!(),
        }
    }

    #[test]
    fn parse_can() {
        let addr = Address::parse("can::socket::can0").unwrap();
        match addr {
            Address::Can { addr } => match addr {
                CanAddress::Socket(interface) => assert_eq!(interface, "can0"),
                _ => panic!(),
            },
            _ => panic!(),
        }

        let addr = Address::parse("can::pcan::usb1::1000000").unwrap();
        match addr {
            Address::Can { addr } => match addr {
                CanAddress::PCan { ifname, bitrate } => {
                    assert_eq!(ifname, "usb1");
                    assert_eq!(bitrate, 1000000);
                }
                _ => panic!(),
            },
            _ => panic!(),
        }

        let addr = Address::parse("can::loopback").unwrap();
        match addr {
            Address::Can { addr } => match addr {
                CanAddress::Loopback => {}
                _ => panic!(),
            },
            _ => panic!(),
        }
    }

    #[test]
    fn parse_sigrok() {
        let addr = Address::parse("sigrok::foobar").unwrap();
        match addr {
            Address::Sigrok { device } => {
                assert_eq!(device, "foobar");
            }
            _ => panic!(),
        }
    }
}
