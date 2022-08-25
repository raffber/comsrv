//! # Client Library for the `comsrv` Communication Relay
//!
//! This library provides some easy-to-use types to facilitate interaction.
//! It's just a slim layer combining `comsrv_protocol` and `wsrpc` crate.
//!
//! Errors are captured in the [`enum@Error`] type.
//!
//! The types used for communication with the `comsrv` accept an [`Rpc`] argument. This `#[asnc_trait]` abstracts
//! over the communication interface to communicate with the `comsrv` tool. There 2 concrete implementations:
//!
//!  * [`http::HttpRpc`] - Communicate over http. This is a reasonable default, but comes at some cost in terms of speed.
//!     Also, it does not allow listening to notifications.
//!  * [`ws::WsRpc`] - Communicate over WebSocket. This is fast and allows listening to notification. However, it comes at comes at the
//!     cost of maintaining some state in the application (the TCP connection).
//!
//! This crate comes with some types that provide an easy-to-use interface to interact with device connected to the `comsrv`:
//!
//!  * [`bytestream::ByteStreamPipe`] - To communicate with devices attached to bytestream-like communication devices (SerialPorts, TCP streams, FTDIs, ..)
//!  * [`modbus::ModBusPipe`] - ModBus/TCP and ModBus/RTU client operating on any [`bytestream::ByteStreamPipe`]
//!  * [`can::CanBus`] - To interact with a CAN bus.
//!  * [`gctcan::GctCanDevice`] - Abstracts over the communication protocol used with a node on a GCT-CAN network
//!
use std::io;
use std::time::Duration;

use async_trait::async_trait;
use broadcast_wsrpc::client::ClientError;
use comsrv_protocol::{Address, Request, Response};
use protocol::FtdiDeviceInfo;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use thiserror::Error;
use tokio::task;
use uuid::Uuid;

pub mod bytestream;
pub mod can;
pub mod gctcan;
pub mod http;
pub mod modbus;
pub mod ws;

pub use comsrv_protocol as protocol;

/// The default timeout for all requests. Most functions offer some way to define custom timeout.
pub const DEFAULT_RPC_TIMEOUT: Duration = Duration::from_millis(1000);

/// Error type unifing errors that may occur on the RPC layer or remote errors i.e. an error occurring in the  `comsrv`.
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

/// An `#[async_trait]` defining a request-response interface to the `comsrv`.
#[async_trait]
pub trait Rpc: Clone + Send + 'static {
    async fn request(&mut self, request: Request, timeout: Duration) -> crate::Result<Response>;
}

/// An `#[async_trait]` which defines a lockable resource of the `comsrv`.
#[async_trait]
pub trait Lockable<T: Rpc> {
    async fn lock(&mut self, timeout: Duration) -> crate::Result<LockGuard<T>>;
}

/// A lock guard returned by a [`Lockable::lock`] implementation. If dropped, unlocks
/// the resource.
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

    /// Manually unlock the resource and returns once the lock was released.
    pub async fn unlock(mut self) -> crate::Result<()> {
        unlock(&mut self.rpc, &self.addr, self.lock_id).await
    }

    /// Each locked is assigned an [uuid::Uuid]. It can be used to access the instrument
    /// or to release the lock.
    pub fn lock_id(&self) -> Uuid {
        self.lock_id
    }

    pub(crate) fn locked(&self) -> Locked {
        Locked {
            unlock: self.unlock.clone(),
            lock_id: Some(self.lock_id),
        }
    }
}

impl<T: Rpc> Drop for LockGuard<T> {
    fn drop(&mut self) {
        let mut rpc = self.rpc.clone();
        let lock = self.lock_id;
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
        self.lock_id
    }
}

/// Lock an instrument returning a [`LockGuard`]. Once the [`LockGuard`] is dropped, the
/// instrument is unlocked.
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

/// Release a lock based on the given [`Uuid`]. Refer to [`LockGuard::lock_id`].
pub async fn unlock<T: Rpc>(rpc: &mut T, addr: &Address, uuid: Uuid) -> crate::Result<()> {
    let req = Request::Unlock {
        addr: addr.clone(),
        id: uuid,
    };
    rpc.request(req, DEFAULT_RPC_TIMEOUT).await.map(|_| ())
}

/// List all serial ports connected to the system
pub async fn list_serial_ports<T: Rpc>(rpc: &mut T) -> crate::Result<Vec<String>> {
    match rpc
        .request(Request::ListSerialPorts, DEFAULT_RPC_TIMEOUT)
        .await
    {
        Ok(Response::SerialPorts(ret)) => Ok(ret),
        Ok(Response::Error(x)) => Err(x.into()),
        Ok(_) => Err(Error::UnexpectdResponse),
        Err(x) => Err(x),
    }
}

/// List all FTDIs connected to the systmem
pub async fn list_ftdis<T: Rpc>(rpc: &mut T) -> crate::Result<Vec<FtdiDeviceInfo>> {
    match rpc
        .request(Request::ListFtdiDevices, DEFAULT_RPC_TIMEOUT)
        .await
    {
        Ok(Response::FtdiDevices(ret)) => Ok(ret),
        Ok(Response::Error(x)) => Err(x.into()),
        Ok(_) => Err(Error::UnexpectdResponse),
        Err(x) => Err(x),
    }
}
