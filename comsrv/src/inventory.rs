use std::collections::HashMap;
use std::sync::Arc;

use futures::channel::oneshot;
use futures::future::Shared;
use std::sync::Mutex;

use crate::instrument::{Instrument, HandleId, Address};
use crate::Error;
use crate::app::Server;

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
        let inner = InventoryShared {
            instruments: Default::default(),
        };
        let inner = Arc::new(Mutex::new(inner));
        let ret = Self(inner);
        ret
    }

    pub fn connect(&self, server: &Server, addr: &Address) -> Instrument {
        let mut inner = self.0.lock().unwrap();
        if let Some(ret) = inner.instruments.get(&addr.handle_id()) {
            return ret.clone();
        }
        let new_instr = Instrument::connect(&server, addr);
        inner.instruments.insert(addr.handle_id(), new_instr.clone());
        new_instr
    }

    pub fn disconnect(&self, addr: &Address) {
        log::debug!("Dropping instrument: {}", addr);
        let mut inner = self.0.lock().unwrap();
        if let Some(x) = inner.instruments.remove(&addr.handle_id()) {
            x.disconnect();
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
}
