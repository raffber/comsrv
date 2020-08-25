use std::collections::HashMap;
use std::sync::Arc;

use tokio::sync::mpsc;
use tokio::sync::Mutex;
use tokio::task;

use crate::Result;
use crate::visa::{asynced as async_visa, VisaOptions};
use crate::app::InstrumentOptions;
use futures::future::Shared;
use futures::FutureExt;
use futures::channel::oneshot;

enum InventoryMsg {
    Disconnected(String),
}

#[derive(Clone)]
pub enum Instrument {
    Visa(async_visa::Instrument),
}

pub enum ConnectingInstrument {
    Instrument(Instrument),
    Future(Shared<oneshot::Receiver<Arc<Mutex<Result<Instrument>>>>>),
}

struct InventoryShared {
    instruments: HashMap<String, ConnectingInstrument>,
    tx: mpsc::UnboundedSender<InventoryMsg>,
}

#[derive(Clone)]
pub struct Inventory(Arc<Mutex<InventoryShared>>);

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

    pub async fn connect(&self, addr: String, options: InstrumentOptions) -> Result<Instrument> {
        let (tx, rx) = oneshot::channel();
        let rx = rx.shared();
        let rx = {
            let mut inner = self.0.lock().await;
            if let Some(ret) = inner.instruments.get(&addr) {
                match ret {
                    ConnectingInstrument::Instrument(instr) => {
                        return Ok(instr.clone());
                    },
                    ConnectingInstrument::Future(fut) => {
                        Some(fut.clone())
                    },
                }
            } else {
                // place a lock into the hashmap for other threads to wait for
                inner.instruments.insert(addr.clone(), ConnectingInstrument::Future(rx));
                None
            }
        };

        if let Some(rx) = rx {
            return match rx.await {
                Ok(res) => res.lock().await.clone(),
                Err(_) => Err(crate::Error::CannotConnect),
            };
        }
        self.do_connect(tx, addr, options).await
    }

    async fn do_connect(&self, tx: oneshot::Sender<Arc<Mutex<Result<Instrument>>>>, addr: String, options: InstrumentOptions) -> Result<Instrument> {
        // perform the actual connection...
        let visa_options = match options {
            InstrumentOptions::Visa(visa) => visa,
            InstrumentOptions::Default => VisaOptions::default(),
        };
        let instr = async_visa::Instrument::connect(addr.clone(), visa_options)
            .await.map(Instrument::Visa);
        let _ = tx.send(Arc::new(Mutex::new(instr.clone())));

        let instr = instr?;

        {
            let mut inner = self.0.lock().await;
            inner.instruments.insert(addr.clone(), ConnectingInstrument::Instrument(instr.clone()));
            Ok(instr)
        }
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

