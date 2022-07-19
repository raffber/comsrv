use serde::{Deserialize, Serialize};

use crate::bytestream::SerialAddress;

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct VxiInstrument {
    address: String, 
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct VisaInstrument {
    address: String,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct PrologixInstrument {
    address: SerialAddress,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct PrologixRequest {
    addr: u8,
    request: ScpiRequest,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub enum ScpiInstrument {
    Vxi(VxiInstrument),
    Visa(VisaInstrument),
}

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
        serialize_with = "crate::util::to_base64",
        deserialize_with = "crate::util::from_base64"
        )]
        data: Vec<u8>,
    },
}