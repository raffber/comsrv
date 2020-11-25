mod loopback;

use async_can::Message;
use serde::{Serialize, Deserialize};
use crate::app::{App, Server};

use std::fmt::Display;
use std::fmt;
use crate::can::loopback::LoopbackDevice;
use async_can::Bus as CanBus;
use crate::iotask::{IoTask, IoHandler};

#[derive(Serialize, Deserialize, Clone)]
pub enum CanRequest {
    Start,
    Stop,
    Send(Message),
}

#[derive(Serialize, Deserialize, Clone)]
pub enum CanResponse {
    Started,
    Stopped,
    Sent,
    Rx(Message),
}

#[derive(Clone, Hash)]
pub enum CanAddress {
    PCan,
    Socket(String),
    Loopback,
}

impl Into<String> for CanAddress {
    fn into(self) -> String {
        match self {
            CanAddress::PCan => "pcan".to_string(),
            CanAddress::Socket(x) => format!("socket::{}", x),
            CanAddress::Loopback => "loopback".to_string(),
        }
    }
}

impl Display for CanAddress {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let x: String = self.clone().into();
        f.write_str(&x)
    }
}

enum CanDevice {
    Loopback(LoopbackDevice),
    Bus(CanBus)
}

impl CanDevice {
    fn new(ifname: &str) -> crate::Result<Self> {
        todo!()
    }
}

#[derive(Clone)]
pub struct Instrument {
    addr: CanAddress,
    io: IoTask<Handler>,
}


impl Instrument {
    pub fn new(server: &Server , addr: CanAddress) -> Self {
        let handler = Handler {
            addr: addr.clone(),
            server: server.clone()
        };
        Self {
            addr,
            io: IoTask::new(handler)
        }
    }

    pub async fn request(&mut self, req: CanRequest) -> crate::Result<CanResponse> {
        self.io.request(req).await
    }

    pub fn disconnect(&self) {
        todo!()
    }
}

struct Handler {
    addr: CanAddress,
    server: Server,
}

#[async_trait::async_trait]
impl IoHandler for Handler {
    type Request = CanRequest;
    type Response = CanResponse;

    async fn handle(&mut self, req: Self::Request) -> crate::Result<Self::Response> {
        unimplemented!()
    }
}

