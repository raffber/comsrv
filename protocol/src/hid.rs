use serde::{Deserialize, Serialize};

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct HidInstrument {
    pub address: HidIdentifier,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub enum HidRequest {
    Write { data: Vec<u8> },
    Read { timeout_ms: i32 },
    GetInfo,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub enum HidResponse {
    Ok,
    Data(Vec<u8>),
    Info(HidDeviceInfo),
    List(Vec<HidDeviceInfo>),
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct HidDeviceInfo {
    pub idn: HidIdentifier,
    pub manufacturer: Option<String>,
    pub product: Option<String>,
    pub serial_number: Option<String>,
}

#[derive(Hash, Clone, Serialize, Deserialize, Debug, PartialEq, Eq)]
pub struct HidIdentifier {
    pub pid: u16,
    pub vid: u16,
}

impl HidIdentifier {
    pub fn new(vid: u16, pid: u16) -> Self {
        HidIdentifier { pid, vid }
    }

    pub fn pid(&self) -> u16 {
        self.pid
    }

    pub fn vid(&self) -> u16 {
        self.vid
    }
}
