use std::time::Duration;

use async_ftdi::Ftdi;
use async_trait::async_trait;
use comsrv_protocol::ByteStreamRequest;
use comsrv_protocol::ByteStreamResponse;
use comsrv_protocol::FtdiDeviceInfo;
use comsrv_protocol::ModBusRequest;
use comsrv_protocol::ModBusResponse;
use tokio::time::sleep;
use tokio_modbus::client::rtu;
use tokio_modbus::prelude::Slave;
use std::cmp::PartialOrd;

use crate::bytestream;
use crate::bytestream::read_all;
use crate::clonable_channel::ClonableChannel;
use crate::iotask::IoHandler;
use crate::iotask::IoTask;
use crate::modbus::handle_modbus_request_timeout;
use crate::serial::params::DataBits;
use crate::serial::params::Parity;
use crate::serial::params::StopBits;
use crate::serial::SerialParams;

pub enum FtdiRequest {
    ModBus {
        params: SerialParams,
        req: ModBusRequest,
        slave_addr: u8,
    },
    Bytes {
        params: SerialParams,
        req: ByteStreamRequest,
    },
}

impl FtdiRequest {
    fn params(&self) -> SerialParams {
        match self {
            FtdiRequest::ModBus { params, .. } => params.clone(),
            FtdiRequest::Bytes { params, .. } => params.clone(),
        }
    }
}

pub enum FtdiResponse {
    Bytes(ByteStreamResponse),
    ModBus(ModBusResponse),
}

#[derive(Hash, Clone)]
pub struct FtdiAddress {
    pub serial_number: String,
    pub params: SerialParams,
}

#[derive(Clone)]
pub struct Instrument {
    inner: IoTask<Handler>,
}

impl Instrument {
    pub fn new(serial_number: &str) -> Self {
        let handler = Handler::new(serial_number);
        Self {
            inner: IoTask::new(handler),
        }
    }

    pub async fn request(&mut self, req: FtdiRequest) -> crate::Result<FtdiResponse> {
        self.inner.request(req).await
    }

    pub fn disconnect(mut self) {
        self.inner.disconnect()
    }
}

pub struct Request {
    request: ByteStreamRequest,
    params: SerialParams,
}

impl Into<async_ftdi::SerialParams> for SerialParams {
    fn into(self) -> async_ftdi::SerialParams {
        let data_bits = match self.data_bits {
            DataBits::Seven => async_ftdi::DataBits::Seven,
            DataBits::Eight => async_ftdi::DataBits::Eight,
        };
        let parity = match self.parity {
            Parity::Even => async_ftdi::Parity::Even,
            Parity::Odd => async_ftdi::Parity::Odd,
            Parity::None => async_ftdi::Parity::None,
        };
        let stop_bits = match self.stop_bits {
            StopBits::One => async_ftdi::StopBits::One,
            StopBits::Two => async_ftdi::StopBits::Two,
        };
        async_ftdi::SerialParams {
            baud: self.baud,
            data_bits,
            stop_bits,
            parity,
        }
    }
}

struct Handler {
    device: Option<(Ftdi, SerialParams)>,
    serial_number: String,
}

impl Handler {
    fn new(serial_number: &str) -> Self {
        Self {
            device: None,
            serial_number: serial_number.to_string(),
        }
    }
}

#[async_trait]
impl IoHandler for Handler {
    type Request = FtdiRequest;
    type Response = FtdiResponse;

    async fn handle(&mut self, req: Self::Request) -> crate::Result<Self::Response> {
        let params = req.params();
        let mut ftdi = if let Some((ftdi, open_params)) = self.device.take() {
            if params != open_params {
                ftdi.close();
                // XXX: unfortunately there is no synchroniziation on closing the handles
                sleep(Duration::from_millis(50)).await;
                let new_params: async_ftdi::SerialParams = params.clone().into();
                Ftdi::open(&self.serial_number, &new_params).await?
            } else {
                ftdi
            }
        } else {
            Ftdi::open(&self.serial_number, &params.clone().into()).await?
        };

        let ret = match req {
            FtdiRequest::ModBus {
                req, slave_addr, ..
            } => {
                let _ = read_all(&mut ftdi).await.unwrap();

                let channel = ClonableChannel::new(ftdi);
                let mut ctx = rtu::connect_slave(channel.clone(), Slave(slave_addr)).await?;
                let timeout = Duration::from_millis(1000);
                let ret = handle_modbus_request_timeout(&mut ctx, req, timeout)
                    .await
                    .map(FtdiResponse::ModBus);
                ftdi = channel.take().unwrap();
                ret
            }
            FtdiRequest::Bytes { req, .. } => bytestream::handle(&mut ftdi, req)
                .await
                .map(FtdiResponse::Bytes),
        };

        if !ret.is_err() {
            self.device.replace((ftdi, params));
        } else {
            ftdi.close();
        }
        ret
    }

    async fn disconnect(&mut self) {
        if let Some((ftdi, ..)) = self.device.take() {
            ftdi.close();
        }
    }
}

fn from_async_ftdi_info(info: async_ftdi::DeviceInfo) -> FtdiDeviceInfo {
    FtdiDeviceInfo {
        port_open: info.port_open,
        vendor_id: info.vendor_id,
        product_id: info.product_id,
        serial_number: info.serial_number,
        description: info.description,
    }
}

pub async fn list_ftdi() -> crate::Result<Vec<FtdiDeviceInfo>> {
    let mut ret: Vec<_> = Ftdi::list_devices()
        .await?
        .drain(..)
        .map(from_async_ftdi_info)
        .collect();
    ret.sort_by(|x, y| x.serial_number.partial_cmp(&y.serial_number).unwrap());
    Ok(ret)
}
