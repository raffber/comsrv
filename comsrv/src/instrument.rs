use std::net::SocketAddr;

use serde::{Deserialize, Serialize};

use crate::Error;
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
}


impl Instrument {
    pub async fn connect(addr: String, options: &InstrumentOptions) -> crate::Result<Instrument> {
        let splits: Vec<_> = addr.split("::")
            .map(|x| x.trim().to_lowercase())
            .collect();
        if splits.len() < 2 {
            return Err(Error::InvalidAddress);
        }
        if splits[0] == "modbus" {
            let addr = &splits[1];
            let addr: SocketAddr = addr.parse().map_err(|_| Error::InvalidAddress)?;
            crate::modbus::Instrument::connect(addr).await
                .map(Instrument::Modbus)
        } else {
            // perform the actual connection...
            let visa_options = match options {
                InstrumentOptions::Visa(visa) => visa.clone(),
                InstrumentOptions::Default => VisaOptions::default(),
            };
            crate::visa::asynced::Instrument::connect(addr, visa_options).await
                .map(Instrument::Visa)
        }
    }
}
