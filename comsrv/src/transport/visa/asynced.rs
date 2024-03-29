use std::sync::{mpsc, Arc, Mutex};
use std::thread;

use async_trait::async_trait;
use tokio::sync::oneshot;

use crate::Error;
use crate::{inventory, transport::visa::blocking::Instrument as BlockingInstrument};
use anyhow::anyhow;
use comsrv_protocol::{ScpiRequest, ScpiResponse};

#[derive(Clone)]
pub struct Instrument {
    tx: Arc<Mutex<mpsc::Sender<Msg>>>,
}

enum Msg {
    Scpi {
        request: ScpiRequest,
        reply: oneshot::Sender<crate::Result<ScpiResponse>>,
    },
    Drop,
}

impl Instrument {
    pub fn new<T: Into<String>>(addr: T) -> Self {
        let (tx, rx) = mpsc::channel();
        let addr = addr.into();
        thread::spawn(move || {
            let mut oinstr = None;
            while let Ok(msg) = rx.recv() {
                let (request, reply) = match msg {
                    Msg::Scpi { request, reply } => (request, reply),
                    Msg::Drop => {
                        break;
                    }
                };
                let instr = if let Some(instr) = oinstr.take() {
                    Ok(instr)
                } else {
                    BlockingInstrument::open(&addr)
                };
                match instr {
                    Ok(instr) => {
                        let _ = reply.send(instr.handle_scpi(request));
                        oinstr.replace(instr);
                    }
                    Err(err) => {
                        let _ = reply.send(Err(err.into()));
                    }
                }
            }
        });

        Instrument {
            tx: Arc::new(Mutex::new(tx)),
        }
    }

    pub async fn request(self, req: ScpiRequest) -> crate::Result<ScpiResponse> {
        let (tx, rx) = oneshot::channel();
        let thmsg = Msg::Scpi {
            request: req,
            reply: tx,
        };
        self.tx
            .lock()
            .unwrap()
            .send(thmsg)
            .map_err(|_| Error::internal(anyhow!("Disconnected")))?;
        rx.await.map_err(|_| Error::internal(anyhow!("Disconnected")))?
    }

    pub fn disconnect(self) {
        let _ = self.tx.lock().unwrap().send(Msg::Drop);
    }
}

#[async_trait]
impl inventory::Instrument for Instrument {
    type Address = String;

    fn connect(_server: &crate::app::Server, addr: &Self::Address) -> crate::Result<Self> {
        Ok(Instrument::new(addr))
    }

    async fn wait_for_closed(&self) {}
}
