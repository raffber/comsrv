use async_can::CanMessage;

#[derive(Serialize, Deserialize, Clone)]
pub enum CanRequest {
    Start,
    Stop,
    Send(CanMessage),
}

#[derive(Serialize, Deserialize, Clone)]
pub enum CanResponse {
    Started,
    Stopped,
    MessageSent,
    MessageReceived(CanMessage),
}

