use std::hash::{Hash, Hasher};
use std::net::SocketAddr;

use serde::{Deserialize, Serialize};

use crate::Error;
use crate::serial::{DataBits, Parity, SerialParams, StopBits};
use crate::visa::VisaOptions;

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
    Visa(crate::visa::asynced::Instrument),
    Modbus(crate::modbus::Instrument),
    Prologix(crate::prologix::Instrument),
    Serial(crate::serial::Instrument),
}

struct HandleId {
    inner: String,
}

impl HandleId {
    fn new(inner: String) -> Self {
        Self {
            inner
        }
    }
}

impl Hash for HandleId {
    fn hash<H: Hasher>(&self, state: &mut H) {
        unimplemented!()
    }
}

enum Address {
    Visa {
        splits: Vec<String>
    },
    Serial {
        params: SerialParams,
    },
    Prologix {
        file: String,
        gpib: u8,
    },
    Modbus {
        addr: SocketAddr,
    },
}

impl Address {
    pub fn parse(addr: &str) -> crate::Result<Self> {
        let splits: Vec<_> = addr.split("::")
            .map(|x| x.to_string())
            .collect();
        if splits.len() < 2 {
            return Err(Error::InvalidAddress);
        }

        if splits[0].to_lowercase() == "modbus" {
            // modbus::192.168.0.1:1234
            // TODO: move to URL?
            let addr = &splits[1].to_lowercase();
            let addr: SocketAddr = addr.parse().map_err(|_| Error::InvalidAddress)?;
            Ok(Address::Modbus {
                addr
            })
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
            let params = SerialParams::from_string(&addr).ok_or(Error::InvalidAddress)?;
            Ok(Address::Serial {
                params
            })
        } else {
            let splits: Vec<_> = splits.iter().map(|x| x.to_lowercase().to_string()).collect();
            Ok(Address::Visa { splits })
        }
    }

    pub fn handle_id(&self) -> HandleId {
        match self {
            Address::Visa { splits } => HandleId::new(splits[1].clone()),
            Address::Serial { params } => HandleId::new(params.path.clone()),
            Address::Prologix { file, .. } => file.clone(),
            Address::Modbus { addr } => addr.to_string(),
        }
    }
}

impl Into<String> for Address {
    fn into(self) -> String {
        unimplemented!()
    }
}

impl Hash for Address {
    fn hash<H: Hasher>(&self, state: &mut H) {
        todo!()
    }
}


impl Instrument {
    pub async fn connect_with_string(addr: &str, options: &InstrumentOptions) -> crate::Result<Instrument> {
        let addr = Address::parse(&addr)?;
        Self::connect(addr, options)
    }

    pub async fn connect(addr: Address, options: &InstrumentOptions) -> crate::Result<Instrument> {
        match addr {
            Address::Visa { splits } => {
                let visa_options = match options {
                    InstrumentOptions::Visa(visa) => visa.clone(),
                    InstrumentOptions::Default => VisaOptions::default(),
                };
                let addr = splits.join("::");
                crate::visa::asynced::Instrument::connect(addr, visa_options).await
                    .map(Instrument::Visa)
            }
            Address::Serial { params } => {
                Ok(Instrument::Serial(crate::serial::Instrument::connect(params)))
            }
            Address::Prologix { file, gpib } => {
                Ok(Instrument::Prologix(crate::prologix::Instrument::connect(&file, *gpib)))
            }
            Address::Modbus { addr } => {
                crate::modbus::Instrument::connect(addr).await
                    .map(Instrument::Modbus)
            }
        }
    }

    pub fn disconnect(self) {
        match self {
            Instrument::Visa(_) => {
                todo!()
            }
            Instrument::Modbus(_) => {
                todo!()
            }
            Instrument::Prologix(x) => {
                x.disconnect();
            }
            Instrument::Serial(x) => {
                x.disconnect()
            }
        }
    }
}
