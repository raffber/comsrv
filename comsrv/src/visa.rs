use tempfile::NamedTempFile;
use std::io::Write;
use dlopen::wrapper::{Container, WrapperApi};
use std::os::raw::c_char;
use lazy_static;
use std::ffi::CStr;
use std::mem::MaybeUninit;


cfg_if::cfg_if! {
    if #[cfg(unix)] {
        const VISA_LIB: &'static [u8] = include_bytes!("../lib/libvisa.so");
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

#[derive(WrapperApi)]
struct Api {
    viOpen: unsafe extern "C" fn(session: ViSession, rsrc: *const c_char,
                                  access_mode: ViAccessMode, timeout: u32, vi: *mut ViObject) -> ViStatus,
    #[allow(non_snake_case)]
    viOpenDefaultRM: extern "C" fn(vi: *mut ViSession) -> ViStatus,
    #[allow(non_snake_case)]
    viClose: unsafe extern "C" fn(vi: ViObject) -> ViStatus,
    #[allow(non_snake_case)]
    viSetAttribute: unsafe extern "C" fn(vi: ViObject, attr: ViAttr, value: ViAttrState) -> ViStatus,
    #[allow(non_snake_case)]
    viGetAttribute: unsafe extern "C" fn(vi: ViObject, attr: ViAttr, value: *mut ()) -> ViStatus,
    #[allow(non_snake_case)]
    viStatusDesc: unsafe extern "C" fn(vi: ViObject, status: ViStatus, value: *mut c_char) -> ViStatus,
    #[allow(non_snake_case)]
    viRead: unsafe extern "C" fn(vi: ViSession, buf: *mut u8, cnt: u32, cnt_ret: *mut u32) -> ViStatus,
    #[allow(non_snake_case)]
    viWrite: unsafe extern "C" fn(vi: ViSession, buf: *mut u8, cnt: u32, cnt_ret: *mut u32) -> ViStatus,
    #[allow(non_snake_case)]
    viClear: unsafe extern "C" fn(vi: ViSession) -> ViStatus,
}

pub struct Visa {
    api: Container<Api>,
    rm: ViSession,
}

lazy_static! {
    pub static ref VISA: Visa = Visa::new();
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
            rm
        }
    }

    fn describe_status(&self, status: ViStatus) -> String {
        unsafe {
            let mut data: [c_char; 512] = MaybeUninit::uninit().assume_init();
            let new_status = self.api.viStatusDesc(self.rm,  status, data.as_mut_ptr());
            println!("{}", new_status);
            let ret = CStr::from_ptr(data.as_ptr());
            ret.to_str().unwrap().to_string()
        }
    }

    pub fn foo(&self) {
        unsafe {
            self.api.viClose(0);
        }
    }
}
