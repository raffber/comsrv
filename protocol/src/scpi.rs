use serde::{Deserialize, Serialize};

use crate::bytestream::SerialAddress;

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct VxiInstrument {
    pub host: String,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct VisaInstrument {
    pub address: String,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct PrologixInstrument {
    pub address: SerialAddress,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct PrologixRequest {
    pub addr: u8,
    pub scpi: ScpiRequest,
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
