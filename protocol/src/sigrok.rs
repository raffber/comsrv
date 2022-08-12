use std::collections::HashMap;

use serde::{Serialize, Deserialize};

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct SigrokInstrument {
    pub address: String,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct SigrokRequest {
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub channels: Vec<String>,
    pub acquire: SigrokAcquire,
    pub sample_rate: u64,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub enum SigrokAcquire {
    Time(f32),
    Samples(u64),
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub enum SigrokResponse {
    Data(SigrokData),
    Devices(Vec<SigrokDevice>),
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct SigrokDevice {
    pub addr: String,
    pub desc: String,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct SigrokData {
    pub tsample: f64,
    pub length: usize,
    pub channels: HashMap<String, Vec<u8>>,
}
