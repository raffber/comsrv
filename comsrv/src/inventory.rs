use std::collections::HashMap;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;
use tokio::sync::Mutex;
use tokio::task;
use either::Either;

use crate::Result;
use crate::visa::{asynced as async_visa, VisaOptions};
use crate::app::{Request, Response};

enum InventoryMsg {
    Disconnected(String),
}

#[derive(Clone)]
pub enum Instrument {
    Visa(async_visa::Instrument),
}

pub enum LockOrInstrument {
    Instrument(Instrument),
    Lock(Arc<Mutex<Result<Instrument>>>),
}

impl Instrument {
    pub fn handle(&self, _req: Request) -> Result<Response> {
        Err(crate::Error::NotSupported) // TODO: ...
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub enum InstrumentOptions {
    Visa(VisaOptions),
    Default,
}

struct InventoryShared {
    instruments: HashMap<String, LockOrInstrument>,
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

    pub async fn connect(&mut self, addr: String, options: InstrumentOptions) -> Result<Instrument> {
        let new_lock = Arc::new(Mutex::new(Err(crate::Error::NotSupported)));
        let old_or_new = {
            let mut inner = self.0.lock().await;
            if let Some(ret) = inner.instruments.get(&addr) {
                match ret {
                    LockOrInstrument::Instrument(instr) => {
                        return Ok(instr.clone());
                    },
                    LockOrInstrument::Lock(lock) => {
                        Either::Left(lock.clone())
                    },
                }
            } else {
                // place a lock into the hashmap for other threads to wait for
                inner.instruments.insert(addr.clone(), LockOrInstrument::Lock(new_lock.clone()));
                Either::Right(new_lock.lock().await)
            }
        };

        let mut guard = match old_or_new {
            Either::Left(old_lock) => {
                // wait for the connection to be there
                return old_lock.lock().await.clone();
            },
            // or proceed with the guard
            Either::Right(guard) => guard,
        };

        // perform the actual connection...
        let visa_options = match options {
            InstrumentOptions::Visa(visa) => visa,
            InstrumentOptions::Default => VisaOptions::default(),
        };
        let instr = async_visa::Instrument::connect(addr.clone(), visa_options)
            .await.map(Instrument::Visa);
        *guard = instr.clone();

        let instr = instr?;

        {
            let mut inner = self.0.lock().await;
            inner.instruments.insert(addr.clone(), LockOrInstrument::Instrument(instr.clone()));
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

