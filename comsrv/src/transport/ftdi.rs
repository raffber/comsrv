use async_ftdi::Ftdi;
use async_trait::async_trait;
use comsrv_protocol::ByteStreamRequest;
use comsrv_protocol::ByteStreamResponse;
use comsrv_protocol::FtdiAddress;
use comsrv_protocol::FtdiDeviceInfo;

use comsrv_protocol::SerialOptions;
use comsrv_protocol::SerialPortConfig;
use std::cmp::PartialOrd;
use std::convert::TryInto;

use crate::iotask::IoContext;
use crate::iotask::IoHandler;
use crate::iotask::IoTask;
use crate::transport::serial::params::DataBits;
use crate::transport::serial::params::Parity;
use crate::transport::serial::params::StopBits;
use crate::transport::serial::SerialParams;

pub struct FtdiRequest {
    pub request: ByteStreamRequest,
    pub port_config: SerialPortConfig,
    pub options: Option<SerialOptions>,
}

impl FtdiRequest {
    fn params(&self) -> crate::Result<SerialParams> {
        self.port_config.clone().try_into()
    }
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

    pub async fn request(&mut self, req: FtdiRequest) -> crate::Result<ByteStreamResponse> {
        self.inner.request(req).await
    }
}

impl From<SerialParams> for async_ftdi::SerialParams {
    fn from(val: SerialParams) -> Self {
        let data_bits = match val.data_bits {
            DataBits::Seven => async_ftdi::DataBits::Seven,
            DataBits::Eight => async_ftdi::DataBits::Eight,
        };
        let parity = match val.parity {
            Parity::Even => async_ftdi::Parity::Even,
            Parity::Odd => async_ftdi::Parity::Odd,
            Parity::None => async_ftdi::Parity::None,
        };
        let stop_bits = match val.stop_bits {
            StopBits::One => async_ftdi::StopBits::One,
            StopBits::Two => async_ftdi::StopBits::Two,
        };
        async_ftdi::SerialParams {
            baud: val.baud,
            data_bits,
            stop_bits,
            parity,
        }
    }
}

#[async_trait]
impl crate::inventory::Instrument for Instrument {
    type Address = FtdiAddress;

    fn connect(_server: &crate::app::Server, addr: &Self::Address) -> crate::Result<Self> {
        Ok(Self::new(&addr.port))
    }

    async fn wait_for_closed(&self) {
        self.inner.wait_for_closed().await
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
    type Response = ByteStreamResponse;

    async fn handle(&mut self, _ctx: &mut IoContext<Self>, req: Self::Request) -> crate::Result<Self::Response> {
        let params = req.params()?;
        let mut ftdi: Ftdi = if let Some((mut ftdi, open_params)) = self.device.take() {
            if params != open_params {
                if let Err(x) = ftdi.set_params(params.clone().into()).await {
                    ftdi.close().await;
                    return Err(x.into());
                }
            }
            ftdi
        } else {
            Ftdi::open(&self.serial_number, &params.clone().into()).await?
        };

        let ret = crate::protocol::bytestream::handle(&mut ftdi, req.request).await;
        match &ret {
            Ok(_) | Err(crate::Error::Protocol(_)) => {
                self.device.replace((ftdi, params));
            }
            Err(_) => {
                ftdi.close().await;
            }
        }
        ret
    }

    async fn disconnect(&mut self) {
        if let Some((ftdi, ..)) = self.device.take() {
            ftdi.close().await;
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
        .filter(|x| !x.serial_number.trim().is_empty())
        .map(from_async_ftdi_info)
        .collect();
    ret.sort_by(|x, y| x.serial_number.partial_cmp(&y.serial_number).unwrap());
    Ok(ret)
}
