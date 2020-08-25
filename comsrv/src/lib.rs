
#![allow(non_snake_case)]
#![allow(dead_code)]

#[macro_use]
extern crate dlopen_derive;
#[macro_use]
extern crate lazy_static;

use std::io;

use visa::VisaError;
use serde::{Serialize, Deserialize};
use thiserror::Error;
use std::sync::Arc;

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
}

pub type Result<T> = std::result::Result<T, Error>;
