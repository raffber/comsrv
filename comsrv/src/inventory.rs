use futures::channel::mpsc;
use std::collections::HashMap;
use crate::highlevel::{Result, Instrument};
use std::sync::{Arc, Mutex};
use async_std::task;
use futures::{Stream, StreamExt};

struct InstrumentThread {
    instr: Instrument,
    rx: mpsc::UnboundedReceiver<ThreadMsg>,
}

impl InstrumentThread {
    fn spawn(instr: Instrument) -> InstrumentHandle {
        let (tx, mut rx) = mpsc::unbounded();
        let mut thread = InstrumentThread { instr, rx };
        task::spawn(async move {
            while let Some(msg) = rx.next() {
                if !thread.handle_msg(msg).await {
                    return;
                }
            }
        });
        InstrumentHandle { tx }
    }

    async fn handle_msg(&mut self, msg: ThreadMsg) -> bool {
        match msg {
            ThreadMsg::Write(arg, mut tx) => {
                tx.send(self.instr.write(arg)).await.is_ok()
            },
            ThreadMsg::Query(arg, mut tx) => {
                tx.send(self.instr.query(arg)).await.is_ok()
            },
            ThreadMsg::QueryBinary(arg, mut tx) => {
                tx.send(self.instr.query_binary(arg)).await.is_ok()
            },
            ThreadMsg::SetTimeout(arg, mut tx) => {
                tx.send(self.instr.set_timeout(arg)).await.is_ok()
            },
            ThreadMsg::GetTimeout(mut tx) => {
                tx.send(self.instr.get_timeout()).await.is_ok()
            },
        }
    }
}

#[derive(Clone)]
struct InstrumentHandle {
    tx: mpsc::UnboundedSender<ThreadMsg>,
}

enum ThreadMsg {
    Write(String, mpsc::Sender<Result<()>>),
    Query(String, mpsc::Sender<Result<String>>),
    QueryBinary(String, mpsc::Sender<Result<Vec<u8>>>),
    SetTimeout(f32, mpsc::Sender<Result<()>>),
    GetTimeout(mpsc::Sender<Result<f32>>),
}

impl InstrumentHandle {
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

    pub async fn query_binary<T: AsRef<str>>(&self, msg: T) -> Result<Vec<u8>> {
        todo!()
    }
}

enum InventoryMsg {
    Disconnected(String),
}

struct InventoryShared {
    instruments: HashMap<String, InstrumentHandle>,
    tx: mpsc::UnboundedSender<InventoryMsg>,
}

impl InventoryShared {
    pub fn new(tx: mpsc::UnboundedSender<InventoryMsg>) -> Self {
        Self {
            instruments: HashMap::new(),
            tx,
        }
    }

    pub fn add(&mut self, instr: Instrument) -> InstrumentHandle {
        let addr = instr.addr().to_string();
        if !self.instruments.contains_key(&addr) {
            let handle = InstrumentThread::spawn(instr);
            self.instruments.insert(addr.clone(), handle);
        }
        self.instruments.get(&addr).unwrap().clone()
    }

    pub fn get<T: AsRef<str>>(&self, addr: T) -> Option<InstrumentHandle> {
        self.instruments.get(addr.as_ref()).map(|x| x.clone())
    }

    pub fn close<T: AsRef<str>>(&mut self, addr: T) {
        self.instruments.remove(addr.as_ref());
    }

    pub fn list(&self) -> Vec<String> {
        self.instruments.keys().map(|x| x.clone()).collect()
    }
}

struct Inventory {
    inner: Arc<Mutex<InventoryShared>>,
}

struct InventoryMonitor {
    inventory: Arc<Mutex<InventoryShared>>,
    rx: mpsc::UnboundedReceiver<InventoryMsg>,
}

impl InventoryMonitor {
    fn start(inventory: Arc<Mutex<InventoryShared>>, mut rx: mpsc::UnboundedReceiver<InventoryMsg>) {
        task::spawn(async move {
            while let Some(msg) = rx.next().await {
                match msg {
                    InventoryMsg::Disconnected(x) => {
                        inventory.lock().unwrap().close(&x);
                    },
                }
            }
        });
    }
}

impl Inventory {
    pub fn new() -> Self {
        let (tx,rx) = mpsc::unbounded();
        let inner =  Arc::new(Mutex::new(InventoryShared::new(tx)));
        InventoryMonitor::start(inner.clone(), rx);

        Self {
            inner
        }
    }

    pub fn add(&self, instr: Instrument) -> InstrumentHandle {
        self.inner.lock().unwrap().add(instr)
    }

    pub fn get<T: AsRef<str>>(&self, addr: T) -> Option<InstrumentHandle> {
        self.inner.lock().unwrap().get(addr)
    }

    pub fn close<T: AsRef<str>>(&mut self, addr: T) {
        self.inner.lock().unwrap().close(addr)
    }

    pub fn list(&self) -> Vec<String> {
        self.inner.lock().unwrap().list()
    }
}