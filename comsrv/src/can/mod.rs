use std::fmt;
use std::fmt::Display;

use async_can::{Error, Message};
pub use async_can::Message as CanMessage;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};
use tokio::sync::mpsc;
use tokio::task;

use crate::app::{Response, RpcError, Server};
use crate::can::device::CanDevice;
use crate::can::gct::{Decoder, GctMessage};
use crate::iotask::{IoHandler, IoTask};

mod loopback;
mod device;
mod gct;

#[derive(Serialize, Deserialize, Clone)]
pub enum CanRequest {
    ListenRaw,
    ListenGct,
    StopAll,
    TxRaw(Message),
    TxGct(GctMessage),
}

#[derive(Serialize, Deserialize, Clone)]
pub enum CanResponse {
    Started(String),
    Stopped(String),
    Sent,
    Rx(Message),
    Gct(GctMessage),
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


#[derive(Debug, Error, Clone, Serialize, Deserialize)]
pub enum CanError {
    #[error("IO Error: {0}")]
    Io(String),
    #[error("Invalid interface address")]
    InvalidInterfaceAddress,
    #[error("Invalid bit rate")]
    InvalidBitRate,
    #[error("PCan error {0}: {1}")]
    PCanError(u32, String),
    #[error("Error in CAN bus: {0}")]
    BusError(async_can::BusError),
    #[error("Transmit Queue full")]
    TransmitQueueFull,
    #[error("Id is too long")]
    IdTooLong,
    #[error("Data is too long")]
    DataTooLong,
}

impl From<async_can::Error> for CanError {
    fn from(err: async_can::Error) -> Self {
        match err {
            Error::Io(err) => CanError::Io(format!("{}", err)),
            Error::InvalidInterfaceAddress => CanError::InvalidInterfaceAddress,
            Error::InvalidBitRate => CanError::InvalidBitRate,
            Error::PCanInitFailed(code, desc) => CanError::PCanError(code, desc),
            Error::PCanWriteFailed(code, desc) => CanError::PCanError(code, desc),
            Error::PCanReadFailed(code, desc) => CanError::PCanError(code, desc),
            Error::BusError(err) => CanError::BusError(err),
            Error::TransmitQueueFull => CanError::TransmitQueueFull,
            Error::IdTooLong => CanError::IdTooLong,
            Error::DataTooLong => CanError::DataTooLong,
        }
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

    pub fn check_disconnect(&self, err: &crate::Error) -> bool {
        match &err {
            crate::Error::Io(_) | crate::Error::Disconnected => {
                true
            }
            crate::Error::Can { addr: _, err } => {
                match err {
                    CanError::Io(_) | CanError::InvalidInterfaceAddress | CanError::InvalidBitRate | CanError::PCanError(_, _) => true,
                    _ => false,
                }
            }
            _ => false
        }
    }
}

struct Handler {
    addr: CanAddress,
    server: Server,
    device: Option<CanDevice>,
    listener: Option<UnboundedSender<ListenerMsg>>,
}

impl Handler {
    fn check_listener(&mut self) {
        if let Some(tx) = self.listener.as_ref() {
            // XXX: this is a hacky way to tell if the channel has been closed.
            // this will be fixed in tokio-0.3.x (and 1.x) series
            // by introduction of the is_closed() function
            if tx.send(ListenerMsg::Ping).is_err() {
                // there was some error... drop listener and device
                self.listener.take();
                self.device.take();
            }
        }
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
        self.check_listener();
        if self.device.is_none() {
            self.device.replace(CanDevice::new(self.addr.clone())?);
        }
        if self.listener.is_none() {
            let device = CanDevice::new(self.addr.clone())?;
            let (tx, rx) = mpsc::unbounded_channel();
            let fut = listener_task(rx, device, self.server.clone());
            task::spawn(fut);
            self.listener.replace(tx);
        }
        // save because we just created it
        let device = self.device.as_ref().unwrap();
        let listener = self.listener.as_ref().unwrap();

        match req {
            CanRequest::ListenRaw => {
                let _ = listener.send(ListenerMsg::StartRaw);
                Ok(CanResponse::Started(self.addr.interface()))
            }
            CanRequest::StopAll => {
                let _ = listener.send(ListenerMsg::StopAll);
                Ok(CanResponse::Stopped(self.addr.interface()))
            }
            CanRequest::TxRaw(msg) => {
                device.send(msg).await?;
                Ok(CanResponse::Sent)
            }
            CanRequest::ListenGct => {
                let _ = listener.send(ListenerMsg::StartGct);
                Ok(CanResponse::Started(self.addr.interface()))
            }
            CanRequest::TxGct(msg) => {
                for msg in gct::encode(msg) {
                    device.send(msg).await?;
                }
                Ok(CanResponse::Sent)
            }
        }
    }
}


enum ListenerMsg {
    StartGct,
    StopGct,
    StartRaw,
    StopRaw,
    StopAll,
    Ping,
}


struct Listener {
    listen_gct: bool,
    listen_raw: bool,
    decoder: Decoder,
    server: Server,
    device: CanDevice,
}

impl Listener {
    fn rx_control(&mut self, msg: ListenerMsg) {
        match msg {
            ListenerMsg::StartGct => {
                if !self.listen_gct {
                    self.decoder.reset();
                }
                self.listen_gct = true;
            }
            ListenerMsg::StopGct => {
                self.listen_gct = false;
                self.decoder.reset();
            }
            ListenerMsg::StartRaw => {
                self.listen_raw = true;
            }
            ListenerMsg::StopRaw => {
                self.listen_raw = false;
            }
            ListenerMsg::StopAll => {
                self.listen_raw = false;
                self.listen_gct = false;
            }
            ListenerMsg::Ping => {}
        }
    }

