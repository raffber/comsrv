use crate::visa::{open_instrument as visa_open_instrument, VisaError};
use crate::visa::Instrument as VisaInstrument;
use std::io;

pub enum Error {
    Visa(VisaError),
    Io(io::Error)
}

pub type Result<T> = std::result::Result<T, Error>;

pub struct Instrument {
    instr: VisaInstrument,
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

    pub fn query_binary_from_string<T: AsRef<str>>(&self, msg: T) {
        todo!()
    }

    pub fn addr(&self) -> &str {
        self.instr.addr()
    }
}
