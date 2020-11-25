use std::fmt;
use std::fmt::Display;

use async_can::Message;
pub use async_can::Message as CanMessage;
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};
use tokio::sync::mpsc;
use tokio::task;

use crate::app::{Response, Server};
use crate::can::device::CanDevice;
use crate::iotask::{IoHandler, IoTask};

mod loopback;
mod device;

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

#[derive(Clone)]
pub struct Instrument {
    io: IoTask<Handler>,
}

impl Instrument {
    pub fn new(server: &Server, addr: CanAddress) -> Self {
        let handler = Handler {
            addr,
            server: server.clone(),
            device: None,
            listener: None,
        };
        Self {
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

#[async_trait::async_trait]
impl IoHandler for Handler {
    type Request = CanRequest;
    type Response = CanResponse;

    async fn handle(&mut self, req: Self::Request) -> crate::Result<Self::Response> {
        // TODO: this is missing reconfigurable bitrate
        // just embed bitrate into CanRequest
        // note that we don't generally support this anyways for socketcan...
        // TODO: we should support a manual drop in the root API, such that this can be worked around
        if self.device.is_none() {
            self.device.replace(CanDevice::new(self.addr.clone())?);
        }
        // save because we just created it
        let device = self.device.as_ref().unwrap();

        match req {
            CanRequest::Start => {
                if let Some(tx) = self.listener.as_ref() {
                    // XXX: this is a hacky way to tell if the channel has been closed.
                    // this will be fixed in tokio-0.3.x (and 1.x) series
                    if tx.send(ListenerMsg::Ping).is_err() {
                        self.listener.take();
                    }
                }
                if self.listener.is_none() {
                    let device = CanDevice::new(self.addr.clone())?;
                    let (tx, rx) = mpsc::unbounded_channel();
                    let fut = listener_task(rx, device, self.server.clone());
                    task::spawn(fut);
                    self.listener.replace(tx);
                }
                Ok(CanResponse::Started)
            }
            CanRequest::Stop => {
                if let Some(tx) = self.listener.take() {
                    let _ = tx.send(ListenerMsg::Stop);
                }
                Ok(CanResponse::Stopped)
            }
            CanRequest::Send(msg) => {
                device.send(msg).await?;
                Ok(CanResponse::Sent)
            }
        }
    }
}


enum ListenerMsg {
    Stop,
    Ping,
}

async fn listener_task(mut rx: UnboundedReceiver<ListenerMsg>, device: CanDevice, server: Server) {
    loop {
        let msg: crate::Result<CanMessage> = tokio::select! {
            msg = rx.recv() => match msg {
                Some(ListenerMsg::Ping) => continue,
                Some(ListenerMsg::Stop) => break, // stop command
                None => break, // instrument dropped
            },
            msg = device.recv() => msg
        };
        match msg {
            Ok(msg) => server.broadcast(Response::Can(CanResponse::Rx(msg))).await,
            Err(_) => {
                // TODO: should probably try to reconnect here
                rx.close();
                break;
            }
        }
    }
}