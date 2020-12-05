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

pub mod app;
mod bytestream;
mod can;
mod cobs;
mod instrument;
mod inventory;
mod iotask;
mod modbus;
mod serial;
mod tcp;
mod util;
pub mod visa;
mod vxi;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum ScpiRequest {
    Write(String),
    QueryString(String),
    QueryBinary(String),
    ReadRaw,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum ScpiResponse {
    Done,
    String(String),
    Binary {
        #[serde(
            serialize_with = "util::to_base64",
            deserialize_with = "util::from_base64"
        )]
        data: Vec<u8>,
    },
}

#[derive(Error, Debug, Clone)]
pub enum Error {
    #[error("Error while communicating with device: {0}")]
    Visa(VisaError),
    #[error("IO Error occurred: {0}")]
    Io(Arc<io::Error>),
    #[error("Instrument is disconnected")]
    Disconnected,
    #[error("Operation not supported")]
    NotSupported,
    #[error("Cannot decode: {0}")]
    DecodeError(FromUtf8Error),
    #[error("Invalid binary header")]
    InvalidBinaryHeader,
    #[error("String message not terminated")]
    NotTerminated,
    #[error("Invalid request data")]
    InvalidRequest,
    #[error("Invalid Address")]
    InvalidAddress,
    #[error("Timeout Occured")]
    Timeout,
    #[error("Vxi11 Error")]
    Vxi(Arc<async_vxi11::Error>),
    #[error("CAN Error from [{addr}]: {err}")]
    Can { addr: String, err: CanError },
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
