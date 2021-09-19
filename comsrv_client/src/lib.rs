#![allow(dead_code)]

use std::io;
use std::time::Duration;

use async_trait::async_trait;
use comsrv_protocol::{Request, Response};
use serde_json::{Value as JsonValue, Value};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use thiserror::Error;
use tokio::task;
use uuid::Uuid;
use wsrpc::client::ClientError;

mod bytestream;
mod http;
mod ws;

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
    Other(String),
    #[error("RPC Error: {typ}, {data}")]
    Rpc { typ: String, data: JsonValue },
}

type Result<T> = std::result::Result<T, Error>;

impl Error {
    pub(crate) fn from_rpc(x: JsonValue) -> Error {
        match x {
            Value::Object(x) => {
                if x.len() != 1 {
                    return Error::UnexpectdResponse;
                }
                let (k, v) = x.into_iter().next().unwrap();
                if k == "Timeout" {
                    return Error::Timeout;
                }
                Error::Rpc {
                    typ: k.clone(),
                    data: v.clone(),
                }
            }
            _ => Error::UnexpectdResponse,
        }
    }
}

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
pub trait Lock<T: Rpc> {
    async fn lock(&mut self, timeout: Duration) -> crate::Result<LockGuard<T>>;
}

pub struct LockGuard<T: Rpc> {
    rpc: T,
    lock_id: Uuid,
    unlock: Arc<AtomicBool>,
}

impl<T: Rpc> LockGuard<T> {
    fn new(rpc: T, lock_id: Uuid) -> Self {
        Self {
            rpc,
            lock_id,
            unlock: Arc::new(Default::default()),
        }
    }

    pub async fn unlock(mut self) -> crate::Result<()> {
        unlock(&mut self.rpc, self.lock_id).await
    }

    pub fn lock_id(&self) -> Uuid {
        self.lock_id
    }

    pub(crate) fn locked(&self) -> Locked {
        Locked {
            unlock: self.unlock.clone(),
            lock_id: Some(self.lock_id.clone()),
        }
    }
}

impl<T: Rpc> Drop for LockGuard<T> {
    fn drop(&mut self) {
        let mut rpc = self.rpc.clone();
        let lock = self.lock_id.clone();
        let fut = async move { unlock(&mut rpc, lock).await };
        task::spawn(fut);
    }
}

struct Locked {
    unlock: Arc<AtomicBool>,
    lock_id: Option<Uuid>,
}

impl Locked {
    fn new() -> Self {
        Self {
            unlock: Arc::new(Default::default()),
            lock_id: None,
        }
    }

    fn check_lock(&mut self) -> Option<Uuid> {
        if !self.unlock.as_ref().load(Ordering::Relaxed) {
            self.lock_id = None;
        }
        self.lock_id.clone()
    }
}

pub async fn lock<T: Rpc>(
    rpc: &mut T,
    addr: &str,
    timeout: Duration,
) -> crate::Result<LockGuard<T>> {
    let ret = rpc
        .request(
            Request::Lock {
                addr: addr.to_string(),
                timeout_ms: (timeout.as_millis() as u32),
            },
            DEFAULT_RPC_TIMEOUT,
        )
        .await?;
    match ret {
        Response::Locked { addr: _, lock_id } => Ok(LockGuard::new(rpc.clone(), lock_id)),
        _ => Err(Error::UnexpectdResponse),
    }
}

pub async fn unlock<T: Rpc>(rpc: &mut T, uuid: Uuid) -> crate::Result<()> {
    rpc.request(Request::Unlock(uuid), DEFAULT_RPC_TIMEOUT.clone())
        .await
        .map(|_| ())
}
