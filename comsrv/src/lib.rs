#![allow(non_snake_case)]
#![allow(dead_code)]

#[macro_use]
extern crate dlopen_derive;
#[macro_use]
extern crate lazy_static;

use std::io;
use std::string::FromUtf8Error;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use thiserror::Error;

use visa::VisaError;

use crate::can::CanError;
use crate::hid::HidError;
use crate::sigrok::SigrokError;
pub use comsrv_protocol::{Response, Request, Error};

pub mod app;
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



impl Into<Response> for Error {
    fn into(self) -> Response {
        let ret = serde_json::to_value(self).unwrap();
        Response::Error(ret)
    }
}

impl From<io::Error> for Error {
    fn from(err: io::Error) -> Self {
        Error::io(err)
    }
}

pub type Result<T> = std::result::Result<T, Error>;

mod serialize {
    use crate::io;
    use serde::Serializer;
    use std::string::FromUtf8Error;
    use std::sync::Arc;

    pub fn io_error<S>(data: &Arc<io::Error>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&data.to_string())
    }

    pub fn utf8_error<S>(data: &FromUtf8Error, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&data.to_string())
    }

    pub fn vxi_error<S>(data: &Arc<async_vxi11::Error>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&format!("{}", &*data))
    }
}

mod deserialize {
    use std::io;

    use serde::Deserializer;
    use std::string::FromUtf8Error;
    use std::sync::Arc;

    pub fn io_error<'a, D>(_: D) -> Result<Arc<io::Error>, D::Error>
    where
        D: Deserializer<'a>,
    {
        panic!()
    }

    pub fn utf8_error<'a, D>(_: D) -> Result<FromUtf8Error, D::Error>
    where
        D: Deserializer<'a>,
    {
        panic!()
    }

    pub fn vxi_error<'a, D>(_: D) -> Result<Arc<async_vxi11::Error>, D::Error>
    where
        D: Deserializer<'a>,
    {
        panic!()
    }
}
