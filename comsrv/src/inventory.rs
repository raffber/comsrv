use tokio::sync::mpsc;
use std::collections::{HashMap, HashSet};
use tokio::sync::RwLock;
use std::sync::Arc;
use tokio::task;
use crate::{Result, Error};
use crate::visa::asynced as async_visa;
use serde::{Serialize, Deserialize};


enum InventoryMsg {
    Disconnected(String),
}

#[derive(Clone)]
pub enum Instrument {
    Visa(async_visa::Instrument),
}

#[derive(Clone, Serialize, Deserialize)]
pub struct ConnectOptions {

}

struct InventoryShared {
    connecting: HashSet<String>,
    instruments: HashMap<String, Instrument>,
    tx: mpsc::UnboundedSender<InventoryMsg>,
}

struct Inventory(Arc<RwLock<InventoryShared>>);

impl Inventory {
    pub fn new() -> Self {
        let (tx,rx) = mpsc::unbounded_channel();
        let inner = InventoryShared {
            connecting: Default::default(),
            instruments: Default::default(),
            tx
        };
        let inner =  Arc::new(RwLock::new(inner));
        InventoryMonitor::start(inner.clone(), rx);
        Self(inner)
    }

    pub async fn connect(&mut self, addr: String, options: Option<ConnectOptions>) -> Result<Instrument> {
        {
            let mut inner = self.0.write().await;
            if let Some(ret) = inner.instruments.get(&addr) {
                return Ok(ret.clone());
            }
            if inner.connecting.contains(&addr) {
                return Err(Error::AlreadyConnecting);
            }
            inner.connecting.insert(addr.clone());
        }
        let instr = async_visa::Instrument::connect(addr.clone(), options).await;
        {
            let mut inner = self.0.write().await;
            inner.connecting.remove(&addr);
            let instr = instr?;
            inner.instruments.insert(addr.clone(), Instrument::Visa(instr));
            Ok(inner.instruments.get(&addr).unwrap().clone())
        }
    }

    pub async fn get<T: AsRef<str>>(&self, addr: T) -> Option<Instrument> {
        let read = self.0.read().await;
        read.instruments.get(addr.as_ref()).map(|x| x.clone())
    }

    pub async fn close<T: AsRef<str>>(&mut self, addr: T) {
        self.0.write().await.instruments.remove(addr.as_ref());
    }

    pub async fn list(&self) -> Vec<String> {
        self.0.read().await.instruments.keys().map(|x| x.clone()).collect()
    }
}

struct InventoryMonitor {
    // inventory: Arc<RwLock<InventoryShared>>,
    rx: mpsc::UnboundedReceiver<InventoryMsg>,
}

impl InventoryMonitor {
    fn start(_inventory: Arc<RwLock<InventoryShared>>, rx: mpsc::UnboundedReceiver<InventoryMsg>) {
        let mut monitor = InventoryMonitor {
            // inventory,
            rx
        };
        task::spawn(async move {
            while let Some(_msg) = monitor.rx.recv().await {
            //     monitor.handle_msg(msg);
            }
        });
    }

    async fn handle_msg(&mut self, msg: InventoryMsg) {
        match msg {
            InventoryMsg::Disconnected(_x) => {
                // let mut write = self.inventory.write().await;
                // write.instruments.remove(&x);
            },
        }
    }
}

