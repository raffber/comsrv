/// This module implements a very simple actor interface to which a request can be sent and a
/// response is returned.
///
/// Thus, the actor must implement the trait `IoHandler`. An `IoHandler` can then be placed in an
/// `IoTask<T: IoHandler>` which implements `Send + Clone` and is thus sharable between threads.
use crate::Error;
use async_trait::async_trait;
use tokio::sync::{mpsc, oneshot};
use tokio::task;

/// Trait constraining the `Request` and `Response` associated types of `IoHandler`.
pub trait Message: 'static + Send {}

impl<T: 'static + Send> Message for T {}

/// Defines an actor interface. Can be wrapped in `IoTask` to produce a `Send`-able and `Clone`-able
/// type to communicate with the actor.
#[async_trait]
pub trait IoHandler: Send {
    type Request: Message;
    type Response: Message;

    async fn handle(&mut self, req: Self::Request) -> crate::Result<Self::Response>;
    async fn disconnect(&mut self) {}
}

/// Wraps the `Request` and provides a return path.
/// Also allows sending a message to drop the actor.
enum RequestMsg<T: IoHandler> {
    Task {
        req: T::Request,
        answer: oneshot::Sender<crate::Result<T::Response>>,
    },
    Drop,
}

/// Allows wrapping an `IoHandler` and provides a type that implements `Send + Clone`. Thus
/// it can be used to communicate with the actor from different threads.
pub struct IoTask<T: IoHandler> {
    tx: mpsc::UnboundedSender<RequestMsg<T>>,
}

impl<T: IoHandler> Clone for IoTask<T> {
    fn clone(&self) -> Self {
        Self {
            tx: self.tx.clone(),
        }
    }
}

impl<T: 'static + IoHandler> IoTask<T> {
    /// Wrap the given `IoHandler` actor in an `IoTask`.
    pub fn new(mut handler: T) -> Self {
        let (tx, mut rx) = mpsc::unbounded_channel::<RequestMsg<T>>();
        task::spawn(async move {
            while let Some(x) = rx.recv().await {
                match x {
                    RequestMsg::Task { req, answer } => {
                        let result = handler.handle(req).await;
                        let _ = answer.send(result);
                    }
                    RequestMsg::Drop => {
                        handler.disconnect().await;
                        break;
                    }
                }
            }
        });
        IoTask { tx }
    }

    /// Drop the internal actor.
    pub fn disconnect(&mut self) {
        let _ = self.tx.send(RequestMsg::Drop);
    }

    /// Send a request and receive a response. In case the actor was dropped in the mean-time,
    /// returns `Err(Error::Disconnected)`.
    pub async fn request(&mut self, req: T::Request) -> crate::Result<T::Response> {
        let (tx, rx) = oneshot::channel();
        let msg = RequestMsg::Task { req, answer: tx };
        self.tx.send(msg).map_err(|_| Error::Disconnected)?;
        let ret = rx.await.map_err(|_| Error::Disconnected)?;
        ret
    }
}
