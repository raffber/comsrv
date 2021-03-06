use std::fmt;
use std::fmt::Display;

use serde::{Deserialize, Serialize};
use thiserror::Error;
use tokio::sync::mpsc;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};
use tokio::task;

use crate::address::Address;
use crate::app::Server;
use crate::can::device::{CanReceiver, CanSender};
use crate::can::gct::Decoder;
use crate::iotask::{IoHandler, IoTask};
use async_can::{CanFrameError, Error};
use comsrv_protocol::{CanMessage, CanRequest, CanResponse, DataFrame, RemoteFrame, Response};
use tokio::sync::oneshot;

mod crc;
mod device;
mod gct;
mod loopback;

pub fn into_protocol_message(msg: async_can::Message) -> CanMessage {
    match msg {
        async_can::Message::Data(x) => CanMessage::Data(DataFrame {
            id: x.id(),
            ext_id: x.ext_id(),
            data: x.data().to_vec(),
        }),
        async_can::Message::Remote(x) => CanMessage::Remote(RemoteFrame {
            id: x.id(),
            ext_id: x.ext_id(),
            dlc: x.dlc(),
        }),
    }
}

pub fn into_async_can_message(msg: CanMessage) -> Result<async_can::Message, CanFrameError> {
    match msg {
        CanMessage::Data(x) => async_can::Message::new_data(x.id, x.ext_id, &x.data),
        CanMessage::Remote(x) => async_can::Message::new_remote(x.id, x.ext_id, x.dlc),
    }
}

#[derive(Clone, Hash)]
pub enum CanAddress {
    PCan { ifname: String, bitrate: u32 },
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

    pub fn bitrate(&self) -> Option<u32> {
        match self {
            CanAddress::PCan { ifname: _, bitrate } => Some(*bitrate),
            _ => None,
        }
    }

    pub fn update_bitrate(&mut self, new_bitrate: u32) {
        match self {
            CanAddress::PCan { ifname: _, bitrate } => {
                *bitrate = new_bitrate;
            }
            _ => {}
        }
    }
}

impl From<CanAddress> for String {
    fn from(addr: CanAddress) -> Self {
        match addr {
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
    #[error("Message is not valid")]
    InvalidMessage,
    #[error("Other: {0}")]
    Other(String),
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
            Error::PCanUnknownInterfaceType(x) => CanError::Other(format!("Unknown interface type: {}", x)),
            Error::PCanOtherError(code, desc) => CanError::PCanError(code, desc),
            Error::Other(x) => CanError::Other(x),
        }
    }
}

impl From<CanFrameError> for CanError {
    fn from(x: CanFrameError) -> Self {
        let err: async_can::Error = x.into();
        err.into()
    }
}

#[derive(Clone)]
pub struct Instrument {
    io: IoTask<Handler>,
}

pub struct Request {
    inner: CanRequest,
    bitrate: Option<u32>,
}

impl Request {
    pub fn from_request_and_address(
        request: CanRequest,
        address: Address,
    ) -> crate::Result<Request> {
        let addr = match address {
            Address::Can { addr } => addr,
            _ => return Err(crate::Error::InvalidAddress),
        };
        Ok(Request {
            inner: request,
            bitrate: addr.bitrate(),
        })
    }
}

impl Instrument {
    pub fn new(server: &Server, addr: CanAddress) -> Self {
        let handler = Handler {
            addr,
            server: server.clone(),
            sender: None,
            listener: None,
            loopback: false,
        };
        Self {
            io: IoTask::new(handler),
        }
    }

    pub async fn request(&mut self, req: Request) -> crate::Result<CanResponse> {
        self.io.request(req).await
    }

    pub fn disconnect(mut self) {
        self.io.disconnect();
    }

    pub fn check_disconnect(&self, err: &crate::Error) -> bool {
        match &err {
            crate::Error::Io(_) | crate::Error::Disconnected => true,
            crate::Error::Can { addr: _, err } => {
                matches!(
                    err,
                    CanError::Io(_)
                        | CanError::InvalidInterfaceAddress
                        | CanError::InvalidBitRate
                        | CanError::PCanError(_, _)
                )
            }
            _ => false,
        }
    }
}

struct Handler {
    addr: CanAddress,
    server: Server,
    sender: Option<CanSender>,
    listener: Option<UnboundedSender<ListenerMsg>>,
    loopback: bool,
}

