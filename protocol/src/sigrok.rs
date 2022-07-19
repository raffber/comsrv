use std::collections::HashMap;

use serde::{Serialize, Deserialize};

#[derive(Clone, Serialize, Deserialize)]
pub struct SigrokInstrument {
    pub address: String,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct SigrokRequest {
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub channels: Vec<String>,
    pub acquire: SigrokAcquire,
    pub sample_rate: u64,
}

#[derive(Clone, Serialize, Deserialize)]
pub enum SigrokAcquire {
    Time(f32),
    Samples(u64),
}

#[derive(Clone, Serialize, Deserialize)]
pub enum SigrokResponse {
    Data(SigrokData),
    Devices(Vec<SigrokDevice>),
}

#[derive(Clone, Serialize, Deserialize)]
pub struct SigrokDevice {
    pub addr: String,
    pub desc: String,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct SigrokData {
    pub tsample: f64,
    pub length: usize,
    pub channels: HashMap<String, Vec<u8>>,
}
