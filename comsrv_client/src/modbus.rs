use std::time::Duration;
use comsrv_protocol::{ModBusRequest, ModBusResponse, Request, Response};
use crate::{DEFAULT_RPC_TIMEOUT, Lock, lock, Locked, LockGuard, Rpc};
use async_trait::async_trait;

pub struct ModBusPipe<T: Rpc> {
    rpc: T,
    addr: String,
    lock: Locked,
    pub timeout: Duration,
}

#[async_trait]
impl<T: Rpc> Lock<T> for ModBusPipe<T> {
    async fn lock(&mut self, timeout: Duration) -> crate::Result<LockGuard<T>> {
        let ret = lock(&mut self.rpc, &self.addr, timeout).await?;
        self.lock = ret.locked();
        Ok(ret)
    }
}

impl<T: Rpc> Clone for ModBusPipe<T> {
    fn clone(&self) -> Self {
        Self {
            rpc: self.rpc.clone(),
            addr: self.addr.clone(),
            lock: Locked::new(),
            timeout: self.timeout.clone(),
        }
    }
}


impl<T: Rpc> ModBusPipe<T> {
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


    pub async fn request(&mut self, task: ModBusRequest) -> crate::Result<ModBusResponse> {
        let ret = self
            .rpc
            .request(
                Request::ModBus {
                    addr: self.addr.clone(),
                    task,
                    lock: self.lock.check_lock(),
                },
                self.timeout.clone(),
            )
            .await?;
        match ret {
            Response::ModBus(x) => Ok(x),
            Response::Error(x) => Err(crate::Error::from_rpc(x)),
            _ => Err(crate::Error::UnexpectdResponse),
        }
    }

    pub async fn read_coil(&mut self, addr: u16, cnt: u16) -> crate::Result<Vec<bool>> {
        match self.request(ModBusRequest::ReadCoil {addr, cnt}).await? {
            ModBusResponse::Bool(ret) => Ok(ret),
            _ => Err(crate::Error::UnexpectdResponse),
        }
    }

    pub async fn read_discrete(&mut self, addr: u16, cnt: u16) -> crate::Result<Vec<bool>> {
        match self.request(ModBusRequest::ReadDiscrete {addr, cnt}).await? {
            ModBusResponse::Bool(ret) => Ok(ret),
            _ => Err(crate::Error::UnexpectdResponse),
        }
    }

    pub async fn read_input(&mut self, addr: u16, cnt: u16) -> crate::Result<Vec<u16>> {
        match self.request(ModBusRequest::ReadInput {addr, cnt}).await? {
            ModBusResponse::Number(ret) => Ok(ret),
            _ => Err(crate::Error::UnexpectdResponse),
        }
    }

    pub async fn read_holding(&mut self, addr: u16, cnt: u16) -> crate::Result<Vec<u16>> {
        match self.request(ModBusRequest::ReadHolding {addr, cnt}).await? {
            ModBusResponse::Number(ret) => Ok(ret),
            _ => Err(crate::Error::UnexpectdResponse),
        }
    }

    pub async fn write_coils(&mut self, addr: u16, data: Vec<bool>) -> crate::Result<()> {
        match self.request(ModBusRequest::WriteCoil {addr, values: data}).await? {
            ModBusResponse::Done => Ok(()),
            _ => Err(crate::Error::UnexpectdResponse),
        }
    }

    pub async fn write_register(&mut self, addr: u16, data: Vec<u16>) -> crate::Result<()> {
        match self.request(ModBusRequest::WriteRegister {addr, data}).await? {
            ModBusResponse::Done => Ok(()),
            _ => Err(crate::Error::UnexpectdResponse),
        }
    }
}
