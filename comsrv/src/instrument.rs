use serde::{Deserialize, Serialize};

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
}


impl Instrument {
    pub async fn connect(addr: String, options: InstrumentOptions) -> crate::Result<Instrument> {
        let splits: Vec<_> = addr.split("::")
            .map(|x| x.trim().to_lowercase())
            .collect();
        if splits[0] == "modbus" {
            todo!()
        } else {
            // perform the actual connection...
            let visa_options = match options {
                InstrumentOptions::Visa(visa) => visa,
                InstrumentOptions::Default => VisaOptions::default(),
            };
            crate::visa::asynced::Instrument::connect(addr, visa_options).await
                .map(Instrument::Visa)
        }
    }
}
