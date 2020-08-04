use crate::visa::{open_instrument as visa_open_instrument, VisaError};
use crate::visa::Instrument as VisaInstrument;
use crate::Result;

pub struct Instrument {
    instr: VisaInstrument,
    pub read_termination: String,
    pub write_terminatin: String,
}

impl Instrument {
    pub fn new(addr: String) -> Result<Self> {
        todo!()
    }

    pub fn write<T: AsRef<str>>(&self, msg: T) -> Result<()> {
        todo!()
    }

    pub fn query<T: AsRef<str>>(&self, msg: T) -> Result<String> {
        todo!()
    }

    pub fn set_timeout(&self, timeout: f32) -> Result<()> {
        todo!()
    }

    pub fn get_timeout(&self) -> Result<f32> {
        todo!()
    }

    pub fn query_binary<T: AsRef<str>>(&self, msg: T) -> Result<Vec<u8>> {
        todo!()
    }

    pub fn addr(&self) -> &str {
        self.instr.addr()
    }
}
