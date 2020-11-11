use async_std::net::SocketAddr;
use crate::{ScpiRequest, ScpiResponse};
use crate::visa::VisaOptions;

#[derive(Clone)]
pub struct Instrument {

}

impl Instrument {
    pub fn new(addr: SocketAddr) -> Self {
        todo!()
    }

    pub fn disconnect(self) {
        todo!()
    }

    pub async fn request(self, req: ScpiRequest, options: VisaOptions) -> crate::Result<ScpiResponse> {
        todo!()
    }
}
