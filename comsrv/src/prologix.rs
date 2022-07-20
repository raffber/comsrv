use crate::bytestream::read_all;
use crate::Error;
use anyhow::anyhow;
use comsrv_protocol::{ScpiRequest, ScpiResponse};
use std::time::{Duration, Instant};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::time::{sleep, timeout};

const PROLOGIX_TIMEOUT: f32 = 1.0;

pub async fn init_prologix<T: AsyncRead + AsyncWrite + Unpin>(serial: &mut T) -> crate::Result<()> {
    log::debug!("Initalizing prologix.");
    write(serial, "++savecfg 0\n").await?;
    write(serial, "++auto 0\n").await?;
    // we manually append termination chars
    write(serial, "++eos 3\n").await
}

pub async fn handle_prologix_request<T: AsyncRead + AsyncWrite + Unpin>(
    serial: &mut T,
    addr: u8,
    req: ScpiRequest,
) -> crate::Result<ScpiResponse> {
    log::debug!("handling prologix request for address {}", addr);
    let _ = read_all(serial).await.map_err(crate::Error::transport)?;
    let addr_set = format!("++addr {}\n", addr);
    serial
        .write(addr_set.as_bytes())
        .await
        .map_err(Error::transport)?;
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
            log::error!("ScpiRequest::QueryBinary not implemented for Prologix.");
            Err(Error::argument(anyhow!(
                "ScpiRequest::QueryBinary not implemented for Prologix."
            )))
        }
        ScpiRequest::ReadRaw => {
            write(serial, "++read eoi\n").await?;
            sleep(Duration::from_millis(100)).await;
            let mut ret = Vec::new();
            serial.read(&mut ret).await.map_err(Error::transport)?;
            Ok(ScpiResponse::Binary { data: ret })
        }
    }
}

async fn write<T: AsyncWrite + Unpin>(serial: &mut T, msg: &str) -> crate::Result<()> {
    serial
        .write(msg.as_bytes())
        .await
        .map(|_| ())
        .map_err(Error::transport)
}

async fn write_prologix<T: AsyncWrite + Unpin>(
    serial: &mut T,
    mut msg: String,
) -> crate::Result<()> {
    if !msg.ends_with('\n') {
        msg.push('\n');
    }
    serial
        .write(msg.as_bytes())
        .await
        .map(|_| ())
        .map_err(Error::transport)
}

async fn read_prologix<T: AsyncRead + Unpin>(serial: &mut T) -> crate::Result<String> {
    let start = Instant::now();
    let mut ret = Vec::new();
    loop {
        let mut x = [0; 1];
        match timeout(
            Duration::from_secs_f32(PROLOGIX_TIMEOUT),
            serial.read_exact(&mut x),
        )
        .await
        {
            Ok(Ok(_)) => {
                let x = x[0];
                if x == b'\n' {
                    let mut garbage = Vec::new();
                    serial.read(&mut garbage).await.map_err(Error::transport)?;
                    break;
                }
                ret.push(x);
            }
            Ok(Err(x)) => {
                log::debug!("read error");
                return Err(Error::transport(x));
            }
            Err(_) => {
                log::debug!("instrument read timeout");
                return Err(Error::protocol_timeout());
            }
        };
        let delta = start.elapsed().as_secs_f32();
        if delta > PROLOGIX_TIMEOUT {
            return Err(Error::protocol_timeout());
        }
    }
    String::from_utf8(ret).map_err(|_| crate::Error::protocol(anyhow!("Could not decode reply.")))
}
