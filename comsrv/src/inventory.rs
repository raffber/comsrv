use futures::channel::mpsc;
use std::collections::HashMap;
use crate::highlevel::{Result, Instrument};

struct InstrumentThread {
    tx: mpsc::UnboundedSender<ThreadMsg>,
    rx: mpsc::UnboundedReceiver<ThreadMsg>,
}

impl InstrumentThread {
    fn spawn(instr: Instrument) -> InstrumentHandle {
        todo!()
    }
}

struct InstrumentHandle {
    tx: mpsc::UnboundedSender<ThreadMsg>,
    rx: mpsc::UnboundedReceiver<ThreadMsg>,
}

enum ThreadMsg {}

impl InstrumentHandle {
    async fn send(&self, msg: ThreadMsg) {}

    pub async fn write<T: AsRef<str>>(&self, msg: T) -> Result<()> {
        todo!()
    }

    pub async fn query<T: AsRef<str>>(&self, msg: T) -> Result<String> {
        todo!()
    }

    pub async fn set_timeout(&self, timeout: f32) -> Result<()> {
        todo!()
    }

    pub async fn get_timeout(&self) -> Result<f32> {
        todo!()
    }

    pub async fn query_binary_from_string<T: AsRef<str>>(&self, msg: T) {
        todo!()
    }
}

struct Inventory {
    instruments: HashMap<String, InstrumentHandle>,
}

impl Inventory {
    pub fn add(&mut self, instr: Instrument) -> &InstrumentHandle {
        let addr = instr.addr().to_string();
        if !self.instruments.contains_key(&addr) {
            let handle = InstrumentThread::spawn(instr);
            self.instruments.insert(addr.clone(), handle);
        }
        self.instruments.get(&addr).unwrap()
    }

    pub fn get<T: AsRef<str>>(&self, addr: T) -> Option<&InstrumentHandle> {
        self.instruments.get(addr.as_ref())
    }

    pub fn close<T: AsRef<str>>(&mut self, addr: T) {
        self.instruments.remove(addr.as_ref());
    }

    pub fn list(&self) -> Vec<String> {
        self.instruments.keys().map(|x| x.clone()).collect()
    }
}
