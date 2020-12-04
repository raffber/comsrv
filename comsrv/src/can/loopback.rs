use crate::can::{CanError, CanMessage};
use lazy_static;
use tokio::sync::broadcast;

const MAX_SIZE: usize = 1000;

struct LoopbackAdapter {
    tx: broadcast::Sender<CanMessage>,
}

impl LoopbackAdapter {
    fn new() -> Self {
        let (tx, _) = broadcast::channel(MAX_SIZE);
        Self { tx }
    }

    fn send(&self, msg: CanMessage) {
        let tx = self.tx.clone();
        let _ = tx.send(msg);
    }
}

lazy_static! {
    static ref LOOPBACK_ADAPTER: LoopbackAdapter = LoopbackAdapter::new();
}

pub struct LoopbackDevice {
    rx: broadcast::Receiver<CanMessage>,
}

impl LoopbackDevice {
    pub fn new() -> Self {
        let rx = LOOPBACK_ADAPTER.tx.subscribe();
        Self { rx }
    }

    pub async fn recv(&mut self) -> Result<CanMessage, CanError> {
        self.rx
            .recv()
            .await
            .map_err(|_| CanError::BusError(async_can::BusError::Off))
    }

    pub fn send(&self, msg: CanMessage) {
        LOOPBACK_ADAPTER.send(msg)
    }
}

