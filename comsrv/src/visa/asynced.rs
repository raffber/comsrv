use std::sync::mpsc;
use std::thread;

use tokio::sync::oneshot;

use comsrv_protocol::{ScpiRequest, ScpiResponse};
use crate::visa::{Instrument as BlockingInstrument};
use crate::Error;

#[derive(Clone)]
pub struct Instrument {
    tx: mpsc::Sender<Msg>,
}

enum Msg {
    Scpi {
        request: ScpiRequest,
        reply: oneshot::Sender<crate::Result<ScpiResponse>>,
    },
    Drop,
}

impl Instrument {
    pub fn connect<T: Into<String>>(addr: T) -> Self {
        let (tx, rx) = mpsc::channel();
        let addr = addr.into();
        thread::spawn(move || {
            let mut oinstr = None;
            while let Ok(msg) = rx.recv() {
                let (request, reply) = match msg {
                    Msg::Scpi {
                        request,
                        reply,
                    } => (request, reply),
                    Msg::Drop => {
                        break;
                    }
                };
                let instr = if let Some(instr) = oinstr.take() {
                    Ok(instr)
                } else {
                    BlockingInstrument::open(&addr).map_err(Error::Visa)
                };
                match instr {
                    Ok(instr) => {
                        let _ = reply.send(instr.handle_scpi(request));
                        oinstr.replace(instr);
                    }
                    Err(err) => {
                        let _ = reply.send(Err(err));
                    }
                }
            }
        });

        Instrument { tx }
    }

    pub async fn request(
        self,
        req: ScpiRequest,
    ) -> crate::Result<ScpiResponse> {
        let (tx, rx) = oneshot::channel();
        let thmsg = Msg::Scpi {
            request: req,
            reply: tx,
        };
        self.tx.send(thmsg).map_err(|_| Error::Disconnected)?;
        rx.await.map_err(|_| Error::Disconnected)?
    }

    pub fn disconnect(self) {
        let _ = self.tx.send(Msg::Drop);
    }
}
