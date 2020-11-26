use async_can::Bus as CanBus;

use crate::can::{CanAddress, CanMessage, CanError};
/// This module is responsible for mapping CAN functionality a device to different backends

use crate::can::loopback::LoopbackDevice;

pub enum CanDevice {
    Loopback(LoopbackDevice),
    Bus {
        device: CanBus,
        addr: CanAddress,
    },
}

impl CanDevice {
    pub async fn send(&self, msg: CanMessage) -> crate::Result<()> {
        match self {
            CanDevice::Loopback(lo) => Ok(lo.send(msg)),
            CanDevice::Bus { device, addr } => {
                let addr = addr.interface();
                let ret = device.send(msg).await;
                ret.map_err(|x| crate::Error::Can {
                        addr,
                        err: x.into()
                })
            }
        }
    }

    pub async fn recv(&self) -> Result<CanMessage, CanError> {
        match self {
            CanDevice::Loopback(lo) => lo.recv().await,
            CanDevice::Bus{ device, addr: _ } => Ok(device.recv().await?)
        }
    }

    pub fn address(&self) -> CanAddress {
        match self {
            CanDevice::Loopback(_) => CanAddress::Loopback,
            CanDevice::Bus { device: _, addr } => addr.clone(),
        }
    }
}

#[cfg(target_os = "linux")]
impl CanDevice {
    pub fn new(addr: CanAddress) -> crate::Result<Self> {
        let addr2 = addr.clone();
        match addr {
            CanAddress::PCan { .. } => {
                Err(crate::Error::NotSupported)
            }
            CanAddress::Socket(ifname) => {
                let device = CanBus::connect(ifname).map_err(|x| {
                    crate::Error::Can {
                        addr: addr2.interface(),
                        err: x.into()
                    }
                })?;
                Ok(CanDevice::Bus {
                    device,
                    addr: addr2
                })
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
