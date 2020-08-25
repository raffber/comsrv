use std::ffi::{CStr, CString};
use std::io::Write;
use std::mem::MaybeUninit;
use std::os::raw::c_char;
use std::fmt::{Display, Formatter};
use thiserror::Error;
use serde::{Deserialize, Serialize};

use dlopen::wrapper::{Container, WrapperApi};
use lazy_static;
use tempfile::NamedTempFile;

#[derive(Error, Clone, Debug, Serialize, Deserialize)]
pub struct VisaError {
    desc: String,
    code: i32,
}

pub type VisaResult<T> = std::result::Result<T, VisaError>;

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

impl From<VisaError> for crate::Error {
    fn from(err: VisaError) -> Self {
        crate::Error::Visa(err)
    }
}


cfg_if::cfg_if! {
    if #[cfg(unix)] {
        const VISA_LIB: &'static [u8] = include_bytes!("../../lib/libvisa.so");
    } else {
        const VISA_LIB: &'static [u8] = include_bytes!("todo.lib");
    }
}

type ViStatus = i32;
type ViAccessMode = u32;
type ViSession = u32;
type ViObject = u32;
type ViAttr = u32;
type ViAttrState = u64;

#[derive(Clone, WrapperApi)]
struct Api {
    viOpen: unsafe extern "C" fn(session: ViSession, rsrc: *const c_char,
                                 access_mode: ViAccessMode, timeout: u32, vi: *mut ViObject) -> ViStatus,
    viOpenDefaultRM: extern "C" fn(vi: *mut ViSession) -> ViStatus,
    viClose: extern "C" fn(vi: ViObject) -> ViStatus,
    viSetAttribute: extern "C" fn(vi: ViObject, attr: ViAttr, value: ViAttrState) -> ViStatus,
    viGetAttribute: unsafe extern "C" fn(vi: ViObject, attr: ViAttr, value: *mut u64) -> ViStatus,
    viStatusDesc: unsafe extern "C" fn(vi: ViObject, status: ViStatus, value: *mut c_char) -> ViStatus,
    viRead: unsafe extern "C" fn(vi: ViSession, buf: *mut u8, cnt: u32, cnt_ret: *mut u32) -> ViStatus,
    viWrite: unsafe extern "C" fn(vi: ViSession, buf: *const u8, cnt: u32, cnt_ret: *mut u32) -> ViStatus,
    viClear: unsafe extern "C" fn(vi: ViSession) -> ViStatus,
}

pub struct Visa {
    api: Container<Api>,
    rm: ViSession,
}

lazy_static! {
    static ref VISA: Visa = Visa::new();
}

impl Visa {
    fn new() -> Self {
        let mut tmpfile = NamedTempFile::new().unwrap();
        tmpfile.write_all(VISA_LIB).unwrap();
        let name = tmpfile.path().to_str().unwrap();
        let cont: Container<Api> = unsafe { Container::load(name) }.unwrap();
        let mut rm: ViSession = 0;
        let ret = cont.viOpenDefaultRM(&mut rm as *mut ViSession);
        if ret < 0 {
            panic!("Could not open resource manager: Error Code {}", ret);
        }
        Visa {
            api: cont,
            rm,
        }
    }
}

fn describe_status(status: ViStatus) -> String {
    unsafe {
        let mut data: [c_char; 512] = MaybeUninit::uninit().assume_init();
        let new_status = VISA.api.viStatusDesc(VISA.rm, status, data.as_mut_ptr());
        println!("{}", new_status);
        let ret = CStr::from_ptr(data.as_ptr());
        ret.to_str().unwrap().to_string()
    }
}

impl Drop for Visa {
    fn drop(&mut self) {
        let status = VISA.api.viClose(self.rm);
        if status < 0 {
            panic!(format!("Error dropping resource manager: {}", describe_status(status)));
        }
    }
}

pub struct Attr {
    instr: ViObject,
    code: ViAttr,
}

impl Attr {
    fn new(instr: ViObject, code: ViAttr) -> Self {
        Attr {
            instr,
            code,
        }
    }

    pub fn set(&self, value: u64) -> Result<(), VisaError> {
        let stat = VISA.api.viSetAttribute(self.instr, self.code, value);
        if stat < 0 {
            Err(VisaError::new(stat))
        } else {
            Ok(())
        }
    }

    pub fn get(&self) -> Result<u64, VisaError> {
        let (stat, ret) = unsafe {
            let mut ret = 0_u64;
            let stat = VISA.api.viGetAttribute(self.instr, self.code, &mut ret as *mut u64);
            (stat, ret)
        };
        if stat < 0 {
            Err(VisaError::new(stat))
        } else {
            Ok(ret)
        }
    }
}

#[derive(Clone)]
pub struct Instrument {
    instr: ViObject,
    addr: String,
}

impl Instrument {
    pub fn open(addr: String, timeout: Option<f32>) -> Result<Instrument, VisaError> {
        let instr = unsafe {
            let cstr = CString::new(addr.clone()).unwrap();
            let tmo = if let Some(tmo) = timeout {
                (tmo * 1000.0).round() as u32
            } else {
                0
            };
            let mut handle: ViObject = 0;
            let status = VISA.api.viOpen(VISA.rm, cstr.as_ptr(), 0, tmo, &mut handle as *mut ViObject);
            if status < 0 {
                return Err(VisaError::new(status));
            }
            handle
        };
        Ok(Instrument { instr, addr })
    }
    pub fn read(&self, size: usize) -> Result<Vec<u8>, VisaError> {
        let mut data: Vec<u8> = Vec::with_capacity(size);
        unsafe {
            let ptr = data.as_mut_ptr();
            let mut actually_read = 0_u32;
            let ret = VISA.api.viRead(self.instr, ptr, size as u32, &mut actually_read as *mut u32);
            if ret < 0 {
                return Err(VisaError::new(ret));
            }
            data.set_len(size);
        }
        Ok(data)
    }

    pub fn write<'a, T: Into<&'a [u8]>>(&self, data: T) -> Result<(), VisaError> {
        let data = data.into();
        let ptr = data.as_ptr();
        let mut actually_written = 0_u32;
        unsafe {
            let ret = VISA.api.viWrite(self.instr, ptr, data.len() as u32, &mut actually_written as *mut u32);
            if ret < 0 {
                return Err(VisaError::new(ret));
            }
        }
        Ok(())
    }

    pub fn timeout(&self) -> Attr {
        Attr::new(self.instr, 0x3FFF001A)
    }

    pub fn addr(&self) -> &str {
        &self.addr
    }
}

impl Drop for Instrument {
    fn drop(&mut self) {
        let status = VISA.api.viClose(self.instr);
        if status < 0 {
            panic!(format!("Error dropping instrument: {}", describe_status(status)));
        }
    }
}

const VI_TMO_INFINITE: u32 = 0xFFFFFFFF;
const VI_TMO_IMMEDIATE: u32 = 0;

struct Timeout {
    attr: Attr,
}

impl Timeout {
    fn set(&self, value: f32) -> Result<(), VisaError> {
        if value == f32::INFINITY {
            return self.attr.set(VI_TMO_INFINITE as u64);
        }
        assert!(value > 0.0);
        let value = (value / 1000.0).round() as u32;
        assert!(value <= 4294967294);
        self.attr.set(value as u64)
    }

    fn get(&self) -> Result<f32, VisaError> {
        let ret = self.attr.get()? as u32;
        if ret == VI_TMO_INFINITE {
            return Ok(f32::INFINITY);
        }
        Ok((ret as f32) * 1000.0)
    }
}
