use std::time::Instant;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::mpsc;
use tokio::sync::oneshot;
use tokio::task;
use tokio::time::Duration;
use tokio::time::timeout;
use tokio_serial::{DataBits, FlowControl, Parity, Serial, SerialPortSettings, StopBits};

use crate::{ScpiRequest, ScpiResponse};
use crate::{Error, Result};
use crate::visa::VisaOptions;

const TIMEOUT: f32 = 1.0;

#[derive(Clone)]
pub struct PrologixPort {
    tx: mpsc::UnboundedSender<Msg>,
}

#[derive(Clone)]
pub struct Instrument {
    port: PrologixPort,
    addr: u8,
}

struct Request {
    addr: u8,
    request: ScpiRequest,
    options: VisaOptions,
    reply: oneshot::Sender<Result<ScpiResponse>>,

}

enum Msg {
    Request(Request),
    Drop,
}

impl Instrument {
    pub async fn connect(port: PrologixPort, addr: u8) -> Self {
        Self {
            port,
            addr,
        }
    }

    pub async fn handle(&self, request: ScpiRequest) -> Result<ScpiResponse> {
        let (tx, rx) = oneshot::channel();
        let msg = Msg::Request(Request {
            addr: self.addr,
            request,
            options: Default::default(),
            reply: tx,
        });
        if self.port.tx.send(msg).is_err() {
            return Err(Error::Disconnected);
        }
        let ret = rx.await.map_err(|_| Error::Disconnected)?;
        ret
    }
}

fn spawn_prologix(addr: String) -> PrologixPort {
    let (tx, rx) = mpsc::unbounded_channel();
    task::spawn(run_prologix(addr, rx));
    PrologixPort { tx }
}

async fn run_prologix(addr: String, mut rx: mpsc::UnboundedReceiver<Msg>) -> Result<()> {
    let settings = SerialPortSettings {
        baud_rate: 9600,
        data_bits: DataBits::Eight,
        flow_control: FlowControl::None,
        parity: Parity::None,
        stop_bits: StopBits::One,
        timeout: Duration::from_secs_f32(0.5),
    };
    let mut serial = Serial::from_path(&addr, &settings).map_err(Error::io)?;
    while let Some(msg) = rx.recv().await {
        match msg {
            Msg::Request(req) => handle_request(&mut serial, req).await?,
            Msg::Drop => break,
        }
    }
    Ok(())
}

async fn write(serial: &mut Serial, mut msg: String) -> Result<()> {
    if !msg.ends_with("\n") {
        msg.push_str("\n");
    }
    serial.write(msg.as_bytes()).await.map(|_| ()).map_err(Error::io)
}

async fn read(serial: &mut Serial) -> Result<String> {
    let start = Instant::now();
    let mut ret = Vec::new();
    loop {
        let mut x = [0; 1];
        match timeout(Duration::from_secs_f32(TIMEOUT), serial.read_exact(&mut x)).await {
            Ok(Ok(_)) => {
                let x = x[0];
                if x == b'\n' {
                    break;
                }
                ret.push(x);
            }
            Ok(Err(x)) => {
                return Err(Error::io(x));
            }
            Err(_) => {
                return Err(Error::Timeout);
            }
        };
        let delta = start.elapsed().as_secs_f32();
        if delta > TIMEOUT {
            return Err(Error::Timeout);
        }
    }
    String::from_utf8(ret).map_err(Error::DecodeError)
}

async fn handle_request(serial: &mut Serial, req: Request) -> Result<()> {
    let mut ret = Vec::with_capacity(128);
    serial.read_to_end(&mut ret).await.map_err(Error::io)?;
    ret.clear();
    let addr_set = format!("++addr {}", req.addr);
    let addr_set = addr_set.as_bytes();
    serial.write(addr_set).await.map_err(Error::io)?;
    match req.request {
        ScpiRequest::Write(x) => {
            write(serial, x).await?;
            if req.reply.send(Ok(ScpiResponse::Done)).is_err() {
                return Err(Error::Disconnected);
            }
        }
        ScpiRequest::QueryString(x) => {
            write(serial, x).await?;
            serial.write("++read eoi".as_bytes()).await.map_err(Error::io)?;
            let reply = read(serial).await;
            let reply = match reply {
                Ok(data) => Ok(ScpiResponse::String(data)),
                Err(Error::Timeout) => Err(Error::Timeout),
                Err(x) => {
                    return Err(x);
                }
            };
            if req.reply.send(reply).is_err() {
                return Err(Error::Disconnected);
            }
        }
        ScpiRequest::QueryBinary(_) => {
            log::error!("ScpiRequest::QueryBinary not implemented for Prologix!!");
            if req.reply.send(Err(Error::NotSupported)).is_err() {
                return Err(Error::Disconnected);
            }
        }
    }
    Ok(())
}

