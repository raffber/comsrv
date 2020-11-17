use tokio::io::{AsyncWrite, AsyncRead, AsyncReadExt, AsyncWriteExt};
use crate::Error;
use crate::cobs::{cobs_pack, cobs_unpack};
use serde::{Serialize, Deserialize};
use tokio::time::{Duration, timeout, Instant};


#[derive(Clone, Serialize, Deserialize)]
pub enum ByteStreamRequest {
    Write(Vec<u8>),
    ReadExact {
        count: u32,
        timeout_ms: u32,
    },
    ReadUpTo(u32),
    ReadAll,
    CobsWrite(Vec<u8>),
    CobsQuery {
        data: Vec<u8>,
        timeout_ms: u32,
    },
}

#[derive(Clone, Serialize, Deserialize)]
pub enum ByteStreamResponse {
    Done,
    Data(Vec<u8>),
}

pub async fn handle<T: AsyncRead + AsyncWrite + Unpin>(stream: &mut T, req: ByteStreamRequest) -> crate::Result<ByteStreamResponse> {
    match req {
        ByteStreamRequest::Write(data) => {
            log::debug!("write: {:?}", data);
            AsyncWriteExt::write_all(stream, &data).await.map_err(Error::io)?;
            Ok(ByteStreamResponse::Done)
        }
        ByteStreamRequest::ReadExact { count, timeout_ms } => {
            log::debug!("read exactly {} bytes", count);
            let mut data = vec![0; count as usize];
            let fut = AsyncReadExt::read_exact(stream, data.as_mut_slice());
            let _ = match timeout(Duration::from_millis(timeout_ms as u64), fut).await {
                Ok(x) => x.map_err(Error::io),
                Err(_) => Err(Error::Timeout),
            }?;
            Ok(ByteStreamResponse::Data(data))
        }
        ByteStreamRequest::ReadUpTo(count) => {
            log::debug!("read up to {} bytes", count);
            let mut data = vec![0; count as usize];
            let fut = AsyncReadExt::read(stream, &mut data);
            let num_read = match timeout(Duration::from_micros(100), fut).await {
                Ok(x) => x.map_err(Error::io)?,
                Err(_) => 0,
            };
            let data = data[..num_read].to_vec();
            Ok(ByteStreamResponse::Data(data))
        }
        ByteStreamRequest::ReadAll => {
            log::debug!("read all bytes");
            let mut ret = Vec::new();
            let fut = AsyncReadExt::read_buf(stream, &mut ret);
            match timeout(Duration::from_micros(100), fut).await {
                Ok(x) => {
                    x.map_err(Error::io)?;
                }
                Err(_) => {}
            };
            Ok(ByteStreamResponse::Data(ret))
        }
        ByteStreamRequest::CobsWrite(data) => {
            let data = cobs_pack(&data);
            AsyncWriteExt::write_all(stream, &data).await.map_err(Error::io)?;
            Ok(ByteStreamResponse::Done)
        }
        ByteStreamRequest::CobsQuery { data, timeout_ms } => {
            cobs_query(stream, data, timeout_ms).await
        }
    }
}

async fn pop<T: AsyncRead + AsyncWrite + Unpin>(stream: &mut T, timeout_ms: u32) -> crate::Result<u8> {
    let fut = AsyncReadExt::read_u8(stream);
    match timeout(Duration::from_millis(timeout_ms as u64), fut).await {
        Ok(x) => x.map_err(Error::io),
        Err(_) => Err(Error::Timeout),
    }
}

async fn cobs_query<T: AsyncRead + AsyncWrite + Unpin>(stream: &mut T, data: Vec<u8>, timeout_ms: u32) -> crate::Result<ByteStreamResponse> {
    let mut garbage = Vec::new();
    let fut = stream.read_buf(&mut garbage);
    match timeout(Duration::from_micros(100), fut).await {
        Ok(x) => {
            x.map_err(Error::io)?;
        }
        Err(_) => {}
    };
    let data = cobs_pack(&data);
    AsyncWriteExt::write_all(stream, &data).await.map_err(Error::io)?;
    let mut ret = Vec::new();
    let start = Instant::now();
    while ret.len() == 0 {
        let x = pop(stream, timeout_ms).await?;
        if (Instant::now() - start).as_millis() > timeout_ms as u128 {
            return Err(Error::Timeout);
        }
        if x == 0 {
            continue;
        }
        ret.push(x);
    }
    loop {
        let x = pop(stream, timeout_ms).await?;
        if (Instant::now() - start).as_millis() > timeout_ms as u128 {
            return Err(Error::Timeout);
        }
        ret.push(x);
        if x == 0 {
            break;
        }
    }
    // unwrap is save because we cancel above loop only in case we pushed x == 0
    let ret = cobs_unpack(&ret).unwrap();
    Ok(ByteStreamResponse::Data(ret))
}
