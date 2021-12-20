use std::io;
use std::pin::Pin;
use std::task::{Context, Poll};
use tokio::io::{AsyncRead, AsyncReadExt, ReadBuf};
use tokio::sync::{mpsc, oneshot};

pub struct ByteBuffer {
    rx: mpsc::Receiver<u8>,
    cancel: Option<oneshot::Sender<()>>,
    error: oneshot::Receiver<io::Error>,
}

struct Fetcher<T: AsyncRead + Unpin + Send> {
    stream: T,
    tx: mpsc::Sender<u8>,
    cancel: Option<oneshot::Receiver<()>>,
    error: oneshot::Sender<io::Error>,
}

impl ByteBuffer {
    pub fn new<T: Send + AsyncRead + Unpin + 'static>(inner: T) -> Self {
        let (tx, rx) = mpsc::channel(10_000_000);
        let (cancel_tx, cancel_rx) = oneshot::channel();
        let (error_tx, error_rx) = oneshot::channel();
        let fetcher = Fetcher {
            stream: inner,
            tx,
            cancel: Some(cancel_rx),
            error: error_tx,
        };
        tokio::spawn(async move {
            fetcher.run().await
        });
        Self {
            rx,
            cancel: Some(cancel_tx),
            error: error_rx,
        }
    }
}

impl<T: AsyncRead + Unpin + Send> Fetcher<T> {
    pub async fn run(mut self) {
        let cancel = self.cancel.take().unwrap();
        tokio::select! {
            _ = self.receive() => {

            }
            _ = cancel => {

            }
        }
    }

    async fn receive(mut self) {
        loop {
            let x = self.stream.read_u8().await;
            match x {
                Ok(x) => {
                    // buffer may be full, in which case we just keep blocking
                    if self.tx.send(x).await.is_err() {
                        break;
                    }
                }
                Err(err) => {
                    // attempt to publish error and quit
                    let _ = self.error.send(err);
                    // this will drop self.stream, which will cause the error to be read
                    break;
                }
            }
        }
    }
}

impl Drop for ByteBuffer {
    fn drop(&mut self) {
        let _  = self.cancel.take().unwrap().send(());
    }
}

impl AsyncRead for ByteBuffer {
    fn poll_read(mut self: Pin<&mut Self>, cx: &mut Context<'_>, buf: &mut ReadBuf<'_>) -> Poll<io::Result<()>> {
        loop {
            match self.rx.poll_recv(cx) {
                Poll::Ready(Some(x)) => {
                    buf.put_slice(&[x]);
                }
                Poll::Ready(None) => {
                    return match self.error.try_recv() {
                        Ok(x) => {
                            Poll::Ready(Err(x))
                        }
                        Err(_) => {
                            Poll::Ready(Ok(()))
                        }
                    };
                }
                Poll::Pending => return Poll::Pending,
            }
        }
    }
}
