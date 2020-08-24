use std::sync::mpsc;
use std::thread;

use tokio::sync::oneshot;
use tokio::task::spawn_blocking;

use crate::Error;
use crate::visa::{Instrument as BlockingInstrument, VisaOptions};
use crate::app::{Request, Response};

pub struct Thread {
    instr: BlockingInstrument,
    rx: mpsc::Receiver<Msg>,
}

#[derive(Clone)]
pub struct Instrument {
    tx: mpsc::Sender<Msg>,
}

enum Msg {
    Request {
        request: Request,
        reply: oneshot::Sender<crate::Result<Response>>,
    },
    Drop,
}

impl Instrument {
    pub async fn connect<T: Into<String>>(addr: T, _options: VisaOptions) -> crate::Result<Instrument> {
        let addr = addr.into();
        let instr = spawn_blocking(move || {
            BlockingInstrument::new(addr)
        }).await.unwrap();
        Ok(Self::spawn(instr?))
    }

    pub fn spawn(instr: BlockingInstrument) -> Instrument {
        let (tx, rx) = mpsc::channel();

        let mut thread = Thread { instr, rx };
        thread::spawn(move || {
            while let Ok(msg) = thread.rx.recv() {
                if !thread.handle(msg) {
                    return;
                }
            }
        });

        Instrument { tx }
    }

    async fn handle(&self, req: Request) -> crate::Result<Response> {
        let (tx, rx) = oneshot::channel();
        let thmsg = Msg::Request {
            request: req,
            reply: tx,
        };
        self.tx.send(thmsg).map_err(|_| Error::Disconnected)?;
        rx.await.map_err(|_| Error::Disconnected)?
    }

    fn disconnect(self) {
        let _ = self.tx.send(Msg::Drop);
    }
}

impl Thread {
    fn handle(&mut self, msg: Msg) -> bool {
        match msg {
            Msg::Request { request, reply } => {
                let _ = reply.send(self.instr.handle(request).map_err(Error::Visa));
                true
            }
            Msg::Drop => false,
        }
    }
}
