use serde::{Deserialize, Serialize};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::time::{timeout, Duration};

use crate::cobs::{cobs_pack, cobs_unpack};
use crate::Error;

#[derive(Clone, Serialize, Deserialize)]
pub enum ByteStreamRequest {
    Write(Vec<u8>),
    ReadToTerm {
        term: u8,
        timeout_ms: u32,
    },
    ReadExact {
        count: u32,
        timeout_ms: u32,
    },
    ReadUpTo(u32),
    ReadAll,
    CobsWrite(Vec<u8>),
    CobsRead(u32), // timeout
    CobsQuery {
        data: Vec<u8>,
        timeout_ms: u32,
    },
    WriteLine {
        line: String,
        term: u8,
    },
    ReadLine {
        timeout_ms: u32,
        term: u8,
    },
    QueryLine {
        line: String,
        timeout_ms: u32,
        term: u8,
    },
}

#[derive(Clone, Serialize, Deserialize)]
pub enum ByteStreamResponse {
    Done,
    Data(Vec<u8>),
    String(String),
}

pub async fn handle<T: AsyncRead + AsyncWrite + Unpin>(
    stream: &mut T,
    req: ByteStreamRequest,
) -> crate::Result<ByteStreamResponse> {
    match req {
        ByteStreamRequest::Write(data) => {
            log::debug!("write: {:?}", data);
            AsyncWriteExt::write_all(stream, &data)
                .await
                .map_err(Error::io)?;
            Ok(ByteStreamResponse::Done)
        }
        ByteStreamRequest::ReadExact { count, timeout_ms } => {
            log::debug!("read exactly {} bytes", count);
            let mut data = vec![0; count as usize];
            let fut = AsyncReadExt::read_exact(stream, data.as_mut_slice());
            let _ = match timeout(Duration::from_millis(timeout_ms as u64), fut).await {
                Ok(x) => Ok(x?),
                Err(_) => Err(Error::Timeout),
            }?;
            Ok(ByteStreamResponse::Data(data))
        }
        ByteStreamRequest::ReadUpTo(count) => {
            log::debug!("read up to {} bytes", count);
            let mut data = vec![0; count as usize];
            let fut = AsyncReadExt::read(stream, &mut data);
            let num_read = match timeout(Duration::from_micros(100), fut).await {
                Ok(x) => x?,
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
                    x?;
                }
                Err(_) => {}
            };
            Ok(ByteStreamResponse::Data(ret))
        }
        ByteStreamRequest::CobsWrite(data) => {
            let data = cobs_pack(&data);
            AsyncWriteExt::write_all(stream, &data).await?;
            Ok(ByteStreamResponse::Done)
        }
        ByteStreamRequest::CobsRead(timeout_ms) => {
            let duration = Duration::from_millis(timeout_ms as u64);
            match timeout(duration, cobs_read(stream)).await {
                Ok(x) => x,
                Err(_) => Err(crate::Error::Timeout),
            }
        }
        ByteStreamRequest::CobsQuery { data, timeout_ms } => {
            let duration = Duration::from_millis(timeout_ms as u64);
            match timeout(duration, cobs_query(stream, data)).await {
                Ok(x) => x,
                Err(_) => Err(crate::Error::Timeout),
            }
        }
        ByteStreamRequest::WriteLine { mut line, term } => {
            check_term(term)?;
            line.push(term as char);
            AsyncWriteExt::write_all(stream, line.as_bytes()).await?;
            Ok(ByteStreamResponse::Done)
        }
        ByteStreamRequest::ReadLine { timeout_ms, term } => {
            check_term(term)?;
            let ret = read_to_term_timeout(stream, term, timeout_ms).await?;
            let ret = String::from_utf8(ret).map_err(crate::Error::DecodeError)?;
            Ok(ByteStreamResponse::String(ret))
        }
        ByteStreamRequest::QueryLine {
            mut line,
            timeout_ms,
            term,
        } => {
            empty_buf(stream).await;
            check_term(term)?;
            line.push(term as char);
            AsyncWriteExt::write_all(stream, line.as_bytes()).await?;
            let ret = read_to_term_timeout(stream, term, timeout_ms).await?;
            let ret = String::from_utf8(ret).map_err(crate::Error::DecodeError)?;
            Ok(ByteStreamResponse::String(ret))
        }
        ByteStreamRequest::ReadToTerm { term, timeout_ms } => {
            let ret = read_to_term_timeout(stream, term, timeout_ms).await?;
            Ok(ByteStreamResponse::Data(ret))
        }
    }
}

async fn empty_buf<T: AsyncRead + Unpin>(stream: &mut T) {
    let mut ret = Vec::new();
    let fut = AsyncReadExt::read_buf(stream, &mut ret);
    let _ = timeout(Duration::from_micros(100), fut).await;
}

async fn pop<T: AsyncRead + Unpin>(stream: &mut T) -> crate::Result<u8> {
    Ok(AsyncReadExt::read_u8(stream).await?)
}

async fn read_to_term_timeout<T: AsyncReadExt + Unpin>(
    stream: &mut T,
    term: u8,
    timeout_ms: u32,
) -> crate::Result<Vec<u8>> {
    let duration = Duration::from_millis(timeout_ms as u64);
    let fut = read_to_term(stream, term);
    match timeout(duration, fut).await {
        Ok(x) => x,
        Err(_) => Err(crate::Error::Timeout),
    }
}

async fn read_to_term<T: AsyncReadExt + Unpin>(stream: &mut T, term: u8) -> crate::Result<Vec<u8>> {
    let mut ret = Vec::new();
    loop {
        let x = pop(stream).await?;
        if x == term {
            break;
        }
        ret.push(x);
    }
    Ok(ret)
}

fn check_term(term: u8) -> crate::Result<()> {
    if term == 0 || term > 128 {
        Err(crate::Error::InvalidRequest)
    } else {
        Ok(())
    }
}

async fn cobs_read<T: AsyncRead + Unpin>(stream: &mut T) -> crate::Result<ByteStreamResponse> {
    let mut ret = Vec::new();
    // keep readings zeroes
    loop {
        let x = pop(stream).await?;
        if x != 0 {
            ret.push(x);
            break;
        }
    }
    // read non-zero values
    loop {
        let x = pop(stream).await?;
        ret.push(x);
        if x == 0 {
            break;
        }
    }
    // unwrap is save because we cancel above loop only in case we pushed x == 0
    let ret = cobs_unpack(&ret).unwrap();
    Ok(ByteStreamResponse::Data(ret))
}

async fn cobs_query<T: AsyncRead + AsyncWrite + Unpin>(
    stream: &mut T,
    data: Vec<u8>,
) -> crate::Result<ByteStreamResponse> {
    let mut garbage = Vec::new();
    let fut = stream.read_buf(&mut garbage);
    match timeout(Duration::from_micros(100), fut).await {
        Ok(x) => {
            x?;
        }
        Err(_) => {}
    };
    let data = cobs_pack(&data);
    AsyncWriteExt::write_all(stream, &data)
        .await
        .map_err(Error::io)?;
    cobs_read(stream).await
}
