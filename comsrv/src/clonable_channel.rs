use std::io;
use std::io::{Error, ErrorKind};
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll};
use tokio::io::{AsyncRead, AsyncWrite};

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
    pub fn new(stream: T) -> Self {
        Self {
            inner: Arc::new(Mutex::new(Some(stream))),
        }
    }
}

impl<T: AsyncRead + AsyncWrite + Unpin> ClonableChannel<T> {
    pub fn take(self) -> Option<T> {
        let mut locked = self.inner.lock().unwrap();
        locked.take()
    }
}

impl<T: AsyncRead + AsyncWrite + Unpin> AsyncRead for ClonableChannel<T> {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut [u8],
    ) -> Poll<io::Result<usize>> {
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
