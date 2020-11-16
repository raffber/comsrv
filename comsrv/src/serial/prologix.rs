use tokio_serial::Serial;
use crate::{Error, ScpiRequest, ScpiResponse};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::time::{delay_for, timeout};
use std::time::{Duration, Instant};

const PROLOGIX_TIMEOUT: f32 = 1.0;

pub async fn init_prologix(serial: &mut Serial) -> crate::Result<()> {
    log::debug!("Initalizing prologix.");
    write(serial, "++savecfg 0\n").await?;
    write(serial, "++auto 0\n").await?;
    // we manually append termination chars
    write(serial, "++eos 3\n").await?;
    write(serial, "++ifc 0\n").await
}

pub async fn handle_prologix_request(serial: &mut Serial, addr: u8, req: ScpiRequest) -> crate::Result<ScpiResponse> {
    log::debug!("handling prologix request for address {}", addr);
    let mut ret = Vec::with_capacity(128);
    let fut = AsyncReadExt::read(serial, &mut ret);
    match timeout(Duration::from_micros(100), fut).await {
        Ok(x) => {
            x.map_err(Error::io)?;
        }
        Err(_) => {}
    };
    log::debug!("Read: {:?}", ret);
    ret.clear();
    let addr_set = format!("++addr {}\n", addr);
    serial.write(addr_set.as_bytes()).await.map_err(Error::io)?;
    match req {
        ScpiRequest::Write(x) => {
            write_prologix(serial, x).await?;
            Ok(ScpiResponse::Done)
        }
        ScpiRequest::QueryString(x) => {
            write_prologix(serial, x).await?;
            write(serial, "++read eoi\n").await?;
            let reply = read_prologix(serial).await?;
            Ok(ScpiResponse::String(reply))
        }
        ScpiRequest::QueryBinary(_) => {
            log::error!("ScpiRequest::QueryBinary not implemented for Prologix!!");
            Err(Error::NotSupported)
        }
        ScpiRequest::ReadRaw => {
            write(serial, "++read eoi\n").await?;
            delay_for(Duration::from_millis(100)).await;
            let mut ret = Vec::new();
            serial.read(&mut ret).await.map_err(Error::io)?;
            Ok(ScpiResponse::Binary { data: ret })
        }
    }
}

async fn write(serial: &mut Serial, msg: &str) -> crate::Result<()> {
    serial.write(msg.as_bytes()).await.map(|_| ()).map_err(Error::io)
}


async fn write_prologix(serial: &mut Serial, mut msg: String) -> crate::Result<()> {
    if !msg.ends_with("\n") {
        msg.push_str("\n");
    }
    serial.write(msg.as_bytes()).await.map(|_| ()).map_err(Error::io)
}

async fn read_prologix(serial: &mut Serial) -> crate::Result<String> {
    let start = Instant::now();
    let mut ret = Vec::new();
    loop {
        let mut x = [0; 1];
        match timeout(Duration::from_secs_f32(PROLOGIX_TIMEOUT), serial.read_exact(&mut x)).await {
            Ok(Ok(_)) => {
                let x = x[0];
                if x == b'\n' {
                    let mut garbage = Vec::new();
                    serial.read(&mut garbage).await.map_err(Error::io)?;
                    break;
                }
                ret.push(x);
            }
            Ok(Err(x)) => {
                log::debug!("read error");
                return Err(Error::io(x));
            }
            Err(_) => {
                log::debug!("instrument read timeout");
                return Err(Error::Timeout);
            }
        };
        let delta = start.elapsed().as_secs_f32();
        if delta > PROLOGIX_TIMEOUT {
            return Err(Error::Timeout);
        }
    }
    String::from_utf8(ret).map_err(Error::DecodeError)
}

