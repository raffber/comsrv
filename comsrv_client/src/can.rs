use crate::ws::WsRpc;
use comsrv_protocol::{Response, CanMessage, CanResponse, GctMessage};
use tokio::sync::mpsc::{Receiver, channel};
use tokio::sync::mpsc::error::TrySendError;
use crate::protocol::{DataFrame, RemoteFrame, CanRequest};
use tokio::task;
use crate::Rpc;
use comsrv_protocol::Request;
use std::time::Duration;

const CHANNEL_CAPACITY: usize = 1000;

#[derive(Clone)]
pub struct CanBus {
    address: String,
    rpc: WsRpc,
}

#[derive(Debug)]
pub enum Message {
    RawData(DataFrame),
    RawRemote(RemoteFrame),
    Gct(GctMessage),
}


impl CanBus {
    pub fn new<T: ToString>(address: T, rpc: WsRpc) -> Self {
        Self {
            address: address.to_string(),
            rpc,
        }
    }

    pub async fn connect(&mut self) -> crate::Result<()> {
        self.rpc.request(Request::Can {
            addr: self.address.clone(),
            task: CanRequest::ListenRaw(true),
            lock: None,
        }, Duration::from_millis(100)).await?;
        self.rpc.request(Request::Can {
            addr: self.address.clone(),
            task: CanRequest::ListenGct(true),
            lock: None,
        }, Duration::from_millis(100)).await.map(|_| ())
    }

    #[must_use]
    pub async fn subscribe<U: 'static + Send, T: Fn(Message) -> Option<U> + Send + 'static>(&self, filter: T) -> Receiver<U> {
        let client = self.rpc.client.clone();
        let (tx, rx) = channel(CHANNEL_CAPACITY);
        let mut notifications = client.notifications();
        task::spawn(async move {
            while let Some(x) = notifications.recv().await {
                let msg = match x {
                    Response::Can(CanResponse::Raw(CanMessage::Data(msg))) => Message::RawData(msg),
                    Response::Can(CanResponse::Raw(CanMessage::Remote(msg))) => Message::RawRemote(msg),
                    Response::Can(CanResponse::Gct(msg)) => Message::Gct(msg),
                    _ => continue
                };
                if let Some(x) = filter(msg) {
                    match tx.try_send(x) {
                        Ok(_) => {}
                        Err(TrySendError::Full(_)) => continue,
                        Err(TrySendError::Closed(_)) => break,
                    }
                }
            }
        });
        rx
    }

    pub async fn send(&mut self, msg: Message) -> crate::Result<()> {
        let can_request = match msg {
            Message::RawData(msg) => CanRequest::TxRaw(CanMessage::Data(msg)),
            Message::RawRemote(msg) => CanRequest::TxRaw(CanMessage::Remote(msg)),
            Message::Gct(msg) => CanRequest::TxGct(msg),
        };
        let request = Request::Can {
            addr: self.address.to_string(),
            task: can_request,
            lock: None,
        };
        match self.rpc.request(request, Duration::from_millis(100)).await {
            Ok(Response::Can(CanResponse::Ok)) => Ok(()),
            Ok(_) => Err(crate::Error::UnexpectdResponse),
            Err(x) => Err(x),
        }
    }
}


