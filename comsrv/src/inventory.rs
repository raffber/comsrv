use futures::channel::{mpsc, oneshot};
use std::collections::HashMap;
use crate::highlevel::Instrument;
use std::sync::{Arc, Mutex};
use async_std::task;
use futures::StreamExt;
use crate::{Result, Error};

struct InstrumentThread {
    instr: Instrument,
    rx: mpsc::UnboundedReceiver<ThreadMsg>,
}

impl InstrumentThread {
    fn spawn(instr: Instrument) -> InstrumentHandle {
        let (tx, rx) = mpsc::unbounded();
        let mut thread = InstrumentThread { instr, rx };
        task::spawn(async move {
            while let Some(msg) = thread.rx.next().await {
                if !thread.handle_msg(msg).await {
                    return;
                }
            }
        });
        InstrumentHandle { tx }
    }

    async fn handle_msg(&mut self, msg: ThreadMsg) -> bool {
        match msg {
            ThreadMsg::Write(arg, tx) => {
                tx.send(self.instr.write(arg)).is_ok()
            },
            ThreadMsg::Query(arg, tx) => {
                tx.send(self.instr.query(arg)).is_ok()
            },
            ThreadMsg::QueryBinary(arg, tx) => {
                tx.send(self.instr.query_binary(arg)).is_ok()
            },
            ThreadMsg::SetTimeout(arg, tx) => {
                tx.send(self.instr.set_timeout(arg)).is_ok()
            },
            ThreadMsg::GetTimeout(tx) => {
                tx.send(self.instr.get_timeout()).is_ok()
            },
        }
    }
}

#[derive(Clone)]
struct InstrumentHandle {
    tx: mpsc::UnboundedSender<ThreadMsg>,
}

enum ThreadMsg {
    Write(String, oneshot::Sender<Result<()>>),
    Query(String, oneshot::Sender<Result<String>>),
    QueryBinary(String, oneshot::Sender<Result<Vec<u8>>>),
    SetTimeout(f32, oneshot::Sender<Result<()>>),
    GetTimeout(oneshot::Sender<Result<f32>>),
}

impl InstrumentHandle {
    pub async fn write<T: AsRef<str>>(&mut self, msg: T) -> Result<()> {
        let (tx, rx) = oneshot::channel();
        self.tx.unbounded_send(ThreadMsg::Write(msg.as_ref().to_string(), tx))
            .map_err(|_| Error::ChannelBroken)?;
        match rx.await {
            Err(_) => Err(Error::ChannelBroken),
            Ok(x) => x,
        }
    }

    pub async fn query<T: AsRef<str>>(&mut self, msg: T) -> Result<String> {
        let (tx, rx) = oneshot::channel();
        self.tx.unbounded_send(ThreadMsg::Query(msg.as_ref().to_string(), tx))
            .map_err(|_| Error::ChannelBroken)?;
        match rx.await {
            Err(_) => Err(Error::ChannelBroken),
            Ok(x) => x,
        }
    }

    pub async fn set_timeout(&mut self, timeout: f32) -> Result<()> {
        let (tx, rx) = oneshot::channel();
        self.tx.unbounded_send(ThreadMsg::SetTimeout(timeout, tx))
            .map_err(|_| Error::ChannelBroken)?;
        match rx.await {
            Err(_) => Err(Error::ChannelBroken),
            Ok(x) => x,
        }
    }

    pub async fn get_timeout(&mut self) -> Result<f32> {
        let (tx, rx) = oneshot::channel();
        self.tx.unbounded_send(ThreadMsg::GetTimeout(tx))
            .map_err(|_| Error::ChannelBroken)?;
        match rx.await {
            Err(_) => Err(Error::ChannelBroken),
            Ok(x) => x,
        }
    }

    pub async fn query_binary<T: AsRef<str>>(&mut self, msg: T) -> Result<Vec<u8>> {
        let (tx, rx) = oneshot::channel();
        self.tx.unbounded_send(ThreadMsg::QueryBinary(msg.as_ref().to_string(), tx))
            .map_err(|_| Error::ChannelBroken)?;
        match rx.await {
            Err(_) => Err(Error::ChannelBroken),
            Ok(x) => x,
        }
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
    fn start(inventory: Arc<Mutex<InventoryShared>>, rx: mpsc::UnboundedReceiver<InventoryMsg>) {
        let mut monitor = InventoryMonitor {
            inventory,
            rx
        };
        task::spawn(async move {
            while let Some(msg) = monitor.rx.next().await {
                monitor.handle_msg(msg);
            }
        });
    }

    fn handle_msg(&mut self, msg: InventoryMsg) {
        match msg {
            InventoryMsg::Disconnected(x) => {
                self.inventory.lock().unwrap().close(&x);
            },
        }
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