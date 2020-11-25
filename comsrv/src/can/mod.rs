mod loopback;

use async_can::{Message, Bus, Error};
use serde::{Serialize, Deserialize};
use crate::app::{App, Server};

use std::fmt::Display;
use std::fmt;
use crate::can::loopback::LoopbackDevice;
use async_can::Bus as CanBus;
use crate::iotask::{IoTask, IoHandler};
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};
use tokio::sync::mpsc;
use tokio::task;

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
    PCan {
        ifname: String,
        bitrate: u32,
    },
    Socket(String),
    Loopback,
}

impl CanAddress {
    pub fn interface(&self) -> String {
        match self {
            CanAddress::PCan { ifname, .. } => ifname.clone(),
            CanAddress::Socket(ifname) => ifname.clone(),
            CanAddress::Loopback => "loopback".to_string(),
        }
    }
}

impl Into<String> for CanAddress {
    fn into(self) -> String {
        match self {
            CanAddress::PCan { ifname, bitrate } => format!("pcan::{}::{}", ifname, bitrate),
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

impl From<async_can::Error> for crate::Error {
    fn from(x: Error) -> Self {
        match x {
            Error::Io(err) => crate::Error::io(err),
        }
    }
}

enum CanDevice {
    Loopback(LoopbackDevice),
    Bus(CanBus),
}

impl CanDevice {
    async fn send(&self, msg: async_can::Message) -> crate::Result<()> {
        todo!()
    }
}

#[cfg(target_os = "linux")]
impl CanDevice {
    fn new(addr: CanAddress) -> crate::Result<Self> {
        match addr {
            CanAddress::PCan { .. } => {
                Err(crate::Error::NotSupported)
            }
            CanAddress::Socket(ifname) => {
                Ok(CanDevice::Bus(CanBus::connect(ifname)?))
            }
            CanAddress::Loopback => {
                Ok(CanDevice::Loopback(LoopbackDevice::new()))
            }
        }
    }
}

#[cfg(target_os = "windows")]
impl CanDevice {
    fn new(addr: CanAddress) -> crate::Result<Self> {
        match addr {
            CanAddress::PCan(ifname) => {
                Ok(CanDevice::Bus(CanBus::connect(ifname)?))
            }
            CanAddress::Socket(_) => {
                Err(crate::Error::NotSupported)
            }
            CanAddress::Loopback => {
                Ok(CanDevice::Loopback(LoopbackDevice::new()))
            }
        }
    }
}

#[derive(Clone)]
pub struct Instrument {
    addr: CanAddress,
    io: IoTask<Handler>,
}

impl Instrument {
    pub fn new(server: &Server, addr: CanAddress) -> Self {
        let handler = Handler {
            addr: addr.clone(),
            server: server.clone(),
            device: None,
            listener: None,
        };
        Self {
            addr,
            io: IoTask::new(handler),
        }
    }

    pub async fn request(&mut self, req: CanRequest) -> crate::Result<CanResponse> {
        self.io.request(req).await
    }

    pub fn disconnect(mut self) {
        self.io.disconnect();
    }
}

struct Handler {
    addr: CanAddress,
    server: Server,
    device: Option<CanDevice>,
    listener: Option<UnboundedSender<ListenerMsg>>,
}

impl Handler {
    fn create_device(&self) -> crate::Result<CanDevice> {
        todo!()
    }
}

#[async_trait::async_trait]
impl IoHandler for Handler {
    type Request = CanRequest;
    type Response = CanResponse;

    async fn handle(&mut self, req: Self::Request) -> crate::Result<Self::Response> {
        // TODO: this is missing reconfigurable bitrate
        // just embed bitrate into CanRequest
        // note that we don't generally support this anyways for socketcan...
        // TODO: we should support a manual drop in the root API, such that this can be worked around
        match req {
            CanRequest::Start => {
                if self.listener.is_none() {
                    let device = self.create_device()?;
                    let (tx, rx) = mpsc::unbounded_channel();
                    let fut = listener_task(rx, device, self.server.clone());
                    task::spawn(fut);
                    self.listener.replace(tx);
                }
                Ok(CanResponse::Started)
            },
            CanRequest::Stop => {
                if let Some(tx) = self.listener.take() {
                    tx.send(ListenerMsg::Stop);
                }
                Ok(CanResponse::Stopped)
            },
            CanRequest::Send(msg) => {
                let device = if let Some(device) = self.device.take() {
                    device
                } else {
                    self.create_device()?
                };
                device.send(msg).await?;
                Ok(CanResponse::Sent)
            },
        }
    }
}


enum ListenerMsg {
    Stop
}

async fn listener_task(rx: UnboundedReceiver<ListenerMsg>, device: CanDevice, server: Server) {
    todo!()
}