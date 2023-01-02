#![allow(dead_code)]

/// This modules implements COBS encode and decode functions as described
/// in [wikipedia](https://en.wikipedia.org/wiki/Consistent_Overhead_Byte_Stuffing).
use std::{io, pin::Pin};

use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

pub struct StreamingCobsDecoder<T: AsyncRead> {
    inner: Pin<Box<T>>,
    buf: Vec<u8>,
    max_length: usize,
    restart: bool,
}

impl<T: AsyncRead> StreamingCobsDecoder<T> {
    pub fn new(stream: T, max_length: usize) -> Self {
        Self {
            inner: Box::pin(stream),
            buf: Vec::with_capacity(1024),
            max_length,
            restart: false,
        }
    }

    pub async fn next(&mut self) -> io::Result<Vec<u8>> {
        loop {
            let x = self.inner.read_u8().await?;
            if x == 0 {
                if self.restart {
                    // we lost a frame, hence reset the framer
                    self.restart = false;
                    continue;
                }
                let ret = cobs_decode(&self.buf);
                self.buf.clear();
                return Ok(ret);
            }
            if self.restart {
                continue;
            }
            if self.buf.len() > self.max_length {
                // drop the frame
                self.buf.clear();
                self.restart = true;
                continue;
            }
            self.buf.push(x);
        }
    }
}

pub struct CobsEncoder<T: AsyncWrite> {
    inner: Pin<Box<T>>,
}

impl<T: AsyncWrite> CobsEncoder<T> {
    pub fn new(stream: T) -> Self {
        Self {
            inner: Box::pin(stream),
        }
    }

    pub async fn send(&mut self, data: &[u8]) -> io::Result<()> {
        let data = cobs_encode(data);
        self.inner.write(&data).await.map(|_| ())
    }
}

/// Encode some data into a COBS frame
pub fn cobs_encode(data: &[u8]) -> Vec<u8> {
    let mut code_index = 0;
    let mut code = 1_u8;
    let max_len = data.len() + (data.len() + 254 - 1) / 254 + 1;
    let mut ret = Vec::with_capacity(max_len);
    ret.push(0);

    for x in data {
        if *x == 0 {
            ret[code_index] = code;
            code = 1;
            code_index = ret.len();
            ret.push(0);
        } else {
            ret.push(*x);
            code += 1;
            if code == 0xFF {
                ret[code_index] = code;
                code = 1;
                code_index = ret.len();
                ret.push(0);
            }
        }
    }
    ret[code_index] = code;
    ret.push(0);
    ret
}

/// Decode some data from a COBS frame
pub fn cobs_decode(mut buf: &[u8]) -> Vec<u8> {
    if buf.is_empty() {
        return Vec::new();
    }
    let mut ret = Vec::with_capacity(buf.len());
    let mut k = 0;
    if buf[buf.len() - 1] == 0 {
        buf = &buf[0..buf.len() - 1];
    }
    while k < buf.len() {
        let code = buf[k];
        k += 1;

        for _ in 1..code {
            if k >= buf.len() {
                break;
            }
            ret.push(buf[k]);
            k += 1;
        }
        if code < 0xFF && k < buf.len() {
            ret.push(0);
        }
    }
    ret
}

#[cfg(test)]
mod test {
    use super::*;

    fn check(data: &[u8]) -> Vec<u8> {
        let ret = cobs_encode(data);
        let decoded = cobs_decode(&ret);
        assert!(data == decoded);
        ret
    }

    #[test]
    fn test_cobs_encode1() {
        let encoded = check(&[1, 2, 3, 4]);
        assert!(encoded == [5, 1, 2, 3, 4, 0])
    }

    #[test]
    fn test_cobs_encode2() {
        let encoded = check(&[1, 2, 0, 4]);
        assert!(encoded == [3, 1, 2, 2, 4, 0])
    }

    #[test]
    fn test_cobs_encode3() {
        let encoded = check(&[1, 2, 0, 4, 5, 6, 7, 0, 10, 11, 12]);
        assert!(encoded == [3, 1, 2, 5, 4, 5, 6, 7, 4, 10, 11, 12, 0])
    }

    #[test]
    fn test_cobs_encode4() {
        let encoded = check(&[0, 0, 0, 1, 2, 3, 0]);
        assert!(encoded == [1, 1, 1, 4, 1, 2, 3, 1, 0])
    }

    #[test]
    fn test_cobs_encode_long() {
        let chunk: Vec<_> = (0..127).map(|x| x + 1).collect();
        let mut test_data = Vec::<u8>::new();
        for _ in 0..10 {
            test_data.extend(&chunk);
        }
        let encoded = check(&test_data);
        let mut ref_data = Vec::new();
        for _ in 0..5 {
            ref_data.push(0xFF_u8);
            ref_data.extend(&chunk);
            ref_data.extend(&chunk);
        }
        ref_data.push(1);
        ref_data.push(0);
        assert!(encoded == ref_data)
    }
}
