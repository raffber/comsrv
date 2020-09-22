use async_trait::async_trait;
use tokio::sync::{mpsc, oneshot};
use tokio::task;
use futures::TryFutureExt;
use crate::Error;

pub trait Message: 'static + Send {}

impl<T: 'static + Send> Message for T {}

#[async_trait]
pub trait IoHandler: Send {
    type Request: Message;
    type Response: Message;

    async fn handle(&mut self, req: Self::Request) -> crate::Result<Self::Response>;
}

enum RequestMsg<T: IoHandler> {
    Task {
        req: T::Request,
        answer: oneshot::Sender<crate::Result<T::Response>>,
    },
    Drop,
}


#[derive(Clone)]
pub struct IoTask<T: IoHandler> {
    tx: mpsc::UnboundedSender<RequestMsg<T>>,
}

impl<T: 'static + IoHandler> IoTask<T> {
    pub fn new(mut handler: T) -> Self {
        let (tx, mut rx) =
            mpsc::unbounded_channel::<RequestMsg<T>>();
        task::spawn(async move {
            while let Some(x) = rx.recv().await {
                match x {
                    RequestMsg::Task { req, answer } => {
                        let result = handler.handle(req).await;
                        let _ = answer.send(result);
                    },
                    RequestMsg::Drop => {
                        break
                    },
                }
            }
        });
        IoTask {
            tx
        }
    }

    pub fn disconnect(&mut self) {
        let _ = self.tx.send(RequestMsg::Drop);
    }

    async fn request(&mut self, req: T::Request) -> crate::Result<T::Response> {
        let (tx, rx) = oneshot::channel();
        let msg = RequestMsg::Task { req, answer: tx };
        self.tx.send(msg).map_err(|_| Error::Disconnected)?;
        let ret = rx.await.map_err(|_| Error::Disconnected)?;
        ret
    }
}


