#![allow(non_snake_case)]
#![allow(dead_code)]

mod inventory;
pub mod visa;

#[macro_use] extern crate dlopen_derive;
#[macro_use] extern crate lazy_static;

use visa::VisaError;
use std::io;


pub enum Error {
    Visa(VisaError),
    Io(io::Error),
    ChannelBroken,
}

pub type Result<T> = std::result::Result<T, Error>;

