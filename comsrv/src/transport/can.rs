/// This module is responsible for mapping CAN functionality a device to different backends
use async_can::{Receiver, Sender};

use async_trait::async_trait;
use comsrv_protocol::CanInstrument;
use tokio::sync::broadcast;

use tokio::sync::mpsc;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};
use tokio::task;

use crate::app::Server;
use crate::iotask::{IoContext, IoHandler, IoTask};
use crate::protocol::can::gct::Decoder;
use anyhow::anyhow;
use async_can::CanFrameError;
use async_can::Error as CanError;
use comsrv_protocol::{CanAddress, CanMessage, CanRequest, CanResponse, DataFrame, RemoteFrame, Response};
use tokio::sync::oneshot;

pub fn map_error(err: CanError) -> crate::Error {
    match err {
        CanError::Io(io) => crate::Error::transport(io),
        err => crate::Error::transport(anyhow!(err)),
    }
}

pub fn map_frame_error(err: CanFrameError) -> crate::Error {
    crate::Error::argument(anyhow!("{:?}", err))
}

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

#[derive(Clone)]
pub struct Instrument {
    io: IoTask<Handler>,
}

pub struct Request {
    pub inner: CanRequest,
    pub instrument: CanInstrument,
}

impl Instrument {
    pub fn new(server: &Server) -> Self {
        let handler = Handler {
            server: server.clone(),
            sender: None,
            listener: None,
            loopback: false,
            last_instrument: None,
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
}

#[async_trait]
impl crate::inventory::Instrument for Instrument {
    type Address = CanAddress;

    fn connect(server: &Server, _addr: &Self::Address) -> crate::Result<Self> {
        Ok(Instrument::new(server))
    }

    async fn wait_for_closed(&self) {
        self.io.wait_for_closed().await
    }
}

struct Handler {
    last_instrument: Option<CanInstrument>,
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
                log::debug!("{:?} - Listener closed", self.last_instrument);
            } // else it's already gone
        }
    }

    async fn update_bitrate(&mut self, instr: &CanInstrument) {
        match instr {
            CanInstrument::PCan { address: _, bitrate } => {
                if let Some(CanInstrument::PCan {
                    address: _,
                    bitrate: old_bitrate,
                }) = &self.last_instrument
                {
                    if old_bitrate != bitrate {
                        log::debug!("{:?} - Updating Bitrate", instr);
                        self.close_listener().await;
                        if let Some(sender) = self.sender.take() {
                            let _ = sender.close().await;
                        }
                        log::debug!("{:?} - Sender closed", instr);
                    }
                }
            }
            _ => {
                log::debug!("Updating Bitrate not supported for {:?}", instr);
            }
        };
        self.last_instrument = Some(instr.clone());
    }

    async fn handle_request(&mut self, req: &Request) -> crate::Result<CanResponse> {
        // save because we just created it
        let device = self.sender.as_ref().unwrap();
        let listener = self.listener.as_ref().unwrap();

        match &req.inner {
            CanRequest::ListenRaw(en) => {
                let _ = listener.send(ListenerMsg::EnableRaw(*en));
                Ok(CanResponse::Started)
            }
            CanRequest::StopAll => {
                let _ = listener.send(ListenerMsg::EnableRaw(false));
                let _ = listener.send(ListenerMsg::EnableGct(false));
                Ok(CanResponse::Stopped)
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
                Ok(CanResponse::Started)
            }
            CanRequest::TxGct(msg) => {
                let msgs = crate::protocol::can::gct::encode(msg.clone())?;
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

    fn make_sender(&self, instr: &CanInstrument) -> crate::Result<CanSender> {
        CanSender::new(instr)
    }

    fn make_receiver(&self, instr: &CanInstrument) -> crate::Result<CanReceiver> {
        CanReceiver::new(instr)
    }
}

#[async_trait::async_trait]
impl IoHandler for Handler {
    type Request = Request;
    type Response = CanResponse;

    async fn handle(&mut self, _ctx: &mut IoContext<Self>, req: Self::Request) -> crate::Result<Self::Response> {
        self.check_listener().await;
        self.update_bitrate(&req.instrument).await;

        if self.sender.is_none() {
            log::debug!("{:?} - Initializing new Sender", req.instrument);
            self.sender.replace(self.make_sender(&req.instrument)?);
        }
        if self.listener.is_none() {
            log::debug!("{:?} - Initializing new Receiver", req.instrument);
            let device = self.make_receiver(&req.instrument)?;
            let (tx, rx) = mpsc::unbounded_channel();
            let fut = listener_task(rx, device, self.server.clone(), req.instrument.clone());
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
    instr: CanInstrument,
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
            let tx = Response::Can {
                source: self.instr.clone().into(),
                response: CanResponse::Raw(msg.clone()),
            };
            log::debug!("Broadcast raw CAN message: {}", serde_json::to_string(&msg).unwrap());
            self.server.broadcast(tx);
        }
        if self.listen_gct {
            if let Some(msg) = self.decoder.decode(msg) {
                log::debug!("Broadcast GCT CAN message: {}", serde_json::to_string(&msg).unwrap());
                let msg = Response::Can {
                    source: self.instr.clone().into(),
                    response: CanResponse::Gct(msg),
                };
                self.server.broadcast(msg);
            }
        }
    }

    async fn err(&mut self, err: crate::Error) -> bool {
        if let Some(_) = &self.device {
            self.server.broadcast(err.clone().into());
            // depending on error, continue listening or quit...
            match err {
                crate::Error::Transport(_x) => {
                    let tx = Response::Can {
                        response: CanResponse::Stopped,
                        source: self.instr.clone().into(),
                    };
                    self.server.broadcast(tx);
                    false
                }
                _ => true,
            }
        } else {
            false
        }
    }

    async fn recv(&mut self) -> Option<crate::Result<CanMessage>> {
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
    instr: CanInstrument,
) {
    let mut listener = Listener {
        listen_gct: true,
        listen_raw: true,
        decoder: Decoder::new(),
        server,
        device: Some(device),
        instr,
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

        let mut instr = Instrument::new(&srv);

        let req = Request {
            inner: CanRequest::ListenRaw(true),
            instrument: CanInstrument::Loopback,
        };

        let resp = instr.request(req).await;
        let _expected_resp = CanResponse::Started;
        assert!(matches!(resp, Ok(_expected_resp)));

        let msg = CanMessage::Data(DataFrame {
            id: 0xABCD,
            ext_id: true,
            data: vec![1, 2, 3, 4],
        });
        let req = Request {
            inner: CanRequest::TxRaw(msg),
            instrument: CanInstrument::Loopback,
        };
        let sent = instr.request(req).await;
        assert!(matches!(sent, Ok(CanResponse::Ok)));

        let rx = client.next().await.unwrap();
        let resp = if let wsrpc::Response::Notify(x) = rx {
            x
        } else {
            panic!()
        };
        let msg = if let Response::Can {
            response: CanResponse::Raw(msg),
            ..
        } = resp
        {
            msg
        } else {
            panic!()
        };
        let msg = if let CanMessage::Data(msg) = msg { msg } else { panic!() };
        assert_eq!(msg.data.len(), 4);
        assert_eq!(&msg.data, &[1, 2, 3, 4]);
        assert!(msg.ext_id);
    }
}

pub enum CanSender {
    Loopback(LoopbackDevice),
    Bus { device: Sender, addr: CanAddress },
}

pub enum CanReceiver {
    Loopback(LoopbackDevice),
    Bus { device: Receiver, addr: CanAddress },
}

impl CanSender {
    pub async fn send(&self, msg: CanMessage) -> crate::Result<()> {
        match self {
            CanSender::Loopback(lo) => {
                lo.send(msg);
                Ok(())
            }
            CanSender::Bus { device, addr: _ } => {
                let msg = into_async_can_message(msg).map_err(map_frame_error)?;
                device.send(msg).await.map_err(map_error)
            }
        }
    }
}

impl CanReceiver {
    pub async fn recv(&mut self) -> crate::Result<CanMessage> {
        match self {
            CanReceiver::Loopback(lo) => lo.recv().await,
            CanReceiver::Bus { device, addr: _ } => Ok(into_protocol_message(device.recv().await.map_err(map_error)?)),
        }
    }

    pub fn address(&self) -> CanAddress {
        match self {
            CanReceiver::Loopback(_) => CanAddress::Loopback,
            CanReceiver::Bus { device: _, addr } => addr.clone(),
        }
    }
}

#[cfg(target_os = "linux")]
impl CanSender {
    pub fn new(instr: &CanInstrument) -> crate::Result<Self> {
        match instr {
            CanInstrument::PCan { .. } => Err(crate::Error::internal(anyhow!("Not Supported"))),
            CanInstrument::SocketCan { interface } => {
                let device = Sender::connect(interface.clone()).map_err(map_error)?;
                Ok(CanSender::Bus {
                    device,
                    addr: instr.clone().into(),
                })
            }
            CanInstrument::Loopback => Ok(CanSender::Loopback(LoopbackDevice::new())),
        }
    }

    pub async fn close(self) -> crate::Result<()> {
        Ok(())
    }
}

#[cfg(target_os = "linux")]
impl CanReceiver {
    pub fn new(instr: &CanInstrument) -> crate::Result<Self> {
        match instr {
            CanInstrument::PCan { .. } => Err(crate::Error::internal(anyhow!("Not supported"))),
            CanInstrument::SocketCan { interface } => {
                let device = Receiver::connect(interface.clone()).map_err(map_error)?;
                Ok(CanReceiver::Bus {
                    device,
                    addr: CanAddress::SocketCan {
                        interface: interface.clone(),
                    },
                })
            }
            CanInstrument::Loopback => Ok(CanReceiver::Loopback(LoopbackDevice::new())),
        }
    }

    pub async fn close(self) -> crate::Result<()> {
        Ok(())
    }
}

#[cfg(target_os = "windows")]
impl CanSender {
    pub fn new(instr: &CanInstrument) -> crate::Result<Self> {
        match instr {
            CanInstrument::PCan { address, bitrate } => {
                let device = Sender::connect(address, *bitrate).map_err(map_error)?;
                Ok(Self::Bus {
                    device,
                    addr: instr.clone().into(),
                })
            }
            CanInstrument::SocketCan { .. } => Err(crate::Error::internal(anyhow!("Not supported"))),
            CanInstrument::Loopback => Ok(Self::Loopback(LoopbackDevice::new())),
        }
    }

    pub async fn close(self) -> crate::Result<()> {
        match self {
            CanSender::Loopback(_) => Ok(()),
            CanSender::Bus { device, addr: _ } => device.close().await.map_err(map_error),
        }
    }
}

#[cfg(target_os = "windows")]
impl CanReceiver {
    pub fn new(instr: &CanInstrument) -> crate::Result<Self> {
        match instr {
            CanInstrument::PCan { address, bitrate } => {
                let device = Receiver::connect(address, *bitrate).map_err(map_error)?;
                Ok(Self::Bus {
                    device,
                    addr: instr.clone().into(),
                })
            }
            CanInstrument::SocketCan { .. } => Err(crate::Error::internal(anyhow!("Not supported"))),
            CanInstrument::Loopback => Ok(Self::Loopback(LoopbackDevice::new())),
        }
    }

    pub async fn close(self) -> crate::Result<()> {
        Ok(())
    }
}

const MAX_SIZE: usize = 1000;

struct LoopbackAdapter {
    tx: broadcast::Sender<CanMessage>,
}

impl LoopbackAdapter {
    fn new() -> Self {
        let (tx, _) = broadcast::channel(MAX_SIZE);
        Self { tx }
    }

    fn send(&self, msg: CanMessage) {
        let tx = self.tx.clone();
        let _ = tx.send(msg);
    }
}

lazy_static! {
    static ref LOOPBACK_ADAPTER: LoopbackAdapter = LoopbackAdapter::new();
}

pub struct LoopbackDevice {
    rx: broadcast::Receiver<CanMessage>,
}

impl LoopbackDevice {
    pub fn new() -> Self {
        let rx = LOOPBACK_ADAPTER.tx.subscribe();
        Self { rx }
    }

    pub async fn recv(&mut self) -> crate::Result<CanMessage> {
        self.rx
            .recv()
            .await
            .map_err(|_| crate::Error::protocol(anyhow!("Loopback closed.")))
    }

    pub fn send(&self, msg: CanMessage) {
        LOOPBACK_ADAPTER.send(msg)
    }
}
