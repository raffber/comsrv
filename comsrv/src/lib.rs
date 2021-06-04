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
use crate::sigrok::SigrokError;

mod address;
pub mod app;
mod bytestream;
mod can;
mod clonable_channel;
mod cobs;
mod instrument;
mod inventory;
mod iotask;
mod modbus;
mod scpi;
mod serial;
mod sigrok;
mod tcp;
mod util;
pub mod visa;
mod vxi;

#[derive(Error, Debug, Clone, Serialize, Deserialize)]
pub enum Error {
    #[error("Error while communicating with device: {0}")]
    Visa(VisaError),
    #[error("IO Error occurred: {0}")]
    #[serde(
        serialize_with = "serialize::io_error",
        deserialize_with = "deserialize::io_error"
    )]
    Io(Arc<io::Error>),
    #[error("Instrument is disconnected")]
    Disconnected,
    #[error("Operation not supported")]
    NotSupported,
    #[error("Cannot decode: {0}")]
    #[serde(
        serialize_with = "serialize::utf8_error",
        deserialize_with = "deserialize::utf8_error"
    )]
    DecodeError(FromUtf8Error),
    #[error("Invalid binary header")]
    InvalidBinaryHeader,
    #[error("String message not terminated")]
    NotTerminated,
    #[error("Invalid request data")]
    InvalidRequest,
    #[error("Invalid response data was received from client device")]
    InvalidResponse,
    #[error("Invalid Address")]
    InvalidAddress,
    #[error("Timeout Occured")]
    Timeout,
    #[error("Vxi11 Error")]
    #[serde(
        serialize_with = "serialize::vxi_error",
        deserialize_with = "deserialize::vxi_error"
    )]
    Vxi(Arc<async_vxi11::Error>),
    #[error("CAN Error from [{addr}]: {err}")]
    Can { addr: String, err: CanError },
    #[error("Sigrok error: {0}")]
    Sigrok(SigrokError),
}

impl Error {
    pub fn io(err: io::Error) -> Error {
        Error::Io(Arc::new(err))
    }

    pub fn vxi(err: async_vxi11::Error) -> Error {
        match err {
            async_vxi11::Error::Io(x) => Error::io(x),
            x => Error::Vxi(Arc::new(x)),
        }
    }

    pub fn should_retry(&self) -> bool {
        match self {
            Error::Io(err) => {
                err.kind() == io::ErrorKind::ConnectionReset
                    || err.kind() == io::ErrorKind::ConnectionAborted
                    || err.kind() == io::ErrorKind::BrokenPipe
                    || err.kind() == io::ErrorKind::TimedOut
                    || err.kind() == io::ErrorKind::UnexpectedEof
            }
            _ => false,
        }
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
