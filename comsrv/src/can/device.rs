use async_can::{Receiver, Sender};

/// This module is responsible for mapping CAN functionality a device to different backends
use crate::can::loopback::LoopbackDevice;
use crate::can::{into_async_can_message, into_protocol_message, CanAddress, CanMessage};
use anyhow::anyhow;

use super::{map_error, map_frame_error};

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
            CanSender::Bus { device, addr: _ } => {
                let msg = into_async_can_message(msg).map_err(map_frame_error)?;
                device.send(msg).await.map_err(map_error)
            }
        }
    }
}

impl CanReceiver {
    pub async fn recv(&mut self) -> crate::Result<CanMessage> {
        match self {
            CanReceiver::Loopback(lo) => lo.recv().await,
            CanReceiver::Bus { device, addr: _ } => Ok(into_protocol_message(device.recv().await.map_err(map_error)?)),
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
            CanAddress::PCan { .. } => Err(crate::Error::internal(anyhow!("Not Supported"))),
            CanAddress::SocketCan { interface } => {
                let device = Sender::connect(interface.clone()).map_err(map_error)?;
                Ok(CanSender::Bus { device, addr: addr2 })
            }
            CanAddress::Loopback => Ok(CanSender::Loopback(LoopbackDevice::new())),
        }
    }

    pub async fn close(self) -> crate::Result<()> {
        Ok(())
    }
}

#[cfg(target_os = "linux")]
impl CanReceiver {
    pub fn new(addr: CanAddress) -> crate::Result<Self> {
        let addr2 = addr.clone();
        match addr {
            CanAddress::PCan { .. } => Err(crate::Error::internal(anyhow!("Not supported"))),
            CanAddress::SocketCan { interface } => {
                let device = Receiver::connect(interface.clone()).map_err(map_error)?;
                Ok(CanReceiver::Bus { device, addr: addr2 })
            }
            CanAddress::Loopback => Ok(CanReceiver::Loopback(LoopbackDevice::new())),
        }
    }

    pub async fn close(self) -> crate::Result<()> {
        Ok(())
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

    pub async fn close(self) -> crate::Result<()> {
        match self {
            CanSender::Loopback(_) => Ok(()),
            CanSender::Bus { device, addr } => device.close().await.map_err(|x| crate::Error::Can {
                addr: addr.interface(),
                err: x.into(),
            }),
        }
    }
}

#[cfg(target_os = "windows")]
impl CanReceiver {
    pub fn new(addr: CanAddress) -> crate::Result<Self> {
        match &addr {
            CanAddress::PCan { ifname, bitrate } => {
                let device = Receiver::connect(ifname, *bitrate).map_err(|x| crate::Error::Can {
                    addr: addr.interface(),
                    err: x.into(),
                })?;
                Ok(Self::Bus { device, addr })
            }
            CanAddress::Socket(_) => Err(crate::Error::NotSupported),
            CanAddress::Loopback => Ok(Self::Loopback(LoopbackDevice::new())),
        }
    }

    pub async fn close(self) -> crate::Result<()> {
        Ok(())
    }
}
