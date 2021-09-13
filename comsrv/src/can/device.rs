use async_can::{Receiver, Sender};

/// This module is responsible for mapping CAN functionality a device to different backends
use crate::can::loopback::LoopbackDevice;
use crate::can::{CanAddress, CanError, CanMessage, into_protocol_message, into_async_can_message};

pub enum CanSender {
    Loopback(LoopbackDevice),
    Bus { device: Sender, addr: CanAddress },
}

pub enum CanReceiver {
    Loopback(LoopbackDevice),
    Bus { device: Receiver, addr: CanAddress },
}

impl CanSender {
    pub async fn send(&self, msg: CanMessage) -> crate::Result<()> {
        match self {
            CanSender::Loopback(lo) => {
                lo.send(msg);
                Ok(())
            }
            CanSender::Bus { device, addr } => {
                let addr = addr.interface();
                let msg = into_async_can_message(msg)
                    .map_err(|err| crate::Error::Can { addr: addr.clone(), err: err.into() })?;

                let ret = device.send(msg).await;
                ret.map_err(|x| crate::Error::Can {
                    addr,
                    err: x.into(),
                })
            }
        }
    }
}

impl CanReceiver {
    pub async fn recv(&mut self) -> Result<CanMessage, CanError> {
        match self {
            CanReceiver::Loopback(lo) => lo.recv().await,
            CanReceiver::Bus { device, addr: _ } => Ok(into_protocol_message(device.recv().await?)),
        }
    }

    pub fn address(&self) -> CanAddress {
        match self {
            CanReceiver::Loopback(_) => CanAddress::Loopback,
            CanReceiver::Bus { device: _, addr } => addr.clone(),
        }
    }
}

#[cfg(target_os = "linux")]
impl CanSender {
    pub fn new(addr: CanAddress) -> crate::Result<Self> {
        let addr2 = addr.clone();
        match addr {
            CanAddress::PCan { .. } => Err(crate::Error::NotSupported),
            CanAddress::Socket(ifname) => {
                let device = Sender::connect(ifname).map_err(|x| crate::Error::Can {
                    addr: addr2.interface(),
                    err: x.into(),
                })?;
                Ok(CanSender::Bus {
                    device,
                    addr: addr2,
                })
            }
            CanAddress::Loopback => Ok(CanSender::Loopback(LoopbackDevice::new())),
        }
    }
}

#[cfg(target_os = "linux")]
impl CanReceiver {
    pub fn new(addr: CanAddress) -> crate::Result<Self> {
        let addr2 = addr.clone();
        match addr {
            CanAddress::PCan { .. } => Err(crate::Error::NotSupported),
            CanAddress::Socket(ifname) => {
                let device = Receiver::connect(ifname).map_err(|x| crate::Error::Can {
                    addr: addr2.interface(),
                    err: x.into(),
                })?;
                Ok(CanReceiver::Bus {
                    device,
                    addr: addr2,
                })
            }
            CanAddress::Loopback => Ok(CanReceiver::Loopback(LoopbackDevice::new())),
        }
    }
}

#[cfg(target_os = "windows")]
impl CanSender {
    pub fn new(addr: CanAddress) -> crate::Result<Self> {
        match &addr {
            CanAddress::PCan { ifname, bitrate } => {
                let device = Sender::connect(ifname, *bitrate).map_err(|x| crate::Error::Can {
                    addr: addr.interface(),
                    err: x.into(),
                })?;
                Ok(Self::Bus { device, addr })
            }
            CanAddress::Socket(_) => Err(crate::Error::NotSupported),
            CanAddress::Loopback => Ok(Self::Loopback(LoopbackDevice::new())),
        }
    }
}

#[cfg(target_os = "windows")]
impl CanReceiver {
    pub fn new(addr: CanAddress) -> crate::Result<Self> {
        match &addr {
            CanAddress::PCan { ifname, bitrate } => {
                let device =
                    Receiver::connect(ifname, *bitrate).map_err(|x| crate::Error::Can {
                        addr: addr.interface(),
                        err: x.into(),
                    })?;
                Ok(Self::Bus { device, addr })
            }
            CanAddress::Socket(_) => Err(crate::Error::NotSupported),
            CanAddress::Loopback => Ok(Self::Loopback(LoopbackDevice::new())),
        }
    }
}
