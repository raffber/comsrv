use std::collections::HashMap;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;
use tokio::sync::Mutex;
use tokio::task;

use crate::Result;
use crate::visa::asynced as async_visa;
use crate::app::{Request, Response};

enum InventoryMsg {
    Disconnected(String),
}

#[derive(Clone)]
pub enum Instrument {
    Visa(async_visa::Instrument),
}

impl Instrument {
    pub fn handle(&self, _req: Request) -> Result<Response> {
        Err(crate::Error::NotSupported) // TODO: ...
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct ConnectOptions {}

struct InventoryShared {
    instruments: HashMap<String, Instrument>,
    tx: mpsc::UnboundedSender<InventoryMsg>,
}

#[derive(Clone)]
struct Inventory(Arc<Mutex<InventoryShared>>);

impl Inventory {
    pub fn new() -> Self {
        let (tx, rx) = mpsc::unbounded_channel();
        let inner = InventoryShared {
            instruments: Default::default(),
            tx,
        };
        let inner = Arc::new(Mutex::new(inner));
        let ret = Self(inner);
        InventoryMonitor::start(ret.clone(), rx);
        ret
    }

    pub async fn connect(&mut self, addr: String, options: Option<ConnectOptions>) -> Result<Instrument> {
        {
            let inner = self.0.lock().await;
            if let Some(ret) = inner.instruments.get(&addr) {
                return Ok(ret.clone());
            }
        }
        // TODO: make sure not to connect multiple instruments at the same time by directly retrieving
        // the handle first and spawning the connection in another task
        let instr = async_visa::Instrument::connect(addr.clone(), options).await;
        {
            let mut inner = self.0.lock().await;
            let instr = instr?;
            inner.instruments.insert(addr.clone(), Instrument::Visa(instr));
            Ok(inner.instruments.get(&addr).unwrap().clone())
        }
    }

    pub async fn get<T: AsRef<str>>(&self, addr: T) -> Option<Instrument> {
        let read = self.0.lock().await;
        read.instruments.get(addr.as_ref()).map(|x| x.clone())
    }

    pub async fn close<T: AsRef<str>>(&mut self, addr: T) {
        self.0.lock().await.instruments.remove(addr.as_ref());
    }

    pub async fn list(&self) -> Vec<String> {
        self.0.lock().await.instruments.keys().map(|x| x.clone()).collect()
    }
}

struct InventoryMonitor {
    inventory: Inventory,
    rx: mpsc::UnboundedReceiver<InventoryMsg>,
}

impl InventoryMonitor {
    fn start(inventory: Inventory, rx: mpsc::UnboundedReceiver<InventoryMsg>) {
        let mut monitor = InventoryMonitor {
            inventory,
            rx,
        };
        task::spawn(async move {
            while let Some(msg) = monitor.rx.recv().await {
                monitor.handle_msg(msg).await;
            }
        });
    }

    async fn handle_msg(&mut self, msg: InventoryMsg) {
        match msg {
            InventoryMsg::Disconnected(x) => {
                let mut write = self.inventory.0.lock().await;
                write.instruments.remove(&x);
            }
        }
    }
}

