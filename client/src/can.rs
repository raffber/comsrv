use crate::protocol::{CanRequest, DataFrame, RemoteFrame};
use crate::ws::WsRpc;
use crate::Rpc;
use comsrv_protocol::{CanAddress, CanInstrument, Request};
use comsrv_protocol::{CanMessage, CanResponse, GctMessage, Response};
use std::time::Duration;
use tokio::sync::mpsc::error::TrySendError;
use tokio::sync::mpsc::{channel, Receiver, Sender, UnboundedReceiver};
use tokio::{select, task};

const CHANNEL_CAPACITY: usize = 1000;

#[derive(Clone)]
pub struct CanBus {
    instrument: CanInstrument,
    rpc: WsRpc,
}

#[derive(Debug)]
pub enum Message {
    RawData(DataFrame),
    RawRemote(RemoteFrame),
    Gct(GctMessage),
}

impl CanBus {
    pub fn new(instrument: CanInstrument, rpc: WsRpc) -> Self {
        Self { instrument, rpc }
    }

    pub async fn connect(&mut self) -> crate::Result<()> {
        self.rpc
            .request(
                Request::Can {
                    instrument: self.instrument.clone(),
                    request: CanRequest::ListenRaw(true),
                    lock: None,
                },
                Duration::from_millis(100),
            )
            .await?;
        self.rpc
            .request(
                Request::Can {
                    instrument: self.instrument.clone(),
                    request: CanRequest::ListenGct(true),
                    lock: None,
                },
                Duration::from_millis(100),
            )
            .await
            .map(|_| ())
    }

    pub async fn subscribe<U: 'static + Send, T: Fn(Message) -> Option<U> + Send + 'static>(
        &self,
        filter: T,
    ) -> Receiver<U> {
        let client = self.rpc.client.clone();
        let (tx, rx) = channel(CHANNEL_CAPACITY);
        let notifications = client.notifications();
        task::spawn(async move {
            select! {
                _ = subscriber_task(&tx, notifications, filter) => {},
                _ = tx.closed() => {}
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
            instrument: self.instrument.clone(),
            request: can_request,
            lock: None,
        };
        match self.rpc.request(request, Duration::from_millis(100)).await {
            Ok(Response::Can {
                source: _,
                response: CanResponse::Ok,
            }) => Ok(()),
            Ok(_) => Err(crate::Error::UnexpectdResponse),
            Err(x) => Err(x),
        }
    }
}

async fn subscriber_task<U: 'static + Send, T: Fn(Message) -> Option<U> + Send + 'static>(
    tx: &Sender<U>,
    mut notifications: UnboundedReceiver<Response>,
    filter: T,
) {
    while let Some(x) = notifications.recv().await {
        let msg = match x {
            Response::Can {
                source: _,
                response: CanResponse::Raw(CanMessage::Data(msg)),
            } => Message::RawData(msg),
            Response::Can {
                source: _,
                response: CanResponse::Raw(CanMessage::Remote(msg)),
            } => Message::RawRemote(msg),
            Response::Can {
                source: _,
                response: CanResponse::Gct(msg),
            } => Message::Gct(msg),
            _ => continue,
        };
        if let Some(x) = filter(msg) {
            match tx.try_send(x) {
                Ok(_) => {}
                Err(TrySendError::Full(_)) => continue,
                Err(TrySendError::Closed(_)) => break,
            }
        }
    }
}
