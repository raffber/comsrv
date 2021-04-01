use std::collections::HashMap;
use std::sync::Arc;
use std::sync::Mutex;

use crate::app::Server;
use crate::instrument::{Address, HandleId, Instrument};
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::sync::oneshot;
use tokio::sync::Mutex as AsyncMutex;
use tokio::time::delay_for;
use uuid::Uuid;

#[derive(Clone)]
struct Lock {
    mutex: Arc<AsyncMutex<()>>,
    unlock: mpsc::Sender<()>,
    id: Uuid,
}

impl Lock {
    fn new(id: Uuid) -> (Self, mpsc::Receiver<()>) {
        let (tx, rx) = mpsc::channel(1);
        (
            Self {
                mutex: Arc::new(AsyncMutex::new(())),
                unlock: tx,
                id,
            },
            rx,
        )
    }

    async fn release(mut self) {
        let _ = self.unlock.send(()).await;
    }
}

#[derive(Clone)]
struct LockableInstrument {
    instr: Instrument,
    lock: Option<Lock>,
}

struct InventoryShared {
    instruments: HashMap<HandleId, LockableInstrument>,
    locks: HashMap<Uuid, HandleId>,
}

#[derive(Clone)]
pub struct Inventory(Arc<Mutex<InventoryShared>>);

impl Inventory {
    pub fn new() -> Self {
        let inner = InventoryShared {
            instruments: Default::default(),
            locks: Default::default(),
        };
        let inner = Arc::new(Mutex::new(inner));
        let ret = Self(inner);
        ret
    }

    pub fn connect(&self, server: &Server, addr: &Address) -> Instrument {
        let mut inner = self.0.lock().unwrap();
        if let Some(ret) = inner.instruments.get(&addr.handle_id()) {
            return ret.instr.clone();
        }
        let ret = Instrument::connect(&server, addr).unwrap();
        let instr = LockableInstrument {
            instr: ret.clone(),
            lock: None,
        };
        inner.instruments.insert(addr.handle_id(), instr);
        ret
    }

    pub fn disconnect(&self, addr: &Address) {
        log::debug!("Dropping instrument: {}", addr);
        let mut inner = self.0.lock().unwrap();
        if let Some(x) = inner.instruments.remove(&addr.handle_id()) {
            x.instr.disconnect();
        }
    }

    pub fn disconnect_all(&self) {
        log::debug!("Dropping all instruments");
        let mut inner = self.0.lock().unwrap();
        inner.instruments.clear();
    }

    pub fn list(&self) -> Vec<String> {
        let inner = self.0.lock().unwrap();
        inner.instruments.keys().map(|x| x.to_string()).collect()
    }

    pub async fn wait_for_lock(&self, addr: &Address, lock_id: Option<&Uuid>) {
        let mutex = {
            let inner = self.0.lock().unwrap();
            match inner.instruments.get(&addr.handle_id()) {
                Some(LockableInstrument {
                    instr: _,
                    lock: Some(lock),
                }) => {
                    if let Some(id) = lock_id {
                        if lock.id == *id {
                            return;
                        }
                    }
                    lock.mutex.clone()
                }
                _ => return,
            }
        };
        mutex.lock().await;
    }

    pub async fn lock(&self, server: &Server, addr: &Address, timeout: Duration) -> Uuid {
        let ret = Uuid::new_v4();

        let (lock, mut unlock) = {
            let mut inner = self.0.lock().unwrap();
            let (lock, unlock) = Lock::new(ret.clone());
            match inner.instruments.get_mut(&addr.handle_id()) {
                Some(mut instr) => {
                    if let Some(old_lock) = instr.lock.take() {
                        tokio::task::spawn(async move {
                            old_lock.release().await;
                        });
                    }
                    instr.lock = Some(lock.clone());
                }
                None => {
                    let instr = Instrument::connect(&server, addr).unwrap();
                    let instr = LockableInstrument {
                        instr,
                        lock: Some(lock.clone()),
                    };
                    inner.instruments.insert(addr.handle_id(), instr);
                }
            };
            let handle_id = addr.handle_id();
            inner.locks.insert(ret.clone(), handle_id.clone());
            (lock, unlock)
        };

        let (tx, rx) = oneshot::channel();

        let lock_id = ret.clone();
        let inv = self.clone();
        tokio::task::spawn(async move {
            let locked = lock.mutex.lock().await;
            let _ = tx.send(());

            tokio::select! {
                _ = delay_for(timeout) => {},
                _ = unlock.recv() => {},
            }

            drop(locked);
            drop(lock);
            inv.unlock(lock_id).await;
        });
        let _ = rx.await;
        ret
    }

    pub async fn unlock(&self, id: Uuid) {
        let lock = {
            let mut inner = self.0.lock().unwrap();
            if let Some(lock) = inner.locks.remove(&id) {
                if let Some(instr) = inner.instruments.get_mut(&lock) {
                    if let Some(lock) = instr.lock.take() {
                        Some(lock)
                    } else {
                        None
                    }
                } else {
                    None
                }
            } else {
                None
            }
        };
        if let Some(lock) = lock {
            lock.release().await;
        }
    }
}
