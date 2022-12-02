use comsrv_protocol::cobs_stream::CobsStreamResponse;
use std::io;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::sync::{mpsc, oneshot};
use tokio::{select, task};

use crate::app::Server;
use crate::protocol::bytestream::cobs::cobs_decode;
use comsrv_protocol::{ByteStreamInstrument, Response};

use super::bytestream::cobs::cobs_encode;

#[derive(Clone)]
pub struct CobsStream {
    cancel: Arc<Mutex<Option<oneshot::Sender<()>>>>,
    tx: mpsc::UnboundedSender<Vec<u8>>,
    use_crc: bool,
}

impl CobsStream {
    pub fn start<Read: AsyncRead + Send + 'static, Write: AsyncWrite + Send + 'static>(
        read: Read,
        write: Write,
        server: Server,
        instr: ByteStreamInstrument,
        use_crc: bool,
    ) -> CobsStream {
        let (cancel_tx, cancel_rx) = oneshot::channel();
        let (transmit_tx, transmit_rx) = mpsc::unbounded_channel();

        let read = Box::pin(read);
        let write = Box::pin(write);

        let fut = async move {
            let mut decoder = CobsDecoder::new(server.clone(), instr);
            let encoder = CobsEncoder::new();
            let err = select! {
                    err = decoder.decode_stream(read) => Some(err),
                    err = encoder.transmit_frames(write, transmit_rx) => err,
                    _ = cancel_rx => None
            };
            server.broadcast(Response::CobsStream(CobsStreamResponse::InstrumentDropped {
                error: err.map(crate::Error::transport),
            }));
        };
        task::spawn(fut);
        CobsStream {
            cancel: Arc::new(Mutex::new(Some(cancel_tx))),
            use_crc,
            tx: transmit_tx,
        }
    }

    pub fn send<D: Into<Vec<u8>>>(&self, data: D) -> crate::Result<()> {
        self.tx.send(data.into()).map_err(crate::Error::internal)
    }

    pub fn cancel(self) {
        let mut lock = self.cancel.lock().unwrap();
        let channel = lock.take();
        if let Some(channel) = channel {
            let _ = channel.send(());
        }
    }

    pub fn use_crc(&self) -> bool {
        self.use_crc
    }

    pub fn is_alive(&self) -> bool {
        let lock = self.cancel.lock().unwrap();
        if let Some(channel) = lock.as_ref() {
            return !channel.is_closed();
        }
        false
    }
}

impl Drop for CobsStream {
    fn drop(&mut self) {
        let mut lock = self.cancel.lock().unwrap();
        let channel = lock.take();
        if let Some(channel) = channel {
            let _ = channel.send(());
        }
    }
}

struct CobsDecoder {
    buf: Vec<u8>,
    server: Server,
    instr: ByteStreamInstrument,
}

impl CobsDecoder {
    fn new(server: Server, instr: ByteStreamInstrument) -> Self {
        Self {
            buf: Vec::new(),
            server,
            instr,
        }
    }

    fn push(&mut self, value: u8) {
        self.buf.push(value);
        if value == 0 {
            let decoded = cobs_decode(&self.buf);
            // TODO: crc
            if let Some(decoded) = decoded {
                self.buf.clear();
                log::info!("COBS frame received (length = {})", decoded.len());
                self.server.broadcast(Response::CobsStream(CobsStreamResponse::MessageReceived {
                    sender: self.instr.clone(),
                    data: decoded,
                }));
            }
        }
    }

    async fn decode_stream<T: AsyncRead + Send + 'static>(&mut self, mut stream: Pin<Box<T>>) -> io::Error {
        loop {
            let byte = stream.read_u8().await;
            match byte {
                Ok(x) => {
                    self.push(x);
                }
                Err(err) => return err,
            }
        }
    }
}

struct CobsEncoder {}

impl CobsEncoder {
    fn new() -> Self {
        Self {}
    }

    async fn transmit_frames<T: AsyncWrite + Send + 'static>(
        &self,
        mut stream: Pin<Box<T>>,
        mut frames_to_transmit: mpsc::UnboundedReceiver<Vec<u8>>,
    ) -> Option<io::Error> {
        while let Some(tx) = frames_to_transmit.recv().await {
            let encoded = cobs_encode(&tx);
            // TODO: crc
            if let Err(err) = stream.write(&encoded).await {
                return Some(err);
            }
        }
        None
    }
}
