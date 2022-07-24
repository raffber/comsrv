
use anyhow::anyhow;
use futures::AsyncRead;
use tokio::io::AsyncWrite;
use crate::modbus::FunctionCode;

pub struct Ddp {
    ddp_cmd: u8,
    sub_cmd: u8,
    request: Vec<u8>,
    response: bool,
}

impl Ddp {
    pub fn new(ddp_cmd: u8, sub_cmd: u8, request: Vec<u8>, response: bool) -> crate::Result<Self> {
        if request.len() > u8::MAX as usize {
            return Err(crate::Error::argument(anyhow!("Maximum DDP length is {} but got {}", u8::MAX, request.len())));
        }
        Ok(Self {
            ddp_cmd,
            sub_cmd,
            request,
            response
        })
    }
}

impl FunctionCode for Ddp {
    type Output = Vec<u8>;

    fn format_request(&self, data: &mut Vec<u8>) {
        let mut ddp_cmd = self.ddp_cmd;
        if self.response {
            ddp_cmd |= 0x80;
        }
        data.push(self.sub_cmd);
        data.push((self.request.len() + 1) as u8);
        data.push(self.ddp_cmd);
        data.extend(&self.request);
    }

    fn get_header_length(&self) -> usize {
        2
    }

    fn get_data_length_from_header(&self, data: &[u8]) -> crate::Result<usize> {
        let sub_cmd = data[0];
        let len = data[1];
        if sub_cmd != self.sub_cmd {
            return Err(crate::Error::protocol(anyhow!("Invalid Response")));
        }
        Ok(len as usize)
    }

    fn parse_frame(&self, data: &[u8]) -> crate::Result<Self::Output> {
        Ok(data.to_vec())
    }

    fn function_code(&self) -> u8 {
        0x44
    }
}
