use serde::{Deserialize, Serialize};
use async_can::Message;

#[derive(Clone, Serialize, Deserialize)]
pub enum GctMessage {}

pub struct Decoder {}

impl Decoder {
    pub fn new() -> Self {
        Self {}
    }

    pub fn reset(&mut self) {}

    pub fn decode(&mut self, msg: Message) -> Option<GctMessage> {
        None
    }
}

pub fn encode(msg: GctMessage) -> Vec<Message> {
    todo!()
}

