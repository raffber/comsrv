use std::io::Read;
use std::process::{Command, Stdio};

use serde::{Deserialize, Serialize};
use tempfile::NamedTempFile;
use tokio::task;

#[derive(Clone, Serialize, Deserialize)]
pub enum Acquire {
    Time(f32),
    Samples(u64),
}

#[derive(Clone, Serialize, Deserialize)]
pub struct SigrokRequest {
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    channels: Vec<String>,
    acquire: Acquire,
    sample_rate: u64,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Channel {
    name: String,
    data: Vec<u8>,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Data {
    tsample: f64,
    channels: Vec<Channel>,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Device {
    addr: String,
    desc: String,
}

#[derive(Clone, Serialize, Deserialize)]
pub enum SigrokResponse {
    Data(Data),
    Devices(Vec<Device>),
}

pub async fn read(device: String, req: SigrokRequest) -> crate::Result<SigrokResponse> {
    let ret = task::spawn_blocking(|| do_read(device, req))
        .await
        .map_err(|_| crate::Error::Disconnected)?;
    ret.map(SigrokResponse::Data)
}

fn run_command(args: &[&str]) -> crate::Result<String> {
    let mut cmd = Command::new("sigrok-cli");
    cmd.stderr(Stdio::piped()).stdout(Stdio::piped());
    for arg in args {
        cmd.arg(arg);
    }
    let child = cmd.spawn().map_err(crate::Error::io)?;
    let output = child.wait_with_output().map_err(crate::Error::io)?;
    let stdout = String::from_utf8(output.stdout).map_err(crate::Error::DecodeError)?;
    let code = output.status.code().unwrap_or(-1);
    if code != 0 {
        let msg = format!("Return Code: {}", code);
        return Err(crate::Error::ProcessFailed(msg));
    }
    Ok(stdout)
}

pub async fn list() -> crate::Result<SigrokResponse> {
    task::spawn_blocking(|| do_list())
        .await
        .map_err(|_| crate::Error::Disconnected)?
}

fn do_list() -> crate::Result<SigrokResponse> {
    let stdout = run_command(&["--scan"])?;
    let mut ret = Vec::new();
    // skip 2, first line is boilerplate, second one is demo
    for line in stdout.split("\n").skip(2) {
        let line = line.trim();
        if line == "" {
            continue;
        }
        let mut parts = line.split("-").map(|x| x.trim());
        let device = Device {
            addr: parts
                .next()
                .ok_or(crate::Error::UnexpectedProcessOutput)?
                .to_string(),
            desc: parts
                .next()
                .ok_or(crate::Error::UnexpectedProcessOutput)?
                .to_string(),
        };
        ret.push(device)
    }
    Ok(SigrokResponse::Devices(ret))
}

fn do_read(device: String, req: SigrokRequest) -> crate::Result<Data> {
    let mut args = vec!["-d", &device];
    let channels = req.channels.join(",");
    if req.channels.len() > 0 {
        args.push("--channels");
        args.push(&channels);
    }
    args.push("--config");
    let sample_rate = format!("samplerate={}", req.sample_rate);
    args.push(&sample_rate);
    let acq = match req.acquire {
        Acquire::Time(t) => {
            args.push("--time");
            format!("{}s", t)
        }
        Acquire::Samples(samples) => {
            args.push("--samples");
            format!("{}", samples)
        }
    };
    args.push(&acq);

    let mut tempfile = NamedTempFile::new()?;
    let fpath = tempfile.path().to_str().unwrap();
    args.push("--output-format");
    args.push("vcd");
    args.push("--output-file");
    args.push(fpath);
    run_command(&args)?;

    let mut vcd = String::new();
    tempfile.read_to_string(&mut vcd)?;
    log::debug!("{}", vcd);
    Ok(Data {
        tsample: 0.0,
        channels: vec![],
    })
}
