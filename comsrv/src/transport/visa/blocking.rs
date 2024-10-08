use std::time::Duration;

use serde::{Deserialize, Serialize};

use super::visa_sys::Instrument as VisaInstrument;
#[allow(unused_imports)]
pub use super::visa_sys::{VisaError, VisaResult};
use anyhow::anyhow;

use crate::{protocol::scpi, Error};
use comsrv_protocol::{ScpiRequest, ScpiResponse};

use super::consts;

const DEFAULT_TIMEOUT: f32 = 3.0;
const DEFAULT_CHUNK_SIZE: usize = 20 * 1024;
// from pyvisa
const DEFAULT_TERMINATION: &str = "\n";

#[derive(Default, Clone, Serialize, Deserialize)]
pub struct VisaOptions {}

pub struct Instrument {
    instr: VisaInstrument,
}

impl Instrument {
    pub fn open(addr: &str) -> VisaResult<Self> {
        Ok(Self {
            instr: VisaInstrument::open(addr.to_string(), Some(DEFAULT_TIMEOUT))?,
        })
    }

    pub fn write<T: AsRef<str>>(&self, msg: T) -> VisaResult<()> {
        let mut msg = msg.as_ref().to_string();
        if !msg.ends_with(DEFAULT_TERMINATION) {
            msg.push_str(DEFAULT_TERMINATION);
        }
        self.instr.write(msg.as_bytes())
    }

    fn read(&self) -> VisaResult<Vec<u8>> {
        let mut ret = Vec::new();
        loop {
            let (data, status) = self.instr.read(DEFAULT_CHUNK_SIZE)?;
            ret.extend(data);
            if status != (consts::VI_SUCCESS_MAX_CNT as i32) {
                break;
            }
        }
        Ok(ret)
    }

    pub fn query_string<T: AsRef<str>>(&self, msg: T) -> crate::Result<String> {
        log::debug!("Query[{}]: `{}`", self.instr.addr(), msg.as_ref());
        self.write(msg).map_err(|x| crate::Error::transport(anyhow!(x)))?;
        let rx = self.read().map_err(|x| crate::Error::transport(anyhow!(x)))?;
        let ret =
            String::from_utf8(rx).map_err(|x| crate::Error::protocol(anyhow!("Invalid UTF-8 received. {}", x)))?;
        log::debug!("Reply[{}]: `{}`", self.instr.addr(), ret);
        if !ret.ends_with(DEFAULT_TERMINATION) {
            return Err(Error::protocol(anyhow!("Invalid Termination")));
        }
        Ok(ret[..ret.len() - DEFAULT_TERMINATION.len()].to_string())
    }

    pub fn set_timeout(&self, _timeout: f32) -> VisaResult<()> {
        todo!()
    }

    pub fn get_timeout(&self) -> VisaResult<f32> {
        todo!()
    }

    pub fn query_binary<T: AsRef<str>>(&self, msg: T) -> crate::Result<Vec<u8>> {
        log::debug!("QueryBinary[{}]: `{}`", self.instr.addr(), msg.as_ref());
        self.write(msg).map_err(|x| crate::Error::transport(anyhow!(x)))?;
        let rx = self.read().map_err(|x| crate::Error::transport(anyhow!(x)))?;
        let (offset, length) = scpi::parse_binary_header(&rx)?;
        Ok(rx[offset..offset + length].to_vec())
    }

    pub fn addr(&self) -> &str {
        self.instr.addr()
    }

    pub fn handle_scpi(&self, req: ScpiRequest, _: Option<Duration>) -> crate::Result<ScpiResponse> {
        // TODO: handle timeout
        match req {
            ScpiRequest::Write(x) => self
                .write(x)
                .map_err(|x| crate::Error::transport(anyhow!(x)))
                .map(|_| ScpiResponse::Done),
            ScpiRequest::QueryString(x) => self.query_string(x).map(ScpiResponse::String),
            ScpiRequest::QueryBinary(x) => self.query_binary(x).map(|data| ScpiResponse::Binary { data }),
            ScpiRequest::ReadRaw => self
                .read()
                .map_err(|x| crate::Error::transport(anyhow!(x)))
                .map(|data| ScpiResponse::Binary { data }),
        }
    }
}
