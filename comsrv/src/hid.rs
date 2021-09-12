use crate::iotask::{IoHandler, IoTask};
use async_trait::async_trait;
use hidapi::{HidApi, HidDevice as HidApiDevice, HidError as HidApiError, HidResult};
use lazy_static::lazy_static;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::fmt::{Display, Formatter};
use tokio::task;
use comsrv_protocol::{HidRequest, HidIdentifier, HidResponse};

lazy_static! {
    static ref HID_API: HidResult<HidApi> = HidApi::new();
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HidError(String);

impl Display for HidError {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        write!(f, "{:?}", self.0)
    }
}

impl From<HidApiError> for HidError {
    fn from(x: HidApiError) -> Self {
        HidError(x.to_string())
    }
}

impl From<HidError> for crate::Error {
    fn from(x: HidError) -> Self {
        crate::Error::Hid(x)
    }
}

impl From<HidApiError> for crate::Error {
    fn from(x: HidApiError) -> Self {
        crate::Error::Hid(x.into())
    }
}

impl From<&HidApiError> for HidError {
    fn from(x: &HidApiError) -> Self {
        HidError(x.to_string())
    }
}

struct Handler {
    device: Option<HidApiDevice>,
    idn: HidIdentifier,
}

fn open_device(idn: &HidIdentifier) -> crate::Result<HidApiDevice> {
    match HID_API.as_ref() {
        Ok(api) => Ok(api.open(idn.vid, idn.pid)?),
        Err(x) => Err(crate::Error::Hid(x.into())),
    }
}

#[async_trait]
impl IoHandler for Handler {
    type Request = HidRequest;
    type Response = HidResponse;

    async fn handle(&mut self, req: Self::Request) -> crate::Result<Self::Response> {
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
    (Some(device), ret)
}

fn handle_request(
    device: &mut HidApiDevice,
    idn: &HidIdentifier,
    req: HidRequest,
) -> crate::Result<HidResponse> {
    match req {
        HidRequest::Write { data } => Ok(device.write(&data).map(|_| HidResponse::Ok)?),
        HidRequest::Read { timeout_ms } => {
            let mut buf = [0u8; 64];
            device
                .read_timeout(&mut buf, timeout_ms)
                .map_err(|x| x.into())
                .and_then(|x| {
                    if x == 0 {
                        return Err(crate::Error::Timeout);
                    }
                    Ok(HidResponse::Data(buf[0..x].to_vec()))
                })
        }
        HidRequest::GetInfo => {
            let mfr = device.get_manufacturer_string()?;
            let product = device.get_product_string()?;
            let serial_number = device.get_serial_number_string()?;
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
    pub fn new(idn: HidIdentifier) -> Instrument {
        let handler = Handler {
            device: None,
            idn: idn.clone(),
        };
        Self {
            idn,
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

fn list_devices_blocking() -> crate::Result<Vec<HidDeviceInfo>> {
    let api = match HID_API.as_ref() {
        Ok(api) => Ok(api),
        Err(x) => Err(crate::Error::Hid(x.into())),
    }?;
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
