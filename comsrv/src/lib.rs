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

mod instrument;
mod prologix;
mod modbus;
mod inventory;
pub mod visa;
pub mod app;


#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum ScpiRequest {
    Write(String),
    QueryString(String),
    QueryBinary(String),
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum ScpiResponse {
    Done,
    String(String),
    Binary(Vec<u8>),
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
    #[error("Cannot connect")]
    CannotConnect,
    #[error("Cannot decode: {0}")]
    DecodeError(FromUtf8Error),
    #[error("Invalid binary header")]
    InvalidBinaryHeader,
    #[error("String message not terminated")]
    NotTerminated,
    #[error("Invalid Address")]
    InvalidAddress,
    #[error("Timeout Occured")]
    Timeout,
}

impl Error {
    pub fn io(err: io::Error) -> Error {
        Error::Io(Arc::new(err))
    }
}

pub type Result<T> = std::result::Result<T, Error>;
