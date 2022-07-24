use crate::{lock, Lock, LockGuard, Locked, Rpc, DEFAULT_RPC_TIMEOUT};
use async_trait::async_trait;
use comsrv_protocol::{
    ByteStreamInstrument, ByteStreamRequest, ByteStreamResponse, ModBusProtocol, ModBusRequest,
    ModBusResponse, Request, Response,
};
use std::time::Duration;

pub struct ModBusPipe<T: Rpc> {
    rpc: T,
    instrument: ByteStreamInstrument,
    lock: Locked,
    pub timeout: Duration,
    station_address: u8,
    protocol: ModBusProtocol,
}

#[async_trait]
impl<T: Rpc> Lock<T> for ModBusPipe<T> {
    async fn lock(&mut self, timeout: Duration) -> crate::Result<LockGuard<T>> {
        let ret = lock(&mut self.rpc, &self.instrument.clone().into(), timeout).await?;
        self.lock = ret.locked();
        Ok(ret)
    }
}

impl<T: Rpc> Clone for ModBusPipe<T> {
    fn clone(&self) -> Self {
        Self {
            rpc: self.rpc.clone(),
            instrument: self.instrument.clone(),
            lock: Locked::new(),
            timeout: self.timeout.clone(),
            station_address: self.station_address.clone(),
            protocol: self.protocol.clone(),
        }
    }
}

impl<T: Rpc> ModBusPipe<T> {
    pub fn new(
        rpc: T,
        instrument: ByteStreamInstrument,
        station_address: u8,
        protocol: ModBusProtocol,
    ) -> Self {
        Self {
            rpc,
            instrument,
            lock: Locked::new(),
            timeout: DEFAULT_RPC_TIMEOUT,
            station_address,
            protocol,
        }
    }

    pub fn with_timeout(
        rpc: T,
        instrument: ByteStreamInstrument,
        timeout: Duration,
        station_address: u8,
        protocol: ModBusProtocol,
    ) -> Self {
        Self {
            rpc,
            instrument,
            lock: Locked::new(),
            timeout,
            station_address,
            protocol,
        }
    }

    pub fn set_station_address(&mut self, station_address: u8) {
        self.station_address = station_address;
    }

    fn station_address(&self) -> u8 {
        self.station_address
    }

    pub async fn request(&mut self, task: ModBusRequest) -> crate::Result<ModBusResponse> {
        let request = Request::ByteStream {
            instrument: self.instrument.clone(),
            request: ByteStreamRequest::ModBus {
                timeout: self.timeout.clone().into(),
                station_address: self.station_address,
                protocol: self.protocol,
                request: task,
            },
            lock: self.lock.check_lock(),
        };

        let ret = self.rpc.request(request, self.timeout.clone()).await?;
        match ret {
            Response::Bytes(ByteStreamResponse::ModBus(x)) => Ok(x),
            Response::Error(x) => Err(x.into()),
            _ => Err(crate::Error::UnexpectdResponse),
        }
    }

    pub async fn read_coil(&mut self, addr: u16, cnt: u16) -> crate::Result<Vec<bool>> {
        match self.request(ModBusRequest::ReadCoil { addr, cnt }).await? {
            ModBusResponse::Bool(ret) => Ok(ret),
            _ => Err(crate::Error::UnexpectdResponse),
        }
    }

    pub async fn read_single_coil(&mut self, addr: u16) -> crate::Result<bool> {
        let ret = self.read_coil(addr, 1).await?;
        if ret.len() != 1 {
            return Err(crate::Error::UnexpectdResponse);
        }
        Ok(ret[0])
    }

    pub async fn read_discrete(&mut self, addr: u16, cnt: u16) -> crate::Result<Vec<bool>> {
        match self
            .request(ModBusRequest::ReadDiscrete { addr, cnt })
            .await?
        {
            ModBusResponse::Bool(ret) => Ok(ret),
            _ => Err(crate::Error::UnexpectdResponse),
        }
    }

    pub async fn read_single_discrete(&mut self, addr: u16) -> crate::Result<bool> {
        let ret = self.read_discrete(addr, 1).await?;
        if ret.len() != 1 {
            return Err(crate::Error::UnexpectdResponse);
        }
        Ok(ret[0])
    }

    pub async fn read_input(&mut self, addr: u16, cnt: u8) -> crate::Result<Vec<u16>> {
        match self.request(ModBusRequest::ReadInput { addr, cnt }).await? {
            ModBusResponse::Number(ret) => Ok(ret),
            _ => Err(crate::Error::UnexpectdResponse),
        }
    }

    pub async fn read_single_input(&mut self, addr: u16) -> crate::Result<u16> {
        let ret = self.read_input(addr, 1).await?;
        if ret.len() != 1 {
            return Err(crate::Error::UnexpectdResponse);
        }
        Ok(ret[0])
    }

    pub async fn read_holding(&mut self, addr: u16, cnt: u8) -> crate::Result<Vec<u16>> {
        match self
            .request(ModBusRequest::ReadHolding { addr, cnt })
            .await?
        {
            ModBusResponse::Number(ret) => Ok(ret),
            _ => Err(crate::Error::UnexpectdResponse),
        }
    }

    pub async fn read_single_holding(&mut self, addr: u16) -> crate::Result<u16> {
        let ret = self.read_holding(addr, 1).await?;
        if ret.len() != 1 {
            return Err(crate::Error::UnexpectdResponse);
        }
        Ok(ret[0])
    }

    pub async fn write_coils(&mut self, addr: u16, data: Vec<bool>) -> crate::Result<()> {
        match self
            .request(ModBusRequest::WriteCoils { addr, values: data })
            .await?
        {
            ModBusResponse::Done => Ok(()),
            _ => Err(crate::Error::UnexpectdResponse),
        }
    }

    pub async fn write_register(&mut self, addr: u16, data: Vec<u16>) -> crate::Result<()> {
        match self
            .request(ModBusRequest::WriteRegisters { addr, values: data })
            .await?
        {
            ModBusResponse::Done => Ok(()),
            _ => Err(crate::Error::UnexpectdResponse),
        }
    }

    pub async fn write_single_register(&mut self, addr: u16, data: u16) -> crate::Result<()> {
        match self
            .request(ModBusRequest::WriteRegisters {
                addr,
                values: vec![data],
            })
            .await?
        {
            ModBusResponse::Done => Ok(()),
            _ => Err(crate::Error::UnexpectdResponse),
        }
    }
}
