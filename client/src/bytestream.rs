use std::time::Duration;

use async_trait::async_trait;
use comsrv_protocol::{
    ByteStreamInstrument, ByteStreamRequest, ByteStreamResponse, ModBusProtocol, ModBusRequest,
    Request, Response,
};

use crate::{lock, Lock, LockGuard, Locked, Rpc, DEFAULT_RPC_TIMEOUT};

pub struct ByteStreamPipe<T: Rpc> {
    rpc: T,
    instrument: ByteStreamInstrument,
    lock: Locked,
    pub timeout: Duration,
}

impl<T: Rpc> Clone for ByteStreamPipe<T> {
    fn clone(&self) -> Self {
        Self {
            rpc: self.rpc.clone(),
            instrument: self.instrument.clone(),
            lock: Locked::new(),
            timeout: self.timeout,
        }
    }
}

#[async_trait]
impl<T: Rpc> Lock<T> for ByteStreamPipe<T> {
    async fn lock(&mut self, timeout: Duration) -> crate::Result<LockGuard<T>> {
        let ret = lock(&mut self.rpc, &self.instrument.address(), timeout).await?;
        self.lock = ret.locked();
        Ok(ret)
    }
}

impl<T: Rpc> ByteStreamPipe<T> {
    pub fn new(rpc: T, instrument: ByteStreamInstrument) -> Self {
        Self {
            rpc,
            instrument,
            lock: Locked::new(),
            timeout: DEFAULT_RPC_TIMEOUT,
        }
    }

    pub fn with_timeout(rpc: T, instrument: ByteStreamInstrument, timeout: Duration) -> Self {
        Self {
            rpc,
            instrument,
            lock: Locked::new(),
            timeout,
        }
    }

    pub async fn request(
        &mut self,
        request: ByteStreamRequest,
    ) -> crate::Result<ByteStreamResponse> {
        let ret = self
            .rpc
            .request(
                Request::Bytes {
                    instrument: self.instrument.clone(),
                    request,
                    lock: self.lock.check_lock(),
                },
                self.timeout,
            )
            .await?;
        match ret {
            Response::Bytes(x) => Ok(x),
            Response::Error(x) => Err(x.into()),
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
        let req = ByteStreamRequest::ReadToTerm {
            term,
            timeout: timeout.into(),
        };
        match self.request(req).await? {
            ByteStreamResponse::Data(x) => Ok(x),
            _ => Err(crate::Error::UnexpectdResponse),
        }
    }

    pub async fn read_exact(&mut self, count: u32, timeout: Duration) -> crate::Result<Vec<u8>> {
        let req = ByteStreamRequest::ReadExact {
            count,
            timeout: timeout.into(),
        };
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
        let req = ByteStreamRequest::CobsRead(timeout.into());
        match self.request(req).await? {
            ByteStreamResponse::Data(x) => Ok(x),
            _ => Err(crate::Error::UnexpectdResponse),
        }
    }

    pub async fn cobs_query(&mut self, write: &[u8], timeout: Duration) -> crate::Result<Vec<u8>> {
        let req = ByteStreamRequest::CobsQuery {
            data: write.to_vec(),
            timeout: timeout.into(),
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
        let req = ByteStreamRequest::ReadLine {
            timeout: timeout.into(),
            term,
        };
        match self.request(req).await? {
            ByteStreamResponse::String(x) => Ok(x),
            _ => Err(crate::Error::UnexpectdResponse),
        }
    }

    pub async fn query_line(
        &mut self,
        write: &str,
        term: u8,
        timeout: Duration,
    ) -> crate::Result<String> {
        let req = ByteStreamRequest::QueryLine {
            line: write.to_string(),
            timeout: timeout.into(),
            term,
        };
        match self.request(req).await? {
            ByteStreamResponse::String(x) => Ok(x),
            _ => Err(crate::Error::UnexpectdResponse),
        }
    }

    pub async fn modbus_ddp(
        &mut self,
        station_address: u8,
        protocol: ModBusProtocol,
        sub_cmd: u8,
        ddp_cmd: u8,
        response: bool,
        data: &[u8],
        timeout: Duration,
    ) -> crate::Result<Vec<u8>> {
        let req = ByteStreamRequest::ModBus {
            timeout: timeout.into(),
            station_address,
            protocol,
            request: ModBusRequest::Ddp {
                sub_cmd,
                ddp_cmd,
                response,
                data: data.to_vec(),
            },
        };
        match self.request(req).await? {
            ByteStreamResponse::Data(x) => Ok(x),
            _ => Err(crate::Error::UnexpectdResponse),
        }
    }
}
