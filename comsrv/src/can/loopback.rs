use lazy_static;
use crate::can::CanMessage;
use std::collections::VecDeque;
use std::sync::Mutex;
use tokio::sync::oneshot;

const MAX_SIZE: usize = 1000;

struct AdapterShared {
    buf: VecDeque<CanMessage>,
    next: Option<oneshot::Sender<CanMessage>>
}

struct LoopbackAdapter {
    state: Mutex<AdapterShared>
}

impl LoopbackAdapter {
    fn new() -> Self {
        Self {
            state: Mutex::new(AdapterShared { buf: Default::default(), next: None }),
        }
    }

    fn send(&self, msg: CanMessage) {
        let mut state = self.state.lock().unwrap();
        while state.buf.len() >= MAX_SIZE {
            state.buf.pop_front();
        }
        if let Some(tx) = state.next.take() {
            let _ = tx.send(msg);
        } else {
            state.buf.push_back(msg);
        }
    }

    async fn recv(&self) -> crate::Result<CanMessage> {
        let rx = {
            let mut state = self.state.lock().unwrap();
            if let Some(x) = state.buf.pop_front() {
                return Ok(x);
            }
            let (tx,rx) = oneshot::channel();
            state.next.replace(tx);
            rx
        };
        rx.await.map_err(|_| crate::Error::Disconnected)
    }
}

lazy_static! {
    static ref LOOPBACK_ADAPTER: LoopbackAdapter = LoopbackAdapter::new();
}

pub struct LoopbackDevice;

impl LoopbackDevice {
    pub fn new() -> Self {
        Self
    }

    async fn recv(&self) -> crate::Result<CanMessage> {
        LOOPBACK_ADAPTER.recv().await
    }

    fn send(&self, msg: CanMessage) {
        LOOPBACK_ADAPTER.send(msg)
    }
}


