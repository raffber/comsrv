use std::sync::Arc;

use crate::iotask::{IoHandler, IoTask, IoContext};
use async_trait::async_trait;
use comsrv_protocol::{HidDeviceInfo, HidIdentifier, HidRequest, HidResponse, TransportError};
use hidapi::{HidApi, HidResult, HidError};
use hidapi::HidDevice as HidApiDevice;
use lazy_static::lazy_static;

use tokio::task;

lazy_static! {
    static ref HID_API: HidResult<HidApi> = HidApi::new();
}

fn get_hidapi() -> crate::Result<&'static HidApi> {
    match HID_API.as_ref() {
        Ok(api) => Ok(api),
        Err(x) => {
            let err: anyhow::Error = x.into();
            Err(crate::Error::transport(err))
        }
    }
}

fn to_error(x: HidError) -> crate::Error {
    let err: anyhow::Error = x.into();
    crate::Error::Transport(TransportError::Other(Arc::new(err))) 
}

struct Handler {
    device: Option<HidApiDevice>,
    idn: HidIdentifier,
}

fn open_device(idn: &HidIdentifier) -> crate::Result<HidApiDevice> {
    get_hidapi()?.open(idn.vid, idn.pid).map_err(to_error)
}

#[async_trait]
impl IoHandler for Handler {
    type Request = HidRequest;
    type Response = HidResponse;

    async fn handle(&mut self, ctx: &mut IoContext<Self>, req: Self::Request) -> crate::Result<Self::Response> {
        let device = self.device.take();
        let idn = self.idn.clone();
        let (device, result) = task::spawn_blocking(move || handle_blocking(device, &idn, req))
            .await
            .unwrap();
        self.device = device;
        result
    }
}

fn handle_blocking(
    device: Option<HidApiDevice>,
    idn: &HidIdentifier,
    req: HidRequest,
) -> (Option<HidApiDevice>, crate::Result<HidResponse>) {
    let mut device = match device {
        Some(dev) => dev,
        None => match open_device(idn) {
            Ok(device) => device,
            Err(e) => return (None, Err(e)),
        },
    };
    let ret = handle_request(&mut device, idn, req);
    if ret.is_ok() {
        (Some(device), ret)
    } else {
        (None, ret)
    }
}

fn handle_request(
    device: &mut HidApiDevice,
    idn: &HidIdentifier,
    req: HidRequest,
) -> crate::Result<HidResponse> {
    match req {
        HidRequest::Write { data } => {
            device.write(&data).map(|_| HidResponse::Ok).map_err(to_error)
        }
        HidRequest::Read { timeout_ms } => {
            let mut buf = [0u8; 64];
            device
                .read_timeout(&mut buf, timeout_ms)
                .map_err(to_error)
                .and_then(|x| {
                    if x == 0 {
                        return Err(crate::Error::protocol_timeout());
                    }
                    Ok(HidResponse::Data(buf[0..x].to_vec()))
                })
        }
        HidRequest::GetInfo => {
            let mfr = device.get_manufacturer_string().map_err(to_error)?;
            let product = device.get_product_string().map_err(to_error)?;
            let serial_number = device.get_serial_number_string().map_err(to_error)?;
            Ok(HidResponse::Info(HidDeviceInfo {
                idn: idn.clone(),
                manufacturer: mfr,
                product,
                serial_number,
            }))
        }
    }
}

#[derive(Clone)]
pub struct Instrument {
    idn: HidIdentifier,
    inner: IoTask<Handler>,
}

impl Instrument {
    pub fn new(idn: &HidIdentifier) -> Instrument {
        let handler = Handler {
            device: None,
            idn: idn.clone(),
        };
        Self {
            idn: idn.clone(),
            inner: IoTask::new(handler),
        }
    }

    pub async fn request(&mut self, req: HidRequest) -> crate::Result<HidResponse> {
        self.inner.request(req).await
    }

    pub fn disconnect(mut self) {
        self.inner.disconnect()
    }
}

impl crate::inventory::Instrument for Instrument {
    type Address = HidIdentifier;

    fn connect(server: &crate::app::Server, addr: &Self::Address) -> Self {
        Instrument::new(addr)
    }
}

fn list_devices_blocking() -> crate::Result<Vec<HidDeviceInfo>> {
    let api = get_hidapi()?;
    let ret = api
        .device_list()
        .map(|device| {
            let idn = HidIdentifier::new(device.vendor_id(), device.product_id());
            HidDeviceInfo {
                idn,
                manufacturer: device.manufacturer_string().map(|x| x.to_string()),
                product: device.product_string().map(|x| x.to_string()),
                serial_number: device.serial_number().map(|x| x.to_string()),
            }
        })
        .collect();
    Ok(ret)
}

pub async fn list_devices() -> crate::Result<Vec<HidDeviceInfo>> {
    task::spawn_blocking(|| list_devices_blocking())
        .await
        .unwrap()
}
