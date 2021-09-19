#![allow(dead_code)]

mod ws;
mod http;
mod bytestream;

use comsrv_protocol::{Response, Request};
use std::io;
use std::time::Duration;
use async_trait::async_trait;
use thiserror::Error;
use wsrpc::client::ClientError;
use tokio::task;
use uuid::Uuid;

pub const DEFAULT_RPC_TIMEOUT: Duration = Duration::from_millis(1000);

#[derive(Error, Debug)]
pub enum Error {
    #[error("IO Error occurred: {0}")]
    Io(io::Error),
    #[error("Timeout occurred")]
    Timeout,
    #[error("EndpointHangUp")]
    EndpointHangUp,
    #[error("Unexpected Response")]
    UnexpectdResponse,
    #[error("Other Error: {0}")]
    Other(String)
}

type Result<T> = std::result::Result<T, Error>;

impl From<ClientError> for Error {
    fn from(x: ClientError) -> Self {
        match x {
            ClientError::Io(x) => Error::Io(x),
            ClientError::Timeout => Error::Timeout,
            ClientError::ReceiverHungUp => Error::EndpointHangUp,
            ClientError::SenderHungUp => Error::EndpointHangUp,
        }
    }
}


#[async_trait]
pub trait Rpc: Clone + Send + 'static {
    async fn request(&mut self, request: Request, timeout: Duration) -> crate::Result<Response>;
}

#[async_trait]
trait Lockable<T: Rpc> {
    async fn lock(&mut self, timeout: Duration) -> crate::Result<LockGuard<T>>;
}

pub struct LockGuard<T: Rpc> {
    rpc: T,
    lock: Uuid,
}

impl<T: Rpc> LockGuard<T> {
    pub async fn unlock(mut self) -> crate::Result<()> {
        unlock(&mut self.rpc, self.lock).await
    }
}

impl<T: Rpc> Drop for LockGuard<T> {
    fn drop(&mut self) {
        let mut rpc = self.rpc.clone();
        let lock = self.lock.clone();
        let fut = async move {
            unlock(&mut rpc, lock).await
        };
        task::spawn(fut);
    }
}

pub async fn lock<T: Rpc>(rpc: &mut T, addr: &str, timeout: Duration) -> crate::Result<LockGuard<T>> {
    let ret = rpc.request(Request::Lock {
        addr: addr.to_string(),
        timeout_ms: (timeout.as_millis() as u32),
    }, DEFAULT_RPC_TIMEOUT).await?;
    match ret {
        Response::Locked {addr: _ , lock_id } => Ok(LockGuard {
            rpc: rpc.clone(),
            lock: lock_id,
        }),
        _ => Err(Error::UnexpectdResponse)
    }
}

pub async fn unlock<T: Rpc>(rpc: &mut T, uuid: Uuid) -> crate::Result<()> {
    rpc.request(Request::Unlock(uuid), DEFAULT_RPC_TIMEOUT.clone()).await.map(|_| ())
}
