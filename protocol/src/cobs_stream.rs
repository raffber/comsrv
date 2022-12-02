use serde::{Deserialize, Serialize};

use crate::ByteStreamInstrument;

#[derive(Clone, Serialize, Deserialize, Debug)]
pub enum CobsStreamRequest {
    Start { use_crc: bool },
    Stop,
    SendFrame { data: Vec<u8> },
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub enum CobsStreamResponse {
    Done,
    MessageReceived {
        sender: ByteStreamInstrument,
        data: Vec<u8>,
    },
    InstrumentDropped {
        error: Option<crate::Error>,
    },
}