impl Handler {
    async fn check_listener(&mut self) {
        if let Some(tx) = self.listener.as_ref() {
            if tx.is_closed() {
                self.listener.take();
                if let Some(device) = self.sender.take() {
                    let _ = device.close().await;
                }
            }
        }
    }

    async fn close_listener(&mut self) {
        if let Some(listener) = self.listener.take() {
            let (tx, rx) = oneshot::channel();
            if listener.send(ListenerMsg::Close(tx)).is_ok() {
                let _ = rx.await;
                log::debug!("{} - Listener closed", self.addr);
            } // else it's already gone
        }
    }

    async fn update_bitrate(&mut self, req: &Request) {
        if let Some(bitrate) = req.bitrate {
            if self.addr.bitrate() != req.bitrate {
                log::debug!("{} - Updating Bitrate", self.addr);
                self.close_listener().await;
                if let Some(sender) = self.sender.take() {
                    let _ = sender.close().await;
                }
                log::debug!("{} - Sender closed", self.addr);
                self.addr.update_bitrate(bitrate);
            }
        }
    }

    async fn handle_request(&mut self, req: &Request) -> crate::Result<CanResponse> {
        // save because we just created it
        let device = self.sender.as_ref().unwrap();
        let listener = self.listener.as_ref().unwrap();

        match &req.inner {
            CanRequest::ListenRaw(en) => {
                let _ = listener.send(ListenerMsg::EnableRaw(*en));
                Ok(CanResponse::Started(self.addr.interface()))
            }
            CanRequest::StopAll => {
                let _ = listener.send(ListenerMsg::EnableRaw(false));
                let _ = listener.send(ListenerMsg::EnableGct(false));
                Ok(CanResponse::Stopped(self.addr.interface()))
            }
            CanRequest::TxRaw(msg) => {
                if self.loopback {
                    let _ = listener.send(ListenerMsg::Loopback(msg.clone()));
                }
                device.send(msg.clone()).await?;
                Ok(CanResponse::Ok)
            }
            CanRequest::ListenGct(en) => {
                let _ = listener.send(ListenerMsg::EnableGct(*en));
                Ok(CanResponse::Started(self.addr.interface()))
            }
            CanRequest::TxGct(msg) => {
                let msgs = gct::encode(msg.clone()).map_err(|err| crate::Error::Can {
                    addr: self.addr.interface(),
                    err,
                })?;
                for msg in msgs {
                    if self.loopback {
                        let _ = listener.send(ListenerMsg::Loopback(msg.clone()));
                    }
                    device.send(msg).await?;
                }
                Ok(CanResponse::Ok)
            }
            CanRequest::EnableLoopback(en) => {
                self.loopback = *en;
                Ok(CanResponse::Ok)
            }
        }
    }
}

#[async_trait::async_trait]
impl IoHandler for Handler {
    type Request = Request;
    type Response = CanResponse;

    async fn handle(&mut self, req: Self::Request) -> crate::Result<Self::Response> {
        self.check_listener().await;
        self.update_bitrate(&req).await;

        if self.sender.is_none() {
            log::debug!("{} - Initializing new Sender", self.addr);
            self.sender.replace(CanSender::new(self.addr.clone())?);
        }
        if self.listener.is_none() {
            log::debug!("{} - Initializing new Receiver", self.addr);
            let device = CanReceiver::new(self.addr.clone())?;
            let (tx, rx) = mpsc::unbounded_channel();
            let fut = listener_task(rx, device, self.server.clone(), self.addr.clone());
            task::spawn(fut);
            self.listener.replace(tx);
        }
        let mut retries = 0;
        let ret = loop {
            let ret = self.handle_request(&req).await;
            if let Err(err) = ret {
                retries += 1;
                if retries > 3 {
                    break err;
                }
                if err.should_retry() {
                    self.sender.take();
                    self.listener.take();
                } else {
                    break err;
                }
            } else {
                return ret;
            }
        };
        Err(ret)
    }
}

enum ListenerMsg {
    EnableGct(bool),
    EnableRaw(bool),
    Loopback(CanMessage),
    Close(oneshot::Sender<()>),
}

struct Listener {
    listen_gct: bool,
    listen_raw: bool,
    decoder: Decoder,
    server: Server,
    device: Option<CanReceiver>,
    address: CanAddress,
}

