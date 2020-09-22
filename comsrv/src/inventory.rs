use std::collections::HashMap;
use std::sync::Arc;

use futures::channel::oneshot;
use futures::future::Shared;
use futures::FutureExt;
use tokio::sync::mpsc;
use tokio::sync::Mutex;
use tokio::task;

use crate::instrument::{Instrument, InstrumentOptions, HandleId, Address};
use crate::{Result, Error};
use crate::prologix::PrologixPort;
use crate::iotask::IoTask;

#[derive(Clone)]
pub struct Connecting {
    inner: Shared<oneshot::Receiver<Option<Error>>>,
}

struct InventoryShared {
    instruments: HashMap<HandleId, Instrument>,
}

#[derive(Clone)]
pub struct Inventory(Arc<Mutex<InventoryShared>>);

impl Inventory {
    pub fn new() -> Self {
        let (tx, rx) = mpsc::unbounded_channel();
        let inner = InventoryShared {
            instruments: Default::default(),
        };
        let inner = Arc::new(Mutex::new(inner));
        let ret = Self(inner);
        ret
    }

    pub fn connect(&self, addr: &Address) -> Instrument {
        let mut inner = self.0.lock().await;
        if let Some(ret) = inner.instruments.get(&addr.handle_id()) {
            return ret.clone();
        }
        let new_instr = Instrument::connect(addr);
        inner.instruments.insert(addr.handle_id(), new_instr.clone());
        new_instr
    }

    pub fn disconnect(&self, addr: &Address) {
        log::debug!("Dropping instrument: {}", addr);
        let mut inner = self.0.lock().await;
        if let Some(x) = inner.instruments.remove(&addr.handle_id()) {
            x.disconnect();
        }
    }
}
