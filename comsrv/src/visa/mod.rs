use visa_sys::Instrument as VisaInstrument;
use thiserror::Error;
use std::fmt::{Display, Formatter};
use crate::Result;
use crate::visa::visa_sys::describe_status;

mod visa_sys;

#[derive(Error, Debug)]
pub struct VisaError {
    desc: String,
    code: i32,
}

impl Display for VisaError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!("VisaError({}): `{}`", self.code, self.desc))
    }
}

impl VisaError {
    pub fn new(code: i32) -> Self {
        let desc = describe_status(code);
        Self {
            desc,
            code
        }
    }
}


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
