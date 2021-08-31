/// This module implements the `Inventory` type, which allows storing and retrieving instruments.
/// Also, access to instruments may be locked for a given amount of time. During this time, only
/// the task accesssing the instrument has access to the instrument.
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::Mutex;
use std::time::Duration;

use tokio::sync::mpsc;
use tokio::sync::oneshot;
use tokio::sync::Mutex as AsyncMutex;
use tokio::time::sleep;
use uuid::Uuid;

use crate::address::{Address, HandleId};
use crate::app::Server;
use crate::instrument::Instrument;

/// Used to lock/unlock an instrument. Allows waiting
/// for the lock because internally an AsyncMutex is used.
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

/// Contains an instrument which can be locked
#[derive(Clone)]
struct LockableInstrument {
    instr: Instrument,
    lock: Option<Lock>,
}

struct InventoryShared {
    instruments: HashMap<HandleId, LockableInstrument>,
    locks: HashMap<Uuid, HandleId>,
}

/// A collect of instruments, public API of this module
#[derive(Clone)]
pub struct Inventory(Arc<Mutex<InventoryShared>>);

/// The `Inventory` type allows storing and retrieving instruments.
/// Also, access to instruments may be locked for a given amount of time. During this time, only
/// the task accesssing the instrument has access to the instrument.
///
/// `Inventory` as well as `Instrument` are `Clone + Send` and can thus be shared between threads.
impl Inventory {
    /// Create a new inventory
    pub fn new() -> Self {
        let inner = InventoryShared {
            instruments: Default::default(),
            locks: Default::default(),
        };
        let inner = Arc::new(Mutex::new(inner));
        Self(inner)
    }

    /// Connect a new instrument. This function creates a new instrument and registers it
    /// in the `Inventory`. However, it does not perform any io, and thus cannot fail.
    ///
    /// # Panics
    ///
    /// This function panics if the there is no `Instrument` associated with the given address type.
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

    /// If there is instrument connected to the given address, this instrument is disconnected and
    /// dropped from the `Inventory`.
    pub fn disconnect(&self, addr: &Address) {
        log::debug!("Dropping instrument: {}", addr);
        let mut inner = self.0.lock().unwrap();
        if let Some(x) = inner.instruments.remove(&addr.handle_id()) {
            x.instr.disconnect();
        }
    }

    /// Drops all instruments
    pub fn disconnect_all(&self) {
        log::debug!("Dropping all instruments");
        let mut inner = self.0.lock().unwrap();
        inner.instruments.clear();
    }

    /// Return a list of keys of instruments.
    pub fn list(&self) -> Vec<String> {
        let inner = self.0.lock().unwrap();
        inner.instruments.keys().map(|x| x.to_string()).collect()
    }

    /// Wait for the lock on a given instrument. If a `lock_id` is provided and matches the
    /// lock which is currently held, access to the `Instrument` is granted.
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
        log::debug!("Waiting for lock...");
        mutex.lock().await;
        log::debug!("Lock acquired, proceeding.");
    }

    /// Lock the given instrument for a given duration and returns an ID representing the
    /// newly created lock. If a lock is still present on the address, the lock is removed and
    /// unlocked.
    /// If this behavior is undesirable, call wait_for_lock() before calling this function.
    pub async fn lock(&self, server: &Server, addr: &Address, timeout: Duration) -> Uuid {
        let ret = Uuid::new_v4();

        let (lock, mut unlock) = {
            let mut inner = self.0.lock().unwrap();
            let (lock, unlock) = Lock::new(ret);
            match inner.instruments.get_mut(&addr.handle_id()) {
                Some(mut instr) => {
                    if let Some(old_lock) = instr.lock.take() {
                        tokio::task::spawn(async move {
                            log::debug!("Unlocking: {}", old_lock.id);
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
            inner.locks.insert(ret, handle_id);
            (lock, unlock)
        };

        let (tx, rx) = oneshot::channel();

        let lock_id = ret;
        let inv = self.clone();
        tokio::task::spawn(async move {
            log::debug!("Locking: {}", lock_id);
            let locked = lock.mutex.lock().await;
            let _ = tx.send(());

            tokio::select! {
                _ = sleep(timeout) => {},
                _ = unlock.recv() => {},
            }

            drop(locked);
            drop(lock);
            inv.unlock(lock_id).await;
        });
        let _ = rx.await;
        ret
    }

    /// Unlock an instrument.
    pub async fn unlock(&self, id: Uuid) {
        let lock = {
            let mut inner = self.0.lock().unwrap();
            inner
                .locks
                .remove(&id)
                .and_then(|lock| inner.instruments.get_mut(&lock))
                .and_then(|x| x.lock.take())
        };
        if let Some(lock) = lock {
            log::debug!("Unlocking: {}", id);
            lock.release().await;
        }
    }
}

impl Default for Inventory {
    fn default() -> Self {
        Self::new()
    }
}
