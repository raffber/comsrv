#![allow(non_snake_case)]
#![allow(dead_code)]

#[macro_use]
extern crate dlopen_derive;
#[macro_use]
extern crate lazy_static;

pub use comsrv_protocol::{Request, Response};

pub mod app;
mod inventory;
mod iotask;
mod protocol;
mod transport;

pub use comsrv_protocol as rpc;

pub type Error = comsrv_protocol::Error;
pub type Result<T> = std::result::Result<T, comsrv_protocol::Error>;
