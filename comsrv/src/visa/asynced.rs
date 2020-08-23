use crate::visa::{Instrument as BlockingInstrument, VisaRequest, VisaReply, VisaResult};
use tokio::sync::oneshot;
use crate::Error;
use std::sync::mpsc;
use std::thread;
use tokio::task::spawn_blocking;
use crate::inventory::ConnectOptions;

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
        request: VisaRequest,
        reply: oneshot::Sender<VisaResult<VisaReply>>,
    },
    Drop
}

impl Instrument {
    pub async fn connect<T: Into<String>>(addr: T, _options: Option<ConnectOptions>) -> crate::Result<Instrument> {
        let addr = addr.into();
        let instr = spawn_blocking(move || {
            BlockingInstrument::new(addr)
        }).await;
        let instr = instr.map_err(|_| Error::ChannelBroken)?;
        Ok(Self::spawn(instr?))
    }

    pub fn spawn(instr: BlockingInstrument) -> Instrument {
        let (tx, rx) = mpsc::channel();

        let mut thread = Thread { instr, rx };
        thread::spawn(move || {
            while let Ok(msg) = thread.rx.recv() {
                if !thread.handle_msg(msg) {
                    return;
                }
            }
        });

        Instrument {  tx }
    }

    async fn handle(&self, req: VisaRequest) -> crate::Result<VisaReply> {
        let (tx, rx) = oneshot::channel();
        let thmsg = Msg::Request {
            request: req,
            reply: tx
        };
        self.tx.send(thmsg).map_err(|_| Error::Disconnected)?;
        let ret: VisaResult<VisaReply> = rx.await.map_err(|_| Error::ChannelBroken)?;
        ret.map_err(Error::Visa)
    }

    fn disconnect(self) {
        let _ = self.tx.send(Msg::Drop);
    }

}

impl Thread {
    fn handle_msg(&mut self, msg: Msg) -> bool {
        match msg {
            Msg::Request { request, reply } => {
                let _ = reply.send(self.instr.handle(request));
                true
            },
            Msg::Drop => false,
        }
    }
}
