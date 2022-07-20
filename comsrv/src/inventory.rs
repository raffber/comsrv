/// This module implements the `Inventory` type, which allows storing and retrieving instruments.
/// Also, access to instruments may be locked for a given amount of time. During this time, only
/// the task accesssing the instrument has access to the instrument.
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::Mutex;
use std::time::Duration;

use std::fmt::Debug;
use std::hash::Hash;
use tokio::sync::mpsc;
use tokio::sync::oneshot;
use tokio::sync::Mutex as AsyncMutex;
use tokio::time::sleep;
use uuid::Uuid;

use crate::app::Server;

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

    async fn release(self) {
        let _ = self.unlock.send(()).await;
    }
}
pub trait InstrumentAddress: 'static + Clone + Send + Hash + PartialEq + Eq + Debug {}
impl<T: 'static + Clone + Send + Hash + PartialEq + Eq + Debug> InstrumentAddress for T {}

pub trait Instrument: 'static + Clone + Send {
    type Address: InstrumentAddress;

    fn connect(server: &Server, addr: &Self::Address) -> crate::Result<Self>;
}

/// Contains an instrument which can be locked
#[derive(Clone)]
struct LockableInstrument<T: Instrument> {
    instr: T,
    lock: Option<Lock>,
}

struct InventoryShared<T: Instrument> {
    instruments: HashMap<T::Address, LockableInstrument<T>>,
    locks: HashMap<Uuid, T::Address>,
}

/// A collect of instruments, public API of this module
#[derive(Clone)]
pub struct Inventory<T: Instrument>(Arc<Mutex<InventoryShared<T>>>);

/// The `Inventory` type allows storing and retrieving instruments.
/// Also, access to instruments may be locked for a given amount of time. During this time, only
/// the task accesssing the instrument has access to the instrument.
///
/// `Inventory` as well as `Instrument` are `Clone + Send` and can thus be shared between threads.
impl<T: Instrument> Inventory<T> {
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
    /// in the `Inventory`. However, it does not perform any io, however it may fail
    /// due to invalid arguments.
    pub fn connect(&self, server: &Server, addr: &T::Address) -> crate::Result<T> {
        log::debug!("Opening instrument: {:?}", addr);
        let mut inner = self.0.lock().unwrap();

        if let Some(ret) = inner.instruments.get(addr) {
            return Ok(ret.instr.clone());
        }
        let ret = T::connect(server, addr)?;

        let instr = LockableInstrument {
            instr: ret.clone(),
            lock: None,
        };
        inner.instruments.insert(addr.clone(), instr);
        Ok(ret)
    }

    pub async fn wait_connect(&self, server: &Server, addr: &T::Address, lock_id: Option<&Uuid>) -> crate::Result<T> {
        self.wait_for_lock(addr, lock_id).await;
        self.connect(server, addr)
    }

    /// If there is instrument connected to the given address, this instrument is disconnected and
    /// dropped from the `Inventory`.
    pub fn disconnect(&self, addr: &T::Address) {
        log::debug!("Dropping instrument: {:?}", addr);
        let mut inner = self.0.lock().unwrap();
        inner.instruments.remove(addr);
    }

    /// Drops all instruments
    pub fn disconnect_all(&self) {
        log::debug!("Dropping all instruments");
        let mut inner = self.0.lock().unwrap();
        inner.instruments.clear();
    }

    /// Return a list of keys of instruments.
    pub fn list(&self) -> Vec<T::Address> {
        let inner = self.0.lock().unwrap();
        inner.instruments.keys().cloned().collect()
    }

    /// Wait for the lock on a given instrument. If a `lock_id` is provided and matches the
    /// lock which is currently held, access to the `Instrument` is granted.
    pub async fn wait_for_lock(&self, addr: &T::Address, lock_id: Option<&Uuid>) {
        let mutex = {
            let inner = self.0.lock().unwrap();
            match inner.instruments.get(&addr) {
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

    pub async fn wait_and_lock(&self, server: &Server, addr: &T::Address, timeout: Duration) -> crate::Result<Uuid> {
        self.wait_for_lock(addr, None).await;
        self.lock(server, addr, timeout).await
    }

    /// Lock the given instrument for a given duration and returns an ID representing the
    /// newly created lock. If a lock is still present on the address, the lock is removed and
    /// unlocked.
    /// If this behavior is undesirable, call wait_for_lock() before calling this function.
    pub async fn lock(&self, server: &Server, addr: &T::Address, timeout: Duration) -> crate::Result<Uuid> {
        let ret = Uuid::new_v4();

        let (lock, mut unlock) = {
            let mut inner = self.0.lock().unwrap();
            let (lock, unlock) = Lock::new(ret);
            match inner.instruments.get_mut(&addr) {
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
                    let instr = Instrument::connect(&server, addr)?;
                    let instr = LockableInstrument {
                        instr,
                        lock: Some(lock.clone()),
                    };
                    inner.instruments.insert(addr.clone(), instr);
                }
            };
            inner.locks.insert(ret, addr.clone());
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
        Ok(ret)
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

impl<T: Instrument> Default for Inventory<T> {
    fn default() -> Self {
        Self::new()
    }
}
