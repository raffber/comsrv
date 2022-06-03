use std::collections::VecDeque;
use std::io;
use std::pin::Pin;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::sync::Mutex;
use std::task::Context;
use std::task::Poll;
use std::thread;
use std::time::Duration;

use async_trait::async_trait;
use comsrv_protocol::ByteStreamRequest;
use libftd2xx::FtStatus;
use libftd2xx::FtdiCommon;
use libftd2xx::TimeoutError;
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::sync::mpsc;
use tokio::sync::mpsc::{UnboundedReceiver as AsyncReceiver, UnboundedSender as AsyncSender};

use crate::iotask::IoHandler;
use crate::iotask::IoTask;
use crate::serial::SerialParams;
use crate::tcp::TcpRequest;
use crate::tcp::TcpResponse;
use libftd2xx::Ftdi;

#[derive(Hash, Clone)]
pub struct FtdiAddress {
    pub serial_number: String,
    pub params: SerialParams,
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
    sender_error: Arc<Mutex<Option<io::Error>>>,
}

impl Bridge {
    fn new(address: FtdiAddress) -> Self {
        let cancel = Arc::new(AtomicBool::new(false));
        let (sender_tx, sender_rx) = mpsc::unbounded_channel();
        let (receiver_tx, receiver_rx) = mpsc::unbounded_channel();
        let sender_error = Arc::new(Mutex::new(None));

        thread::spawn({
            let address = address.clone();
            let sender_error = sender_error.clone();
            move || {Self::sender(address, sender_rx, sender_error);}
        });

        thread::spawn({
            let address = address.clone();
            let cancel = cancel.clone();
            move || {Self::receiver(address.clone(), receiver_tx, cancel);}
        });

        Self {
            cancel,
            sender: sender_tx,
            receier: receiver_rx,
            buffer: VecDeque::with_capacity(1024),
            sender_error: sender_error,
        }
    }

    fn status_to_io_error(status: FtStatus) -> io::Error {
        io::Error::new(io::ErrorKind::Other, status.to_string())
    }

    async fn close(&mut self) {
        todo!()
    }

    fn receiver(
        address: FtdiAddress,
        data_tx: AsyncSender<io::Result<Vec<u8>>>,
        cancel: Arc<AtomicBool>,
    ) {
        let mut device = match Ftdi::with_serial_number(&address.serial_number) {
            Ok(device) => device,
            Err(status) => {
                let _ = data_tx.send(Err(Self::status_to_io_error(status)));
                return;
            }
        };
        device.set_timeouts(Duration::from_millis(10), Duration::from_millis(100));
        loop {
            if cancel.load(Ordering::Relaxed) {
                break;
            }
            const BUF_SIZE: usize = 256;
            let mut buf = [0_u8; BUF_SIZE];
            match device.read_all(&mut buf) {
                Ok(_) => {
                    let data = buf.to_vec();
                    if data_tx.send(Ok(data)).is_err() {
                        // remote channel dropped
                        break;
                    }
                }
                Err(TimeoutError::Timeout { actual: bytes_read, .. }) => {
                    let data = buf[0 .. bytes_read].to_vec();
                    if data_tx.send(Ok(data)).is_err() {
                        // remote channel dropped
                        break;
                    }
                }
                Err(TimeoutError::FtStatus(status)) => {
                    let err = Self::status_to_io_error(status);
                    let _ = data_tx.send(Err(err));
                    break;
                }
            }
        }
        device.close();
    }

    fn sender(
        address: FtdiAddress,
        mut rx: AsyncReceiver<BridgeSendMessage>,
        error: Arc<Mutex<Option<io::Error>>>,
    ) {
        let mut device = match Ftdi::with_serial_number(&address.serial_number) {
            Ok(device) => device,
            Err(status) => {
                let mut locked_error = error.lock().unwrap();
                let err = io::Error::new(io::ErrorKind::Other, status.to_string());
                locked_error.replace(err);
                return;
            }
        };
        while let Some(tx_msg) = rx.blocking_recv() {
            match tx_msg {
                BridgeSendMessage::Cancel => break,
                BridgeSendMessage::Data(data) => {
                    if let Err(err) = device.write_all(&data) {
                        let mut locked_error = error.lock().unwrap();
                        let err = io::Error::new(io::ErrorKind::TimedOut, "Write timeout occurred");
                        locked_error.replace(err);
                    }
                }
            }
        }
        device.close();
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
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut tokio::io::ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        if self.push_to_output_buffer(buf) {
            return Poll::Ready(Ok(()));
        }
        loop {
            match self.receier.poll_recv(cx) {
                Poll::Ready(Some(Ok(x))) => {
                    self.buffer.extend(&x);
                    if self.push_to_output_buffer(buf) {
                        return Poll::Ready(Ok(()));
                    }
                }
                Poll::Ready(Some(Err(x))) => {
                    return Poll::Ready(Err(x));
                }
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
        let mut lock = self.sender_error.lock().unwrap();
        if let Some(err) = lock.take() {
            return Poll::Ready(Err(err));
        }
        if self
            .sender
            .send(BridgeSendMessage::Data(buf.to_vec()))
            .is_err()
        {
            return Poll::Ready(Err(io::Error::new(io::ErrorKind::Other, "Disconnected")));
        }
        Poll::Ready(Ok(buf.len()))
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), std::io::Error>> {
        // TODO: send a flush message and send a oneshot
        // then keep polling that oneshot
        Poll::Ready(Ok(()))
    }

    fn poll_shutdown(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Result<(), std::io::Error>> {
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

#[derive(Clone)]
pub struct Instrument {
    inner: IoTask<Handler>,
}

impl Instrument {
    pub fn new(addr: FtdiAddress) -> Self {
        let handler = Handler::new(addr);
        Self {
            inner: IoTask::new(handler),
        }
    }

    pub async fn request(&mut self, req: TcpRequest) -> crate::Result<TcpResponse> {
        self.inner.request(req).await
    }

    pub fn disconnect(mut self) {
        self.inner.disconnect()
    }
}

struct Handler {
    device: Option<Bridge>,
    current_addr: FtdiAddress,
}

impl Handler {
    fn new(addr: FtdiAddress) -> Self {
        Self {
            device: None,
            current_addr: addr,
        }
    }
}

#[async_trait]
impl IoHandler for Handler {
    type Request = TcpRequest;
    type Response = TcpResponse;

    async fn handle(&mut self, req: Self::Request) -> crate::Result<Self::Response> {
        todo!()
    }

    async fn disconnect(&mut self) {
        if let Some(mut bridge) = self.device.take() {
            bridge.close().await;
        }
    }
}

pub async fn list_ftdi() -> crate::Result<Vec<String>> {
    todo!()
}
