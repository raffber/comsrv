use std::io;
use std::io::{Error, ErrorKind};
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll};
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};

/// Implements a wrapper on top of `AsyncRead + AsyncWrite` that implements `Clone`.
/// It uses `Arc<Mutex<..>>` internally. The main purpose of the struct is to pass a stream
/// into it, perform some IO operation with it that require `Clone`
/// and extract it back out using the `take()` function.
///
/// After calling `take()` all pending futures will fail.
#[derive(Debug)]
pub struct ClonableChannel<T: AsyncRead + AsyncWrite + Unpin> {
    inner: Arc<Mutex<Option<T>>>,
}

impl<T: AsyncRead + AsyncWrite + Unpin> Clone for ClonableChannel<T> {
    fn clone(&self) -> Self {
        ClonableChannel {
            inner: self.inner.clone(),
        }
    }
}

impl<T: AsyncRead + AsyncWrite + Unpin> ClonableChannel<T> {
    /// Create a new `ClonableChannel` wrapping the given stream.
    pub fn new(stream: T) -> Self {
        Self {
            inner: Arc::new(Mutex::new(Some(stream))),
        }
    }

    /// Extract the underlying stream. After calling this function all pending futures
    /// will fail, since they don't have access to the underlying stream anymore.
    pub fn take(self) -> Option<T> {
        let mut locked = self.inner.lock().unwrap();
        locked.take()
    }
}

impl<T: AsyncRead + AsyncWrite + Unpin> AsyncRead for ClonableChannel<T> {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        let inner = &mut self.inner.lock().unwrap();
        if inner.is_none() {
            return Poll::Ready(Err(Error::new(ErrorKind::Other, "Channel closed.")));
        }
        let inner = inner.as_mut().unwrap();
        Pin::new(inner).poll_read(cx, buf)
    }
}

impl<T: AsyncRead + AsyncWrite + Unpin> AsyncWrite for ClonableChannel<T> {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        let inner = &mut self.inner.lock().unwrap();
        if inner.is_none() {
            return Poll::Ready(Err(Error::new(ErrorKind::Other, "Channel closed.")));
        }
        let inner = inner.as_mut().unwrap();
        Pin::new(inner).poll_write(cx, buf)
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        let inner = &mut self.inner.lock().unwrap();
        if inner.is_none() {
            return Poll::Ready(Err(Error::new(ErrorKind::Other, "Channel closed.")));
        }
        let inner = inner.as_mut().unwrap();
        Pin::new(inner).poll_flush(cx)
    }

    fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        let inner = &mut self.inner.lock().unwrap();
        if inner.is_none() {
            return Poll::Ready(Err(Error::new(ErrorKind::Other, "Channel closed.")));
        }
        let inner = inner.as_mut().unwrap();
        Pin::new(inner).poll_shutdown(cx)
    }
}
