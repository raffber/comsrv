use std::collections::VecDeque;
use std::io;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use std::task::Poll;
use std::task::Context;

use comsrv_protocol::ByteStreamRequest;
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::sync::mpsc::{UnboundedSender as AsyncSender, UnboundedReceiver as AsyncReceiver};

use crate::serial::SerialParams;


pub struct FtdiAddress {
    index: u8,
    params: SerialParams,
}

pub struct Request {
    request: ByteStreamRequest,
    params: SerialParams,
}

enum BridgeSendMessage {
    Cancel,
    Data(Vec<u8>),
}

pub struct Bridge {
    cancel: Arc<AtomicBool>,
    sender: AsyncSender<BridgeSendMessage>,
    receier: AsyncReceiver<io::Result<Vec<u8>>>,
    buffer: VecDeque<u8>,
    tx_error: Mutex<Option<io::Error>>,
}

impl Bridge {
    fn new(address: FtdiAddress) {

    }

    fn push_to_output_buffer(&mut self, buf: &mut tokio::io::ReadBuf<'_>) -> bool {
        if !self.buffer.is_empty() {
            loop {
                if buf.remaining() == 0 {
                    return true;
                }
                if let Some(x) = self.buffer.pop_front() {
                    buf.put_slice(&[x]);
                } else {
                    break;
                }
            }
        }
        return buf.remaining() == 0;
    }
}

impl AsyncRead for Bridge {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut tokio::io::ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        if self.push_to_output_buffer(buf) {
            return Poll::Ready(Ok(()));
        }
        loop {
            match self.receier.poll_recv( cx) {
                Poll::Ready(Some(Ok(x))) => {
                    self.buffer.extend(&x);
                    if self.push_to_output_buffer(buf) {
                        return Poll::Ready(Ok(()));
                    }
                },
                Poll::Ready(Some(Err(x))) => {
                    return Poll::Ready(Err(x));
                },
                Poll::Ready(None) => {
                    return Poll::Ready(Err(io::Error::new(io::ErrorKind::Other, "Disconnected")));
                }
                Poll::Pending => return Poll::Pending,
            }
        }
    }
}

impl AsyncWrite for Bridge {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<Result<usize, io::Error>> {
        let lock = self.tx_error.lock().unwrap();
        if let Some(err) = lock.take() {
            return Poll::Ready(Err(err)); 
        }
        if self.sender.send(BridgeSendMessage::Data(buf.to_vec())).is_err() {
            return Poll::Ready(Err(io::Error::new(io::ErrorKind::Other, "Disconnected")));
        }
        Poll::Ready(Ok(buf.len()))
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), std::io::Error>> {
        // TODO: send a flush message and send a oneshot
        // then keep polling that oneshot
        Poll::Ready(Ok(()))
    }

    fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), std::io::Error>> {
        let _ = self.sender.send(BridgeSendMessage::Cancel);
        self.cancel.store(true, Ordering::Relaxed);
        Poll::Ready(Ok(()))
    }
}

impl Drop for Bridge {
    fn drop(&mut self) {
        let _ = self.sender.send(BridgeSendMessage::Cancel);
        self.cancel.store(true, Ordering::Relaxed);
    }
}

pub struct Instrument {

}

impl Instrument {

}


pub async fn list_ftdi() -> crate::Result<Vec<String>> {
    todo!() 
}