impl Listener {
    async fn rx_control(&mut self, msg: ListenerMsg) -> bool {
        match msg {
            ListenerMsg::EnableGct(en) => {
                if !self.listen_gct {
                    self.decoder.reset();
                }
                self.listen_gct = en;
                true
            }
            ListenerMsg::EnableRaw(en) => {
                self.listen_raw = en;
                true
            }
            ListenerMsg::Loopback(msg) => {
                self.rx(msg);
                true
            }
            ListenerMsg::Close(fut) => {
                if let Some(device) = self.device.take() {
                    let _ = device.close().await;
                    let _ = fut.send(());
                }
                false
            }
        }
    }

    fn rx(&mut self, msg: CanMessage) {
        log::debug!("CAN received - ID = {:x}", msg.id());
        if self.listen_raw {
            let tx = Response::Can(CanResponse::Raw(msg.clone()));
            log::debug!(
                "Broadcast raw CAN message: {}",
                serde_json::to_string(&msg).unwrap()
            );
            self.server.broadcast(tx);
        }
        if self.listen_gct {
            if let Some(msg) = self.decoder.decode(msg) {
                log::debug!(
                    "Broadcast GCT CAN message: {}",
                    serde_json::to_string(&msg).unwrap()
                );
                let msg = Response::Can(CanResponse::Gct(msg));
                self.server.broadcast(msg);
            }
        }
    }

    async fn err(&mut self, err: CanError) -> bool {
        if let Some(device) = &self.device {
            let send_err = crate::Error::Can {
                addr: device.address().into(),
                err: err.clone(),
            };
            self.server.broadcast(send_err.into());
            // depending on error, continue listening or quit...
            match err {
                CanError::Io(_)
                | CanError::InvalidInterfaceAddress
                | CanError::InvalidBitRate
                | CanError::PCanError(_, _) => {
                    let tx = Response::Can(CanResponse::Stopped(device.address().interface()));
                    self.server.broadcast(tx);
                    false
                }
                _ => true,
            }
        } else {
            false
        }
    }

    async fn recv(&mut self) -> Option<Result<CanMessage, CanError>> {
        match &mut self.device {
            None => None,
            Some(device) => Some(device.recv().await),
        }
    }
}

async fn listener_task(
    mut rx: UnboundedReceiver<ListenerMsg>,
    device: CanReceiver,
    server: Server,
    address: CanAddress,
) {
    let mut listener = Listener {
        listen_gct: true,
        listen_raw: true,
        decoder: Decoder::new(),
        server,
        device: Some(device),
        address,
    };
    loop {
        tokio::select! {
            msg = rx.recv() => match msg {
                Some(msg) => {
                    if !listener.rx_control(msg).await {
                        break;
                    }
                },
                None => break
            },
            msg = listener.recv() => match msg {
                Some(Ok(x)) => listener.rx(x),
                Some(Err(err)) => if !listener.err(err).await { break; },
                None => { break },
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn loopback() {
        let (srv, _) = Server::new();

        let mut client = srv.loopback().await;

        let mut instr = Instrument::new(&srv, CanAddress::Loopback);

        let req = Request {
            inner: CanRequest::ListenRaw(true),
            bitrate: None,
        };
        let resp = instr.request(req).await;
        let _expected_resp = CanResponse::Started(CanAddress::Loopback.interface());
        assert!(matches!(resp, Ok(_expected_resp)));

        let msg = CanMessage::Data(DataFrame {
            id: 0xABCD,
            ext_id: true,
            data: vec![1, 2, 3, 4],
        });
        let req = Request {
            inner: CanRequest::TxRaw(msg),
            bitrate: None,
        };
        let sent = instr.request(req).await;
        assert!(matches!(sent, Ok(CanResponse::Ok)));

        let rx = client.next().await.unwrap();
        let resp = if let wsrpc::Response::Notify(x) = rx {
            x
        } else {
            panic!()
        };
        let msg = if let Response::Can(CanResponse::Raw(msg)) = resp {
            msg
        } else {
            panic!()
        };
        let msg = if let CanMessage::Data(msg) = msg {
            msg
        } else {
            panic!()
        };
        assert_eq!(msg.data.len(), 4);
        assert_eq!(&msg.data, &[1, 2, 3, 4]);
        assert!(msg.ext_id);
    }
}
