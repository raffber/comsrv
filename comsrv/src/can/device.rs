use async_can::Bus as CanBus;

use crate::can::{CanAddress, CanMessage};
/// This module is responsible for mapping CAN functionality a device to different backends

use crate::can::loopback::LoopbackDevice;

impl From<async_can::Error> for crate::Error {
    fn from(x: async_can::Error) -> Self {
        match x {
            async_can::Error::Io(err) => crate::Error::io(err),
        }
    }
}


pub enum CanDevice {
    Loopback(LoopbackDevice),
    Bus(CanBus),
}

impl CanDevice {
    pub async fn send(&self, msg: CanMessage) -> crate::Result<()> {
        match self {
            CanDevice::Loopback(lo) => Ok(lo.send(msg)),
            CanDevice::Bus(bus) => Ok(bus.send(msg).await?)
        }
    }

    pub async fn recv(&self) -> crate::Result<CanMessage> {
        match self {
            CanDevice::Loopback(lo) => lo.recv().await,
            CanDevice::Bus(bus) => Ok(bus.recv().await?)
        }
    }
}

#[cfg(target_os = "linux")]
impl CanDevice {
    pub fn new(addr: CanAddress) -> crate::Result<Self> {
        match addr {
            CanAddress::PCan { .. } => {
                Err(crate::Error::NotSupported)
            }
            CanAddress::Socket(ifname) => {
                Ok(CanDevice::Bus(CanBus::connect(ifname)?))
            }
            CanAddress::Loopback => {
                Ok(CanDevice::Loopback(LoopbackDevice::new()))
            }
        }
    }
}

#[cfg(target_os = "windows")]
impl CanDevice {
    pub fn new(addr: CanAddress) -> crate::Result<Self> {
        match addr {
            CanAddress::PCan(ifname) => {
                Ok(CanDevice::Bus(CanBus::connect(ifname)?))
            }
            CanAddress::Socket(_) => {
                Err(crate::Error::NotSupported)
            }
            CanAddress::Loopback => {
                Ok(CanDevice::Loopback(LoopbackDevice::new()))
            }
        }
    }
}