    async fn rx(&mut self, msg: Message) {
        if self.listen_raw {
            let tx = Response::Can(CanResponse::Rx(msg.clone()));
            self.server.broadcast(tx).await;
        }
        if self.listen_gct {
            if let Some(msg) = self.decoder.decode(msg) {
                let msg = Response::Can(CanResponse::Gct(msg));
                self.server.broadcast(msg).await;
            }
        }
    }

    async fn err(&mut self, err: CanError) -> bool {
        let send_err = RpcError::Can {
            addr: self.device.address().into(),
            err: err.clone(),
        };
        self.server.broadcast(Response::Error(send_err)).await;
        // depending on error, continue listening or quit...
        match err {
            CanError::Io(_) | CanError::InvalidInterfaceAddress | CanError::InvalidBitRate | CanError::PCanError(_, _) => {
                let tx = Response::Can(CanResponse::Stopped(self.device.address().interface()));
                self.server.broadcast(tx).await;
                false
            }
            _ => true
        }
    }
}

async fn listener_task(mut rx: UnboundedReceiver<ListenerMsg>, device: CanDevice, server: Server) {
    let mut listener = Listener {
        listen_gct: false,
        listen_raw: false,
        decoder: Decoder::new(),
        server,
        device,
    };
    loop {
        tokio::select! {
            msg = rx.recv() => match msg {
                Some(msg) => listener.rx_control(msg),
                None => break
            },
            msg = listener.device.recv() => match msg {
                Ok(msg) => listener.rx(msg).await,
                Err(err) => if !listener.err(err).await { break; }
            }
        }
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn loopback() {
        let srv = Server::new();

        let (_, mut client) = srv.loopback().await;

        let mut instr = Instrument::new(&srv, CanAddress::Loopback);
        let resp = instr.request(CanRequest::ListenRaw).await;
        let _expected_resp = CanResponse::Started(CanAddress::Loopback.interface());
        assert!(matches!(resp, Ok(_expected_resp)));

        let msg = CanMessage::new_data(0xABCD, true, &[1, 2, 3, 4]).unwrap();
        let sent = instr.request(CanRequest::TxRaw(msg)).await;
        assert!(matches!(sent, Ok(CanResponse::Sent)));

        let rx = client.next().await.unwrap();
        let resp = if let wsrpc::Response::Notify(x) = rx { x } else { panic!() };
        let msg = if let Response::Can(CanResponse::Rx(msg)) = resp { msg } else { panic!() };
        let msg = if let Message::Data(msg) = msg { msg } else { panic!() };
        assert_eq!(msg.dlc(), 4);
        assert_eq!(&msg.data(), &[1, 2, 3, 4]);
        assert!(msg.ext_id());
    }
}