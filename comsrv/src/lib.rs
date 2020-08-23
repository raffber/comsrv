#![allow(non_snake_case)]
#![allow(dead_code)]

#[macro_use]
extern crate dlopen_derive;
#[macro_use]
extern crate lazy_static;

use std::io;

use visa::VisaError;

mod inventory;
pub mod visa;

pub enum Error {
    Visa(VisaError),
    Io(io::Error),
    Disconnected,
    ChannelBroken,
    AlreadyConnecting,
}

pub type Result<T> = std::result::Result<T, Error>;

