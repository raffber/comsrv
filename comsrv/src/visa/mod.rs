use serde::{Deserialize, Serialize};
pub use visa_sys::{VisaError, VisaResult};
use visa_sys::Instrument as VisaInstrument;

use crate::{ScpiRequest, ScpiResponse};

pub mod asynced;
mod visa_sys;

const DEFAULT_TIMEOUT: f32 = 3.0;


#[derive(Clone, Serialize, Deserialize)]
pub struct VisaOptions {
}

impl Default for VisaOptions {
    fn default() -> Self {
        Self {}
    }
}


pub struct Instrument {
    instr: VisaInstrument,
}

impl Instrument {
    pub fn open(addr: String, _options: VisaOptions) -> VisaResult<Self> {
        Ok(Self {
            instr: VisaInstrument::open(addr, Some(DEFAULT_TIMEOUT))?
        })
    }

    pub fn write<T: AsRef<str>>(&self, _msg: T) -> VisaResult<()> {
        todo!()
    }

    pub fn query_string<T: AsRef<str>>(&self, _msg: T) -> VisaResult<String> {
        todo!()
    }

    pub fn set_timeout(&self, _timeout: f32) -> VisaResult<()> {
        todo!()
    }

    pub fn get_timeout(&self) -> VisaResult<f32> {
        todo!()
    }

    pub fn query_binary<T: AsRef<str>>(&self, _msg: T) -> VisaResult<Vec<u8>> {
        todo!()
    }

    pub fn addr(&self) -> &str {
        self.instr.addr()
    }

    pub fn handle_scpi(&self, _req: ScpiRequest) -> VisaResult<ScpiResponse> {
        todo!()
    }
}
