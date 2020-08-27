use serde::{Deserialize, Serialize};

pub use visa_sys::{VisaError, VisaResult};
use visa_sys::Instrument as VisaInstrument;

use crate::{ScpiRequest, ScpiResponse};
use crate::Error;

mod consts;

pub mod asynced;
mod visa_sys;

const DEFAULT_TIMEOUT: f32 = 3.0;
const DEFAULT_CHUNK_SIZE: usize = 20 * 1024; // from pyvisa
const DEFAULT_TERMINATION: &'static str = "\r\n";


#[derive(Clone, Serialize, Deserialize)]
pub struct VisaOptions {}

impl Default for VisaOptions {
    fn default() -> Self {
        Self {}
    }
}


pub struct Instrument {
    instr: VisaInstrument,
}

fn parse_binary_header(rx: &[u8]) -> crate::Result<(usize, usize)> {
    let begin = rx.iter().position(|x| *x == b'#').ok_or(Error::InvalidBinaryHeader)?;

    const DEFAULT_LENGTH_BEFORE_BLOCK: usize = 25;

    if begin > DEFAULT_LENGTH_BEFORE_BLOCK {
        return Err(Error::InvalidBinaryHeader);
    }
    let header_length = if rx.len() < begin + 2 {
        0
    } else {
        let data = String::from_utf8(vec![rx[begin + 1]])
            .map_err(|_| Error::InvalidBinaryHeader)?;
        data.parse::<usize>().map_err(|_| Error::InvalidBinaryHeader)?
    };
    let offset = begin + 2 + header_length;
    if offset > rx.len() {
        return Err(Error::InvalidBinaryHeader);
    }
    let data_length = if header_length > 0 {
        let x: Vec<_> = rx[begin + 2..offset].iter().cloned().collect();
        let data = String::from_utf8(x)
            .map_err(|_| Error::InvalidBinaryHeader)?;
        data.parse::<usize>().map_err(|_| Error::InvalidBinaryHeader)?
    } else {
        0
    };
    if offset + data_length > rx.len() {
        Err(Error::InvalidBinaryHeader)
    } else {
        Ok((offset, data_length))
    }
}

impl Instrument {
    pub fn open(addr: String, _options: &VisaOptions) -> VisaResult<Self> {
        Ok(Self {
            instr: VisaInstrument::open(addr, Some(DEFAULT_TIMEOUT))?
        })
    }

    pub fn write<T: AsRef<str>>(&self, msg: T, _options: &VisaOptions) -> VisaResult<()> {
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

    pub fn query_string<T: AsRef<str>>(&self, msg: T, options: &VisaOptions) -> crate::Result<String> {
        self.write(msg, options).map_err(Error::Visa)?;
        let rx = self.read().map_err(Error::Visa)?;
        let ret = String::from_utf8(rx).map_err(Error::DecodeError)?;
        if !ret.ends_with(DEFAULT_TERMINATION) {
            return Err(Error::NotTerminated);
        }
        Ok(ret[..ret.len()-DEFAULT_TERMINATION.len()].to_string())
    }

    pub fn set_timeout(&self, _timeout: f32) -> VisaResult<()> {
        todo!()
    }

    pub fn get_timeout(&self) -> VisaResult<f32> {
        todo!()
    }

    pub fn query_binary<T: AsRef<str>>(&self, msg: T, option: &VisaOptions) -> crate::Result<Vec<u8>> {
        self.write(msg, option).map_err(Error::Visa)?;
        let rx = self.read().map_err(Error::Visa)?;
        let (offset, length) = parse_binary_header(&rx)?;
        Ok(rx[offset..offset+length].iter().cloned().collect())
    }

    pub fn addr(&self) -> &str {
        self.instr.addr()
    }

    pub fn handle_scpi(&self, req: ScpiRequest, options: &VisaOptions) -> crate::Result<ScpiResponse> {
        match req {
            ScpiRequest::Write(x) => self.write(x, options).map_err(Error::Visa).map(|_| ScpiResponse::Done),
            ScpiRequest::QueryString(x) => self.query_string(x, options).map(ScpiResponse::String),
            ScpiRequest::QueryBinary(x) => self.query_binary(x, options).map(ScpiResponse::Binary),
        }
    }
}
