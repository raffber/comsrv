use std::time::Duration;

use async_trait::async_trait;
use comsrv_protocol::{ByteStreamRequest, ByteStreamResponse, Request, Response};

use crate::{lock, Lock, LockGuard, Locked, Rpc, DEFAULT_RPC_TIMEOUT};

pub struct ByteStreamPipe<T: Rpc> {
    rpc: T,
    addr: String,
    lock: Locked,
    pub timeout: Duration,
}

impl<T: Rpc> Clone for ByteStreamPipe<T> {
    fn clone(&self) -> Self {
        Self {
            rpc: self.rpc.clone(),
            addr: self.addr.clone(),
            lock: Locked::new(),
            timeout: self.timeout.clone()
        }
    }
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

    pub async fn request(&mut self, task: ByteStreamRequest) -> crate::Result<ByteStreamResponse> {
        let ret = self
            .rpc
            .request(
                Request::Bytes {
                    addr: self.addr.clone(),
                    task,
                    lock: self.lock.check_lock(),
                },
                self.timeout.clone(),
            )
            .await?;
        match ret {
            Response::Bytes(x) => Ok(x),
            Response::Error(x) => Err(crate::Error::from_rpc(x)),
            _ => Err(crate::Error::UnexpectdResponse),
        }
    }

    pub async fn write(&mut self, data: &[u8]) -> crate::Result<()> {
        let req = ByteStreamRequest::Write(data.to_vec());
        match self.request(req).await? {
            ByteStreamResponse::Done => Ok(()),
            _ => Err(crate::Error::UnexpectdResponse),
        }
    }

    pub async fn read_all(&mut self) -> crate::Result<Vec<u8>> {
        match self.request(ByteStreamRequest::ReadAll).await? {
            ByteStreamResponse::Data(x) => Ok(x),
            _ => Err(crate::Error::UnexpectdResponse),
        }
    }

    pub async fn read_to_term(&mut self, term: u8, timeout: Duration) -> crate::Result<Vec<u8>> {
        let timeout_ms = timeout.as_millis() as u32;
        let req = ByteStreamRequest::ReadToTerm { term, timeout_ms };
        match self.request(req).await? {
            ByteStreamResponse::Data(x) => Ok(x),
            _ => Err(crate::Error::UnexpectdResponse),
        }
    }

    pub async fn read_exact(&mut self, count: u32, timeout: Duration) -> crate::Result<Vec<u8>> {
        let timeout_ms = timeout.as_millis() as u32;
        let req = ByteStreamRequest::ReadExact { count, timeout_ms };
        match self.request(req).await? {
            ByteStreamResponse::Data(x) => Ok(x),
            _ => Err(crate::Error::UnexpectdResponse),
        }
    }

    pub async fn read_upto(&mut self, count: u32) -> crate::Result<Vec<u8>> {
        let req = ByteStreamRequest::ReadUpTo(count);
        match self.request(req).await? {
            ByteStreamResponse::Data(x) => Ok(x),
            _ => Err(crate::Error::UnexpectdResponse),
        }
    }

    pub async fn cobs_write(&mut self, data: &[u8]) -> crate::Result<()> {
        let req = ByteStreamRequest::CobsWrite(data.to_vec());
        match self.request(req).await? {
            ByteStreamResponse::Done => Ok(()),
            _ => Err(crate::Error::UnexpectdResponse),
        }
    }

    pub async fn cobs_read(&mut self, timeout: Duration) -> crate::Result<Vec<u8>> {
        let timeout_ms = timeout.as_millis() as u32;
        let req = ByteStreamRequest::CobsRead(timeout_ms);
        match self.request(req).await? {
            ByteStreamResponse::Data(x) => Ok(x),
            _ => Err(crate::Error::UnexpectdResponse),
        }
    }

    pub async fn cobs_query(&mut self, write: &[u8], timeout: Duration) -> crate::Result<Vec<u8>> {
        let timeout_ms = timeout.as_millis() as u32;
        let req = ByteStreamRequest::CobsQuery {
            data: write.to_vec(),
            timeout_ms,
        };
        match self.request(req).await? {
            ByteStreamResponse::Data(x) => Ok(x),
            _ => Err(crate::Error::UnexpectdResponse),
        }
    }

    pub async fn write_line(&mut self, write: &str, term: u8) -> crate::Result<()> {
        let req = ByteStreamRequest::WriteLine {
            line: write.to_string(),
            term,
        };
        match self.request(req).await? {
            ByteStreamResponse::Done => Ok(()),
            _ => Err(crate::Error::UnexpectdResponse),
        }
    }

    pub async fn read_line(&mut self, term: u8, timeout: Duration) -> crate::Result<String> {
        let timeout_ms = timeout.as_millis() as u32;
        let req = ByteStreamRequest::ReadLine {
            timeout_ms,
            term,
        };
        match self.request(req).await? {
            ByteStreamResponse::String(x) => Ok(x),
            _ => Err(crate::Error::UnexpectdResponse),
        }
    }

    pub async fn query_line(&mut self, write: &str, term: u8, timeout: Duration) -> crate::Result<String> {
        let timeout_ms = timeout.as_millis() as u32;
        let req = ByteStreamRequest::QueryLine {
            line: write.to_string(),
            timeout_ms,
            term,
        };
        match self.request(req).await? {
            ByteStreamResponse::String(x) => Ok(x),
            _ => Err(crate::Error::UnexpectdResponse),
        }
    }
}
