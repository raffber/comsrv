#![allow(dead_code)]

use std::io;
use std::time::Duration;

use async_trait::async_trait;
use comsrv_protocol::{Address, Request, Response};
use protocol::FtdiDeviceInfo;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use thiserror::Error;
use tokio::task;
use uuid::Uuid;
use wsrpc::client::ClientError;

pub mod bytestream;
pub mod can;
pub mod gctcan;
pub mod http;
pub mod modbus;
pub mod ws;

pub use comsrv_protocol as protocol;

pub const DEFAULT_RPC_TIMEOUT: Duration = Duration::from_millis(1000);

#[derive(Error, Debug)]
pub enum Error {
    #[error("IO Error occurred: {0}")]
    Io(io::Error),
    #[error("Timeout")]
    Timeout,
    #[error("EndpointHangUp")]
    EndpointHangUp,
    #[error("Unexpected Response")]
    UnexpectdResponse,
    #[error("Other Error: {0}")]
    Other(anyhow::Error),
    #[error("Remote Error: {0}")]
    Remote(protocol::Error),
}

type Result<T> = std::result::Result<T, Error>;

impl From<protocol::Error> for Error {
    fn from(x: protocol::Error) -> Self {
        Error::Remote(x)
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
    addr: Address,
    unlock: Arc<AtomicBool>,
}

impl<T: Rpc> LockGuard<T> {
    fn new(rpc: T, addr: Address, lock_id: Uuid) -> Self {
        Self {
            rpc,
            lock_id,
            addr,
            unlock: Arc::new(Default::default()),
        }
    }

    pub async fn unlock(mut self) -> crate::Result<()> {
        unlock(&mut self.rpc, &self.addr, self.lock_id).await
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
        let addr = self.addr.clone();
        let fut = async move { unlock(&mut rpc, &addr, lock).await };
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
    addr: &Address,
    timeout: Duration,
) -> crate::Result<LockGuard<T>> {
    let ret = rpc
        .request(
            Request::Lock {
                addr: addr.clone(),
                timeout: timeout.into(),
            },
            DEFAULT_RPC_TIMEOUT,
        )
        .await?;
    match ret {
        Response::Locked { lock_id } => Ok(LockGuard::new(rpc.clone(), addr.clone(), lock_id)),
        _ => Err(Error::UnexpectdResponse),
    }
}

pub async fn unlock<T: Rpc>(rpc: &mut T, addr: &Address, uuid: Uuid) -> crate::Result<()> {
    let req = Request::Unlock {
        addr: addr.clone(),
        id: uuid,
    };
    rpc.request(req, DEFAULT_RPC_TIMEOUT.clone())
        .await
        .map(|_| ())
}

pub async fn list_serial_ports<T: Rpc>(rpc: &mut T) -> crate::Result<Vec<String>> {
    match rpc
        .request(Request::ListSerialPorts, DEFAULT_RPC_TIMEOUT.clone())
        .await
    {
        Ok(Response::SerialPorts(ret)) => Ok(ret),
        Ok(Response::Error(x)) => Err(x.into()),
        Ok(_) => Err(Error::UnexpectdResponse),
        Err(x) => Err(x),
    }
}

pub async fn list_ftdis<T: Rpc>(rpc: &mut T) -> crate::Result<Vec<FtdiDeviceInfo>> {
    match rpc
        .request(Request::ListFtdiDevices, DEFAULT_RPC_TIMEOUT.clone())
        .await
    {
        Ok(Response::FtdiDevices(ret)) => Ok(ret),
        Ok(Response::Error(x)) => Err(x.into()),
        Ok(_) => Err(Error::UnexpectdResponse),
        Err(x) => Err(x),
    }
}
