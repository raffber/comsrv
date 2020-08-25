use std::fmt::{Display, Formatter};

use serde::{Deserialize, Serialize};
use thiserror::Error;

use visa_sys::Instrument as VisaInstrument;

use crate::{Result, ScpiRequest, ScpiResponse};
use crate::visa::visa_sys::describe_status;

pub mod asynced;
mod visa_sys;


#[derive(Clone, Serialize, Deserialize)]
pub struct VisaOptions {

}

impl Default for VisaOptions {
    fn default() -> Self {
        Self {}
    }
}

#[derive(Error, Clone, Debug, Serialize, Deserialize)]
pub struct VisaError {
    desc: String,
    code: i32,
}

type VisaResult<T> = std::result::Result<T, VisaError>;

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
            code,
        }
    }
}


pub struct Instrument {
    instr: VisaInstrument,
    pub read_termination: String,
    pub write_terminatin: String,
}

impl Instrument {
    pub fn new(_addr: String) -> Result<Self> {
        todo!()
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
