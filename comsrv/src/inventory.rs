use std::collections::HashMap;
use std::sync::Arc;

use futures::channel::oneshot;
use futures::future::Shared;
use futures::FutureExt;
use tokio::sync::mpsc;
use tokio::sync::Mutex;
use tokio::task;

use crate::instrument::{Instrument, InstrumentOptions};
use crate::Result;
use crate::prologix::PrologixPort;

enum InventoryMsg {
    Disconnected(String),
}

#[derive(Clone)]
pub enum ConnectingInstrument {
    Instrument(Instrument),
    Future(Shared<oneshot::Receiver<Arc<Mutex<Result<Instrument>>>>>),
}

struct InventoryShared {
    instruments: HashMap<String, ConnectingInstrument>,
    tx: mpsc::UnboundedSender<InventoryMsg>,
    prologix: HashMap<String, PrologixPort>,
}

#[derive(Clone)]
pub struct Inventory(Arc<Mutex<InventoryShared>>);

impl Inventory {
    pub fn new() -> Self {
        let (tx, rx) = mpsc::unbounded_channel();
        let inner = InventoryShared {
            instruments: Default::default(),
            tx,
            prologix: Default::default()
        };
        let inner = Arc::new(Mutex::new(inner));
        let ret = Self(inner);
        InventoryMonitor::start(ret.clone(), rx);
        ret
    }

    pub async fn connect(&self, addr: &str, options: &InstrumentOptions) -> Result<Instrument> {
        let (tx, rx) = oneshot::channel();
        let rx = rx.shared();
        let rx = {
            let mut inner = self.0.lock().await;
            if let Some(ret) = inner.instruments.get(addr) {
                match ret {
                    ConnectingInstrument::Instrument(instr) => {
                        return Ok(instr.clone());
                    }
                    ConnectingInstrument::Future(fut) => {
                        Some(fut.clone())
                    }
                }
            } else {
                // place a lock into the hashmap for other threads to wait for
                inner.instruments.insert(addr.to_string(), ConnectingInstrument::Future(rx));
                None
            }
        };

        // wait for the instrument to be available
        if let Some(rx) = rx {
            return match rx.await {
                Ok(res) => res.lock().await.clone(),
                Err(_) => Err(crate::Error::CannotConnect),
            };
        }
        self.do_connect(tx, addr, options).await
    }

    async fn do_connect(&self, tx: oneshot::Sender<Arc<Mutex<Result<Instrument>>>>, addr: &str, options: &InstrumentOptions) -> Result<Instrument> {
        let instr = Instrument::connect(addr.to_string(), options).await;
        let _ = tx.send(Arc::new(Mutex::new(instr.clone())));
        let instr = instr?;
        {
            let mut inner = self.0.lock().await;
            inner.instruments.insert(addr.to_string(), ConnectingInstrument::Instrument(instr.clone()));
            Ok(instr)
        }
    }

    pub async fn disconnect(&self, addr: &str) {
        log::debug!("Dropping instrument: {}", addr);
        if let Some(instr) = self.0.lock().await.instruments.remove(addr) {
            match instr {
                ConnectingInstrument::Instrument(x) => {
                    x.disconnect()
                },
                ConnectingInstrument::Future(_) => {},
            }
        }
    }

    pub async fn list(&self) -> Vec<String> {
        self.0.lock().await.instruments.keys().map(|x| x.clone()).collect()
    }

    /// collect all instruments, waiting until they are connected
    pub async fn instruments(&self) -> HashMap<String, Instrument> {
        let mut ret = HashMap::new();
        let instrs = self.0.lock().await.instruments.clone();
        for (addr, instr) in instrs {
            match instr {
                ConnectingInstrument::Instrument(x) => {
                    ret.insert(addr.clone(), x);
                },
                ConnectingInstrument::Future(fut) => {
                    match fut.await {
                        Ok(x) => {
                            let instr = x.lock().await.clone();
                            if let Ok(instr) = instr {
                                ret.insert(addr, instr);
                            }
                        },
                        _ => {},
                    }
                },
            }

        }
        ret
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

