mod consts;

use serde::{Deserialize, Serialize};
pub use visa_sys::{VisaError, VisaResult};
use visa_sys::Instrument as VisaInstrument;

use crate::{ScpiRequest, ScpiResponse};

pub mod asynced;
mod visa_sys;

const DEFAULT_TIMEOUT: f32 = 3.0;
const DEFAULT_CHUNK_SIZE: usize = 20*1024; // from pyvisa


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

    pub fn write<T: AsRef<str>>(&self, msg: T, _options: VisaOptions) -> VisaResult<()> {
        let msg = msg.as_ref().as_bytes();
        self.instr.write(msg)
    }

    fn read(&self) -> VisaResult<Vec<u8>> {
        let mut ret = Vec::new();
        loop {
            let (data, status) = self.instr.read(DEFAULT_CHUNK_SIZE)?;
            ret.extend(data);
            if status != (consts::VI_SUCCESS_MAX_CNT as i32) {
                break
            }
        }
        Ok(ret)
    }

    pub fn query_string<T: AsRef<str>>(&self, _msg: T, _options: VisaOptions) -> VisaResult<String> {
        // self.write(msg)?;

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
