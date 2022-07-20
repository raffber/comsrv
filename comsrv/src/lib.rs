#![allow(non_snake_case)]
#![allow(dead_code)]

#[macro_use]
extern crate dlopen_derive;
#[macro_use]
extern crate lazy_static;

pub use comsrv_protocol::{Request, Response};

pub mod app;
mod bytestream;
mod can;
mod ftdi;
mod hid;
mod inventory;
mod iotask;
mod prologix;
mod scpi;
mod serial;
mod sigrok;
mod tcp;
pub mod visa;
mod vxi;

pub use comsrv_protocol as protocol;

pub type Error = comsrv_protocol::Error;
pub type Result<T> = std::result::Result<T, comsrv_protocol::Error>;
