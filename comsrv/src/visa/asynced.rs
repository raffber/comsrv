use std::sync::mpsc;
use std::thread;

use tokio::sync::oneshot;

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
    pub async fn connect<T: Into<String>>(addr: T, options: VisaOptions) -> Self {
        let (tx, rx) = mpsc::channel();
        let addr = addr.into();
        thread::spawn(move || {
            let mut oinstr = None;
            while let Ok(msg) = rx.recv() {
                if matches!(msg, Msg::Drop) {
                    break;
                }
                let instr = if let Some(instr) = oinstr.take() {
                    Ok(instr)
                } else {
                    BlockingInstrument::open(&addr, &options).map_err(Error::Visa)
                };
                match instr {
                    Ok(instr) => {
                        match msg {
                            Msg::Scpi { request, options, reply } => {
                                let _ = reply.send(instr.handle_scpi(request, &options));
                            }
                            _ => {}
                        }
                        oinstr.replace(instr);
                    }
                    Err(err) => {
                        match msg {
                            Msg::Scpi { request: _, options: _, reply } => {
                                let _ = reply.send(Err(err));
                            }
                            _ => {}
                        }
                    }
                }
            }
        });

        Instrument { tx }
    }

    pub async fn request(self, req: ScpiRequest) -> crate::Result<ScpiResponse> {
        let (tx, rx) = oneshot::channel();
        let thmsg = Msg::Scpi {
            request: req,
            options: VisaOptions::default(),
            reply: tx,
        };
        self.tx.send(thmsg).map_err(|_| Error::Disconnected)?;
        rx.await.map_err(|_| Error::Disconnected)?
    }

    pub fn disconnect(self) {
        let _ = self.tx.send(Msg::Drop);
    }
}

impl Thread {
    fn handle(&mut self, msg: Msg) -> bool {}
}
