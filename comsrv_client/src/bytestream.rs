use crate::{lock, Lock, LockGuard, Locked, Rpc, DEFAULT_RPC_TIMEOUT};
use async_trait::async_trait;
use comsrv_protocol::{ByteStreamRequest, ByteStreamResponse, Request, Response};
use std::time::Duration;

pub struct ByteStreamPipe<T: Rpc> {
    rpc: T,
    addr: String,
    lock: Locked,
    pub timeout: Duration,
}

#[async_trait]
impl<T: Rpc> Lock<T> for ByteStreamPipe<T> {
    async fn lock(&mut self, timeout: Duration) -> crate::Result<LockGuard<T>> {
        let ret = lock(&mut self.rpc, &self.addr, timeout).await?;
        self.lock = ret.locked();
        Ok(ret)
    }
}

impl<T: Rpc> ByteStreamPipe<T> {
    pub fn new(rpc: T, addr: &str) -> Self {
        Self {
            rpc,
            addr: addr.to_string(),
            lock: Locked::new(),
            timeout: DEFAULT_RPC_TIMEOUT,
        }
    }

    pub fn with_timeout(rpc: T, addr: &str, timeout: Duration) -> Self {
        Self {
            rpc,
            addr: addr.to_string(),
            lock: Locked::new(),
            timeout,
        }
    }

    pub async fn write(&mut self, data: &[u8]) -> crate::Result<()> {
        let ret = self
            .rpc
            .request(
                Request::Bytes {
                    addr: self.addr.clone(),
                    task: ByteStreamRequest::Write(data.to_vec()),
                    lock: self.lock.check_lock(),
                },
                self.timeout.clone(),
            )
            .await?;
        match ret {
            Response::Bytes(ByteStreamResponse::Done) => Ok(()),
            Response::Error(x) => Err(crate::Error::from_rpc(x)),
            _ => Err(crate::Error::UnexpectdResponse),
        }
    }
}
