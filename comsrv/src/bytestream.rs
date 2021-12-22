use std::future::Future;
use std::io;
use std::pin::Pin;
use std::task::{Context, Poll};
/// This module implements a request handler for handling operation on a bytesstream-like
/// instrument, for example TCP streams or serial ports
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, ReadBuf};
use tokio::time::{timeout, Duration};

use crate::cobs::{cobs_decode, cobs_encode};
use crate::Error;
use comsrv_protocol::{ByteStreamRequest, ByteStreamResponse};

struct ReadAll<'a, T: AsyncRead + Unpin> {
    inner: &'a mut T,
}

impl<'a, T: AsyncRead + Unpin> Future for ReadAll<'a, T> {
    type Output = io::Result<Vec<u8>>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut ret = Vec::new();
        loop {
            let mut buf_data = [0_u8; 1000];
            let mut buf = ReadBuf::new(&mut buf_data);
            match Pin::new(&mut self.inner).poll_read(cx, &mut buf) {
                Poll::Ready(Ok(())) => {
                    ret.extend_from_slice(buf.filled());
                    continue;
                }
                Poll::Ready(Err(err)) => {
                    return Poll::Ready(Err(err));
                }
                Poll::Pending => {
                    return Poll::Ready(Ok(ret));
                }
            }
        }
    }
}

pub async fn read_all<T: AsyncRead + Unpin>(stream: &mut T) -> io::Result<Vec<u8>> {
    let fut = ReadAll {
        inner: stream
    };
    fut.await
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
            let ret = read_all(stream).await?;
            Ok(ByteStreamResponse::Data(ret))
        }
        ByteStreamRequest::CobsWrite(data) => {
            let data = cobs_encode(&data);
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
            read_all(stream).await?;
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
            read_all(stream).await?;
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
        ByteStreamRequest::ModBusRtuDdp { timeout_ms, station_address, custom_command, sub_cmd, ddp_cmd, response, data } => {
            let duration = Duration::from_millis(timeout_ms as u64);
            let fut = modbus_ddp_rtu(stream, station_address, custom_command, sub_cmd, ddp_cmd, response, data);
            match timeout(duration, fut).await {
                Ok(x) => x,
                Err(_) => Err(crate::Error::Timeout),
            }
        }
    }
}

/// pop a u8 from a byte stream
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
    let ret = cobs_decode(&ret).unwrap();
    Ok(ByteStreamResponse::Data(ret))
}

async fn cobs_query<T: AsyncRead + AsyncWrite + Unpin>(
    stream: &mut T,
    data: Vec<u8>,
) -> crate::Result<ByteStreamResponse> {
    let mut garbage = Vec::new();
    let fut = stream.read_buf(&mut garbage);
    if let Ok(x) = timeout(Duration::from_micros(100), fut).await {
        x?;
    };
    let data = cobs_encode(&data);
    AsyncWriteExt::write_all(stream, &data)
        .await
        .map_err(Error::io)?;
    cobs_read(stream).await
}

pub fn ddp_crc(data: &[u8]) -> u16 {
    let mut crc = 0xFFFF_u16;
    for x in data {
        crc ^= *x as u16;
        for _ in 0..8 {
            if crc & 1 != 0 {
                crc = (crc >> 1) ^ 0xA001;
            } else {
                crc >>= 1;
            }
        }
    }
    crc
}

async fn modbus_ddp_rtu<T: AsyncRead + AsyncWrite + Unpin>(
    stream: &mut T,
    station_address: u8,
    custom_cmd: u8,
    sub_cmd: u8,
    mut ddp_cmd: u8,
    response: bool,
    data: Vec<u8>) -> crate::Result<ByteStreamResponse> {
    let _ = read_all(stream).await;
    if response {
        ddp_cmd |= 0x80;
    }
    let mut req = vec![
        station_address,
        custom_cmd,
        sub_cmd,
        (data.len() + 1) as u8,
        ddp_cmd,
    ];
    req.extend(data);
    let msg_crc = ddp_crc(&req);
    req.push((msg_crc & 0xFF) as u8);
    req.push(((msg_crc >> 8) & 0xFF) as u8);
    stream.write(&req).await?;
    let mut data = vec![0_u8; 300];
    stream.read_exact(&mut data[0..4]).await?;
    if data[0] != station_address
        || data[1] != custom_cmd
        || data[2] != sub_cmd
    {
        return Err(crate::Error::InvalidResponse);
    }
    if !response {
        return Ok(ByteStreamResponse::Data(vec![]));
    }
    let len = data[3];
    if len == 0 {
        return Err(crate::Error::InvalidResponse);
    }
    stream.read_exact(&mut data[4..6 + len as usize]).await?;

    if ddp_crc(&data[0..6 + len as usize]) != 0 {
        return Err(crate::Error::InvalidResponse);
    }
    let reply = &data[4..4 + len as usize];
    Ok(ByteStreamResponse::Data(reply.to_vec()))
}
