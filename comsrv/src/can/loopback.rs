use crate::can::CanMessage;
use anyhow::anyhow;
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

    pub async fn recv(&mut self) -> crate::Result<CanMessage> {
        self.rx
            .recv()
            .await
            .map_err(|_| crate::Error::protocol(anyhow!("Loopback closed.")))
    }

    pub fn send(&self, msg: CanMessage) {
        LOOPBACK_ADAPTER.send(msg)
    }
}
