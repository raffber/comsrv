use std::sync::mpsc;
use std::thread;

use tokio::sync::oneshot;
use tokio::task::spawn_blocking;

use crate::{Error, ScpiRequest, ScpiResponse};
use crate::visa::{Instrument as BlockingInstrument, VisaOptions};

pub struct Thread {
    instr: BlockingInstrument,
    rx: mpsc::Receiver<Msg>,
}

#[derive(Clone)]
pub struct Instrument {
    tx: mpsc::Sender<Msg>,
}

enum Msg {
    Scpi {
        request: ScpiRequest,
        options: VisaOptions,
        reply: oneshot::Sender<crate::Result<ScpiResponse>>,
    },
    Drop,
}

impl Instrument {
    pub async fn connect<T: Into<String>>(addr: T, options: VisaOptions) -> crate::Result<Instrument> {
        let addr = addr.into();
        let instr = spawn_blocking(move || {
            BlockingInstrument::open(addr, &options)
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

    pub async fn handle_scpi(self, req: ScpiRequest) -> crate::Result<ScpiResponse> {
        let (tx, rx) = oneshot::channel();
        let thmsg = Msg::Scpi {
            request: req,
            options: VisaOptions::default(),
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
            Msg::Scpi { request, options, reply } => {
                let _ = reply.send(self.instr.handle_scpi(request, &options));
                true
            }
            Msg::Drop => false,
        }
    }
}
